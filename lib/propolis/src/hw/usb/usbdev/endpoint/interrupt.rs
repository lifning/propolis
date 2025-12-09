// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    common::GuestAddr,
    hw::{
        pci,
        usb::xhci::{
            bits::{ring_data::TrbCompletionCode, MINIMUM_INTERVAL_TIME},
            controller::XhciPortHandle,
            device_slots::{EndpointId, SlotId},
            interrupter::Error as InterrupterError,
            rings::consumer::transfer::{PointerOrImmediate, TransferTrb},
        },
    },
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Immediate data (rather than pointer) given in Interrupt-IN endpoint transfer TRB at {0:x?}")]
    ImmediateDataInTransfer(GuestAddr),
    #[error("Reference to xHCI PCI device dropped")]
    PciDeviceReferenceGone,
    #[error("Failed to get MemAccessor")]
    MemAccessorFail,
    #[error(
        "Failed to place transfer completion event TRBs in event ring: {0}"
    )]
    Completion(#[from] InterrupterError),
}

#[usdt::provider(provider = "propolis")]
mod probes {
    fn usb_interrupt_xfer_complete(
        slot_id: u8,
        endpoint_id: u8,
        ptr: u64,
        bytes: usize,
    ) {
    }
    fn usb_interrupt_xfer_shortpacket(
        slot_id: u8,
        endpoint_id: u8,
        ptr: u64,
        bytes_requested: usize,
        bytes_received: usize,
    ) {
    }
}

pub struct InterruptInData {
    /// Each VecDeque<TransferTrb> within the outer VecDeque is a TD.
    transfers: VecDeque<VecDeque<TransferTrb>>,
    /// Data from the USB device to write into the transfers
    payload: Option<Vec<u8>>,
    period: Duration,
    terminate: bool,
    phase: InterruptInPhase,
}

impl InterruptInData {
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = Some(payload);
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum InterruptInPhase {
    WaitForTransferDescriptors,
    WaitForPayloadPeriod,
    // wait indefinitely after a short-packet (i.e. missing a periodic transfer due to no data from usb dev)
    WaitForPayloadOrTransfer,
    StoppedEndpoint,
    Writing,
    TerminateLoop,
}

struct PeriodicTransferPollThread {
    weak_data: Weak<(Mutex<InterruptInData>, Condvar)>,
    weak_pci_state: Weak<pci::DeviceState>,
    port_hdl: Weak<XhciPortHandle>,
    slot_id: SlotId,
    endpoint_id: EndpointId,
    log: slog::Logger,
}

impl PeriodicTransferPollThread {
    fn spawn(
        port_hdl: Weak<XhciPortHandle>,
        pci_state: &Arc<pci::DeviceState>,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        data: &Arc<(Mutex<InterruptInData>, Condvar)>,
        log: slog::Logger,
    ) -> JoinHandle<()> {
        let periodic_poll_thread = PeriodicTransferPollThread {
            weak_data: Arc::downgrade(data),
            weak_pci_state: Arc::downgrade(pci_state),
            port_hdl,
            slot_id,
            endpoint_id,
            log,
        };
        std::thread::Builder::new()
            .name(format!(
                "xhci interrupt in endpoint {slot_id:?} {endpoint_id:?}"
            ))
            .spawn(move || {
                periodic_poll_thread.main_loop();
            })
            .unwrap()
    }

    fn main_loop(self) {
        while let Some(pair) = self.weak_data.upgrade() {
            let (mtx, cvar) = &*pair;
            let guard = mtx.lock().unwrap();
            match guard.phase {
                InterruptInPhase::WaitForTransferDescriptors => {
                    let mut guard = cvar
                        .wait_while(guard, |x| {
                            x.transfers.is_empty()
                                && x.phase == InterruptInPhase::WaitForTransferDescriptors
                        })
                        .unwrap();
                    guard.phase = InterruptInPhase::WaitForPayloadPeriod;
                }
                InterruptInPhase::WaitForPayloadPeriod => {
                    let timeout = guard.period;
                    let (mut guard, timeout_result) = cvar
                        .wait_timeout_while(guard, timeout, |x| {
                            x.payload.is_none()
                                && x.phase
                                    == InterruptInPhase::WaitForPayloadPeriod
                        })
                        .unwrap();

                    if !timeout_result.timed_out() {
                        // we have a payload, proceed
                        guard.phase = InterruptInPhase::Writing;
                    } else if let Some(xfer) =
                        guard.transfers.iter().flatten().next()
                    {
                        let Some(port_hdl) = self.port_hdl.upgrade() else {
                            guard.phase = InterruptInPhase::TerminateLoop;
                            continue;
                        };
                        if let Err(e) =
                            self.notify_short_packet(&xfer, &port_hdl)
                        {
                            slog::error!(
                                    self.log,
                                    "Failed to notify guest of short packet in USB Interrupt-IN transfer: {e}"
                                );
                        }
                        guard.phase =
                            InterruptInPhase::WaitForPayloadOrTransfer;
                    } else {
                        guard.phase =
                            InterruptInPhase::WaitForTransferDescriptors;
                    }
                }
                InterruptInPhase::WaitForPayloadOrTransfer => {
                    let num_tds = guard.transfers.len();
                    let mut guard = cvar
                        .wait_while(guard, |x| {
                            x.payload.is_none()
                                && x.transfers.len() == num_tds
                                && x.phase == InterruptInPhase::WaitForPayloadOrTransfer
                                && !x.terminate // TODO: remove
                        })
                        .unwrap();
                    if guard.payload.is_some() {
                        guard.phase = InterruptInPhase::Writing;
                    } else if guard.transfers.len() != num_tds {
                        // abandon and move onto a new transfer if one has been given to us
                        // XXX (TODO: check spec again for citation, is this correct to do?) XXX
                        guard.transfers.pop_front();
                        if guard.transfers.is_empty() {
                            guard.phase =
                                InterruptInPhase::WaitForTransferDescriptors;
                        } else {
                            guard.phase =
                                InterruptInPhase::WaitForPayloadPeriod;
                        }
                    }
                }
                // TODO: no more than one TD consumed per ESIT if software gives us
                // too many at once (xHCI 1.2 sect 4.14.3)
                InterruptInPhase::Writing => {
                    // no waiting here
                    let mut guard = guard;
                    if let Some(td) = guard.transfers.front_mut() {
                        if let Some(trb) = td.pop_front() {
                            let PointerOrImmediate::Pointer(_) =
                                trb.data_buffer()
                            else {
                                slog::error!(self.log, "Immediate data found in USB Interrupt-IN endpoint at {:x?}", trb.trb_pointer());
                                continue;
                            };
                            let Some(port_hdl) = self.port_hdl.upgrade() else {
                                guard.phase = InterruptInPhase::TerminateLoop;
                                continue;
                            };

                            if let Some(data) = guard.payload.take() {
                                if let Err(e) =
                                    self.complete_transfer(data, trb, &port_hdl)
                                {
                                    slog::error!(
                                        self.log,
                                        "Failed to complete USB Interrupt-IN transfer after receiving packet: {e}"
                                    );
                                }
                            } else {
                                slog::error!(
                                    self.log,
                                    "USB Interrupt-IN endpoint in writing state with no payload"
                                );
                            }
                        } else {
                            // TD empty
                            guard.transfers.pop_front();
                        }
                    } else {
                        guard.phase =
                            InterruptInPhase::WaitForTransferDescriptors;
                    };
                }
                InterruptInPhase::StoppedEndpoint => {
                    // TODO
                    // did we try to reset/stop the endpoint with a transction in flight?
                }
                InterruptInPhase::TerminateLoop => {
                    // FIXME: remove once unnecessary
                    guard.terminate = true;
                    cvar.notify_one();
                    break;
                }
            }
            cvar.notify_one();
        }
        slog::debug!(
            self.log,
            "USB Interrupt-IN Endpoint packet processing loop terminated"
        );
    }

    fn notify_short_packet(
        &self,
        xfer: &TransferTrb,
        port_hdl: &Arc<XhciPortHandle>,
    ) -> Result<(), Error> {
        if let PointerOrImmediate::Pointer(region) = xfer.data_buffer() {
            probes::usb_interrupt_xfer_shortpacket!(|| (
                u8::from(self.slot_id),
                u8::from(self.endpoint_id),
                region.0 .0,
                region.1,
                0,
            ));
        };
        port_hdl
            .event_sender
            .send_completion_events_for_trb(
                xfer,
                TrbCompletionCode::ShortPacket,
                0,
                self.slot_id,
                self.endpoint_id,
            )
            .map_err(Into::into)
    }

    fn complete_transfer(
        &self,
        data: Vec<u8>,
        xfer: TransferTrb,
        port_hdl: &Arc<XhciPortHandle>,
    ) -> Result<(), Error> {
        let PointerOrImmediate::Pointer(region) = xfer.data_buffer() else {
            return Err(Error::ImmediateDataInTransfer(xfer.trb_pointer()));
        };
        let bytes_transferred = data.len().min(region.1);
        let Some(pci_state) = self.weak_pci_state.upgrade() else {
            return Err(Error::PciDeviceReferenceGone);
        };
        let Some(memctx) = pci_state.acc_mem.access() else {
            return Err(Error::MemAccessorFail);
        };
        memctx.write_many(region.0, &data[..bytes_transferred]);
        probes::usb_interrupt_xfer_complete!(|| (
            u8::from(self.slot_id),
            u8::from(self.endpoint_id),
            region.0 .0,
            region.1,
        ));
        port_hdl
            .event_sender
            .send_completion_events_for_trb(
                &xfer,
                TrbCompletionCode::Success,
                bytes_transferred,
                self.slot_id,
                self.endpoint_id,
            )
            .map_err(Into::into)
    }
}

pub struct InterruptInEndpoint {
    data: Arc<(Mutex<InterruptInData>, Condvar)>,
    slot_id: SlotId,
    endpoint_id: EndpointId,
    _jh: JoinHandle<()>,
}

impl InterruptInEndpoint {
    pub fn new(
        period: Duration,
        port_hdl: Weak<XhciPortHandle>,
        pci_state: &Arc<pci::DeviceState>,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        log: &slog::Logger,
    ) -> Self {
        let data = Arc::new((
            Mutex::new(InterruptInData {
                transfers: VecDeque::new(),
                payload: None,
                period,
                terminate: false,
                block_migration: false,
            }),
            Condvar::new(),
        ));
        let log = log.new(slog::o!("endpoint_type" => "interrupt_in", "endpoint_id" => u8::from(endpoint_id)));
        let _jh = PeriodicTransferPollThread::spawn(
            port_hdl,
            pci_state,
            slot_id,
            endpoint_id,
            &data,
            log,
        );
        Self { data, slot_id, endpoint_id, _jh }
    }

    pub fn new_migrated(
        value: &migrate::InterruptInEndpointV1,
        port_hdl: Weak<XhciPortHandle>,
        pci_state: &Arc<pci::DeviceState>,
        log: &slog::Logger,
    ) -> Self {
        let migrate::InterruptInEndpointV1 {
            transfers,
            payload,
            period_ticks,
            slot_id,
            endpoint_id,
        } = value;
        let slot_id = SlotId::from(*slot_id);
        let endpoint_id = EndpointId::from(*endpoint_id);
        let data = Arc::new((
            Mutex::new(InterruptInData {
                transfers: transfers
                    .into_iter()
                    .map(|trbs| trbs.iter().map(From::from).collect())
                    .collect(),
                payload: payload.to_owned(),
                period: MINIMUM_INTERVAL_TIME.mul_f64(*period_ticks),
                terminate: false,
                block_migration: false,
            }),
            Condvar::new(),
        ));
        let log = log.new(slog::o!("endpoint_type" => "interrupt_in", "endpoint_id" => u8::from(endpoint_id)));
        let _jh = PeriodicTransferPollThread::spawn(
            port_hdl,
            pci_state,
            slot_id,
            endpoint_id,
            &data,
            log,
        );
        Self { data, slot_id, endpoint_id, _jh }
    }

    pub fn normal_transfer(&self, td_trbs: Vec<TransferTrb>) {
        let mut data = self.data.0.lock().unwrap();
        data.transfers.push_back(td_trbs.into()); // Vec into VecDeque guaranteed O(1) by stdlib
        self.data.1.notify_one();
    }

    pub fn data_ref(&self) -> Weak<(Mutex<InterruptInData>, Condvar)> {
        Arc::downgrade(&self.data)
    }

    pub fn stop_transfers(&mut self) -> TODO {
        // store these elsewhere so we can resume them on doorbell ring
        self.data.0.lock().unwrap().transfers.drain(..);
    }

    pub fn import(
        &mut self,
        ep: &migrate::InterruptInEndpointV1,
    ) -> Result<(), crate::migrate::MigrateStateError> {
        // TODO: can we unify the way this is represented for periodic / bulk / control
        let migrate::InterruptInEndpointV1 {
            transfers,
            payload,
            period_ticks,
            slot_id,
            endpoint_id,
        } = ep;
        let guard = self.data.0.lock().unwrap();
        let mut guard =
            self.data.1.wait_while(guard, |x| x.block_migration).unwrap();
        let InterruptInData {
            transfers: transfers_mut,
            payload: payload_mut,
            period,
            terminate,
            block_migration: _,
        } = &mut *guard;
        if *terminate {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                "Interrupt-IN endpoint periodic transfer loop was terminated for missing its handle to the xHC".to_string()
            ));
        }
        self.slot_id = SlotId::from(*slot_id);
        self.endpoint_id = EndpointId::from(*endpoint_id);
        *transfers_mut = transfers
            .iter()
            .map(|trbs| trbs.map(From::from).collect())
            .collect();
        *payload_mut = payload.to_owned();
        *period = MINIMUM_INTERVAL_TIME.mul_f64(*period_ticks);
        Ok(())
    }

    pub fn export(
        &self,
    ) -> Result<super::migrate::EndpointV1, crate::migrate::MigrateStateError>
    {
        let guard = self.data.0.lock().unwrap();
        let guard =
            self.data.1.wait_while(guard, |x| x.block_migration).unwrap();
        let InterruptInData {
            transfers,
            payload,
            period,
            terminate,
            block_migration: _,
        } = &*guard;
        if *terminate {
            return Err(crate::migrate::MigrateStateError::NotReadyForExport); // loop bailed from missing handle
        }
        let period_ticks =
            period.as_secs_f64() / MINIMUM_INTERVAL_TIME.as_secs_f64();
        Ok(super::migrate::EndpointV1::InterruptIn(
            migrate::InterruptInEndpointV1 {
                transfers: transfers
                    .iter()
                    .map(|trbs| trbs.iter().map(From::from).collect())
                    .collect(),
                payload: payload.to_owned(),
                period_ticks,
                slot_id: u8::from(self.slot_id),
                endpoint_id: u8::from(self.endpoint_id),
            },
        ))
    }
}

impl Drop for InterruptInEndpoint {
    fn drop(&mut self) {
        self.data.0.lock().unwrap().terminate = true;
        self.data.1.notify_one();
    }
}

pub mod migrate {
    use serde::{Deserialize, Serialize};

    use crate::hw::usb::xhci::rings::consumer::transfer::migrate::TransferTrbV1;

    #[derive(Serialize, Deserialize)]
    pub struct InterruptInEndpointV1 {
        pub transfers: Vec<Vec<TransferTrbV1>>, // maybe?
        pub payload: Option<Vec<u8>>,
        pub period_ticks: f64,
        pub slot_id: u8,
        pub endpoint_id: u8,
    }
}

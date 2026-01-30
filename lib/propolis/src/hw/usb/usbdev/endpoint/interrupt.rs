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
}

pub struct InterruptInData {
    /// Each VecDeque<TransferTrb> within the outer VecDeque is a TD.
    transfers: VecDeque<VecDeque<TransferTrb>>,
    /// Data from the USB device to write into the transfers
    payload: VecDeque<u8>,
    period: Duration,
    phase: InterruptInPhase,
}

impl InterruptInData {
    pub fn set_payload(&mut self, payload: impl Into<VecDeque<u8>>) {
        self.payload = payload.into();
    }
    #[allow(unused)]
    pub fn append_payload(&mut self, payload: impl Iterator<Item = u8>) {
        self.payload.extend(payload);
    }
    fn first_trb(&self) -> Option<&TransferTrb> {
        self.transfers.iter().flatten().next()
    }
    fn sufficient_payload_for_current_trb(&self) -> bool {
        self.payload.len()
            >= self.first_trb().map(|trb| trb.data_buffer().len()).unwrap_or(0)
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
                    if guard.phase
                        == InterruptInPhase::WaitForTransferDescriptors
                    {
                        guard.phase = InterruptInPhase::WaitForPayloadPeriod;
                    }
                }
                InterruptInPhase::WaitForPayloadPeriod => {
                    let timeout = guard.period;
                    let (mut guard, timeout_result) = cvar
                        .wait_timeout_while(guard, timeout, |guard| {
                            !guard.sufficient_payload_for_current_trb()
                                && guard.phase
                                    == InterruptInPhase::WaitForPayloadPeriod
                        })
                        .unwrap();

                    if guard.phase != InterruptInPhase::WaitForPayloadPeriod {
                        // if phase was changed out-of-band, loop around to match again
                    } else if !timeout_result.timed_out() {
                        // we have a payload, proceed
                        guard.phase = InterruptInPhase::Writing;
                    } else if guard.first_trb().is_some() {
                        // wait for either a payload from device or a new TD from guest
                        guard.phase =
                            InterruptInPhase::WaitForPayloadOrTransfer;
                    } else {
                        // endpoint was stopped, go back to wait for next doorbell
                        guard.phase =
                            InterruptInPhase::WaitForTransferDescriptors;
                    }
                }
                InterruptInPhase::WaitForPayloadOrTransfer => {
                    let num_tds = guard.transfers.len();
                    let mut guard = cvar
                        .wait_while(guard, |guard| {
                            !guard.sufficient_payload_for_current_trb()
                                && guard.transfers.len() == num_tds
                                && guard.phase == InterruptInPhase::WaitForPayloadOrTransfer
                        })
                        .unwrap();
                    if guard.phase != InterruptInPhase::WaitForPayloadOrTransfer
                    {
                        // if phase was changed out-of-band, loop around to match again
                    } else if guard.sufficient_payload_for_current_trb() {
                        guard.phase = InterruptInPhase::Writing;
                    } else if guard.transfers.len() != num_tds {
                        // abandon and move onto a new transfer if one has been given to us.
                        // (xHCI 1.2 sect 4.9.1: if xHC receives Short Packet from device,
                        // retire current TD and advance to next TD from the Transfer Ring)
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

                            let data = guard
                                .payload
                                .drain(..trb.data_buffer().len())
                                .collect();
                            if let Err(e) =
                                self.complete_transfer(data, trb, &port_hdl)
                            {
                                slog::error!(
                                    self.log,
                                    "Failed to complete USB Interrupt-IN transfer after receiving packet: {e}"
                                );
                            }
                            guard.phase =
                                InterruptInPhase::WaitForTransferDescriptors;
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
                    // wait until we're resumed
                    let _guard = cvar
                        .wait_while(guard, |x| {
                            x.phase == InterruptInPhase::StoppedEndpoint
                        })
                        .unwrap();
                    // TODO:
                    // need we handle anything else here about reset/stopping the endpoint with a transction in flight?
                }
                InterruptInPhase::TerminateLoop => {
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
                payload: VecDeque::new(),
                period,
                phase: InterruptInPhase::WaitForTransferDescriptors,
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
            phase,
        } = value;
        let slot_id = SlotId::from(*slot_id);
        let endpoint_id = EndpointId::from(*endpoint_id);
        let data = Arc::new((
            Mutex::new(InterruptInData {
                transfers: transfers
                    .into_iter()
                    .map(|trbs| trbs.iter().map(From::from).collect())
                    .collect(),
                payload: payload.iter().copied().collect(),
                period: MINIMUM_INTERVAL_TIME.mul_f64(*period_ticks),
                phase: phase.into(),
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

    pub fn stop_endpoint(&self) -> Option<TransferTrb> {
        let mut guard = self.data.0.lock().unwrap();
        // FIXME is this necesssary (vs. just returning to WaitForTransferDescriptors, since a doorbell ring will necessarily have happened for that)
        // guard.phase = InterruptInPhase::StoppedEndpoint;
        guard.phase = InterruptInPhase::WaitForTransferDescriptors;

        // return the current TRB, whose pointer and cycle state values will be
        // written back to the transfer ring's dequeue pointer, and whose
        // transfer size will be sent in the resulting 'Stopped' Transfer Event
        let current_trb = guard.first_trb().copied();

        // clear out all cached TDs.
        guard.transfers.clear();
        self.data.1.notify_one();

        current_trb
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
            phase,
        } = ep;
        let guard = self.data.0.lock().unwrap();
        let mut guard = self
            .data
            .1
            .wait_while(guard, |x| x.phase == InterruptInPhase::Writing)
            .unwrap();
        let InterruptInData {
            transfers: transfers_mut,
            payload: payload_mut,
            period,
            phase: phase_mut,
        } = &mut *guard;
        if let InterruptInPhase::TerminateLoop = phase_mut {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                "Interrupt-IN endpoint periodic transfer loop was terminated for missing its handle to the xHC".to_string()
            ));
        }
        self.slot_id = SlotId::from(*slot_id);
        self.endpoint_id = EndpointId::from(*endpoint_id);
        *transfers_mut = transfers
            .iter()
            .map(|trbs| trbs.iter().map(From::from).collect())
            .collect();
        *payload_mut = payload.iter().copied().collect();
        *period = MINIMUM_INTERVAL_TIME.mul_f64(*period_ticks);
        *phase_mut = phase.into();
        Ok(())
    }

    pub fn export(
        &self,
    ) -> Result<super::migrate::EndpointV1, crate::migrate::MigrateStateError>
    {
        let guard = self.data.0.lock().unwrap();
        let guard = self
            .data
            .1
            .wait_while(guard, |x| x.phase == InterruptInPhase::Writing)
            .unwrap();
        let InterruptInData { transfers, payload, period, phase } = &*guard;
        if guard.phase == InterruptInPhase::TerminateLoop {
            // loop bailed from missing handle (TODO: better variant than 'not ready' unless we can make it become ready)
            return Err(crate::migrate::MigrateStateError::NotReadyForExport);
        }
        let period_ticks =
            period.as_secs_f64() / MINIMUM_INTERVAL_TIME.as_secs_f64();
        Ok(super::migrate::EndpointV1::InterruptIn(
            migrate::InterruptInEndpointV1 {
                transfers: transfers
                    .iter()
                    .map(|trbs| trbs.iter().map(From::from).collect())
                    .collect(),
                payload: payload.iter().copied().collect(),
                period_ticks,
                slot_id: u8::from(self.slot_id),
                endpoint_id: u8::from(self.endpoint_id),
                phase: phase.into(),
            },
        ))
    }
}

impl Drop for InterruptInEndpoint {
    fn drop(&mut self) {
        self.data.0.lock().unwrap().phase = InterruptInPhase::TerminateLoop;
        self.data.1.notify_one();
    }
}

pub mod migrate {
    use serde::{Deserialize, Serialize};

    use crate::hw::usb::xhci::rings::consumer::transfer::migrate::TransferTrbV1;

    #[derive(Serialize, Deserialize)]
    pub struct InterruptInEndpointV1 {
        pub transfers: Vec<Vec<TransferTrbV1>>, // maybe?
        pub payload: Vec<u8>,
        pub period_ticks: f64,
        pub slot_id: u8,
        pub endpoint_id: u8,
        pub phase: InterruptInPhaseV1,
    }

    #[derive(Serialize, Deserialize)]
    pub enum InterruptInPhaseV1 {
        WaitForTransferDescriptors,
        WaitForPayloadPeriod,
        WaitForPayloadOrTransfer,
        StoppedEndpoint,
        Writing,
        TerminateLoop,
    }

    impl From<&super::InterruptInPhase> for InterruptInPhaseV1 {
        fn from(value: &super::InterruptInPhase) -> Self {
            use super::InterruptInPhase::*;
            match value {
                WaitForTransferDescriptors => Self::WaitForTransferDescriptors,
                WaitForPayloadPeriod => Self::WaitForPayloadPeriod,
                WaitForPayloadOrTransfer => Self::WaitForPayloadOrTransfer,
                StoppedEndpoint => Self::StoppedEndpoint,
                Writing => Self::Writing,
                TerminateLoop => Self::TerminateLoop,
            }
        }
    }
    impl From<&InterruptInPhaseV1> for super::InterruptInPhase {
        fn from(value: &InterruptInPhaseV1) -> Self {
            use InterruptInPhaseV1::*;
            match value {
                WaitForTransferDescriptors => Self::WaitForTransferDescriptors,
                WaitForPayloadPeriod => Self::WaitForPayloadPeriod,
                WaitForPayloadOrTransfer => Self::WaitForPayloadOrTransfer,
                StoppedEndpoint => Self::StoppedEndpoint,
                Writing => Self::Writing,
                TerminateLoop => Self::TerminateLoop,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        sync::{Arc, Condvar, Mutex},
        time::Duration,
    };

    use crate::{
        accessors::Guard,
        common::GuestAddr,
        hw::{
            pci,
            usb::{
                usbdev::endpoint::{
                    interrupt::InterruptInPhase, test::test_pci_state,
                },
                xhci::{
                    bits::ring_data::{
                        EventRingSegment, Trb, TrbControlField,
                        TrbControlFieldNormal, TrbStatusField,
                        TrbStatusFieldTransfer, TrbType,
                    },
                    controller::XhciPortHandle,
                    device_slots::{EndpointId, SlotId},
                    interrupter::{EventSender, InterruptRegulation},
                    rings::{
                        consumer::transfer::TransferTrb,
                        producer::event::EventRing,
                    },
                },
            },
        },
        vmm::MemAccessed,
    };

    // memory layout
    // 1 KiB: destination for USB data transfers
    // 6 KiB: the event ring
    // 7 KiB: the event ring segment table
    // 8 KiB: the 'transfer ring'
    struct TestScaffold {
        _log: slog::Logger,
        pci_state: Arc<pci::DeviceState>,
        _interrupts: Arc<(Mutex<InterruptRegulation>, Condvar)>,
        _port_hdl: Arc<XhciPortHandle>,
        int_in_ep: super::InterruptInEndpoint,
        data_ref: Arc<(Mutex<super::InterruptInData>, Condvar)>,
    }

    impl TestScaffold {
        const ERDP: GuestAddr = GuestAddr(6 * 1024);
        const ERSTBA: GuestAddr = GuestAddr(7 * 1024);
        const TRDP: GuestAddr = GuestAddr(8 * 1024);
        fn new() -> Self {
            let _log = slog::Logger::root(slog::Discard, slog::o!());
            let pci_state = test_pci_state();
            let memctx = pci_state.acc_mem.access().unwrap();

            memctx.write_many(
                Self::ERSTBA,
                &[EventRingSegment {
                    base_address: Self::ERDP,
                    segment_trb_count: 16,
                }],
            );
            let event_ring =
                EventRing::new(Self::ERSTBA, 1, Self::ERDP, &memctx).unwrap();

            let event_sender = Arc::new(EventSender::new(&pci_state));
            let _interrupts = Arc::new((
                Mutex::new(InterruptRegulation::new_test(
                    event_ring, &pci_state, &_log,
                )),
                Condvar::new(),
            ));
            event_sender.set_interrupts(&_interrupts);

            let _port_hdl = Arc::new(XhciPortHandle::new_test(event_sender));

            let int_in_ep = super::InterruptInEndpoint::new(
                Duration::from_millis(10),
                Arc::downgrade(&_port_hdl),
                &pci_state,
                SlotId::from(1),
                EndpointId::from(3),
                &_log,
            );

            let data_ref = int_in_ep.data_ref().upgrade().unwrap();

            Self {
                _log,
                pci_state,
                _interrupts,
                _port_hdl,
                int_in_ep,
                data_ref,
            }
        }
        fn memctx(&self) -> Guard<'_, MemAccessed> {
            self.pci_state.acc_mem.access().unwrap()
        }

        fn normal_trb(tgt_addr: GuestAddr, len: usize) -> Trb {
            Trb {
                parameter: tgt_addr.0,
                status: TrbStatusField {
                    transfer: TrbStatusFieldTransfer(0)
                        .with_trb_transfer_length(len as u32),
                },
                control: TrbControlField {
                    normal: TrbControlFieldNormal(0)
                        .with_trb_type(TrbType::Normal)
                        .with_interrupt_on_completion(true),
                },
            }
        }
    }

    // simple golden-path test of a normal transfer request being submitted
    // and filled with data from the USB device.
    #[test]
    fn single_trb_transfer() {
        let test = TestScaffold::new();
        let tgt_addr = GuestAddr(1 * 1024);
        const TGT_LEN: usize = 7;

        test.memctx().write(tgt_addr, &[0u8; TGT_LEN]);

        test.int_in_ep.normal_transfer(vec![TransferTrb::new(
            &TestScaffold::normal_trb(tgt_addr, TGT_LEN),
            &TestScaffold::TRDP,
            None,
        )
        .unwrap()]);

        // still haven't provided a payload to endpoint, should be unchanged
        let value = test.memctx().read::<[u8; TGT_LEN]>(tgt_addr).unwrap();
        assert_eq!(*value, [0u8; TGT_LEN]);

        test.data_ref.0.lock().unwrap().set_payload(vec![1; TGT_LEN]);
        test.data_ref.1.notify_one();

        // wait until the payload and pending transfer have been processed together
        let (guard, timeout_result) = test
            .data_ref
            .1
            .wait_timeout_while(
                test.data_ref.0.lock().unwrap(),
                Duration::from_millis(100),
                |guard| {
                    !guard.payload.is_empty() || guard.first_trb().is_some()
                },
            )
            .unwrap();
        drop(guard);
        assert!(!timeout_result.timed_out());

        // payload was written
        let value = test.memctx().read::<[u8; TGT_LEN]>(tgt_addr).unwrap();
        assert_eq!(*value, [1u8; TGT_LEN]);

        // and completion event was enqueued
        let xfer_evt_trb =
            test.memctx().read::<Trb>(TestScaffold::ERDP).unwrap();
        assert_eq!(xfer_evt_trb.control.trb_type(), TrbType::TransferEvent);
    }

    // underlying function of Stop Endpoint Command when used to temporarily
    // stop transfer ring activity until a doorbell ring, as discussed in
    // xHCI 1.2 sect 4.6.9
    #[test]
    fn stop_resume_endpoint() {
        let test = TestScaffold::new();
        let tgt_addr = GuestAddr(1 * 1024);
        const TGT_LEN: usize = 7;

        test.memctx().write(tgt_addr, &[0u8; TGT_LEN]);

        // two-TRB TD
        test.int_in_ep.normal_transfer(vec![
            TransferTrb::new(
                &TestScaffold::normal_trb(tgt_addr, TGT_LEN),
                &TestScaffold::TRDP,
                None,
            )
            .unwrap(),
            TransferTrb::new(
                &TestScaffold::normal_trb(tgt_addr, TGT_LEN),
                &TestScaffold::TRDP.offset::<Trb>(1),
                None,
            )
            .unwrap(),
        ]);
        // additional dummy TD to verify absent after clearing cached TRBs
        test.int_in_ep.normal_transfer(vec![
            TransferTrb::new(
                &TestScaffold::normal_trb(tgt_addr, TGT_LEN),
                &TestScaffold::TRDP.offset::<Trb>(2),
                None,
            )
            .unwrap(),
            TransferTrb::new(
                &TestScaffold::normal_trb(tgt_addr, TGT_LEN),
                &TestScaffold::TRDP.offset::<Trb>(3),
                None,
            )
            .unwrap(),
        ]);

        // provide a payload for first TRB of TD
        test.data_ref.0.lock().unwrap().set_payload(vec![1; TGT_LEN]);
        test.data_ref.1.notify_one();

        // wait for it to be processed
        let (guard, timeout_result) = test
            .data_ref
            .1
            .wait_timeout_while(
                test.data_ref.0.lock().unwrap(),
                Duration::from_millis(100),
                |guard| {
                    !guard.payload.is_empty()
                        || guard.phase == InterruptInPhase::Writing
                },
            )
            .unwrap();
        drop(guard);
        assert!(!timeout_result.timed_out());
        let xfer_evt_trb =
            test.memctx().read::<Trb>(TestScaffold::ERDP).unwrap();
        assert_eq!(xfer_evt_trb.control.trb_type(), TrbType::TransferEvent);

        // stop endpoint with second TRB still in progress (awaiting payload)
        let in_progress_trb = test.int_in_ep.stop_endpoint().unwrap();
        assert_eq!(
            in_progress_trb.trb_pointer(),
            TestScaffold::TRDP.offset::<Trb>(1)
        );

        // provide a payload to stopped endpoint
        test.data_ref.0.lock().unwrap().set_payload(vec![2; TGT_LEN]);
        test.data_ref.1.notify_one();

        // second payload not yet written to guest memory
        let value = test.memctx().read::<[u8; TGT_LEN]>(tgt_addr).unwrap();
        assert_eq!(*value, [1u8; TGT_LEN]);

        // resume by re-applying TD, should advance us through Writing since
        // the payload is already present
        test.int_in_ep.normal_transfer(vec![in_progress_trb]);

        let (guard, timeout_result) = test
            .data_ref
            .1
            .wait_timeout_while(
                test.data_ref.0.lock().unwrap(),
                Duration::from_millis(100),
                |guard| {
                    !guard.payload.is_empty()
                        || guard.phase == InterruptInPhase::Writing
                },
            )
            .unwrap();
        drop(guard);
        assert!(!timeout_result.timed_out());

        // payload was written
        let value = test.memctx().read::<[u8; TGT_LEN]>(tgt_addr).unwrap();
        assert_eq!(*value, [2u8; TGT_LEN]);

        // and a second completion event was enqueued
        let xfer_evt_trb = test
            .memctx()
            .read::<Trb>(TestScaffold::ERDP.offset::<Trb>(1))
            .unwrap();
        assert_eq!(xfer_evt_trb.control.trb_type(), TrbType::TransferEvent);
    }
}

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
    hw::usb::xhci::{
        bits::ring_data::TrbCompletionCode,
        controller::XhciPortWakeHandle,
        device_slots::SlotId,
        rings::{
            consumer::transfer::{PointerOrImmediate, TDNormal},
            producer::event::EventInfo,
        },
    },
};

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
    transfers: VecDeque<TDNormal>,
    payload: Option<Vec<u8>>,
    period: Duration,
    ids: Option<(SlotId, u8)>,
    terminate: bool,
    block_migration: bool,
}

impl InterruptInData {
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = Some(payload);
    }
}

pub struct InterruptInEndpoint {
    data: Arc<(Mutex<InterruptInData>, Condvar)>,
    _jh: JoinHandle<()>,
}

fn periodic_xfer_wait_loop(
    weak_data: Weak<(Mutex<InterruptInData>, Condvar)>,
    port_hdl: Weak<XhciPortWakeHandle>,
) {
    while let Some(pair) = weak_data.upgrade() {
        let (mtx, cvar) = &*pair;
        let mut guard = cvar
            .wait_while(mtx.lock().unwrap(), |x| {
                x.transfers.is_empty() && !x.terminate
            })
            .unwrap();
        if guard.terminate {
            break;
        }

        // position in thread loop becomes state we must consider
        guard.block_migration = true;

        let timeout = guard.period;
        let (mut guard, timeout_result) = cvar
            .wait_timeout_while(guard, timeout, |x| x.payload.is_none())
            .unwrap();

        // unwrap: this loop is the only pop from transfers & we wait_while it's empty
        let xfer = guard.transfers.pop_front().unwrap();

        let PointerOrImmediate::Pointer(_) = xfer.data_buffer else {
            continue;
        };

        // TODO: no more than one TD consumed per ESIT if software gives us
        // too many at once (xHCI 1.2 sect 4.14.3)

        let Some(port_hdl) = port_hdl.upgrade() else { break };
        let Some((slot_id, endpoint_id)) = guard.ids else {
            continue;
        };
        if timeout_result.timed_out() {
            notify_short_packet(&xfer, &port_hdl, slot_id, endpoint_id);

            let mut guard = cvar
                .wait_while(guard, |x| {
                    x.payload.is_none()
                        && x.transfers.is_empty()
                        && !x.terminate
                })
                .unwrap();
            if let Some(data) = guard.payload.take() {
                complete_transfer(data, xfer, port_hdl, slot_id, endpoint_id);
            }

            guard.block_migration = false;
        } else {
            // unwrap: if we didn't time out, then payload is some
            let data = guard.payload.take().unwrap();
            complete_transfer(data, xfer, port_hdl, slot_id, endpoint_id);

            guard.block_migration = false;
        }
    }
    // TODO: slog::error!
    eprintln!("int-in loop: bailed");
}

fn notify_short_packet(
    xfer: &TDNormal,
    port_hdl: &Arc<XhciPortWakeHandle>,
    slot_id: SlotId,
    endpoint_id: u8,
) {
    let PointerOrImmediate::Pointer(region) = xfer.data_buffer else {
        unreachable!()
    };
    let completion_code = TrbCompletionCode::ShortPacket;
    let mut evts = Vec::new();
    let should_interrupt_xfer = false; // XXX xfer.interrupt_on_short_packet; XXX why not?
    evts.extend(should_interrupt_xfer.then_some(EventInfo::Transfer {
        trb_pointer: xfer.trb_pointer,
        completion_code,
        // xHCI 1.2 sect 4.10.1, table 6-22:
        // > The Length field of the Transfer Event shall be set to the residual number
        // > of bytes *not* written to the Transfer TRBs’ data buffer.
        //
        // xHCI 1.2 sect 4.10.1.1.2:
        // > TRB Transfer Length field shall indicate the residue bytes *in* the buffer.
        //
        // (both emphases mine) So... is this the right thing to do?
        trb_transfer_length: region.1 as u32,
        slot_id,
        endpoint_id,
        event_data: false,
    }));
    if let Some(event_data) = &xfer.event_data {
        let should_interrupt_ed = event_data.interrupt_on_short_packet;
        evts.extend(should_interrupt_ed.then_some(EventInfo::Transfer {
            trb_pointer: GuestAddr(event_data.event_data),
            completion_code,
            // xHCI 1.2 sect 4.10.1.1.1:
            // > an Event Data Transfer Event shall be generated with the
            // > Completion Code set to Short Packet and the Length field
            // > set to the actual number of bytes received by the TD.
            trb_transfer_length: 0,
            slot_id,
            endpoint_id,
            event_data: true,
        }))
    }
    probes::usb_interrupt_xfer_shortpacket!(|| (
        u8::from(slot_id),
        endpoint_id,
        region.0 .0,
        region.1,
        0,
    ));
    port_hdl.write_data_and_send_events(&[], region, evts);
}

fn complete_transfer(
    data: Vec<u8>,
    xfer: TDNormal,
    port_hdl: Arc<XhciPortWakeHandle>,
    slot_id: SlotId,
    endpoint_id: u8,
) {
    let PointerOrImmediate::Pointer(region) = xfer.data_buffer else {
        unreachable!()
    };
    // TODO: compare ptr.1 with data.len()
    let completion_code = TrbCompletionCode::Success;
    let should_interrupt_xfer =
        xfer.interrupt_on_short_packet || xfer.interrupt_on_completion;
    let mut evts = Vec::new();
    evts.extend(should_interrupt_xfer.then_some(EventInfo::Transfer {
        trb_pointer: xfer.trb_pointer,
        completion_code,
        // As above, so below.
        // The wording in the xHCI spec about this field evidently trips up a lot of devices:
        // https://github.com/torvalds/linux/commit/34b67198244f2d7d8409fa4eb76204c409c0c97e
        trb_transfer_length: 0,
        slot_id,
        endpoint_id,
        event_data: false,
    }));
    if let Some(event_data) = xfer.event_data {
        let should_interrupt_ed = event_data.interrupt_on_short_packet
            || event_data.interrupt_on_completion;
        evts.extend(should_interrupt_ed.then_some(EventInfo::Transfer {
            trb_pointer: GuestAddr(event_data.event_data),
            completion_code,
            // xHCI 1.2 sect 4.10.1.1.1
            // > If a Short Packet does not occur, then the last Event Data Transfer TRB shall
            // > generate an Event Data Transfer Event with its Completion Code = Success
            // > (assuming no errors) and TRB Transfer Length field equal to the number of bytes
            // > transferred since the beginning of the TD
            trb_transfer_length: data.len() as u32,
            slot_id,
            endpoint_id,
            event_data: true,
        }))
    }
    probes::usb_interrupt_xfer_complete!(|| (
        u8::from(slot_id),
        endpoint_id,
        region.0 .0,
        region.1,
    ));
    port_hdl.write_data_and_send_events(&data, region, evts);
}

impl InterruptInEndpoint {
    pub fn new(period: Duration, port_hdl: Weak<XhciPortWakeHandle>) -> Self {
        let data = Arc::new((
            Mutex::new(InterruptInData {
                transfers: VecDeque::new(),
                payload: None,
                period,
                ids: None,
                terminate: false,
                block_migration: false,
            }),
            Condvar::new(),
        ));
        let weak_data = Arc::downgrade(&data);
        let _jh = std::thread::spawn(move || {
            periodic_xfer_wait_loop(weak_data, port_hdl);
        });
        Self { data, _jh }
    }
    pub fn normal(
        &self,
        slot_id: SlotId,
        endpoint_id: u8,
        normal_td: TDNormal,
    ) {
        let mut data = self.data.0.lock().unwrap();
        data.transfers.push_back(normal_td);
        data.ids = Some((slot_id, endpoint_id));
        self.data.1.notify_one();
    }
    pub fn data_ref(&self) -> Weak<(Mutex<InterruptInData>, Condvar)> {
        Arc::downgrade(&self.data)
    }

    pub fn import(
        &mut self,
        ep: &super::migrate::EndpointV1,
    ) -> Result<(), crate::migrate::MigrateStateError> {
        todo!()
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
            ids,
            terminate,
            block_migration: _,
        } = &*guard;
        todo!()
    }
}
impl Drop for InterruptInEndpoint {
    fn drop(&mut self) {
        self.data.0.lock().unwrap().terminate = true;
        self.data.1.notify_one();
    }
}

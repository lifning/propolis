// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::JoinHandle,
    time::Duration,
};

use crate::hw::usb::xhci::{
    bits::ring_data::TrbCompletionCode,
    controller::XhciPortWakeHandle,
    device_slots::SlotId,
    rings::{
        consumer::transfer::{PointerOrImmediate, TDNormal},
        producer::event::EventInfo,
    },
};

pub struct InterruptInData {
    transfers: VecDeque<TDNormal>,
    payload: Option<Vec<u8>>,
    period: Duration,
    ids: Option<(SlotId, u8)>,
}

impl InterruptInData {
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = Some(payload);
    }
}

pub struct InterruptInEndpoint {
    data: Arc<(Mutex<InterruptInData>, Condvar)>,
    jh: JoinHandle<()>,
}

fn periodic_xfer_wait_loop(
    weak_data: Weak<(Mutex<InterruptInData>, Condvar)>,
    port_hdl: Weak<XhciPortWakeHandle>,
) {
    while let Some(pair) = weak_data.upgrade() {
        let (mtx, cvar) = &*pair;
        // eprintln!("int-in: acquire main lock");
        let guard = mtx.lock().unwrap();
        // eprintln!("int-in: wait 1");
        let guard = cvar.wait_while(guard, |x| x.transfers.is_empty()).unwrap();

        // eprintln!("int-in: wait 2");
        let timeout = guard.period;
        let (mut guard, timeout_result) = cvar
            .wait_timeout_while(guard, timeout, |x| x.payload.is_none())
            .unwrap();

        // eprintln!("int-in: waits over");
        // unwrap: this loop is the only pop from transfers & we wait_while it's empty
        let xfer = guard.transfers.pop_front().unwrap();
        let PointerOrImmediate::Pointer(region) = xfer.data_buffer else {
            continue;
        };
        // TODO: no more than one TD consumed per ESIT if software gives us
        // too many at once (xHCI 1.2 sect 4.14.3)

        let Some(port_hdl) = port_hdl.upgrade() else { break };
        let Some((slot_id, endpoint_id)) = guard.ids else {
            continue;
        };
        if timeout_result.timed_out() {
            // eprintln!("timed out. {} tds", guard.transfers.len());
            let evt = if xfer.interrupt_on_short_packet {
                // TODO: event w/ShortPacket
                Some(EventInfo::Transfer {
                    trb_pointer: xfer.trb_pointer,
                    completion_code: TrbCompletionCode::ShortPacket,
                    // xHCI 1.2 sect 4.10.1, table 6-22:
                    // > The Length field of the Transfer Event shall be set to the residual number
                    // > of bytes *not* written to the Transfer TRBs’ data buffer.
                    //
                    // xHCI 1.2 sect 4.10.1.1.2:
                    // > TRB Transfer Length field shall indicate the residue bytes *in* the buffer.
                    //
                    // (both emphases mine)
                    trb_transfer_length: region.1 as u32,
                    slot_id,
                    endpoint_id,
                    event_data: false,
                })
            } else {
                None
            };
            port_hdl.finish_xfer(&[], region, evt);
        } else {
            // eprintln!("success. {} tds", guard.transfers.len());
            // unwrap: if we didn't time out, then payload is some
            let data = guard.payload.take().unwrap();
            // TODO: compare ptr.1 with data.len()
            let evt = if xfer.interrupt_on_completion {
                Some(EventInfo::Transfer {
                    trb_pointer: xfer.trb_pointer,
                    completion_code: TrbCompletionCode::Success,
                    // As above, so below.
                    // The wording in the xHCI spec about this field evidently trips up a lot of devices:
                    // https://github.com/torvalds/linux/commit/34b67198244f2d7d8409fa4eb76204c409c0c97e
                    trb_transfer_length: 0,
                    slot_id,
                    endpoint_id,
                    event_data: false,
                })
            } else {
                None
            };
            port_hdl.finish_xfer(&data, region, evt);
        }
        // eprintln!("int-in: release report lock");
    }
    eprintln!("int-in loop: bailed");
}

impl InterruptInEndpoint {
    pub fn new(period: Duration, port_hdl: Weak<XhciPortWakeHandle>) -> Self {
        let data = Arc::new((
            Mutex::new(InterruptInData {
                transfers: VecDeque::new(),
                payload: None,
                period,
                ids: None,
            }),
            Condvar::new(),
        ));
        let weak_data = Arc::downgrade(&data);
        let jh = std::thread::spawn(move || {
            periodic_xfer_wait_loop(weak_data, port_hdl);
        });
        Self { data, jh }
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
}

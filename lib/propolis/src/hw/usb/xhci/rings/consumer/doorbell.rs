// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::common::GuestAddr;
use crate::hw::usb::xhci::controller::XhciState;
use crate::hw::usb::xhci::device_slots::{EndpointId, SlotId};
use crate::hw::usb::xhci::rings::consumer;
use crate::hw::usb::xhci::rings::producer::event::EventInfo;
use crate::vmm::MemCtx;

use super::command::CommandInfo;
use super::transfer::{TransferEventParams, TransferInfo};
use super::TrbCompletionCode;

pub fn command_ring_stop(
    state: &mut XhciState,
    completion_code: TrbCompletionCode,
    memctx: &MemCtx,
    log: &slog::Logger,
) {
    state.crcr.set_command_ring_running(false);

    let cmd_trb_addr = state
        .command_ring
        .as_ref()
        .map(|cmd_ring| cmd_ring.current_dequeue_pointer())
        .unwrap_or(GuestAddr(0));
    let event_info = EventInfo::CommandCompletion {
        completion_code,
        slot_id: SlotId::from(0),
        cmd_trb_addr,
    };
    // xHCI 1.2 table 5-24
    if let Err(e) = state.event_sender.enqueue_event(event_info, false) {
        slog::error!(log, "couldn't inform xHCD of stopped Control Ring: {e}");
    } else {
        slog::debug!(log, "stopped Command Ring with {completion_code:?}");
    }
}

pub fn process_transfer_ring(
    state: &mut XhciState,
    slot_id: SlotId,
    endpoint_id: EndpointId,
    memctx: &MemCtx,
    log: &slog::Logger,
) {
    loop {
        let Some(xfer_ring) =
            state.dev_slots.transfer_ring(slot_id, endpoint_id)
        else {
            break;
        };

        slog::trace!(log, "Transfer Ring at {:#x}", xfer_ring.start_addr.0);

        match xfer_ring
            .dequeue_work_item(&memctx)
            .and_then(TransferInfo::try_from)
        {
            Ok(xfer) => {
                let usbdev = match state.dev_slots.usbdev_for_slot(slot_id) {
                    Ok(dev) => dev,
                    Err(e) => {
                        slog::error!(log, "No USB device in slot: {e}");
                        break;
                    }
                };
                // WIP: move the event-enqueueing to the USB device's transfer handling
                xfer.run(slot_id, endpoint_id, usbdev, &memctx, log);
            }
            Err(consumer::Error::EmptyTransferDescriptor) => {
                slog::trace!(log, "Transfer Ring empty");
                break;
            }
            Err(e) => {
                slog::error!(log, "dequeueing TD from endpoint failed: {e}");
                break;
            }
        }
    }
}

pub fn process_command_ring(
    state: &mut XhciState,
    memctx: &MemCtx,
    log: &slog::Logger,
) {
    loop {
        if !state.crcr.command_ring_running() {
            break;
        }

        let cmd_opt = if let Some(ref mut cmd_ring) = state.command_ring {
            slog::trace!(
                log,
                "executing Command Ring from {:#x}",
                cmd_ring.start_addr.0,
            );
            match cmd_ring.dequeue_work_item(&memctx) {
                Ok(work_item) => Some(work_item),
                Err(consumer::Error::CommandDescriptorSize) => {
                    // HACK - matching cycle bits in uninitialized memory trips this,
                    // should do away with this error entirely
                    None
                }
                Err(e) => {
                    slog::error!(
                        log,
                        "Failed to dequeue item from Command Ring: {e}"
                    );
                    None
                }
            }
        } else {
            slog::error!(log, "Command Ring not initialized via CRCR yet");
            None
        };
        if let Some(cmd_desc) = cmd_opt {
            let cmd_trb_addr = cmd_desc.1;
            match CommandInfo::try_from(cmd_desc) {
                Ok(cmd) => {
                    slog::trace!(log, "Command TRB running: {cmd:?}");
                    if let Err(e) = cmd.run(
                        cmd_trb_addr,
                        &mut state.dev_slots,
                        memctx,
                        &state.event_sender,
                    ) {
                        slog::error!(
                            log,
                            "couldn't signal Command TRB completion: {e}"
                        );
                    }
                }
                Err(e) => slog::error!(log, "Command Ring processing: {e}"),
            }
        } else {
            // command ring absent or empty
            break;
        }
    }
}

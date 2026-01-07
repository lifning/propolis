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
use super::transfer::TransferInfo;
use super::TrbCompletionCode;

pub fn command_ring_stop(
    state: &mut XhciState,
    completion_code: TrbCompletionCode,
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
    while let Some(raw_td) =
        state.dev_slots.transfer_ring(slot_id, endpoint_id).map(|xfer_ring| {
            slog::trace!(log, "Transfer Ring at {:#x}", xfer_ring.start_addr.0);
            xfer_ring.dequeue_work_item(&memctx)
        })
    {
        match raw_td.and_then(TransferInfo::try_from) {
            Ok(xfer) => {
                // unwrap: checked at start of fn
                let Ok(usbdev) = state.dev_slots.usbdev_for_slot(slot_id)
                else {
                    slog::error!(log, "No USB device in {slot_id:?}");
                    return;
                };
                let trb_ptr_opt = xfer.first_trb_pointer();
                if let Err(e) = xfer.run(
                    slot_id,
                    endpoint_id,
                    usbdev,
                    &memctx,
                    &state.event_sender,
                    log,
                ) {
                    slog::error!(log, "Error executing Transfer Ring TRB: {e}");
                    if let Some(trb_pointer) = trb_ptr_opt {
                        // TODO: do we send an error for Event Data TRBs that were part of the TD too?
                        let evt_info = EventInfo::Transfer {
                            trb_pointer,
                            completion_code:
                                TrbCompletionCode::UsbTransactionError,
                            trb_transfer_length: 0,
                            slot_id,
                            endpoint_id,
                            event_data: false,
                        };
                        if let Err(e) =
                            state.event_sender.enqueue_event(evt_info, false)
                        {
                            slog::error!(log, "Failed to enqueue USB Transaction Error event: {e}");
                        }
                    }
                }
            }
            Err(consumer::Error::EmptyTransferDescriptor) => {
                slog::trace!(log, "Transfer Ring empty");
                break;
            }
            Err(consumer::Error::IncompleteWorkItem(trbs)) => {
                // TODO: special-case handling for storing them and completing it
                // (would need adjustment to command trb impls as well)
                slog::warn!(log, "Rewound dequeue pointer after trying to pull incomplete TD from Transfer Ring: {trbs:?}");
                break;
            }
            Err(e) => {
                slog::error!(log, "Dequeueing TD from endpoint failed: {e}");
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

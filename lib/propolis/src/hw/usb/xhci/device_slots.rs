// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use zerocopy::{FromBytes, FromZeros};

use crate::common::GuestAddr;
use crate::hw::pci;
use crate::hw::usb::usbdev::{UsbDevice, UsbDeviceType};
use crate::hw::usb::xhci::interrupter::EventSender;
use crate::hw::usb::xhci::rings::producer::event::EventInfo;
use crate::vmm::MemCtx;

use super::bits::device_context::{
    EndpointContext, EndpointState, InputControlContext, SlotContext,
    SlotContextFirst, SlotState,
};
use super::bits::ring_data::TrbCompletionCode;
use super::port::PortId;
use super::rings::consumer::transfer::TransferRing;
use super::{MAX_DEVICE_SLOTS, MAX_PORTS};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cannot use 0-value SlotId as array index")]
    SlotIdZero,
    #[error("{0:?} greater than max device slots ({MAX_DEVICE_SLOTS})")]
    SlotIdBounds(SlotId),
    #[error("USB device not found in {0:?}")]
    NoUsbDevInSlot(SlotId),
    #[error("{0:?} not enabled")]
    SlotNotEnabled(SlotId),
    #[error("No port associated with {0:?}")]
    NoPortAssociatedWithSlot(SlotId),
    #[error(
        "Could not set TR Dequeue Pointer {0:x?} and cycle state {1}: {2}"
    )]
    SetTRDPFailed(GuestAddr, bool, super::rings::consumer::Error),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct SlotId(u8);

impl From<u8> for SlotId {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<SlotId> for u8 {
    fn from(value: SlotId) -> Self {
        value.0
    }
}

impl SlotId {
    pub fn as_index(&self) -> Result<usize, Error> {
        if self.0 <= MAX_DEVICE_SLOTS {
            (self.0 as usize).checked_sub(1).ok_or(Error::SlotIdZero)
        } else {
            Err(Error::SlotIdBounds(*self))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct EndpointId(u8);

impl From<u8> for EndpointId {
    fn from(value: u8) -> Self {
        if value > 31 {
            panic!("EndpointId must be <= 31, got {value}")
        } else {
            Self(value)
        }
    }
}

impl From<EndpointId> for u8 {
    fn from(value: EndpointId) -> Self {
        value.0
    }
}

struct MemCtxValue<'a, T: Copy + FromBytes> {
    value: T,
    addr: GuestAddr,
    memctx: &'a MemCtx,
}

impl<T: Copy + FromBytes + core::fmt::Debug> core::fmt::Debug
    for MemCtxValue<'_, T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        core::fmt::Debug::fmt(&self.value, f)
    }
}

impl<'a, T: Copy + FromBytes> MemCtxValue<'a, T> {
    pub fn new(addr: GuestAddr, memctx: &'a MemCtx) -> Option<Self> {
        memctx.read(addr).map(move |value| Self { value: *value, addr, memctx })
    }
    pub fn mutate(&mut self, mut f: impl FnMut(&mut T)) -> bool {
        f(&mut self.value);
        self.memctx.write(self.addr, &self.value)
    }
}

impl<T: Copy + FromBytes> Deref for MemCtxValue<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

struct DeviceSlot {
    endpoints: HashMap<EndpointId, TransferRing>,
    port_address: Option<PortId>,
}
impl DeviceSlot {
    fn new() -> Self {
        Self { endpoints: HashMap::new(), port_address: None }
    }

    fn set_endpoint_transfer_ring(
        &mut self,
        endpoint_id: EndpointId,
        ep_ctx: EndpointContext,
    ) {
        self.endpoints.insert(
            endpoint_id,
            // unwrap: tr_dequeue_pointer's lower 4 bits are 0, will always be TRB-aligned
            TransferRing::new(
                ep_ctx.tr_dequeue_pointer(),
                ep_ctx.dequeue_cycle_state(),
            )
            .unwrap(),
        );
    }

    fn unset_endpoint_transfer_ring(&mut self, endpoint_id: EndpointId) {
        self.endpoints.remove(&endpoint_id);
    }

    fn import(
        &mut self,
        value: &migrate::DeviceSlotV1,
    ) -> Result<(), crate::migrate::MigrateStateError> {
        let migrate::DeviceSlotV1 { endpoints, port_address } = value;
        self.port_address = port_address
            .map(PortId::try_from)
            .transpose()
            .map_err(crate::migrate::MigrateStateError::ImportFailed)?;
        for ep_id in
            self.endpoints.keys().copied().map(u8::from).collect::<Vec<_>>()
        {
            if !endpoints.contains_key(&ep_id) {
                self.endpoints.remove(&EndpointId::from(ep_id));
            }
        }
        for (ep_id, ep) in endpoints {
            self.endpoints.insert(
                EndpointId::from(*ep_id),
                TransferRing::try_from(ep).map_err(|e| {
                    crate::migrate::MigrateStateError::ImportFailed(format!(
                        "{e:?}"
                    ))
                })?,
            );
        }
        Ok(())
    }

    fn export(&self) -> migrate::DeviceSlotV1 {
        let Self { endpoints, port_address } = self;
        migrate::DeviceSlotV1 {
            endpoints: endpoints
                .iter()
                .map(|(ep_id, ep)| (u8::from(*ep_id), ep.export()))
                .collect(),
            port_address: port_address.map(|port_id| port_id.as_raw_id()),
        }
    }
}

pub struct DeviceSlotTable {
    /// Device Context Base Address Array Pointer (DCBAAP)
    ///
    /// Points to an array of address pointers referencing the device context
    /// structures for each attached device.
    ///
    /// See xHCI 1.2 Section 5.4.6
    dcbaap: Option<GuestAddr>,
    slots: Vec<Option<DeviceSlot>>,
    port_devs: [Option<Box<dyn UsbDevice>>; MAX_PORTS as usize],
    log: slog::Logger,
}

impl DeviceSlotTable {
    pub fn new(log: slog::Logger) -> Self {
        Self {
            dcbaap: None,
            slots: vec![],
            // seemingly asinine, but NullUsbDevice: !Copy,
            // which invalidates Option<>
            port_devs: [None, None, None, None, None, None, None, None],
            log,
        }
    }
}

impl DeviceSlotTable {
    pub fn set_dcbaap(&mut self, addr: GuestAddr) {
        self.dcbaap = Some(addr)
    }
    pub fn dcbaap(&self) -> Option<&GuestAddr> {
        self.dcbaap.as_ref()
    }

    pub fn attach_to_root_hub_port_address(
        &mut self,
        port_id: PortId,
        usb_dev: Box<dyn UsbDevice>,
    ) -> Result<(), Box<dyn UsbDevice>> {
        if let Some(dev) = self.port_devs[port_id.as_index()].replace(usb_dev) {
            Err(dev)
        } else {
            Ok(())
        }
    }

    pub fn detach_all_for_reset(
        &mut self,
    ) -> impl Iterator<Item = (PortId, Box<dyn UsbDevice>)> + '_ {
        self.port_devs.iter_mut().enumerate().flat_map(|(i, opt_dev)| {
            opt_dev
                .take()
                .map(|dev| (PortId::try_from(i as u8 + 1).unwrap(), dev))
        })
    }

    pub fn usbdev_for_slot(
        &mut self,
        slot_id: SlotId,
    ) -> Result<&mut Box<dyn UsbDevice>, Error> {
        let slot = self.slot_mut(slot_id)?;
        let idx = slot
            .port_address
            .ok_or(Error::NoPortAssociatedWithSlot(slot_id))?;
        self.port_devs[idx.as_index()]
            .as_mut()
            .ok_or(Error::NoUsbDevInSlot(slot_id))
    }

    fn slot(&self, slot_id: SlotId) -> Result<&DeviceSlot, Error> {
        self.slots
            .get(slot_id.as_index()?)
            .and_then(|res| res.as_ref())
            .ok_or(Error::SlotNotEnabled(slot_id))
    }

    fn slot_mut(&mut self, slot_id: SlotId) -> Result<&mut DeviceSlot, Error> {
        self.slots
            .get_mut(slot_id.as_index()?)
            .and_then(|res| res.as_mut())
            .ok_or(Error::SlotNotEnabled(slot_id))
    }

    fn endpoint_context(
        slot_addr: GuestAddr,
        endpoint_id: EndpointId,
        memctx: &'_ MemCtx,
    ) -> Option<MemCtxValue<'_, EndpointContext>> {
        const { assert!(size_of::<SlotContext>() == size_of::<EndpointContext>()) };
        MemCtxValue::new(
            slot_addr.offset::<SlotContext>(1).offset::<EndpointContext>(
                u8::from(endpoint_id).checked_sub(1)? as usize,
            ),
            memctx,
        )
    }

    pub fn enable_slot(&mut self, slot_type: u8) -> Option<SlotId> {
        // USB protocol slot type is 0 (xHCI 1.2 section 7.2.2.1.4)
        if slot_type != 0 {
            return None;
        }
        let slot_id_opt = self
            .slots
            .iter()
            .position(Option::is_none)
            .map(|i| SlotId::from(i as u8 + 1))
            .or_else(|| {
                if self.slots.len() < MAX_DEVICE_SLOTS as usize {
                    self.slots.push(None);
                    Some(SlotId::from(self.slots.len() as u8))
                } else {
                    None
                }
            });
        if let Some(slot_id) = slot_id_opt {
            self.slots[slot_id.as_index().unwrap()] = Some(DeviceSlot::new());
        }
        slot_id_opt
    }

    pub fn disable_slot(
        &mut self,
        slot_id: SlotId,
        memctx: &MemCtx,
    ) -> TrbCompletionCode {
        if self.slot(slot_id).is_ok() {
            // terminate any transfers on the slot

            // slot ctx is first element of dev context table
            if let Some(slot_ptr) = self.dev_context_addr(slot_id, memctx) {
                if let Some(mut slot_ctx) =
                    MemCtxValue::<SlotContextFirst>::new(slot_ptr, memctx)
                {
                    slot_ctx.mutate(|ctx| {
                        ctx.set_slot_state(SlotState::DisabledEnabled)
                    });
                }
            }
            // unwrap: self.slot(slot_id).is_ok(), above
            self.slots[slot_id.as_index().unwrap()] = None;
            TrbCompletionCode::Success
        } else {
            TrbCompletionCode::SlotNotEnabledError
        }
    }

    fn dev_context_addr(
        &self,
        slot_id: SlotId,
        memctx: &MemCtx,
    ) -> Option<GuestAddr> {
        self.dcbaap.as_ref().and_then(|base_ptr| {
            memctx
                .read::<u64>(
                    // lower 6 bits are reserved (xHCI 1.2 table 6-2)
                    // software is supposed to clear them to 0, but
                    // let's double-tap for safety's sake.
                    (*base_ptr & !0b11_1111)
                        // NOTE: index 0 *is* reserved in DCBAA, so we don't SlotId::as_index()
                        .offset::<u64>(u8::from(slot_id) as usize),
                )
                .map(|x| GuestAddr(*x))
        })
    }

    /// xHCI 1.2 sect 4.6.5
    pub fn address_device(
        &mut self,
        slot_id: SlotId,
        input_context_ptr: GuestAddr,
        block_set_address_request: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        if self.slot(slot_id).is_err() {
            return Some(TrbCompletionCode::SlotNotEnabledError);
        }

        // xHCI 1.2 Figure 6-5: differences between input context indeces
        // and device context indeces.

        let out_slot_addr = self.dev_context_addr(slot_id, memctx)?;
        let out_ep0_addr = out_slot_addr.offset::<SlotContext>(1);

        let in_slot_addr = input_context_ptr.offset::<InputControlContext>(1);
        let in_ep0_addr = in_slot_addr.offset::<SlotContext>(1);

        let mut slot_ctx = memctx.read::<SlotContext>(in_slot_addr)?;
        let mut ep0_ctx = memctx.read::<EndpointContext>(in_ep0_addr)?;

        // we'll just use the root hub port number as the USB address for
        // whatever's in this slot
        if let Ok(port_id) = slot_ctx.root_hub_port_number() {
            // unwrap: we've checked is_none already
            self.slot_mut(slot_id).unwrap().port_address = Some(port_id);
        }

        let usb_addr = if block_set_address_request {
            if matches!(slot_ctx.slot_state(), SlotState::DisabledEnabled) {
                // set output slot context state to default
                slot_ctx.set_slot_state(SlotState::Default);

                0 // because BSR, just set field to 0 in slot ctx
            } else {
                return Some(TrbCompletionCode::ContextStateError);
            }
        } else if matches!(
            slot_ctx.slot_state(),
            SlotState::DisabledEnabled | SlotState::Default
        ) {
            // we'll just use the root hub port number as our USB address
            if let Some(port_id) =
                self.slot(slot_id).ok().and_then(|slot| slot.port_address)
            {
                // issue 'set address' to USB device itself,
                // which configures its Default Control Endpoint
                self.usbdev_for_slot(slot_id)
                    .ok()?
                    .set_address(slot_id, port_id);

                // set output slot context state to addressed
                slot_ctx.set_slot_state(SlotState::Addressed);

                port_id.as_raw_id()
            } else {
                // root hub port ID in slot ctx not in range of our ports
                return Some(TrbCompletionCode::ContextStateError);
            }
        } else {
            return Some(TrbCompletionCode::ContextStateError);
        };

        // set usb device address in output slot ctx to chosen addr (or 0 if BSR)
        slot_ctx.set_usb_device_address(usb_addr);
        // copy input slot ctx to output slot ctx
        memctx.write(out_slot_addr, &slot_ctx);

        // set output ep0 state to running
        ep0_ctx.set_endpoint_state(EndpointState::Running);
        // copy input ep0 ctx to output ep0 ctx
        memctx.write(out_ep0_addr, &ep0_ctx);

        slog::trace!(
            self.log,
            "slot_ctx: in@{:#x} out@{:#x} {slot_ctx:?}",
            in_slot_addr.0,
            out_slot_addr.0
        );
        slog::trace!(
            self.log,
            "ep0_ctx: in@{:#x} out@{:#x} {ep0_ctx:?}",
            in_ep0_addr.0,
            out_ep0_addr.0
        );

        // unwrap: we've checked self.slot() at function begin,
        // we just can't hold a &mut for the whole duration
        let device_slot = self.slot_mut(slot_id).unwrap();

        // add default control endpoint to scheduling list
        device_slot.set_endpoint_transfer_ring(EndpointId::from(1), *ep0_ctx);

        Some(TrbCompletionCode::Success)
    }

    /// See xHCI 1.2 sect 3.3.5, 4.6.6, 6.2.3.2, and figure 4-3.
    // NOTE: if we were concerned with bandwidth/resource limits
    // as real hardware is, this function would
    // - add/subtract resources allocated to the endpoint from a
    //   Resource Required variable
    // - if endpoint is periodic, add/subtract bandwidth allocated to the
    //   endpoint from a Bandwidth Required variable
    // in various points throughout the function where contexts are added
    // and dropped. these steps (as outlined in xHCI 1.2 sect 4.6.6)
    // are elided, since we're a virtual machine and can both bend reality
    // to our will and (at time of writing) control what devices are
    // allowed to be connected to us in the first place.
    pub fn configure_endpoint(
        &mut self,
        input_context_ptr: GuestAddr,
        slot_id: SlotId,
        deconfigure: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        // following xHC behavior described in xHCI 1.2 sect 4.6.6:
        // if not previously enabled by an Enable Slot command
        if self.slot(slot_id).is_err() {
            return Some(TrbCompletionCode::SlotNotEnabledError);
        }

        // retrieve the output device context of the selected device slot
        let out_slot_addr = self.dev_context_addr(slot_id, memctx)?;
        let mut out_slot_ctx =
            MemCtxValue::<SlotContext>::new(out_slot_addr, memctx)?;

        // if output dev context slot state is not addressed or configured
        if !matches!(
            out_slot_ctx.slot_state(),
            SlotState::Addressed | SlotState::Configured
        ) {
            return Some(TrbCompletionCode::ContextStateError);
        }

        // setting deconfigure (DC) is equivalent to setting Input Context
        // Drop Context flags 2..=31 to 1 and Add Context flags 2..=31 to 0.
        // if DC=1, Input Context Pointer field shall be *ignored* by the xHC,
        // the Output Slot Context 'Context Entries' field shall be set to 1.
        let input_ctx = if deconfigure {
            let mut in_ctx = InputControlContext::new_zeroed();
            for i in (2..=31).map(EndpointId::from) {
                in_ctx.set_add_context_bit(i, false);
                in_ctx.set_drop_context_bit(i, true);
            }
            out_slot_ctx.mutate(|out_ctx| {
                out_ctx.set_context_entries(1);
            });
            in_ctx
        } else {
            *memctx.read::<InputControlContext>(input_context_ptr)?
        };

        // if the output slot state is configured
        if deconfigure {
            if out_slot_ctx.slot_state() == SlotState::Configured {
                // for each endpoint context not in disabled state:
                for i in (1..=31).map(EndpointId::from) {
                    let mut out_ep_ctx =
                        Self::endpoint_context(out_slot_addr, i, memctx)?;
                    if out_ep_ctx.endpoint_state() != EndpointState::Disabled {
                        // set output EP State field to disabled
                        out_ep_ctx.mutate(|ctx| {
                            ctx.set_endpoint_state(EndpointState::Disabled)
                        });
                        // xHCI 1.2 sect 4.5.3.5: only doorbell enabled when
                        // slot is in Addressed is control EP 0.
                        // unwrap: returned early if self.slot(slot_id) was none
                        self.slot_mut(slot_id)
                            .unwrap()
                            .unset_endpoint_transfer_ring(i);
                    }
                }
                // set Slot State in output slot context to Addressed
                out_slot_ctx
                    .mutate(|ctx| ctx.set_slot_state(SlotState::Addressed));
            }
        }
        // if output slot state is addressed or configured and DC=0
        // (for slot state, we've already returned ContextStateError otherwise)
        else {
            let in_slot_addr =
                input_context_ptr.offset::<InputControlContext>(1);

            // for each endpoint context designated by a Drop Context flag = 2
            for i in (2..=31).map(EndpointId::from) {
                // unwrap: only None when index not in 2..=31
                if input_ctx.drop_context_bit(i).unwrap() {
                    let mut out_ep_ctx =
                        Self::endpoint_context(out_slot_addr, i, memctx)?;
                    // set output EP State field to disabled
                    out_ep_ctx.mutate(|ctx| {
                        ctx.set_endpoint_state(EndpointState::Disabled)
                    });
                    // drop endpoint from 'pipe scheduling list', in effect
                    self.slot_mut(slot_id)
                        .unwrap()
                        .unset_endpoint_transfer_ring(i);
                }
            }

            // if all input endpoint contexts with Add Context = 1 are valid
            for i in (1..=31).map(EndpointId::from) {
                if input_ctx.add_context_bit(i).unwrap() {
                    let in_ep_ctx =
                        Self::endpoint_context(in_slot_addr, i, memctx)?;
                    // not all input ep contexts valid -> Parameter Error
                    if !in_ep_ctx.valid_for_configure_endpoint() {
                        return Some(TrbCompletionCode::ParameterError);
                    }
                }
            }

            let mut any_endpoint_enabled = false;
            // for each endpoint context designated by an Add Context flag = 1
            for i in (1..=31).map(EndpointId::from) {
                let mut out_ep_ctx =
                    Self::endpoint_context(out_slot_addr, i, memctx)?;

                // unwrap: only None when index > 31
                if input_ctx.add_context_bit(i).unwrap() {
                    // copy all fields of input ep context to output ep context;
                    // set output EP state field to running.
                    let in_ep_ctx =
                        Self::endpoint_context(in_slot_addr, i, memctx)?;
                    self.usbdev_for_slot(slot_id)
                        .unwrap()
                        .configure_endpoint(slot_id, i, &in_ep_ctx);
                    out_ep_ctx.mutate(|ctx| {
                        *ctx = *in_ep_ctx;
                        ctx.set_endpoint_state(EndpointState::Running);
                    });
                    // load the xHC enqueue and dequeue pointers with the
                    // value of TR Dequeue Pointer field from Endpoint Context
                    let device_slot = self.slot_mut(slot_id).unwrap();
                    device_slot.set_endpoint_transfer_ring(i, *out_ep_ctx);
                }

                if out_ep_ctx.endpoint_state() != EndpointState::Disabled {
                    any_endpoint_enabled = true;
                }
            }

            // if all Endpoints are Disabled
            if !any_endpoint_enabled {
                out_slot_ctx.mutate(|ctx| {
                    // set Slot State in Output Slot Context to Addressed
                    ctx.set_slot_state(SlotState::Addressed);
                    // set Context Entries in Output Slot Context to 1
                    ctx.set_context_entries(1);
                });
            } else {
                out_slot_ctx.mutate(|ctx| {
                    // set Slot State in Output Slot Context to Configured
                    ctx.set_slot_state(SlotState::Configured);
                    // set Context Entries to the index of the last valid
                    // Endpoint Context in the Output Device Context
                });
            }
        }

        Some(TrbCompletionCode::Success)
    }

    /// xHCI 1.2 sect 4.6.7, 6.2.2.3, 6.2.3.3
    pub fn evaluate_context(
        &self,
        slot_id: SlotId,
        input_context_ptr: GuestAddr,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        if self.slot(slot_id).is_err() {
            return Some(TrbCompletionCode::SlotNotEnabledError);
        }

        let input_ctx =
            memctx.read::<InputControlContext>(input_context_ptr)?;

        // retrieve the output device context of the selected device slot
        let out_slot_addr = self.dev_context_addr(slot_id, memctx)?;

        let mut out_slot_ctx =
            MemCtxValue::<SlotContext>::new(out_slot_addr, memctx)?;
        Some(match out_slot_ctx.slot_state() {
            // if the output slot state is default, addressed, or configured:
            SlotState::Default
            | SlotState::Addressed
            | SlotState::Configured => {
                slog::debug!(
                    self.log,
                    "input_ctx: {:#x} {input_ctx:?}",
                    input_context_ptr.0
                );

                let in_slot_addr =
                    input_context_ptr.offset::<InputControlContext>(1);

                // for each context designated by an add context flag = 1,
                // evaluate the parameter settings defined by the selected contexts.
                // (limited to context indeces 0 and 1, per xHCI 1.2 sect 6.2.3.3)

                // xHCI 1.2 sect 6.2.2.3: interrupter target & max exit latency
                if input_ctx.add_context_bit(EndpointId(0)).unwrap() {
                    let in_slot_ctx =
                        memctx.read::<SlotContext>(in_slot_addr)?;
                    out_slot_ctx.mutate(|ctx| {
                        ctx.set_interrupter_target(
                            in_slot_ctx.interrupter_target(),
                        );
                        ctx.set_max_exit_latency_micros(
                            in_slot_ctx.max_exit_latency_micros(),
                        );
                    });
                    slog::debug!(
                        self.log,
                        "out_slot_ctx: in@{:#x} out@{:#x} {out_slot_ctx:?}",
                        in_slot_addr.0,
                        out_slot_addr.0
                    );
                }
                // xHCI 1.2 sect 6.2.3.3: pay attention to max packet size
                const EP_1: EndpointId = EndpointId(1);
                if input_ctx.add_context_bit(EP_1).unwrap() {
                    let in_ep0_addr = input_context_ptr
                        .offset::<InputControlContext>(1)
                        .offset::<SlotContext>(1);
                    let out_ep0_addr = out_slot_addr.offset::<SlotContext>(1);

                    let in_ep0_ctx =
                        memctx.read::<EndpointContext>(in_ep0_addr)?;

                    let mut out_ep0_ctx =
                        Self::endpoint_context(out_slot_addr, EP_1, memctx)?;
                    out_ep0_ctx.mutate(|ctx| {
                        ctx.set_max_packet_size(in_ep0_ctx.max_packet_size())
                    });

                    slog::debug!(
                        self.log,
                        "out_slot_ctx: {:#x} {out_slot_ctx:?}",
                        out_slot_addr.0
                    );
                    slog::debug!(
                        self.log,
                        "out_ep0_ctx: in@{:#x} out@{:#x} {out_ep0_ctx:?}",
                        in_ep0_addr.0,
                        out_ep0_addr.0
                    );
                }
                // (again, per xHCI 1.2 sect 6.2.3.3: contexts 2 through 31
                // are not evaluated by this command.)
                TrbCompletionCode::Success
            }
            x => {
                slog::error!(
                    self.log,
                    "Evaluate Context for slot in state {x:?}"
                );
                TrbCompletionCode::ContextStateError
            }
        })
    }

    // xHCI 1.2 sect 4.6.8
    pub fn reset_endpoint(
        &mut self,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        transfer_state_preserve: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        let slot_addr = self.dev_context_addr(slot_id, memctx)?;

        let mut ep_ctx =
            Self::endpoint_context(slot_addr, endpoint_id, memctx)?;
        Some(match ep_ctx.endpoint_state() {
            // endpoint must be in halted state for reset endpoint command
            EndpointState::Halted => {
                if transfer_state_preserve {
                    // when TSP=1, we wish to retry last transaction the next time
                    // the doorbell is rung, if no other commands have been issued
                    // to the endpoint
                    match self.usbdev_for_slot(slot_id) {
                        Ok(dev) => match dev.stop_endpoint(endpoint_id) {
                            Ok(Some(trb)) => {
                                let slot = self.slot_mut(slot_id).unwrap();
                                if let Err(e) = Self::write_trdp_and_ccs(
                                    slot.endpoints.get_mut(&endpoint_id)?,
                                    &mut ep_ctx,
                                    trb.trb_pointer(),
                                    trb.cycle_state(),
                                ) {
                                    slog::error!(
                                        self.log,
                                        "Error in {slot_id:?} {endpoint_id:?}: {e}"
                                    );
                                    return Some(
                                        TrbCompletionCode::ContextStateError,
                                    );
                                }
                            }
                            Ok(None) => (),
                            Err(e) => {
                                slog::error!(
                                    self.log,
                                    "Error stopping {slot_id:?} {endpoint_id:?}: {e}"
                                );
                                return Some(
                                    TrbCompletionCode::ContextStateError,
                                );
                            }
                        },
                        Err(e) => {
                            slog::error!(self.log, "{e}");
                            return Some(TrbCompletionCode::ContextStateError);
                        }
                    }
                } else {
                    // TODO when/if becomes relevant:
                    // reset data toggle for usb2 device / sequence number for usb3 device
                    // reset any usb2 split transaction state on this endpoint

                    // invalidate all cached Transfer TRBs
                    match self.usbdev_for_slot(slot_id) {
                        Ok(dev) => {
                            if let Err(e) = dev.stop_endpoint(endpoint_id) {
                                slog::error!(
                                    self.log,
                                    "Error stopping {slot_id:?} {endpoint_id:?}: {e}"
                                );
                                return Some(
                                    TrbCompletionCode::ContextStateError,
                                );
                            }
                        }
                        Err(e) => {
                            slog::error!(self.log, "{e}");
                            return Some(TrbCompletionCode::ContextStateError);
                        }
                    }
                }
                ep_ctx.mutate(|ctx| {
                    ctx.set_endpoint_state(EndpointState::Stopped)
                });

                TrbCompletionCode::Success
            }
            _ => TrbCompletionCode::ContextStateError,
        })
    }

    // xHCI 1.2 sect 4.6.9
    pub fn stop_endpoint(
        &mut self,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        _suspend: bool,
        memctx: &MemCtx,
        event_sender: &EventSender,
    ) -> Option<TrbCompletionCode> {
        // if enabled by previous enable slot command
        Some(if self.slot(slot_id).is_ok() {
            // retrieve dev ctx
            let slot_addr = self.dev_context_addr(slot_id, memctx)?;
            let output_slot_ctx = memctx.read::<SlotContext>(slot_addr)?;
            match output_slot_ctx.slot_state() {
                SlotState::Default
                | SlotState::Addressed
                | SlotState::Configured => {
                    let mut trdp;
                    let mut ccs;
                    {
                        // unwrap: self.slot(slot_id).is_ok(), above
                        let slot = self.slot_mut(slot_id).unwrap();
                        let xfer_ring = slot.endpoints.get_mut(&endpoint_id)?;
                        trdp = xfer_ring.current_dequeue_pointer();
                        ccs = xfer_ring.consumer_cycle_state();
                    }
                    let mut ep_ctx =
                        Self::endpoint_context(slot_addr, endpoint_id, memctx)?;
                    match ep_ctx.endpoint_state() {
                        EndpointState::Running => {
                            if let Ok(dev) = self.usbdev_for_slot(slot_id) {
                                // stop USB activity for pipe
                                // stop transfer ring activity for pipe
                                // remove endpoint from pipe schedule
                                if let Ok(Some(trb)) =
                                    dev.stop_endpoint(endpoint_id)
                                {
                                    trdp = trb.trb_pointer();
                                    ccs = trb.cycle_state();
                                    // if we interrupted the execution of a TD, insert a transfer event
                                    if let Err(e) = event_sender.enqueue_event(
                                        EventInfo::Transfer {
                                            trb_pointer: trb.trb_pointer(),
                                            completion_code:
                                                TrbCompletionCode::Stopped,
                                            trb_transfer_length: trb
                                                .data_buffer()
                                                .len()
                                                as u32,
                                            slot_id,
                                            endpoint_id,
                                            // Event Data TRBs are rolled up into the TransferTrb
                                            // struct and processed at the same time as the TRB
                                            // they immediately follow, so we will always be
                                            // pointing to a non-Event-Data TRB in this case
                                            event_data: false,
                                        },
                                        // will interrupt after enqueueing the command completion event
                                        true,
                                    ) {
                                        slog::error!(self.log, "Failed to insert event for interrupted TD in {slot_id:?} {endpoint_id:?}: {e}");
                                    }
                                }
                            }

                            // unwrap: self.slot(slot_id).is_ok(), above
                            let slot = self.slot_mut(slot_id).unwrap();
                            let xfer_ring =
                                slot.endpoints.get_mut(&endpoint_id)?;

                            if let Err(e) = Self::write_trdp_and_ccs(
                                xfer_ring,
                                &mut ep_ctx,
                                trdp,
                                ccs,
                            ) {
                                slog::error!(
                                    self.log,
                                    "Error in {slot_id:?} {endpoint_id:?}: {e}"
                                );
                                TrbCompletionCode::ContextStateError
                            } else {
                                // set ep state to stopped
                                ep_ctx.mutate(|ctx| {
                                    ctx.set_endpoint_state(
                                        EndpointState::Stopped,
                                    )
                                });

                                // n/a: wait for any partially completed split transactions
                                TrbCompletionCode::Success
                            }
                        }
                        x => {
                            slog::error!(
                                self.log,
                                "Stop Endpoint for endpoint in state {x:?}"
                            );
                            TrbCompletionCode::ContextStateError
                        }
                    }
                }
                x => {
                    slog::error!(
                        self.log,
                        "Stop Endpoint for slot in state {x:?}"
                    );
                    TrbCompletionCode::ContextStateError
                }
            }
        } else {
            TrbCompletionCode::SlotNotEnabledError
        })
    }

    // xHCI 1.2 sect 4.6.10
    pub fn set_tr_dequeue_pointer(
        &mut self,
        new_tr_dequeue_ptr: GuestAddr,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        dequeue_cycle_state: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        // if enabled by previous enable slot command
        Some(if self.slot(slot_id).is_ok() {
            // retrieve dev ctx
            let slot_addr = self.dev_context_addr(slot_id, memctx)?;
            let output_slot_ctx = memctx.read::<SlotContext>(slot_addr)?;
            if let SlotState::Default
            | SlotState::Addressed
            | SlotState::Configured = output_slot_ctx.slot_state()
            {
                let mut ep_ctx =
                    Self::endpoint_context(slot_addr, endpoint_id, memctx)?;
                // TODO(USB3): cmd trb decode currently assumes MaxPStreams and StreamID are 0
                if let EndpointState::Stopped | EndpointState::Error =
                    ep_ctx.endpoint_state()
                {
                    // invalidate any cached TDs
                    if let Ok(dev) = self.usbdev_for_slot(slot_id) {
                        if let Err(e) = dev.stop_endpoint(endpoint_id) {
                            slog::error!(
                                self.log,
                                "Failed to stop endpoint {slot_id:?} {endpoint_id:?}: {e}"
                            );
                        }
                    }
                    // unwrap: self.slot(slot_id).is_ok(), above
                    let slot = self.slot_mut(slot_id).unwrap();
                    let xfer_ring = slot.endpoints.get_mut(&endpoint_id)?;
                    // copy new_tr_dequeue_ptr to target Endpoint Context
                    // copy dequeue_cycle_state to target Endpoint Context
                    if let Err(e) = Self::write_trdp_and_ccs(
                        xfer_ring,
                        &mut ep_ctx,
                        new_tr_dequeue_ptr,
                        dequeue_cycle_state,
                    ) {
                        slog::error!(
                            self.log,
                            "Error on {slot_id:?} {endpoint_id:?}: {e}"
                        );
                    } else {
                        return Some(TrbCompletionCode::Success);
                    }
                }
            }
            TrbCompletionCode::ContextStateError
        } else {
            TrbCompletionCode::SlotNotEnabledError
        })
    }

    fn write_trdp_and_ccs(
        xfer_ring: &mut TransferRing,
        ep_ctx: &mut MemCtxValue<'_, EndpointContext>,
        new_tr_dequeue_ptr: GuestAddr,
        dequeue_cycle_state: bool,
    ) -> Result<(), Error> {
        ep_ctx.mutate(|ctx| {
            ctx.set_tr_dequeue_pointer(new_tr_dequeue_ptr);
            ctx.set_dequeue_cycle_state(dequeue_cycle_state)
        });

        xfer_ring
            .set_dequeue_pointer_and_cycle(
                new_tr_dequeue_ptr,
                dequeue_cycle_state,
            )
            .map_err(|e| {
                Error::SetTRDPFailed(new_tr_dequeue_ptr, dequeue_cycle_state, e)
            })
    }

    // xHCI 1.2 sect 4.6.11
    pub fn reset_device(
        &self,
        slot_id: SlotId,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        let slot_addr = self.dev_context_addr(slot_id, memctx)?;
        let mut output_slot_ctx =
            MemCtxValue::<SlotContext>::new(slot_addr, memctx)?;
        Some(match output_slot_ctx.slot_state() {
            SlotState::Addressed | SlotState::Configured => {
                // TODO: abort any usb transactions to the device

                // set slot state to default
                // set context entries to 1
                // set usb dev address to 0
                output_slot_ctx.mutate(|ctx| {
                    ctx.set_slot_state(SlotState::Default);
                    ctx.set_context_entries(1);
                    ctx.set_usb_device_address(0);
                });

                // for each endpoint context (except the default control endpoint)
                let last_endpoint = output_slot_ctx.context_entries();
                for endpoint_id in (1..=last_endpoint).map(EndpointId::from) {
                    let mut ep_ctx =
                        Self::endpoint_context(slot_addr, endpoint_id, memctx)?;
                    // set ep state to disabled
                    ep_ctx.mutate(|ctx| {
                        ctx.set_endpoint_state(EndpointState::Disabled)
                    });
                }

                TrbCompletionCode::Success
            }
            _ => TrbCompletionCode::ContextStateError,
        })
    }

    pub fn transfer_ring_for_doorbell(
        &mut self,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        memctx: &MemCtx,
    ) -> Option<&mut TransferRing> {
        let log = self.log.clone();

        let slot_addr = self.dev_context_addr(slot_id, memctx)?;
        match self.slot_mut(slot_id) {
            Ok(slot) => {
                let Some(endpoint) = slot.endpoints.get_mut(&endpoint_id)
                else {
                    slog::error!(log, "rang Doorbell for {slot_id:?}'s {endpoint_id:?}, which was absent");
                    return None;
                };
                // xHCI 1.2 figure 4-5: transition state to Running from Stopped
                let mut ep_ctx =
                    Self::endpoint_context(slot_addr, endpoint_id, memctx)?;
                ep_ctx.mutate(|ctx| {
                    if ctx.endpoint_state() == EndpointState::Stopped {
                        ctx.set_endpoint_state(EndpointState::Running)
                    }
                });
                Some(endpoint)
            }
            Err(e) => {
                slog::error!(
                    log,
                    "rang Doorbell for {slot_id:?}, which was absent: {e}"
                );
                None
            }
        }
    }

    pub fn export(
        &self,
    ) -> Result<migrate::DeviceSlotTableV1, crate::migrate::MigrateStateError>
    {
        let Self { dcbaap, slots, port_devs, log: _ } = self;
        Ok(migrate::DeviceSlotTableV1 {
            dcbaap: dcbaap.as_ref().map(|x| x.0),
            slots: slots
                .iter()
                .map(|opt| opt.as_ref().map(|slot| slot.export()))
                .collect(),
            port_devs: port_devs
                .iter()
                .map(|opt| {
                    opt.as_ref().map(|usbdev| usbdev.export()).transpose()
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn import(
        &mut self,
        value: &migrate::DeviceSlotTableV1,
        ctx: &crate::migrate::MigrateCtx,
        port_handles: &super::controller::XhciPortHandleCollection,
        pci_state: &Arc<pci::DeviceState>,
    ) -> Result<(), crate::migrate::MigrateStateError> {
        let migrate::DeviceSlotTableV1 { dcbaap, slots, port_devs } = value;
        self.dcbaap = dcbaap.map(GuestAddr);
        if port_devs.len() != self.port_devs.len() {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                format!(
                    "payload port devs array size wrong: {} != {}",
                    port_devs.len(),
                    self.port_devs.len()
                ),
            ));
        }
        for (port_index_raw, (dst, src)) in
            self.port_devs.iter_mut().zip(port_devs).enumerate()
        {
            if src.is_none() {
                *dst = None;
            } else {
                let src_dev = src.as_ref().unwrap();
                if let Some(dst_dev) = dst {
                    // device exists in this port already, update its state
                    dst_dev.import(src_dev)?;
                } else {
                    let port_id =
                        PortId::try_from((port_index_raw + 1) as u8).unwrap();
                    // FIXME: dedupe
                    let dst_dev = UsbDeviceType::create_from_payload(
                        src_dev,
                        ctx.hid_report,
                        port_handles.handle_for_port(port_id),
                        pci_state,
                        &self.log,
                    )?;
                    *dst = Some(dst_dev);
                }
            }
        }
        for (dst, src) in self.slots.iter_mut().zip(slots) {
            if src.is_none() {
                *dst = None;
            } else {
                let src_slot = src.as_ref().unwrap();
                if let Some(dst_slot) = dst {
                    dst_slot.import(src_slot)?;
                } else {
                    let mut dst_slot = DeviceSlot::new();
                    dst_slot.import(src_slot)?;
                    *dst = Some(dst_slot);
                }
            }
        }
        Ok(())
    }
}

pub mod migrate {
    use crate::hw::usb::xhci::port::PortId;
    use crate::hw::usb::xhci::rings::consumer::migrate::ConsumerRingV1;
    use crate::hw::usb::{
        usbdev::migrate::UsbDeviceV1,
        xhci::rings::consumer::transfer::TransferRing,
    };
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use super::EndpointId;

    #[derive(Serialize, Deserialize)]
    pub struct DeviceSlotV1 {
        pub endpoints: HashMap<u8, ConsumerRingV1>,
        pub port_address: Option<u8>,
    }

    impl TryFrom<DeviceSlotV1> for super::DeviceSlot {
        type Error = crate::migrate::MigrateStateError;

        fn try_from(value: DeviceSlotV1) -> Result<Self, Self::Error> {
            let DeviceSlotV1 { endpoints, port_address } = value;
            Ok(Self {
                endpoints: endpoints
                    .into_iter()
                    .map(|(ep_id, ring)| {
                        Ok((
                            EndpointId::from(ep_id),
                            TransferRing::try_from(&ring)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, Self::Error>>()?
                    .into_iter()
                    .collect(),
                port_address: port_address
                    .map(|x| {
                        PortId::try_from(x).map_err(
                            crate::migrate::MigrateStateError::ImportFailed,
                        )
                    })
                    .transpose()?,
            })
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct DeviceSlotTableV1 {
        pub dcbaap: Option<u64>,
        pub slots: Vec<Option<DeviceSlotV1>>,
        pub port_devs: Vec<Option<UsbDeviceV1>>,
    }
}

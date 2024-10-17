//! Emulated USB Host Controller

use std::ops::{Deref, DerefMut};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex};

use crate::accessors::Accessor;
use crate::common::{GuestAddr, Lifecycle, RWOp, ReadOp, WriteOp, PAGE_SIZE};
use crate::hw::ids::pci::{PROPOLIS_XHCI_DEV_ID, VENDOR_OXIDE};
use crate::hw::pci;
use crate::hw::usb::xhci::bits::ring_data::TrbCompletionCode;
use crate::hw::usb::xhci::rings::consumer::CommandInfo;
use crate::hw::usb::xhci::rings::producer::{EventInfo, EventRing};
use crate::tasks::ThreadGroup;
use crate::vmm::MemCtx;

use super::bits;
use super::bits::device_context::{
    EndpointContext, EndpointState, InputControlContext, SlotContext,
    SlotContextFirst, SlotState,
};
use super::registers::*;
use super::rings::consumer::CommandRing;

/// The number of USB2 ports the controller supports.
pub(super) const NUM_USB2_PORTS: u8 = 4;

/// The number of USB3 ports the controller supports.
pub(super) const NUM_USB3_PORTS: u8 = 4;

/// Max number of device slots the controller supports.
// (up to 255)
// (Windows needs at least 64? TODO: source other than bhyve C frontend comment)
pub(super) const MAX_DEVICE_SLOTS: u8 = 64;

/// Max number of interrupters the controller supports (up to 1024).
pub(super) const NUM_INTRS: u16 = 1;

struct IntrRegSet {
    management: bits::InterrupterManagement,
    moderation: bits::InterrupterModeration,
    evt_ring_seg_tbl_size: bits::EventRingSegmentTableSize,
    evt_ring_seg_base_addr: bits::EventRingSegmentTableBaseAddress,
    evt_ring_deq_ptr: bits::EventRingDequeuePointer,
}

struct MemCtxValue<'a, T: Copy> {
    value: T,
    addr: GuestAddr,
    memctx: &'a MemCtx,
}

impl<'a, T: Copy> MemCtxValue<'a, T> {
    pub fn new(addr: GuestAddr, memctx: &'a MemCtx) -> Option<Self> {
        let value = memctx.read(addr)?;
        Some(Self { value, addr, memctx })
    }
    pub fn mutate(&mut self, mut f: impl FnMut(&mut T)) -> bool {
        f(&mut self.value);
        self.memctx.write(self.addr, &self.value)
    }
}

impl<'a, T: Copy> Deref for MemCtxValue<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

enum CommandRingTaskMessage {
    ProcessOneCommand,
}

struct DeviceSlot {}
impl DeviceSlot {
    fn new(slot_type: u8) -> Self {
        Self {}
    }
}

struct DeviceSlotTable {
    /// Device Context Base Address Array Pointer (DCBAAP)
    ///
    /// Points to an array of address pointers referencing the device context
    /// structures for each attached device.
    ///
    /// See xHCI 1.2 Section 5.4.6
    dcbaap: Option<GuestAddr>,
    slots: Vec<Option<DeviceSlot>>,
}

impl Default for DeviceSlotTable {
    fn default() -> Self {
        Self {
            dcbaap: None,
            // HACK: placeholder at slot 0, which is where the scratch
            // pointer goes in the DCBAA & has a special doorbell &c.
            slots: vec![Some(DeviceSlot {})],
        }
    }
}

impl DeviceSlotTable {
    fn slot(&self, slot_id: u8) -> Option<&DeviceSlot> {
        self.slots.get(slot_id as usize).and_then(|opt| opt.as_ref())
    }

    fn enable_slot(&mut self, slot_type: u8) -> Option<u8> {
        let slot_id_opt =
            self.slots.iter().position(Option::is_none).or_else(|| {
                if self.slots.len() < MAX_DEVICE_SLOTS as usize {
                    self.slots.push(None);
                    Some(self.slots.len() - 1)
                } else {
                    None
                }
            });
        if let Some(slot_id) = slot_id_opt {
            self.slots[slot_id] = Some(DeviceSlot::new(slot_type));
        }
        slot_id_opt.map(|x| x as u8)
    }

    fn disable_slot(
        &mut self,
        slot_id: u8,
        memctx: &MemCtx,
    ) -> TrbCompletionCode {
        if self.slot(slot_id).is_some() {
            // TODO: terminate any transfers on the slot

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
            self.slots[slot_id as usize] = None;
            TrbCompletionCode::Success
        } else {
            TrbCompletionCode::SlotNotEnabledError
        }
    }

    fn dev_context_addr(
        &self,
        slot_id: u8,
        memctx: &MemCtx,
    ) -> Option<GuestAddr> {
        self.dcbaap.as_ref().and_then(|base_ptr| {
            memctx
                .read::<u64>(
                    // lower 6 bits are reserved (xHCI 1.2 table 6-2)
                    // software is supposed to clear them to 0, but
                    // let's double-tap for safety's sake.
                    base_ptr.offset::<u64>(slot_id as usize) & !0b11_1111,
                )
                .map(GuestAddr)
        })
    }

    fn address_device(
        &self,
        slot_id: u8,
        input_context_ptr: GuestAddr,
        block_set_address_request: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        if self.slot(slot_id).is_none() {
            return Some(TrbCompletionCode::SlotNotEnabledError);
        }
        let out_slot_ptr = self.dev_context_addr(slot_id, memctx)?;
        // xHCI 1.2 Figure 6-5: differences between input context indeces
        // and device context indeces.
        let mut slot_ctx = memctx.read::<SlotContext>(
            input_context_ptr.offset::<InputControlContext>(1),
        )?;
        let mut ep0_ctx = memctx.read::<EndpointContext>(
            input_context_ptr
                .offset::<InputControlContext>(1)
                .offset::<SlotContext>(1),
        )?;

        Some(if block_set_address_request {
            if matches!(slot_ctx.slot_state(), SlotState::DisabledEnabled) {
                // copy input slot ctx to output slot ctx, and
                // set usb device address in output slot ctx to 0
                slot_ctx.set_usb_device_address(0);
                memctx.write(out_slot_ptr, &slot_ctx);
                // copy input ep0 ctx to output ep0 ctx, and
                // set output ep0 state to running
                ep0_ctx.set_endpoint_state(EndpointState::Running);
                memctx.write(out_slot_ptr.offset::<SlotContext>(1), &ep0_ctx);
                TrbCompletionCode::Success
            } else {
                TrbCompletionCode::ContextStateError
            }
        } else {
            if matches!(
                slot_ctx.slot_state(),
                SlotState::DisabledEnabled | SlotState::Default
            ) {
                // select address, issue 'set address' to device
                // copy input slot ctx to output slot ctx
                // set output slot context state to addressed
                // set usb device address in output slot ctx to chosen addr
                const HARDCODED_USB_ADDRESS: u8 = 1; // FIXME, obviously
                slot_ctx.set_usb_device_address(HARDCODED_USB_ADDRESS);
                slot_ctx.set_slot_state(SlotState::Addressed);
                memctx.write(out_slot_ptr, &slot_ctx);
                // copy input ep0 ctx to output ep0 ctx
                // set output ep0 state to running
                ep0_ctx.set_endpoint_state(EndpointState::Running);
                memctx.write(out_slot_ptr.offset::<SlotContext>(1), &ep0_ctx);
                TrbCompletionCode::Success
            } else {
                TrbCompletionCode::ContextStateError
            }
        })
    }

    // xHCI 1.2 sect 4.6.7, 6.2.2.3, 6.2.3.3
    fn evaluate_context(
        &self,
        slot_id: u8,
        input_context_ptr: GuestAddr,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        if self.slot(slot_id).is_none() {
            return Some(TrbCompletionCode::SlotNotEnabledError);
        }

        let input_ctx =
            memctx.read::<InputControlContext>(input_context_ptr)?;

        let slot_addr = self.dev_context_addr(slot_id, memctx)?;

        let mut output_slot_ctx =
            MemCtxValue::<SlotContext>::new(slot_addr, memctx)?;
        Some(match output_slot_ctx.slot_state() {
            SlotState::Default
            | SlotState::Addressed
            | SlotState::Configured => {
                // xHCI 1.2 sect 6.2.2.3: interrupter target & max exit latency
                if input_ctx.add_context(0).unwrap() {
                    let input_slot_ctx = memctx.read::<SlotContext>(
                        input_context_ptr.offset::<InputControlContext>(1),
                    )?;
                    output_slot_ctx.mutate(|ctx| {
                        ctx.set_interrupter_target(
                            input_slot_ctx.interrupter_target(),
                        );
                        ctx.set_max_exit_latency_micros(
                            input_slot_ctx.max_exit_latency_micros(),
                        );
                    });
                }
                // xHCI 1.2 sect 6.2.3.3: pay attention to max packet size
                if input_ctx.add_context(1).unwrap() {
                    let input_ep0_ctx = memctx.read::<EndpointContext>(
                        input_context_ptr
                            .offset::<InputControlContext>(1)
                            .offset::<SlotContext>(1),
                    )?;
                    let ep0_addr = slot_addr.offset::<SlotContext>(1);
                    let mut output_ep0_ctx =
                        MemCtxValue::<EndpointContext>::new(ep0_addr, memctx)?;
                    output_ep0_ctx.mutate(|ctx| {
                        ctx.set_max_packet_size(input_ep0_ctx.max_packet_size())
                    });
                }
                // per xHCI 1.2 sect 6.2.3.3: contexts 2 through 31 are not evaluated by this command.
                TrbCompletionCode::Success
            }
            _ => TrbCompletionCode::ContextStateError,
        })
    }

    // xHCI 1.2 sect 4.6.8
    fn reset_endpoint(
        &self,
        slot_id: u8,
        endpoint_id: u8,
        transfer_state_preserve: bool,
        memctx: &MemCtx,
    ) -> Option<TrbCompletionCode> {
        let ep_addr = self
            .dev_context_addr(slot_id, memctx)?
            .offset::<SlotContext>(1)
            .offset::<EndpointContext>(endpoint_id as usize);

        let mut ep_ctx = MemCtxValue::<EndpointContext>::new(ep_addr, memctx)?;
        Some(match ep_ctx.endpoint_state() {
            EndpointState::Halted => TrbCompletionCode::ContextStateError,
            _ => {
                if transfer_state_preserve {
                    // retry last transaction the next time the doorbell is rung,
                    // if no other commands have been issued to the endpoint
                } else {
                    // reset data toggle for usb2 device / sequence number for usb3 device
                    // reset any usb2 split transaction state on this endpoint
                    // invalidate cached Transfer TRBs
                }
                ep_ctx.mutate(|ctx| {
                    ctx.set_endpoint_state(EndpointState::Stopped)
                });
                todo!("enable doorbell register");
                TrbCompletionCode::Success
            }
        })
    }
}

struct XhciState {
    /// USB Command Register
    usb_cmd: bits::UsbCommand,

    /// USB Status Register
    usb_sts: bits::UsbStatus,

    /// Device Notification Control Register
    dnctrl: bits::DeviceNotificationControl,

    /// Microframe counter (125 ms per tick while running)
    mf_index: bits::MicroframeIndex,

    /// Interrupter register sets
    intr_reg_sets: [IntrRegSet; NUM_INTRS as usize],

    event_rings: [Option<EventRing>; NUM_INTRS as usize],
    command_ring: Option<CommandRing>,
    command_ring_running: bool,

    dev_slots: DeviceSlotTable,

    /// Configure Register
    config: bits::Configure,
}

impl Default for XhciState {
    fn default() -> Self {
        // The controller is initially halted and asserts CNR (controller not ready)
        let usb_sts = bits::UsbStatus(0)
            .with_host_controller_halted(true)
            .with_controller_not_ready(true);

        XhciState {
            usb_cmd: bits::UsbCommand(0),
            usb_sts,
            dnctrl: bits::DeviceNotificationControl::new([0]),
            dev_slots: DeviceSlotTable::default(),
            config: bits::Configure(0),
            mf_index: bits::MicroframeIndex(0),
            intr_reg_sets: [IntrRegSet {
                management: bits::InterrupterManagement(0),
                moderation: bits::InterrupterModeration(0)
                    .with_interval(0x4000),
                evt_ring_seg_tbl_size: bits::EventRingSegmentTableSize(0),
                evt_ring_seg_base_addr:
                    bits::EventRingSegmentTableBaseAddress::default(),
                evt_ring_deq_ptr: bits::EventRingDequeuePointer(0),
            }],
            event_rings: [None; NUM_INTRS as usize],
            command_ring: None,
            command_ring_running: false,
        }
    }
}

/// An emulated USB Host Controller attached over PCI
pub struct PciXhci {
    /// PCI device state
    pci_state: pci::DeviceState,

    /// Controller state
    state: Arc<Mutex<XhciState>>,

    /// Threads for processing rings
    workers: ThreadGroup,
    cmd_send: SyncSender<CommandRingTaskMessage>,
}

impl PciXhci {
    /// Create a new pci-xhci device
    pub fn create() -> Arc<Self> {
        let pci_builder = pci::Builder::new(pci::Ident {
            vendor_id: VENDOR_OXIDE,
            device_id: PROPOLIS_XHCI_DEV_ID,
            sub_vendor_id: VENDOR_OXIDE,
            sub_device_id: PROPOLIS_XHCI_DEV_ID,
            class: pci::bits::CLASS_SERIAL_BUS,
            subclass: pci::bits::SUBCLASS_USB,
            prog_if: pci::bits::PROGIF_USB3,
            ..Default::default()
        });

        let pci_state = pci_builder
            // .add_bar_mmio64(pci::BarN::BAR0, 0x2000)
            // Place MSI-X in BAR4
            .add_cap_msix(pci::BarN::BAR4, NUM_INTRS)
            .add_custom_cfg(bits::USB_PCI_CFG_OFFSET, bits::USB_PCI_CFG_REG_SZ)
            .finish();

        let state = Arc::new(Mutex::new(XhciState::default()));

        // TODO: more than just cmd ring
        let workers = ThreadGroup::new();
        let worker_acc =
            pci_state.acc_mem.child(Some(format!("xhci command ring")));
        let worker_state = state.clone();
        // size 0: rendesvous (sender blocks until receiver receives)
        let (cmd_send, cmd_recv) =
            std::sync::mpsc::sync_channel::<CommandRingTaskMessage>(0);
        let worker_thread = std::thread::Builder::new()
            .name(format!("xhci command ring"))
            .spawn(|| {
                Self::process_command_ring(cmd_recv, worker_state, worker_acc)
            });

        workers.extend(core::iter::once(worker_thread)).unwrap();

        Arc::new(Self { pci_state, state, workers, cmd_send })
    }

    /// Handle read of register in USB-specific PCI configuration space
    fn usb_cfg_read(&self, id: UsbPciCfgReg, ro: &mut ReadOp) {
        match id {
            UsbPciCfgReg::SerialBusReleaseNumber => {
                // USB 3.0
                ro.write_u8(0x30);
            }
            UsbPciCfgReg::FrameLengthAdjustment => {
                // We don't support adjusting the SOF cycle
                let fladj = bits::FrameLengthAdjustment(0).with_nfc(true);
                ro.write_u8(fladj.0);
            }
            UsbPciCfgReg::DefaultBestEffortServiceLatencies => {
                // We don't support link power management so return 0
                ro.write_u8(bits::DefaultBestEffortServiceLatencies(0).0);
            }
        }
    }

    /// Handle write to register in USB-specific PCI configuration space
    fn usb_cfg_write(&self, id: UsbPciCfgReg, _wo: &mut WriteOp) {
        match id {
            // Ignore writes to read-only register
            UsbPciCfgReg::SerialBusReleaseNumber => {}

            // We don't support adjusting the SOF cycle
            UsbPciCfgReg::FrameLengthAdjustment => {}

            // We don't support link power management
            UsbPciCfgReg::DefaultBestEffortServiceLatencies => {}
        }
    }

    /// Handle read of memory-mapped host controller register
    fn reg_read(&self, id: Registers, ro: &mut ReadOp) {
        use CapabilityRegisters::*;
        use OperationalRegisters::*;
        use Registers::*;
        use RuntimeRegisters::*;

        match id {
            Reserved => ro.fill(0),

            // Capability registers
            Cap(CapabilityLength) => {
                // xHCI 1.2 Section 5.3.1: Used to find the beginning of
                // operational registers.
                ro.write_u8(XHC_REGS.operational_offset() as u8);
            }
            Cap(HciVersion) => {
                // xHCI 1.2 Section 5.3.2: xHCI Version 1.2.0
                ro.write_u16(0x0120);
            }
            Cap(HcStructuralParameters1) => {
                let hcs_params1 = bits::HcStructuralParameters1(0)
                    .with_max_slots(MAX_DEVICE_SLOTS)
                    .with_max_intrs(NUM_INTRS)
                    .with_max_ports(NUM_USB2_PORTS + NUM_USB3_PORTS);
                ro.write_u32(hcs_params1.0);
            }
            Cap(HcStructuralParameters2) => {
                let hcs_params2 = bits::HcStructuralParameters2(0)
                    .with_ist_as_frame(true)
                    .with_iso_sched_threshold(0b111)
                    // We don't need any scratchpad buffers
                    .with_max_scratchpad_bufs(0)
                    .with_scratchpad_restore(false);
                ro.write_u32(hcs_params2.0);
            }
            Cap(HcStructuralParameters3) => {
                let hcs_params3 = bits::HcStructuralParameters3(0);
                ro.write_u32(hcs_params3.0);
            }
            Cap(HcCapabilityParameters1) => {
                let hcc_params1 =
                    bits::HcCapabilityParameters1(0).with_ac64(true).with_xecp(
                        /* TODO: set valid extended capabilities offset */
                        0,
                    );
                ro.write_u32(hcc_params1.0);
            }
            Cap(HcCapabilityParameters2) => {
                let hcc_params2 = bits::HcCapabilityParameters2(0);
                ro.write_u32(hcc_params2.0);
            }
            Cap(DoorbellOffset) => {
                // Per layout defined in XhcRegMap.
                ro.write_u32(XHC_REGS.doorbell_offset() as u32);
            }
            Cap(RuntimeRegisterSpaceOffset) => {
                ro.write_u32(XHC_REGS.runtime_offset() as u32);
            }

            // Operational registers
            Op(UsbCommand) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.usb_cmd.0);
            }
            Op(UsbStatus) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.usb_sts.0);
            }
            Op(PageSize) => {
                // Report supported page sizes (we only support 1).
                // bit n = 1, if 2^(n+12) is a supported page size
                let shift = PAGE_SIZE.trailing_zeros() - 12;
                ro.write_u32(1 << shift);
            }
            Op(DeviceNotificationControl) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.dnctrl.data[0]);
            }
            Op(CommandRingControlRegister) => {
                // Most of these fields read as 0, except for CRR
                let state = self.state.lock().unwrap();
                let crcr = bits::CommandRingControl(0)
                    .with_command_ring_running(state.command_ring_running);
                ro.write_u64(crcr.0);
            }
            Op(DeviceContextBaseAddressArrayPointerRegister) => {
                let state = self.state.lock().unwrap();
                let addr =
                    state.dev_slots.dcbaap.as_ref().map(|x| x.0).unwrap_or(0);
                ro.write_u64(addr);
            }
            Op(Configure) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.config.0);
            }
            Op(Port(..)) => {}

            // Runtime registers
            Runtime(MicroframeIndex) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.mf_index.0);
            }
            Runtime(Interrupter(i, intr_regs)) => {
                let i = i as usize;
                if i < NUM_INTRS as usize {
                    let state = self.state.lock().unwrap();
                    let reg_set = &state.intr_reg_sets[i];
                    match intr_regs {
                        InterrupterRegisters::Management => {
                            ro.write_u32(reg_set.management.0);
                        }
                        InterrupterRegisters::Moderation => {
                            ro.write_u32(reg_set.moderation.0);
                        }
                        InterrupterRegisters::EventRingSegmentTableSize => {
                            ro.write_u32(reg_set.evt_ring_seg_tbl_size.0);
                        }
                        InterrupterRegisters::EventRingSegmentTableBaseAddress => {
                            ro.write_u64(reg_set.evt_ring_seg_base_addr.address().0);
                        }
                        InterrupterRegisters::EventRingDequeuePointer => {
                            ro.write_u64(reg_set.evt_ring_deq_ptr.0);
                        }
                    }
                } else {
                    // invalid interrupter index given.
                }
            }

            // Only for software to write, returns 0 when read.
            Doorbell(_) => ro.write_u32(0),
        }
    }

    /// Handle write to memory-mapped host controller register
    fn reg_write(&self, id: Registers, wo: &mut WriteOp) {
        use OperationalRegisters::*;
        use Registers::*;
        use RuntimeRegisters::*;

        match id {
            // Ignore writes to reserved bits
            Reserved => {}

            // Capability registers are all read-only; ignore any writes
            Cap(_) => {}

            // Operational registers
            Op(UsbCommand) => {
                let mut state = self.state.lock().unwrap();
                let cmd = bits::UsbCommand(wo.read_u32());

                // xHCI 1.2 Section 5.4.1.1
                if cmd.run_stop() && !state.usb_cmd.run_stop() {
                    if !state.usb_sts.host_controller_halted() {
                        todo!(
                            "xhci: run while not halted: undefined behavior!"
                        );
                    }
                    state.usb_sts.set_host_controller_halted(false);
                    todo!("xhci: run dev slots");
                } else if !cmd.run_stop() && state.usb_cmd.run_stop() {
                    // TODO: can we *actually* stop on a dime like this?:
                    state.usb_sts.set_host_controller_halted(true);
                    // xHCI 1.2 table 5-24: cleared to 0 when R/S is.
                    state.command_ring_running = false;
                    todo!("xhci: stop dev slots");
                }

                // xHCI 1.2 table 5-20: Any transactions in progress are
                // immediately terminated; all internal pipelines, registers,
                // timers, counters, state machines, etc. are reset to their
                // initial value.
                if cmd.host_controller_reset() {
                    *state = XhciState::default();
                    todo!("xhci: host controller reset");
                }

                /*
                if cmd.interrupter_enable() {
                    todo!("xhci: interrupter enable");
                }

                // xHCI 1.2 Section 4.10.2.6
                if cmd.host_system_error_enable() {
                    todo!("xhci: host system error enable");
                }

                // xHCI 1.2 Section 4.23.2
                if cmd.controller_save_state() {
                    if state.usb_sts.save_state_status() {
                        todo!("xhci: save state while saving: undefined behavior!");
                    }
                    if state.usb_sts.host_controller_halted() {
                        todo!("xhci: save state");
                    }
                }
                // xHCI 1.2 Section 4.23.2
                if cmd.controller_restore_state() {
                    if state.usb_sts.save_state_status() {
                        todo!("xhci: restore state while saving: undefined behavior!");
                    }
                    if state.usb_sts.host_controller_halted() {
                        todo!("xhci: restore state");
                    }
                }

                // xHCI 1.2 Section 4.14.2
                if cmd.enable_wrap_event() {
                    todo!("xhci: enable wrap event");
                }

                // xHCI 1.2 Section 4.14.2
                if cmd.enable_u3_mfindex_stop() {
                    todo!("xhci: enable u3 mfindex stop");
                }

                // xHCI 1.2 Section 4.23.5.2.2
                if cmd.cem_enable() {
                    todo!("xhci: cem enable");
                }

                // xHCI 1.2 Section 4.11.2.3
                if cmd.ete() {
                    todo!("xhci: extended tbc enable");
                }

                // xHCI 1.2 Section 4.11.2.3
                if cmd.tsc_enable() {
                    todo!("xhci: extended tsc trb status enable");
                }

                if cmd.vtio_enable() {
                    todo!("xhci: vtio enable");
                }
                */

                // LHCRST is optional, and when it is not implemented
                // (HCCPARAMS1), it must always return 0 when read.
                // CSS and CRS also must always return 0 when read.
                state.usb_cmd = cmd
                    .with_controller_save_state(false)
                    .with_controller_restore_state(false)
                    .with_light_host_controller_reset(false);
            }
            // xHCI 1.2 Section 5.4.2
            Op(UsbStatus) => {
                let mut state = self.state.lock().unwrap();
                // HCH, SSS, RSS, CNR, and HCE are read-only (ignored here).
                // HSE, EINT, PCD, and SRE are RW1C (guest writes a 1 to
                // clear a field to 0, e.g. to ack an interrupt we gave it).
                let sts = bits::UsbStatus(wo.read_u32());
                if sts.host_system_error() {
                    state.usb_sts.set_host_system_error(false);
                }
                if sts.event_interrupt() {
                    state.usb_sts.set_event_interrupt(false);
                }
                if sts.port_change_detect() {
                    state.usb_sts.set_port_change_detect(false);
                }
                if sts.save_restore_error() {
                    state.usb_sts.set_save_restore_error(false);
                }
            }
            // Read-only.
            Op(PageSize) => {}
            Op(DeviceNotificationControl) => {
                let mut state = self.state.lock().unwrap();
                state.dnctrl.data[0] = wo.read_u32() & 0xFFFFu32;
                todo!("xhci: opreg write dev notif ctrl");
            }
            Op(CommandRingControlRegister) => {
                let crcr = bits::CommandRingControl(wo.read_u64());
                let mut state = self.state.lock().unwrap();
                // xHCI 1.2 sections 4.9.3, 5.4.5
                if !state.command_ring_running {
                    // if CRCR written while ring is stopped, init ring
                    state.command_ring = Some(CommandRing::new(
                        crcr.command_ring_pointer(),
                        crcr.ring_cycle_state(),
                    ));
                } else {
                    // xHCI 1.2 table 5-24
                    if crcr.command_stop() {
                        state.command_ring_running = false;
                        todo!("xhci: wait for command ring idle, generate command completion event");
                    } else if crcr.command_abort() {
                        state.command_ring_running = false;
                        todo!("xhci: abort command ring processing, generate command completion event");
                    } else {
                        // TODO log error?
                    }
                }
            }
            Op(DeviceContextBaseAddressArrayPointerRegister) => {
                let mut state = self.state.lock().unwrap();
                state.dev_slots.dcbaap = Some(GuestAddr(wo.read_u64()));
                todo!("xhci: opreg write devctxbaseaddrarrptrreg (gesundheit)");
            }
            Op(Configure) => {
                let mut state = self.state.lock().unwrap();
                state.config = bits::Configure(wo.read_u32());
                todo!("xhci: opreg write conf");
            }
            Op(Port(i, regs)) => {
                todo!("xhci: opreg write port {} {:?}", i, regs);
            }

            // Runtime registers
            Runtime(MicroframeIndex) => {} // Read-only
            Runtime(Interrupter(i, intr_regs)) => {
                let i = i as usize;
                if i < NUM_INTRS as usize {
                    let mut state = self.state.lock().unwrap();
                    match intr_regs {
                        InterrupterRegisters::Management => {
                            state.intr_reg_sets[i].management = bits::InterrupterManagement(wo.read_u32());
                        }
                        InterrupterRegisters::Moderation => {
                            state.intr_reg_sets[i].moderation = bits::InterrupterModeration(wo.read_u32());
                        }
                        InterrupterRegisters::EventRingSegmentTableSize => {
                            state.intr_reg_sets[i].evt_ring_seg_tbl_size = bits::EventRingSegmentTableSize(wo.read_u32());
                        }
                        InterrupterRegisters::EventRingSegmentTableBaseAddress => {
                            state.intr_reg_sets[i].evt_ring_seg_base_addr = bits::EventRingSegmentTableBaseAddress(wo.read_u64());
                        }
                        InterrupterRegisters::EventRingDequeuePointer => {
                            state.intr_reg_sets[i].evt_ring_deq_ptr = bits::EventRingDequeuePointer(wo.read_u64());
                        }
                    }

                    let erstba =
                        state.intr_reg_sets[i].evt_ring_seg_base_addr.address();
                    let erstsz = state.intr_reg_sets[i]
                        .evt_ring_seg_tbl_size
                        .size() as usize;
                    let erdp =
                        state.intr_reg_sets[i].evt_ring_deq_ptr.pointer();

                    // TODO: get rid of unwraps
                    let memctx = self.pci_state.acc_mem.access().unwrap();
                    if let Some(event_ring) = &mut state.event_rings[i] {
                        match intr_regs {
                            InterrupterRegisters::EventRingSegmentTableSize
                            | InterrupterRegisters::EventRingSegmentTableBaseAddress => {
                                event_ring.update_segment_table(erstba, erstsz, &memctx).unwrap()
                            }
                            InterrupterRegisters::EventRingDequeuePointer => {
                                event_ring.update_dequeue_pointer(erdp)
                            }
                            _ => (),
                        }
                    } else {
                        match intr_regs {
                            InterrupterRegisters::EventRingSegmentTableBaseAddress => {
                                state.event_rings[i] = Some(EventRing::new(erstba, erstsz, erdp, &memctx).unwrap())
                            }
                            _ => ()
                        }
                    }
                } else {
                    // invalid interrupter index given.
                }
            }

            Doorbell(0) => {
                // xHCI 1.2 section 4.9.3, table 5-43
                if wo.read_u32() & 0xff == 0 {
                    {
                        let mut state = self.state.lock().unwrap();
                        // xHCI 1.2 table 5-24: only set to 1 if R/S is 1
                        if state.usb_cmd.run_stop() {
                            state.command_ring_running = true;
                        }
                        // drop state lock
                    }
                    // blocks until cmd ring task receives it
                    self.cmd_send
                        .send(CommandRingTaskMessage::ProcessOneCommand);
                }
            }
            Doorbell(i) => {
                todo!("xhci: doorbell {} write", i);
            }
        }
    }

    fn process_command_ring(
        cmd_recv: Receiver<CommandRingTaskMessage>,
        state: Arc<Mutex<XhciState>>,
        acc_mem: Accessor<MemCtx>,
    ) {
        loop {
            match cmd_recv.recv() {
                Ok(CommandRingTaskMessage::ProcessOneCommand) => {
                    let mut state = state.lock().unwrap();
                    if state.command_ring_running {
                        let Some(memctx) = acc_mem.access() else { break };
                        let cmd_opt = if let Some(ref mut cmd_ring) =
                            state.command_ring
                        {
                            if let Err(_) = cmd_ring.update_from_guest(&memctx)
                            {
                                break;
                            }
                            // TODO: do we do one command at a time or all available?
                            let cmd_trb_addr =
                                cmd_ring.current_dequeue_pointer();
                            cmd_ring
                                .dequeue_work_item()
                                .map(|x| (x, cmd_trb_addr))
                        } else {
                            None
                        };
                        if let Some((Ok(cmd_desc), cmd_trb_addr)) = cmd_opt {
                            match cmd_desc.try_into() {
                                Ok(cmd) => {
                                    let event_info = Self::run_command(
                                        cmd,
                                        cmd_trb_addr,
                                        &mut state,
                                        &memctx,
                                    );
                                    if let Some(evt_ring) =
                                        state.event_rings[0].as_mut()
                                    {
                                        evt_ring.enqueue(
                                            event_info.into(),
                                            &memctx,
                                        );
                                    } else {
                                        // TODO: log error: event ring not init'd before command run
                                    }
                                }
                                Err(_) => {
                                    // TODO: log error: invalid command trb
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    fn run_command(
        cmd: CommandInfo,
        cmd_trb_addr: GuestAddr,
        state: &mut XhciState,
        memctx: &MemCtx,
    ) -> EventInfo {
        match cmd {
            // xHCI 1.2 sect 3.3.1, 4.6.2
            CommandInfo::NoOp => EventInfo::CommandCompletion {
                completion_code: TrbCompletionCode::Success,
                slot_id: 0, // 0 for no-op (table 6-42)
                cmd_trb_addr,
            },
            // xHCI 1.2 sect 3.3.2, 4.6.3
            CommandInfo::EnableSlot { slot_type } => {
                match state.dev_slots.enable_slot(slot_type) {
                    Some(slot_id) => EventInfo::CommandCompletion {
                        completion_code: TrbCompletionCode::Success,
                        slot_id,
                        cmd_trb_addr,
                    },
                    None => EventInfo::CommandCompletion {
                        completion_code:
                            TrbCompletionCode::NoSlotsAvailableError,
                        slot_id: 0,
                        cmd_trb_addr,
                    },
                }
            }
            // xHCI 1.2 sect 3.3.3, 4.6.4
            CommandInfo::DisableSlot { slot_id } => {
                EventInfo::CommandCompletion {
                    completion_code: state
                        .dev_slots
                        .disable_slot(slot_id, memctx),
                    slot_id,
                    cmd_trb_addr,
                }
            }
            // xHCI 1.2 sect 3.3.4, 4.6.5
            CommandInfo::AddressDevice {
                input_context_ptr,
                slot_id,
                block_set_address_request,
            } => {
                // xHCI 1.2 pg. 113
                let completion_code = state
                    .dev_slots
                    .address_device(
                        slot_id,
                        input_context_ptr,
                        block_set_address_request,
                        memctx,
                    )
                    // we'll call invalid pointers a context state error
                    .unwrap_or(TrbCompletionCode::ContextStateError);

                EventInfo::CommandCompletion {
                    completion_code,
                    slot_id,
                    cmd_trb_addr,
                }
            }
            // xHCI 1.2 sect 3.3.5, 4.6.6
            CommandInfo::ConfigureEndpoint {
                input_context_ptr,
                slot_id,
                deconfigure,
            } => {
                // TODO: implement. right now we just say we ran out of internal resources
                EventInfo::CommandCompletion {
                    completion_code: TrbCompletionCode::ResourceError,
                    slot_id,
                    cmd_trb_addr,
                }
            }
            CommandInfo::EvaluateContext { input_context_ptr, slot_id } => {
                let completion_code = state
                    .dev_slots
                    .evaluate_context(slot_id, input_context_ptr, memctx)
                    // TODO: handle properly. for now just:
                    .unwrap_or(TrbCompletionCode::ContextStateError);
                EventInfo::CommandCompletion {
                    completion_code,
                    slot_id,
                    cmd_trb_addr,
                }
            }
            CommandInfo::ResetEndpoint {
                slot_id,
                endpoint_id,
                transfer_state_preserve,
            } => {
                let completion_code = state
                    .dev_slots
                    .reset_endpoint(
                        slot_id,
                        endpoint_id,
                        transfer_state_preserve,
                        memctx,
                    )
                    .unwrap_or(TrbCompletionCode::ContextStateError);
                EventInfo::CommandCompletion {
                    completion_code,
                    slot_id,
                    cmd_trb_addr,
                }
            }
            CommandInfo::StopEndpoint { slot_id, endpoint_id, suspend } => {
                todo!()
            }
            CommandInfo::SetTRDequeuePointer {
                new_tr_dequeue_ptr,
                dequeue_cycle_state,
                slot_id,
                endpoint_id,
            } => {
                todo!()
            }
            CommandInfo::ResetDevice { slot_id } => {
                todo!()
            }
            CommandInfo::ForceHeader {
                packet_type,
                header_info,
                root_hub_port_number,
            } => {
                todo!()
            }
            // optional, unimplemented
            CommandInfo::ForceEvent
            | CommandInfo::NegotiateBandwidth
            | CommandInfo::SetLatencyToleranceValue => {
                EventInfo::CommandCompletion {
                    completion_code: TrbCompletionCode::TrbError,
                    slot_id: 0,
                    cmd_trb_addr,
                }
            }
            // optional, unimplemented
            CommandInfo::GetPortBandwidth { hub_slot_id: slot_id, .. }
            | CommandInfo::GetExtendedProperty { slot_id, .. }
            | CommandInfo::SetExtendedProperty { slot_id, .. } => {
                EventInfo::CommandCompletion {
                    completion_code: TrbCompletionCode::TrbError,
                    slot_id,
                    cmd_trb_addr,
                }
            }
        }
    }

    fn reset_controller(&self) {
        let state = self.state.lock().unwrap();
        todo!("xhci: reset all device slots");
    }
}

impl Lifecycle for PciXhci {
    fn type_name(&self) -> &'static str {
        "pci-xhci"
    }
}

impl pci::Device for PciXhci {
    fn device_state(&self) -> &pci::DeviceState {
        &self.pci_state
    }

    fn cfg_rw(&self, region: u8, mut rwo: RWOp) {
        assert_eq!(region, bits::USB_PCI_CFG_OFFSET);

        USB_PCI_CFG_REGS.process(
            &mut rwo,
            |id: &UsbPciCfgReg, rwo: RWOp<'_, '_>| match rwo {
                RWOp::Read(ro) => self.usb_cfg_read(*id, ro),
                RWOp::Write(wo) => self.usb_cfg_write(*id, wo),
            },
        )
    }

    fn bar_rw(&self, bar: pci::BarN, mut rwo: RWOp) {
        assert_eq!(bar, pci::BarN::BAR0);

        XHC_REGS.map.process(&mut rwo, |id: &Registers, rwo: RWOp<'_, '_>| {
            match rwo {
                RWOp::Read(ro) => self.reg_read(*id, ro),
                RWOp::Write(wo) => self.reg_write(*id, wo),
            }
        })
    }
}

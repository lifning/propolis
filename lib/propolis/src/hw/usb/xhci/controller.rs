//! Emulated USB Host Controller

use std::sync::{Arc, Mutex};

use crate::common::{GuestAddr, Lifecycle, RWOp, ReadOp, WriteOp, PAGE_SIZE};
use crate::hw::ids::pci::{PROPOLIS_XHCI_DEV_ID, VENDOR_OXIDE};
use crate::hw::pci;

use super::bits;
use super::registers::*;

/// The number of USB2 ports the controller supports.
pub(super) const NUM_USB2_PORTS: u8 = 4;

/// The number of USB3 ports the controller supports.
pub(super) const NUM_USB3_PORTS: u8 = 4;

/// Max number of device slots the controller supports.
const MAX_DEVICE_SLOTS: u8 = 64;

/// Max number of interrupters the controller supports.
const NUM_INTRS: u16 = 1024;

struct XhciState {
    /// USB Command Register
    usb_cmd: bits::UsbCommand,

    /// USB Status Register
    usb_sts: bits::UsbStatus,

    /// Device Notification Control Register
    dnctrl: bits::DeviceNotificationControl,

    /// Device Context Base Address Array Pointer (DCBAAP)
    ///
    /// Points to an array of address pointers referencing the device context
    /// structures for each attached device.
    ///
    /// See xHCI 1.2 Section 5.4.6
    dev_ctx_table_base: Option<GuestAddr>,

    /// Configure Register
    config: bits::Configure,
}

/// An emulated USB Host Controller attached over PCI
pub struct PciXhci {
    /// PCI device state
    pci_state: pci::DeviceState,

    /// Controller state
    state: Mutex<XhciState>,
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

        // The controller is initially halted and asserts CNR (controller not ready)
        let usb_sts = bits::UsbStatus(0)
            .with_host_controller_halted(true)
            .with_controller_not_ready(true);

        let state = Mutex::new(XhciState {
            usb_cmd: bits::UsbCommand(0),
            usb_sts,
            dnctrl: bits::DeviceNotificationControl::new([0]),
            dev_ctx_table_base: None,
            config: bits::Configure(0),
        });

        Arc::new(Self { pci_state, state })
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

        match id {
            Reserved => ro.fill(0),

            // Capability registers
            Cap(CapabilityLength) => {
                // xHCI 1.2 Section 5.3 (Table 5-9) shows 0x20 bytes of cap regs.
                // (TODO: expand if implementing extended capabilities?)
                ro.write_u8(XHC_REGS.cap_len as u8);
            }
            Cap(HciVersion) => {
                // xHCI Version 1.2.0
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
                // TODO: is this right
                ro.write_u32((XHC_REGS.cap_len + XHC_REGS.op_len) as u32);
            }
            Cap(RuntimeRegisterSpaceOffset) => {
                // TODO: write valid runtime register space offset
                ro.write_u32(0);
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
                let crcr = bits::CommandRingControl(0)
                    .with_command_ring_running(
                        /* TODO: processing commands */ false,
                    );
                ro.write_u64(crcr.0);
            }
            Op(DeviceContextBaseAddressArrayPointerRegister) => {
                let state = self.state.lock().unwrap();
                let addr = state.dev_ctx_table_base.unwrap_or(GuestAddr(0)).0;
                ro.write_u64(addr);
            }
            Op(Configure) => {
                let state = self.state.lock().unwrap();
                ro.write_u32(state.config.0);
            }
            Op(Port(..)) => {}
        }
    }

    /// Handle write to memory-mapped host controller register
    fn reg_write(&self, id: Registers, wo: &mut WriteOp) {
        use Registers::*;

        match id {
            // Ignore writes to reserved bits
            Reserved => {}

            // Capability registers are all read-only; ignore any writes
            Cap(_) => {}

            // Operational registers
            Op(opreg) => match opreg {
                OperationalRegisters::UsbCommand => {
                    let mut state = self.state.lock().unwrap();
                    let cmd = bits::UsbCommand(wo.read_u32());

                    // xHCI 1.2 Section 5.4.1.1
                    if cmd.run_stop() && !state.usb_cmd.run_stop() {
                        if !state.usb_sts.host_controller_halted() {
                            todo!("xhci: run while not halted: undefined behavior!");
                        }
                        state.usb_sts.set_host_controller_halted(false);
                        todo!("xhci: run");
                    } else if !cmd.run_stop() && state.usb_cmd.run_stop() {
                        // TODO: can we *actually* stop on a dime like this?:
                        state.usb_sts.set_host_controller_halted(true);
                        // TODO: do we stop CRCR too?
                        todo!("xhci: stop");
                    }

                    if cmd.host_controller_reset() {
                        todo!("xhci: host controller reset");
                    }

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

                    // LHCRST is optional, and when it is not implemented
                    // (HCCPARAMS1), it must always return 0 when read.
                    // CSS and CRS also must always return 0 when read.
                    state.usb_cmd = cmd
                        .with_controller_save_state(false)
                        .with_controller_restore_state(false)
                        .with_light_host_controller_reset(false);
                }
                // xHCI 1.2 Section 5.4.2
                OperationalRegisters::UsbStatus => {
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
                OperationalRegisters::PageSize => {}
                OperationalRegisters::DeviceNotificationControl => {
                    let mut state = self.state.lock().unwrap();
                    state.dnctrl.data[0] = wo.read_u32() & 0xFFFFu32;
                    todo!("xhci: opreg write dev notif ctrl");
                }
                OperationalRegisters::CommandRingControlRegister => {
                    let crcr = bits::CommandRingControl(wo.read_u64());
                    todo!("xhci: opreg write crcr (and is the 64-bit done all at once?)");
                }
                OperationalRegisters::DeviceContextBaseAddressArrayPointerRegister => {
                    let mut state = self.state.lock().unwrap();
                    state.dev_ctx_table_base = Some(GuestAddr(wo.read_u64()));
                    todo!("xhci: opreg write devctxbaseaddrarrptrreg (gesundheit) ((does 64bit require special handling?))");
                }
                OperationalRegisters::Configure => {
                    let mut state = self.state.lock().unwrap();
                    state.config = bits::Configure(wo.read_u32());
                    todo!("xhci: opreg write conf");
                }
                OperationalRegisters::Port(i, regs) => {
                    todo!("xhci: opreg write port {} {:?}", i, regs);
                }
            }
        }
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

//! XHCI Registers

#![allow(dead_code)]

use crate::util::regmap::RegMap;

use super::bits;
use super::controller::{MAX_DEVICE_SLOTS, NUM_USB2_PORTS, NUM_USB3_PORTS};

use lazy_static::lazy_static;

/// USB-specific PCI configuration registers.
///
/// See xHCI 1.2 Section 5.2 PCI Configuration Registers (USB)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UsbPciCfgReg {
    /// Serial Bus Release Number Register (SBRN)
    ///
    /// Indicates which version of the USB spec the controller implements.
    ///
    /// See xHCI 1.2 Section 5.2.3
    SerialBusReleaseNumber,

    /// Frame Length Adjustment Register (FLADJ)
    ///
    /// See xHCI 1.2 Section 5.2.4
    FrameLengthAdjustment,

    /// Default Best Effort Service Latency [Deep] (DBESL / DBESLD)
    ///
    /// See xHCI 1.2 Section 5.2.5 & 5.2.6
    DefaultBestEffortServiceLatencies,
}

lazy_static! {
    pub static ref USB_PCI_CFG_REGS: RegMap<UsbPciCfgReg> = {
        use UsbPciCfgReg::*;

        let layout = [
            (SerialBusReleaseNumber, 1),
            (FrameLengthAdjustment, 1),
            (DefaultBestEffortServiceLatencies, 1),
        ];

        RegMap::create_packed(bits::USB_PCI_CFG_REG_SZ.into(), &layout, None)
    };
}

/// Registers in MMIO space pointed to by BAR0/1
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Registers {
    Reserved,
    Cap(CapabilityRegisters),
    Op(OperationalRegisters),
    Doorbell(u8),
}

/// eXtensible Host Controller Capability Registers
///
/// See xHCI 1.2 Section 5.3 Host Controller Capability Registers
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CapabilityRegisters {
    CapabilityLength,
    HciVersion,
    HcStructuralParameters1,
    HcStructuralParameters2,
    HcStructuralParameters3,
    HcCapabilityParameters1,
    HcCapabilityParameters2,
    DoorbellOffset,
    RuntimeRegisterSpaceOffset,
}

/// eXtensible Host Controller Operational Port Registers
///
/// See xHCI 1.2 Sections 5.4.8-5.4.11
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PortRegisters {
    PortStatusControl,
    PortPowerManagementStatusControl,
    PortLinkInfo,
    PortHardwareLpmControl,
}

/// eXtensible Host Controller Operational Registers
///
/// See xHCI 1.2 Section 5.4 Host Controller Operational Registers
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OperationalRegisters {
    UsbCommand,
    UsbStatus,
    PageSize,
    DeviceNotificationControl,
    CommandRingControlRegister,
    DeviceContextBaseAddressArrayPointerRegister,
    Configure,
    Port(u8, PortRegisters),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterrupterRegisters {
    Management,
    Moderation,
    EventRingSegmentTableSize,
    EventRingSegmentTableBaseAddress,
    EventRingDequeuePointer,
}

/// eXtensible Host Controller Runtime Registers
///
/// See xHCI 1.2 Section 5.5 Host Controller Runtime Registers
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RuntimeRegisters {
    MicroframeIndex,
    Interrupter(u16, InterrupterRegisters),
}

pub struct XhcRegMap {
    pub map: RegMap<Registers>,
    pub cap_len: usize,
    pub op_len: usize,
    pub run_len: usize,
    pub db_len: usize,
}

impl XhcRegMap {
    pub(super) const fn operational_offset(&self) -> usize {
        self.cap_len
    }
    pub(super) const fn runtime_offset(&self) -> usize {
        self.cap_len + self.op_len
    }
    pub(super) const fn doorbell_offset(&self) -> usize {
        self.cap_len + self.op_len + self.run_len
    }
}

lazy_static! {
    pub static ref XHC_REGS: XhcRegMap = {
        use CapabilityRegisters::*;
        use OperationalRegisters::*;
        use Registers::*;

        // xHCI 1.2 Table 5-9
        // (may be expanded if implementing extended capabilities)
        let cap_layout = [
            (Cap(CapabilityLength), 1),
            (Reserved, 1),
            (Cap(HciVersion), 2),
            (Cap(HcStructuralParameters1), 4),
            (Cap(HcStructuralParameters2), 4),
            (Cap(HcStructuralParameters3), 4),
            (Cap(HcCapabilityParameters1), 4),
            (Cap(DoorbellOffset), 4),
            (Cap(RuntimeRegisterSpaceOffset), 4),
            (Cap(HcCapabilityParameters2), 4),
        ].into_iter();

        let op_layout = [
            (Op(UsbCommand), 4),
            (Op(UsbStatus), 4),
            (Op(PageSize), 4),
            (Reserved, 8),
            (Op(DeviceNotificationControl), 4),
            (Op(CommandRingControlRegister), 8),
            (Reserved, 16),
            (Op(DeviceContextBaseAddressArrayPointerRegister), 8),
            (Op(Configure), 4),
            (Reserved, 964),
        ].into_iter();

        // Add the port registers
        let num_ports = NUM_USB2_PORTS + NUM_USB3_PORTS;
        let op_layout = op_layout.chain((0..num_ports).flat_map(|i| {
            use PortRegisters::*;
            [
                (Op(OperationalRegisters::Port(i, PortStatusControl)), 4),
                (Op(OperationalRegisters::Port(i, PortPowerManagementStatusControl)), 4),
                (Op(OperationalRegisters::Port(i, PortLinkInfo)), 4),
                (Op(OperationalRegisters::Port(i, PortHardwareLpmControl)), 4),
            ]
        }));

        let run_layout = todo!();

        // +1: 0th doorbell is Command Ring's.
        // TODO: it's a different layout, define a different variant for it?
        let db_layout = (0..MAX_DEVICE_SLOTS + 1).map(|i| (Doorbell(i), 4));

        // Stash the lengths for later use.
        let cap_len = cap_layout.clone().map(|(_, sz)| sz).sum();
        let op_len = op_layout.clone().map(|(_, sz)| sz).sum();
        let run_len = run_layout.clone().map(|(_, sz)| sz).sum();
        let db_len = db_layout.clone().map(|(_, sz)| sz).sum();

        let layout = cap_layout
            .chain(op_layout)
            .chain(run_layout)
            .chain(db_layout);

        let xhc_reg_map = XhcRegMap {
            map: RegMap::create_packed_iter(
                bits::XHC_CAP_BASE_REG_SZ,
                layout,
                Some(Reserved),
            ),
            cap_len,
            op_len,
            run_len,
            db_len,
        };

        // xHCI 1.2 Table 5-1:
        // Capability registers must be page-aligned, and they're first.
        // Operational-registers must be 4-byte-aligned. They follow cap regs.
        // `cap_len` is a multiple of 4 (32 at time of writing).
        assert_eq!(xhc_reg_map.operational_offset() % 4, 0);
        // Runtime registers must be 32-byte-aligned.
        // Both `cap_len` and `op_len` are (at present, cap_len is 1024 + 16*8),
        // so we can safely put Runtime registers immediately after them.
        // (Note: if VTIO is implemented, virtual fn's must be *page*-aligned)
        assert_eq!(xhc_reg_map.runtime_offset() % 32, 0);
        // Finally, the Doorbell array merely must be 4-byte-aligned.
        assert_eq!(xhc_reg_map.doorbell_offset() % 4, 0);
    };
}

//! XHCI Registers

#![allow(dead_code)]

use crate::util::regmap::RegMap;

use super::bits;

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
    // Op(OperationalRegisters),
}

/// eXtensible Host Controller Capability Registers
///
/// See xHCI 1.2 Section 5.3 Host Controller Capability Registers
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CapabilityRegisters {
    CapabilityLength,
    HciVersion,
    HcsParameters1,
    HcsParameters2,
    HcsParameters3,
    HccParameters1,
    HccParameters2,
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

lazy_static! {
    pub static ref XHC_REGS: RegMap<Registers> = {
        use CapabilityRegisters::*;
        use Registers::*;

        let layout = [
            (Cap(CapabilityLength), 1),
            (Reserved, 1),
            (Cap(HciVersion), 2),
            (Cap(HcsParameters1), 4),
            (Cap(HcsParameters2), 4),
            (Cap(HcsParameters3), 4),
            (Cap(HccParameters1), 4),
            (Cap(DoorbellOffset), 4),
            (Cap(RuntimeRegisterSpaceOffset), 4),
            (Cap(HccParameters2), 4),
        ];

        RegMap::create_packed(
            bits::XHC_CAP_BASE_REG_SZ,
            &layout,
            Some(Reserved),
        )
    };
}

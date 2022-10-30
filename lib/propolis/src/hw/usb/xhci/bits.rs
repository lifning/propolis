//! Constants and structures for XHCI.

use bitstruct::bitstruct;

/// Size of the USB-specific PCI configuration space.
///
/// See xHCI 1.2 Section 5.2 PCI Configuration Registers (USB)
pub const USB_PCI_CFG_REG_SZ: u8 = 3;

/// Offset of the USB-specific PCI configuration space.
///
/// See xHCI 1.2 Section 5.2 PCI Configuration Registers (USB)
pub const USB_PCI_CFG_OFFSET: u8 = 0x60;

/// Size of the Host Controller Capability Registers (excluding extended capabilities)
pub const XHC_CAP_BASE_REG_SZ: usize = 0x20;

bitstruct! {
    /// Representation of the Frame Length Adjustment Register (FLADJ).
    ///
    /// See xHCI 1.2 Section 5.2.4
    #[derive(Clone, Copy, Debug, Default)]
    pub struct FrameLengthAdjustment(pub u8) {
        /// Frame Length Timing Value (FLADJ)
        ///
        /// Used to select an SOF cycle time by adding 59488 to the value in this field.
        /// Ignored if NFC is set to 1.
        pub fladj: u8 = 0..6;

        /// No Frame Length Timing Capability (NFC)
        ///
        /// If set to 1, the controller does not support a Frame Length Timing Value.
        pub nfc: bool = 6;

        /// Reserved
        reserved: u8 = 7..8;
    }
}

bitstruct! {
    /// Representation of the Default Best Effort Service Latency [Deep] registers (DBESL / DBESLD).
    ///
    /// See xHCI 1.2 Section 5.2.5 & 5.2.6
    #[derive(Clone, Copy, Debug, Default)]
    pub struct DefaultBestEffortServiceLatencies(pub u8) {
        /// Default Best Effort Service Latency (DBESL)
        pub dbesl: u8 = 0..4;

        /// Default Best Effort Service Latency Deep (DBESLD)
        pub dbesld: u8 = 4..8;
    }
}

//! Constants and structures for XHCI.

// Not all of these fields may be relevant to us, but they're here for completeness.
#![allow(dead_code)]

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

bitstruct! {
    /// Representation of the Structural Parameters 1 (HCSPARAMS1) register.
    ///
    /// See xHCI 1.2 Section 5.3.3
    #[derive(Clone, Copy, Debug, Default)]
    pub struct HcStructuralParameters1(pub u32) {
        /// Number of Device Slots (MaxSlots)
        ///
        /// Indicates the number of device slots that the host controller supports
        /// (max num of Device Context Structures and Doorbell Array entries).
        ///
        /// Valid values are 1-255, 0 is reserved.
        pub max_slots: u8 = 0..8;

        /// Number of Interrupters (MaxIntrs)
        ///
        /// Indicates the number of interrupters that the host controller supports
        /// (max addressable Interrupter Register Sets).
        /// The value is 1 less than the actual number of interrupters.
        ///
        /// Valid values are 1-1024, 0 is undefined.
        pub max_intrs: u16 = 8..19;

        /// Reserved
        reserved: u8 = 19..24;

        /// Number of Ports (MaxPorts)
        ///
        /// Indicates the max Port Number value.
        ///
        /// Valid values are 1-255.
        pub max_ports: u8 = 24..32;
    }
}

bitstruct! {
    /// Representation of the Structural Parameters 2 (HCSPARAMS2) register.
    ///
    /// See xHCI 1.2 Section 5.3.4
    #[derive(Clone, Copy, Debug, Default)]
    pub struct HcStructuralParameters2(pub u32) {
        /// Isochronous Scheduling Threshold (IST)
        ///
        /// Minimum distance (in time) required to stay ahead of the controller while adding TRBs.
        pub iso_sched_threshold: u8 = 0..3;

        /// Indicates whether the IST value is in terms of frames (true) or microframes (false).
        pub ist_as_frame: bool = 3;

        /// Event Ring Segment Table Max (ERST Max)
        ///
        /// Max num. of Event Ring Segment Table entries = 2^(ERST Max).
        ///
        /// Valid values are 0-15.
        pub erst_max: u8 = 4..8;

        /// Reserved
        reserved: u16 = 8..21;

        /// Number of Scratchpad Buffers (Max Scratchpad Bufs Hi)
        ///
        /// High order 5 bits of the number of Scratchpad Buffers that shall be reserved for the
        /// controller.
        max_scratchpad_bufs_hi: u8 = 21..26;

        /// Scratchpad Restore (SPR)
        ///
        /// Whether Scratchpad Buffers should be maintained across power events.
        pub scratchpad_restore: bool = 26;

        /// Number of Scratchpad Buffers (Max Scratchpad Bufs Lo)
        ///
        /// Low order 5 bits of the number of Scratchpad Buffers that shall be reserved for the
        /// controller.
        max_scratchpad_bufs_lo: u8 = 27..32;
    }
}

impl HcStructuralParameters2 {
    pub fn max_scratchpad_bufs(&self) -> u16 {
        let lo = self.max_scratchpad_bufs_lo() as u16 | 0b11111;
        let hi = self.max_scratchpad_bufs_hi() as u16 | 0b11111;
        (hi << 5) | lo
    }

    pub fn with_max_scratchpad_bufs(self, max: u16) -> Self {
        let lo = max & 0b11111;
        let hi = (max >> 5) & 0b11111;
        self.with_max_scratchpad_bufs_lo(lo as u8)
            .with_max_scratchpad_bufs_hi(hi as u8)
    }
}

bitstruct! {
    /// Representation of the Structural Parameters 3 (HCSPARAMS3) register.
    ///
    /// See xHCI 1.2 Section 5.3.5
    #[derive(Clone, Copy, Debug, Default)]
    pub struct HcStructuralParameters3(pub u32) {
        /// U1 Device Exit Latency
        ///
        /// Worst case latency to transition from U1 to U0.
        ///
        /// Valid values are 0-10 indicating microseconds.
        pub u1_dev_exit_latency: u8 = 0..8;

        /// Reserved
        reserved: u8 = 8..16;

        /// U2 Device Exit Latency
        ///
        /// Worst case latency to transition from U2 to U0.
        ///
        /// Valid values are 0-2047 indicating microseconds.
        pub u2_dev_exit_latency: u16 = 16..32;
    }
}

bitstruct! {
    /// Representation of the Capability Parameters 1 (HCCPARAMS1) register.
    ///
    /// See xHCI 1.2 Section 5.3.6
    #[derive(Clone, Copy, Debug, Default)]
    pub struct HcCapabilityParameters1(pub u32) {
        /// 64-Bit Addressing Capability (AC64)
        ///
        /// Whether the controller supports 64-bit addressing.
        pub ac64: bool = 0;

        /// BW Negotiation Capability (BNC)
        ///
        /// Whether the controller supports Bandwidth Negotiation.
        pub bnc: bool = 1;

        /// Context Size (CSZ)
        ///
        /// Whether the controller uses the 64-byte Context data structures.
        pub csz: bool = 2;

        /// Port Power Control (PPC)
        ///
        /// Whether the controller supports Port Power Control.
        pub ppc: bool = 3;

        /// Port Indicators (PIND)
        ///
        /// Whether the xHC root hub supports port indicator control.
        pub pind: bool = 4;

        /// Light HC Reset Capability (LHRC)
        ///
        /// Whether the controller supports a Light Host Controller Reset.
        pub lhrc: bool = 5;

        /// Latency Tolerance Messaging Capability (LTC)
        ///
        /// Whether the controller supports Latency Tolerance Messaging.
        pub ltc: bool = 6;

        /// No Secondary SID Support (NSS)
        ///
        /// Whether the controller supports Secondary Stream IDs.
        pub nss: bool = 7;

        /// Parse All Event Data (PAE)
        ///
        /// Whether the controller parses all event data TRBs while advancing to the next TD
        /// after a Short Packet, or it skips all but the first Event Data TRB.
        pub pae: bool = 8;

        /// Stopped - Short Packet Capability (SPC)
        ///
        /// Whether the controller is capable of generating a Stopped - Short Packet
        /// Completion Code.
        pub spc: bool = 9;

        /// Stopped EDTLA Capability (SEC)
        ///
        /// Whether the controller's Stream Context supports a Stopped EDTLA field.
        pub sec: bool = 10;

        /// Contiguous Frame ID Capability (CFC)
        ///
        /// Whether the controller is capable of matching the Frame ID of consecutive
        /// isochronous TDs.
        pub cfc: bool = 11;

        /// Maximum Primary Stream Array Size (MaxPSASize)
        ///
        /// The maximum number of Primary Stream Array entries supported by the controller.
        ///
        /// Primary Stream Array size = 2^(MaxPSASize + 1)
        /// Valid values are 0-15, 0 indicates that Streams are not supported.
        pub max_primary_streams: u8 = 12..16;

        /// xHCI Extended Capabilities Pointer (xECP)
        ///
        /// Offset of the first Extended Capability (in 32-bit words).
        pub xecp: u16 = 16..32;
    }
}

bitstruct! {
    /// Representation of the Capability Parameters 2 (HCCPARAMS2) register.
    ///
    /// See xHCI 1.2 Section 5.3.9
    #[derive(Clone, Copy, Debug, Default)]
    pub struct HcCapabilityParameters2(pub u32) {
        /// U3 Entry Capability (U3C)
        ///
        /// Whether the controller root hub ports support port Suspend Complete
        /// notification.
        pub u3c: bool = 0;

        /// Configure Endpoint Command Max Exit Latency Too Large Capability (CMC)
        ///
        /// Indicates whether a Configure Endpoint Command is capable of generating
        /// a Max Exit Latency Too Large Capability Error.
        pub cmc: bool = 1;

        /// Force Save Context Capability (FSC)
        ///
        /// Whether the controller supports the Force Save Context Capability.
        pub fsc: bool = 2;

        /// Compliance Transition Capability (CTC)
        ///
        /// Inidcates whether the xHC USB3 root hub ports support the Compliance Transition
        /// Enabled (CTE) flag.
        pub ctc: bool = 3;

        /// Large ESIT Payload Capability (LEC)
        ///
        /// Indicates whether the controller supports ESIT Payloads larger than 48K bytes.
        pub lec: bool = 4;

        /// Configuration Information Capability (CIC)
        ///
        /// Indicates whether the controller supports extended Configuration Information.
        pub cic: bool = 5;

        /// Extended TBC Capability (ETC)
        ///
        /// Indicates if the TBC field in an isochronous TRB supports the definition of
        /// Burst Counts greater than 65535 bytes.
        pub etc: bool = 6;

        /// Extended TBC TRB Status Capability (ETC_TSC)
        ///
        /// Indicates if the TBC/TRBSts field in an isochronous TRB has additional
        /// information regarding TRB in the TD.
        pub etc_tsc: bool = 7;

        /// Get/Set Extended Property Capability (GSC)
        ///
        /// Indicates if the controller supports the Get/Set Extended Property commands.
        pub gsc: bool = 8;

        /// Virtualization Based Trusted I/O Capability (VTC)
        ///
        /// Whether the controller supports the Virtualization-based Trusted I/O Capability.
        pub vtc: bool = 9;

        /// Reserved
        reserved: u32 = 10..32;
    }
}

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
    #[inline]
    pub fn max_scratchpad_bufs(&self) -> u16 {
        let lo = self.max_scratchpad_bufs_lo() as u16 | 0b11111;
        let hi = self.max_scratchpad_bufs_hi() as u16 | 0b11111;
        (hi << 5) | lo
    }

    #[inline]
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

bitstruct! {
    /// Representation of the USB Command (USBCMD) register.
    ///
    /// See xHCI 1.2 Section 5.4.1
    #[derive(Clone, Copy, Debug, Default)]
    pub struct UsbCommand(pub u32) {
        /// Run/Stop (R/S)
        ///
        /// The controller continues execution as long as this bit is set to 1.
        pub run_stop: bool = 0;

        /// Host Controller Reset (HCRST)
        ///
        /// This control bit is used to reset the controller.
        pub host_controller_reset: bool = 1;

        /// Interrupter Enable (INTE)
        ///
        /// Enables or disables interrupts generated by Interrupters.
        pub interrupter_enable: bool = 2;

        /// Host System Error Enable (HSEE)
        ///
        /// Whether the controller shall assert out-of-band error signaling to the host.
        /// See xHCI 1.2 Section 4.10.2.6
        pub host_system_error_enable: bool = 3;

        /// Reserved
        reserved: u8 = 4..7;

        /// Light Host Controller Reset (LHCRST)
        ///
        /// This control bit is used to initiate a soft reset of the controller.
        /// (If the LHRC bit in HCCPARAMS is set to 1.)
        pub light_host_controller_reset: bool = 7;

        /// Controller Save State (CSS)
        ///
        /// When set to 1, the controller shall save any internal state.
        /// Always returns 0 when read.
        /// See xHCI 1.2 Section 4.23.2
        pub controller_save_state: bool = 8;

        /// Controller Restore State (CRS)
        ///
        /// When set to 1, the controller shall perform a Restore State operation.
        /// Always returns 0 when read.
        /// See xHCI 1.2 Section 4.23.2
        pub controller_restore_state: bool = 9;

        /// Enable Wrap Event (EWE)
        ///
        /// When set to 1, the controller shall generate an MFINDEX Wrap Event
        /// every time the MFINDEX register transitions from 0x3FFF to 0.
        /// See xHCI 1.2 Section 4.14.2
        pub enable_wrap_event: bool = 10;

        /// Enable U3 MFINDEX Stop (EU3S)
        ///
        /// When set to 1, the controller may stop incrementing MFINDEX if all
        /// Root Hub ports are in the U3, Disconnected, Disabled or Powered-off states.
        /// See xHCI 1.2 Section 4.14.2
        pub enable_u3_mfindex_stop: bool = 11;

        /// Reserved
        reserved2: u32 = 12;

        /// CEM Enable (CME)
        ///
        /// When set to 1, a Max Exit Latency Too Large Capability Error may be
        /// returned by a Configure Endpoint Command.
        /// See xHCI 1.2 Section 4.23.5.2.2
        pub cem_enable: bool = 13;

        /// Extended TBC Enable (ETE)
        ///
        /// Indicates whether the controller supports Transfer Burst Count (TBC)
        /// values greate than 4 in isochronous TDs.
        /// See xHCI 1.2 Section 4.11.2.3
        pub ete: bool = 14;

        /// Extended TBC TRB Status Enable (TSC_EN)
        ///
        /// Indicates whether the controller supports the ETC_TSC capability.
        /// See xHCI 1.2 Section 4.11.2.3
        pub tsc_enable: bool = 15;

        /// VTIO Enable (VTIOE)
        ///
        /// When set to 1, the controller shall enable the VTIO capability.
        pub vtio_enable: bool = 16;

        /// Reserved
        pub reserved3: u32 = 17..32;
    }
}

bitstruct! {
    /// Representation of the USB Status (USBSTS) register.
    ///
    /// See xHCI 1.2 Section 5.4.2
    #[derive(Clone, Copy, Debug, Default)]
    pub struct UsbStatus(pub u32) {
        /// Host Controller Halted (HCH)
        ///
        /// This bit is set to 0 whenever the R/S bit is set to 1. It is set to 1
        /// when the controller has stopped executing due to the R/S bit being cleared.
        pub host_controller_halted: bool = 0;

        /// Reserved
        reserved: u8 = 1;

        /// Host System Error (HSE)
        ///
        /// Indicates an error condition preventing continuing normal operation.
        pub host_system_error: bool = 2;

        /// Event Interrupt (EINT)
        ///
        /// The controller sets this bit to 1 when the IP bit of any interrupter
        /// goes from 0 to 1.
        pub event_interrupt: bool = 3;

        /// Port Change Detect (PCD)
        ///
        /// The controller sets this bit to 1 when any port has a change bit flip
        /// from 0 to 1.
        pub port_change_detect: bool = 4;

        /// Reserved
        reserved2: u8 = 5..8;

        /// Save State Status (SSS)
        ///
        /// A write to the CSS bit in the USBCMD register causes this bit to flip to
        /// 1. The controller shall clear this bit to 0 when the Save State operation
        /// has completed.
        pub save_state_status: bool = 8;

        /// Restore State Status (RSS)
        ///
        /// A write to the CRS bit in the USBCMD register causes this bit to flip to
        /// 1. The controller shall clear this bit to 0 when the Restore State operation
        /// has completed.
        pub restore_state_status: bool = 9;

        /// Save/Restore Error (SRE)
        ///
        /// Indicates that the controller has detected an error condition
        /// during a Save or Restore State operation.
        pub save_restore_error: bool = 10;

        /// Controller Not Ready (CNR)
        ///
        /// Indicates that the controller is not ready to accept doorbell
        /// or runtime register writes.
        pub controller_not_ready: bool = 11;

        /// Host Controller Error (HCE)
        ///
        /// Indicates if the controller has encountered an internal error
        /// that requires a reset to recover.
        pub host_controller_error: bool = 12;

        /// Reserved
        reserved3: u32 = 13..32;
    }
}

/// Representation of the Device Notification Control (DNCTRL) register.
///
/// Bits: 0-15 Notification Enable (N0-N15)
///
/// When set to 1, the controller shall generate a Device Notification Event
/// when a Device Notification Transaction Packet matching the set bit is received.
///
/// See xHCI 1.2 Section 5.4.4
pub type DeviceNotificationControl = bitvec::BitArr!(for 16, in u32);

bitstruct! {
    /// Representation of the Command Ring Control (CRCR) register.
    ///
    /// See xHCI 1.2 Section 5.4.5
    #[derive(Clone, Copy, Debug, Default)]
    pub struct CommandRingControl(pub u64) {
        /// Ring Cycle State (RCS)
        ///
        /// Indicates the Consumer Cycle State (CCS) flag for the TRB
        /// referenced by the Command Ring Pointer (CRP).
        pub ring_cycle_state: bool = 0;

        /// Command Stop (CS)
        ///
        /// When set to 1, the controller shall stop the Command Ring operation
        /// after the currently executing command has completed.
        pub command_stop: bool = 1;

        /// Command Abort (CA)
        ///
        /// When set to 1, the controller shall abort the currently executing
        /// command and stop the Command Ring operation.
        pub command_abort: bool = 2;

        /// Command Ring Running (CRR)
        ///
        /// This bit is set to 1 if the R/S bit is 1 and software submitted
        /// a Host Controller Command.
        pub command_ring_running: bool = 3;

        /// Reserved
        reserved: u8 = 4..6;

        /// Command Ring Pointer (CRP)
        ///
        /// The high order bits of the initial value of the Command Ring Dequeue Pointer.
        command_ring_pointer_: u64 = 6..64;
    }
}

impl CommandRingControl {
    /// The Command Ring Dequeue Pointer.
    #[inline]
    pub fn command_ring_pointer(&self) -> u64 {
        self.command_ring_pointer_() << 6
    }
}

bitstruct! {
    /// Representation of the Configure (CONFIG) register.
    ///
    /// See xHCI 1.2 Section 5.4.7
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Configure(pub u32) {
        /// Max Device Slots Enabled (MaxSlotsEn)
        ///
        /// The maximum number of enabled device slots.
        /// Valid values are 0 to MaxSlots.
        pub max_device_slots_enabled: u8 = 0..8;

        /// U3 Entry Enable (U3E)
        ///
        /// When set to 1, the controller shall assert the PLC flag
        /// when a Root hub port enters U3 state.
        pub u3_entry_enable: bool = 8;

        /// Configuration Information Enable (CIE)
        ///
        /// When set to 1, the software shall initialize the
        /// Configuration Value, Interface Number, and Alternate Setting
        /// fields in the Input Control Context.
        pub configuration_information_enable: bool = 9;

        /// Reserved
        reserved: u32 = 10..32;
    }
}

bitstruct! {
    /// Representation of a Doorbell Register.
    ///
    /// From the guest's perspective, this should be write-only (reads 0).
    /// See xHCI 1.2 Section 5.6
    #[derive(Clone, Copy, Debug, Default)]
    pub struct DoorbellRegister(pub u32) {
        /// Doorbell Target
        ///
        /// Written value corresponds to a specific xHC notification.
        ///
        /// Values 1..=31 correspond to enqueue pointer updates (see spec).
        /// Values 0 and 32..=247 are reserved.
        /// Values 248..=255 are vendor-defined (and we're the vendor).
        pub db_target: u8 = 0..8;

        /// Reserved
        reserved: u8 = 8..16;

        /// Doorbell Stream ID
        pub db_stream_id: u16 = 16..32;
    }
}

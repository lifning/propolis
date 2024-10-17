use std::ops::{Deref, DerefMut};

use bitstruct::bitstruct;
use strum::FromRepr;

use crate::common::GuestAddr;

/// See xHCI 1.2 sect 4.5.3 & table 6-7
#[derive(Copy, Clone, FromRepr)]
#[repr(u8)]
pub enum SlotState {
    DisabledEnabled = 0,
    Default = 1,
    Addressed = 2,
    Configured = 3,
    Reserved4 = 4,
    Reserved5 = 5,
    Reserved6 = 6,
    Reserved7 = 7,
    Reserved8 = 8,
    Reserved9 = 9,
    Reserved10 = 10,
    Reserved11 = 11,
    Reserved12 = 12,
    Reserved13 = 13,
    Reserved14 = 14,
    Reserved15 = 15,
    Reserved16 = 16,
    Reserved17 = 17,
    Reserved18 = 18,
    Reserved19 = 19,
    Reserved20 = 20,
    Reserved21 = 21,
    Reserved22 = 22,
    Reserved23 = 23,
    Reserved24 = 24,
    Reserved25 = 25,
    Reserved26 = 26,
    Reserved27 = 27,
    Reserved28 = 28,
    Reserved29 = 29,
    Reserved30 = 30,
    Reserved31 = 31,
}

impl Into<u8> for SlotState {
    fn into(self) -> u8 {
        self as u8
    }
}
impl From<u8> for SlotState {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect("SlotState should only be converted from a 5-bit field in SlotContext")
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct SlotContext {
    first: SlotContextFirst,
    reserved: u128,
}

// HACK: the .with_* from bitstruct will only return First,
// but we ultimately only care about .set_*
impl Deref for SlotContext {
    type Target = SlotContextFirst;
    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
impl DerefMut for SlotContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.first
    }
}

bitstruct! {
    /// Representation of the first half of a Slot Context.
    /// (the second half is 128-bits of reserved.)
    ///
    /// See xHCI 1.2 Section 6.2.2
    #[derive(Clone, Copy, Debug, Default)]
    pub struct SlotContextFirst(pub u128) {
        /// Used by hubs to route packets to the correct port. (USB3 section 8.9)
        pub route_string: u32 = 0..20;

        /// Deprecated in xHCI.
        speed: u8 = 20..24;

        reserved0: bool = 24;

        // TODO: doc
        pub multi_tt: bool = 25;

        pub hub: bool = 26;

        /// Index of the last valid endpoint context within the Device Context
        /// that contains this Slot Context. Valid values are 1 through 31.
        pub context_entries: u8 = 27..32;

        /// Indicates the worst-case time it takes to wake up all the links
        /// in the path to the device, given the current USB link level power
        /// management settings, in microseconds.
        pub max_exit_latency_micros: u16 = 32..48;

        /// Indicates the root hub port number used to access this device.
        /// Valid values are 1 through the controller's max number of ports.
        /// (See xHCI 1.2 sect 4.19.7 for numbering info)
        pub root_hub_port_number: u8 = 48..56;

        /// If this device is a hub, guest sets this to the number of
        /// downstream-facing ports supported by the hub. (USB2 table 11-13)
        pub number_of_ports: u8 = 56..64;

        pub parent_hub_slot_id: u8 = 64..72;

        pub parent_port_number: u8 = 72..80;

        pub tt_think_time: u8 = 80..82;

        reserved1: u8 = 82..86;

        pub interrupter_target: u16 = 86..96;

        pub usb_device_address: u8 = 96..104;

        reserved2: u32 = 104..123;

        /// Updated by xHC when device slot transitions states.
        pub slot_state: SlotState = 123..128
    }
}

/// See xHCI 1.2 table 6-8
#[derive(Copy, Clone, FromRepr)]
#[repr(u8)]
pub enum EndpointState {
    Disabled = 0,
    Running = 1,
    Halted = 2,
    Stopped = 3,
    Error = 4,
    Reserved5 = 5,
    Reserved6 = 6,
    Reserved7 = 7,
}

impl Into<u8> for EndpointState {
    fn into(self) -> u8 {
        self as u8
    }
}
impl From<u8> for EndpointState {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect("EndpointState should only be converted from a 3-bit field in EndpointContext")
    }
}

/// See xHCI 1.2 table 6-8
#[derive(Copy, Clone, FromRepr)]
#[repr(u8)]
pub enum EndpointType {
    NotValid = 0,
    IsochOut = 1,
    BulkOut = 2,
    InterruptOut = 3,
    Control = 4,
    IsochIn = 5,
    BulkIn = 6,
    InterruptIn = 7,
}

impl Into<u8> for EndpointType {
    fn into(self) -> u8 {
        self as u8
    }
}
impl From<u8> for EndpointType {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect("EndpointType should only be converted from a 3-bit field in EndpointContext")
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct EndpointContext {
    first: EndpointContextFirst,
    second: EndpointContextSecond,
}

impl Deref for EndpointContext {
    type Target = EndpointContextFirst;
    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
impl DerefMut for EndpointContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.first
    }
}

impl EndpointContext {
    pub fn average_trb_length(&self) -> u16 {
        self.second.average_trb_length()
    }
    pub fn set_average_trb_length(&mut self, value: u16) {
        self.second.set_average_trb_length(value)
    }
}

bitstruct! {
    /// Representation of the first half of an Endpoint Context.
    ///
    /// See xHCI 1.2 Section 6.2.3
    #[derive(Clone, Copy, Debug, Default)]
    pub struct EndpointContextFirst(pub u128) {
        pub endpoint_state: EndpointState = 0..3;

        reserved1: u8 = 3..8;

        pub mult: u8 = 8..10;

        pub max_primary_streams: u8 = 10..15;

        pub linear_stream_array: bool = 15;

        pub interval: u8 = 16..24;

        pub max_endpoint_service_time_interval_payload_high: u8 = 24..32;

        reserved2: bool = 32;

        pub error_count: u8 = 33..35;

        pub endpoint_type: EndpointType = 35..38;

        reserved3: bool = 38;

        pub host_initiate_disable: bool = 39;

        pub max_burst_size: u8 = 40..48;

        pub max_packet_size: u16 = 48..64;

        pub dequeue_cycle_state: bool = 64;

        reserved4: u8 = 65..68;

        tr_dequeue_pointer_: u64 = 68..128;
    }
}

impl EndpointContextFirst {
    pub fn tr_dequeue_pointer(&self) -> GuestAddr {
        GuestAddr(self.tr_dequeue_pointer_() << 4)
    }
    #[must_use]
    pub const fn with_tr_dequeue_pointer(self, value: GuestAddr) -> Self {
        self.with_tr_dequeue_pointer_(value.0 >> 4)
    }
    pub fn set_address(&mut self, value: GuestAddr) {
        self.set_tr_dequeue_pointer_(value.0 >> 4);
    }
}

bitstruct! {
    /// Representation of the second half of an Endpoint Context.
    ///
    /// See xHCI 1.2 Section 6.2.3
    #[derive(Clone, Copy, Debug, Default)]
    pub struct EndpointContextSecond(pub u128) {
        pub average_trb_length: u16 = 0..16;

        pub max_endpoint_service_time_interval: u16 = 16..32;

        reserved0: u128 = 32..128;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct InputControlContext {
    drop_context: AddDropContextFlags,
    add_context: AddDropContextFlags,
    reserved: [u32; 5],
    last: InputControlContextLast,
}
pub type AddDropContextFlags = bitvec::BitArr!(for 32, in u32);

bitstruct! {
    /// Represrentation of the last 32-bits of an InputControlContext.
    ///
    /// See xHCI 1.2 table 6-17
    #[derive(Clone, Copy, Debug, Default)]
    pub struct InputControlContextLast(pub u32) {
        pub configuration_value: u8 = 0..8;
        pub interface_number: u8 = 8..16;
        pub alternate_setting: u8 = 16..24;
        reserved0: u8 = 24..32;
    }
}

impl Deref for InputControlContext {
    type Target = InputControlContextLast;
    fn deref(&self) -> &Self::Target {
        &self.last
    }
}
impl DerefMut for InputControlContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.last
    }
}

impl InputControlContext {
    pub fn drop_context(&self, index: usize) -> Option<bool> {
        if index < 2 {
            None
        } else {
            self.drop_context.get(index).map(|bit| *bit)
        }
    }
    pub fn set_drop_context(&mut self, index: usize, value: bool) {
        if index > 2 {
            if let Some(bitref) = self.drop_context.get_mut(index) {
                bitref.commit(value);
            }
        }
    }
    pub fn add_context(&self, index: usize) -> Option<bool> {
        self.add_context.get(index).map(|bit| *bit)
    }
    pub fn set_add_context(&mut self, index: usize, value: bool) {
        if let Some(bitref) = self.add_context.get_mut(index) {
            bitref.commit(value);
        }
    }
}

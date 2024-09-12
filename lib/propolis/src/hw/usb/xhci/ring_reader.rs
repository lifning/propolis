use crate::{common::GuestAddr, hw::usb::xhci::*, vmm::mem::MemCtx};
use bitstruct::bitstruct;
use strum::FromRepr;

pub struct Ring<T: Copy + Default> {
    addr: GuestAddr,
    shadow_copy: Vec<T>,
    enqueue_index: usize,
    dequeue_index: usize,
    producer_cycle_state: bool,
    consumer_cycle_state: bool,
}

/// See xHCI 1.2 section 4.14 "Managing Transfer Rings"
impl<T: Copy + Default> Ring<T> {
    fn new(addr: GuestAddr, num_elem: usize) -> Self {
        Self {
            addr,
            shadow_copy: vec![Default::default(); num_elem],
            enqueue_index: 0,
            dequeue_index: 0,
            producer_cycle_state: false,
            consumer_cycle_state: false,
        }
    }
    #[cfg(test)]
    fn new_synthetic(shadow_copy: Vec<T>) -> Self {
        Self {
            addr: GuestAddr(0),
            shadow_copy,
            enqueue_index: 0,
            dequeue_index: 0,
            producer_cycle_state: false,
            consumer_cycle_state: false,
        }
    }
    fn update_from_guest(&mut self, memctx: &mut MemCtx) {
        let many =
            memctx.read_many::<T>(self.addr, self.shadow_copy.len()).unwrap();
        self.shadow_copy.clear();
        self.shadow_copy.extend(many);
        //let byte_len = self.shadow_copy.len() * core::mem::size_of::<T>();
        //memctx.direct_read_into(self.addr, &mut self.shadow_copy, byte_len);
    }
    fn write_to_guest(&self, memctx: &mut MemCtx) {
        assert!(memctx.write_many(self.addr, &self.shadow_copy))
    }
    fn is_empty(&self) -> bool {
        self.enqueue_index == self.dequeue_index
    }
    // fn is_full(&self) -> bool {
    //     // TODO note: depends on Link TRBs, 4.11.5.1
    //     todo!("enqueue index + 1") == self.dequeue_index
    // }
    fn enqueue(&mut self, value: T) -> Result<(), T> {
        // TODO: here's how a naive circular buffer would work...
        // but this is NOT how xHCI does it.
        //
        // let next_enq = (self.enqueue_index + 1) % self.shadow_copy.len();
        // if next_enq != self.dequeue_index {
        //     self.shadow_copy[self.enqueue_index] = value;
        //     self.enqueue_index = next_enq;
        //     Ok(())
        // } else {
        //     Err(value)
        // }
        todo!()
    }
    fn dequeue(&mut self) -> Option<T> {
        // TODO: ditto.
        //
        // if self.dequeue_index != self.enqueue_index {
        //     let value = self.shadow_copy[self.dequeue_index];
        //     self.dequeue_index =
        //         (self.dequeue_index + 1) % self.shadow_copy.len();
        //     Some(value)
        // } else {
        //     None
        // }
        todo!()
    }
}

pub type TransferRing = Ring<TransferRequestBlock>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransferRequestBlock {
    /// may be an address or immediate data
    parameter: u64,
    status: TrbStatusField,
    control: TrbControlField,
}

impl Default for TransferRequestBlock {
    fn default() -> Self {
        Self {
            parameter: 0,
            status: Default::default(),
            control: TrbControlField { normal: Default::default() },
        }
    }
}

/// xHCI 1.2 Section 6.4.6
#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TrbType {
    Reserved0 = 0,
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    NoOp = 8,
    EnableSlotCmd = 9,
    DisableSlotCmd = 10,
    AddressDeviceCmd = 11,
    ConfigureEndpointCmd = 12,
    EvaluateContextCmd = 13,
    ResetEndpointCmd = 14,
    StopEndpointCmd = 15,
    SetTRDequeuePointerCmd = 16,
    ResetDeviceCmd = 17,
    ForceEventCmd = 18,
    NegotiateBandwidthCmd = 19,
    SetLatencyToleranceValueCmd = 20,
    GetPortBandwidthCmd = 21,
    ForceHeaderCmd = 22,
    NoOpCmd = 23,
    GetExtendedPropertyCmd = 24,
    SetExtendedPropertyCmd = 25,
    Reserved26 = 26,
    Reserved27 = 27,
    Reserved28 = 28,
    Reserved29 = 29,
    Reserved30 = 30,
    Reserved31 = 31,
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    BandwidthRequestEvent = 35,
    DoorbellEvent = 36,
    HostControllerEvent = 37,
    DeviceNotificationEvent = 38,
    MfIndexWrapEvent = 39,
    Reserved40 = 40,
    Reserved41 = 41,
    Reserved42 = 42,
    Reserved43 = 43,
    Reserved44 = 44,
    Reserved45 = 45,
    Reserved46 = 46,
    Reserved47 = 47,
    Vendor48 = 48,
    Vendor49 = 49,
    Vendor50 = 50,
    Vendor51 = 51,
    Vendor52 = 52,
    Vendor53 = 53,
    Vendor54 = 54,
    Vendor55 = 55,
    Vendor56 = 56,
    Vendor57 = 57,
    Vendor58 = 58,
    Vendor59 = 59,
    Vendor60 = 60,
    Vendor61 = 61,
    Vendor62 = 62,
    Vendor63 = 63,
}

impl From<u8> for TrbType {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect("TrbType should only be converted from a 6-bit field in TrbControlField")
    }
}
impl Into<u8> for TrbType {
    fn into(self) -> u8 {
        self as u8
    }
}

/// Or "TRT". See xHCI 1.2 Table 6-26 and Section 4.11.2.2
#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TrbTransferType {
    NoDataStage = 0,
    Reserved = 1,
    OutDataStage = 2,
    InDataStage = 3,
}
impl From<u8> for TrbTransferType {
    fn from(value: u8) -> Self {
        Self::from_repr(value).expect("TrbTransferType should only be converted from a 2-bit field in TrbControlField")
    }
}
impl Into<u8> for TrbTransferType {
    fn into(self) -> u8 {
        self as u8
    }
}

/// Or "TRT". See xHCI 1.2 Table 6-26 and Section 4.11.2.2
#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TrbDirection {
    Out = 0,
    In = 1,
}
impl From<bool> for TrbDirection {
    fn from(value: bool) -> Self {
        unsafe { core::mem::transmute(value as u8) }
    }
}
impl Into<bool> for TrbDirection {
    fn into(self) -> bool {
        self == Self::In
    }
}

pub struct TransferDescriptor {
    trbs: Vec<TransferRequestBlock>,
}
impl TransferDescriptor {
    /// xHCI 1.2 sect 4.14: The TD Transfer Size is defined by the sum of the
    /// TRB Transfer Length fields in all TRBs that comprise the TD.
    pub fn transfer_size(&self) -> usize {
        self.trbs
            .iter()
            .map(|trb| trb.status.trb_transfer_length() as usize)
            .sum()
    }

    // TODO: validate my read of the below
    /// xHCI 1.2 sect 4.9.1: To generate a zero-length USB transaction,
    /// software shall define a TD with a single Transfer TRB with its
    /// transfer length set to 0. (it may include others, such as Link TRBs or
    /// Event Data TRBs, but only one 'Transfer TRB')
    /// (see also xHCI 1.2 table 6-21; as 4.9.1 is ambiguously worded.
    /// we're looking at *Normal* Transfer TRBs)
    pub fn is_zero_length(&self) -> bool {
        let mut trb_transfer_length = None;
        for trb in &self.trbs {
            if trb.control.trb_type() == TrbType::Normal {
                let x = trb.status.trb_transfer_length();
                if x != 0 {
                    return false;
                }
                // more than one Normal encountered
                if trb_transfer_length.replace(x).is_some() {
                    return false;
                }
            }
        }
        return true;
    }
}

// TODO: move to ::bits
/// Representations of the 'control' field of Transfer Request Block (TRB).
/// The field definitions differ depending on the TrbType.
/// See xHCI 1.2 Section 6.4.1 (Comments are paraphrases thereof)
#[derive(Copy, Clone)]
pub union TrbControlField {
    normal: TrbControlFieldNormal,
    setup_stage: TrbControlFieldSetupStage,
    data_stage: TrbControlFieldDataStage,
    status_stage: TrbControlFieldStatusStage,
}

impl TrbControlField {
    fn trb_type(&self) -> TrbType {
        // all variants are alike in TRB type location
        unsafe { self.normal.trb_type() }
    }
}

bitstruct! {
    /// Normal TRB control fields (xHCI 1.2 table 6-22)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbControlFieldNormal(pub u32) {
        /// Used to mark the Enqueue Pointer of the Transfer Ring.
        pub cycle: bool = 0;

        /// Or "ENT". If set, the xHC shall fetch and evaluate the next TRB
        /// before saving the endpoint state (see xHCI 1.2 Section 4.12.3)
        pub evaluate_next_trb: bool = 1;

        /// Or "ISP". If set, and a Short Packet is encountered for this TRB
        /// (less than the amount specified in the TRB Transfer Length),
        /// then a Transfer Event TRB shall be generated with its
        /// Completion Code set to Short Packet and its TRB Transfer Length
        /// field set to the residual number of bytes not transfered into
        /// the associated data buffer.
        pub interrupt_on_short_packet: bool = 2;

        /// Or "NS".
        // TODO: description
        pub no_snoop: bool = 3;

        /// Or "CH".
        // TODO: description
        pub chain_bit: bool = 4;

        /// Or "IOC".
        // TODO: description
        pub interrupt_on_completion: bool = 5;

        /// Or "IDT".
        // TODO: description
        pub immediate_data: bool = 6;

        reserved1: u8 = 7..9;

        /// Or "BEI".
        // TODO: description
        pub block_event_interrupt: bool = 9;

        // TODO: description
        pub trb_type: TrbType = 10..16;

        reserved2: u16 = 16..32;
    }
}

bitstruct! {
    /// Setup Stage TRB control fields (xHCI 1.2 table 6-26)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbControlFieldSetupStage(pub u32) {
        /// Used to mark the Enqueue Pointer of the Transfer Ring.
        pub cycle: bool = 0;

        reserved1: u8 = 1..5;

        /// Or "IOC".
        // TODO: description
        pub interrupt_on_completion: bool = 5;

        /// Or "IDT".
        // TODO: description
        pub immediate_data: bool = 6;

        reserved2: u8 = 7..10;

        // TODO: description
        pub trb_type: TrbType = 10..16;

        /// Or "TRT"
        // TODO: description
        pub transfer_type: TrbTransferType = 16..18;

        reserved3: u16 = 18..32;
    }
}

bitstruct! {
    /// Data Stage TRB control fields (xHCI 1.2 table 6-29)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbControlFieldDataStage(pub u32) {
        /// Used to mark the Enqueue Pointer of the Transfer Ring.
        pub cycle: bool = 0;

        /// Or "ENT". If set, the xHC shall fetch and evaluate the next TRB
        /// before saving the endpoint state (see xHCI 1.2 Section 4.12.3)
        pub evaluate_next_trb: bool = 1;

        /// Or "ISP". If set, and a Short Packet is encountered for this TRB
        /// (less than the amount specified in the TRB Transfer Length),
        /// then a Transfer Event TRB shall be generated with its
        /// Completion Code set to Short Packet and its TRB Transfer Length
        /// field set to the residual number of bytes not transfered into
        /// the associated data buffer.
        pub interrupt_on_short_packet: bool = 2;

        /// Or "NS".
        // TODO: description
        pub no_snoop: bool = 3;

        /// Or "CH".
        // TODO: description
        pub chain_bit: bool = 4;

        /// Or "IOC".
        // TODO: description
        pub interrupt_on_completion: bool = 5;

        /// Or "IDT".
        // TODO: description
        pub immediate_data: bool = 6;

        reserved1: u8 = 7..10;

        // TODO: description
        pub trb_type: TrbType = 10..16;

        /// Or "DIR".
        pub direction: TrbDirection = 16;

        reserved2: u16 = 17..32;
    }
}

bitstruct! {
    /// Status Stage TRB control fields (xHCI 1.2 table 6-31)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbControlFieldStatusStage(pub u32) {
        /// Used to mark the Enqueue Pointer of the Transfer Ring.
        pub cycle: bool = 0;

        /// Or "ENT". If set, the xHC shall fetch and evaluate the next TRB
        /// before saving the endpoint state (see xHCI 1.2 Section 4.12.3)
        pub evaluate_next_trb: bool = 1;

        reserved1: u8 = 2..4;

        /// Or "CH".
        // TODO: description
        pub chain_bit: bool = 4;

        /// Or "IOC".
        // TODO: description
        pub interrupt_on_completion: bool = 5;

        reserved2: u8 = 6..10;

        // TODO: description
        pub trb_type: TrbType = 10..16;

        /// Or "DIR".
        pub direction: TrbDirection = 16;

        reserved3: u16 = 17..32;
    }
}

bitstruct! {
    /// Representation of the 'status' field of Transfer Request Block (TRB).
    ///
    /// See xHCI 1.2 Section 6.4.1 (Comments are paraphrases thereof)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbStatusField(pub u32) {
        /// For OUT, this field defines the number of data bytes the xHC shall
        /// send during the execution of this TRB. If this field is 0 when the
        /// xHC fetches this TRB, xHC shall execute a zero-length transaction.
        /// (See xHCI 1.2 section 4.9.1 for zero-length TRB handling)
        ///
        /// For IN, this field indicates the size of the data buffer referenced
        /// by the Data Buffer Pointer, i.e. the number of bytes the host
        /// expects the endpoint to deliver.
        ///
        /// "Valid values are 0 to 64K."
        pub trb_transfer_length: u32 = 0..17;

        /// Indicates number of packets remaining in the Transfer Descriptor.
        /// (See xHCI 1.2 section 4.10.2.4)
        pub td_size: u8 = 17..22;

        /// The index of the Interrupter that will receive events generated
        /// by this TRB. "Valid values are between 0 and MaxIntrs-1."
        pub interrupter_target: u16 = 22..32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_device_descriptor_ring() {
        // mimicking pg. 85 of xHCI 1.2
        let ring_contents = vec![
            TransferRequestBlock {
                parameter: 0, // todo!("bmRequestType 0x80, bRequest 6, wValue 0x100, wIndex 0, wLength 8"),
                status: TrbStatusField::default()
                    .with_td_size(8)
                    .with_interrupter_target(0),
                control: TrbControlField {
                    setup_stage: TrbControlFieldSetupStage::default()
                        .with_cycle(true)
                        .with_immediate_data(true)
                        .with_trb_type(TrbType::SetupStage)
                        .with_transfer_type(TrbTransferType::InDataStage),
                },
            },
            TransferRequestBlock {
                parameter: 0x123456789abcdef0u64,
                status: TrbStatusField::default().with_trb_transfer_length(8),
                control: TrbControlField {
                    data_stage: TrbControlFieldDataStage::default()
                        .with_cycle(true)
                        .with_trb_type(TrbType::DataStage)
                        .with_direction(TrbDirection::In),
                },
            },
            TransferRequestBlock {
                parameter: 0,
                status: TrbStatusField::default(),
                control: TrbControlField {
                    status_stage: TrbControlFieldStatusStage::default()
                        .with_cycle(true)
                        .with_interrupt_on_completion(true)
                        .with_trb_type(TrbType::StatusStage)
                        .with_direction(TrbDirection::Out),
                },
            },
        ];
        let ring = Ring::new_synthetic(ring_contents.clone());
        // TODO: read from ring
        let td = TransferDescriptor { trbs: ring_contents };
        assert_eq!(td.transfer_size(), 8);
    }
}

#![allow(unused)]

use std::marker::PhantomData;

use crate::{
    common::{GuestAddr, GuestRegion},
    vmm::mem::MemCtx,
};
use bitstruct::bitstruct;
use strum::FromRepr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error in xHC Ring: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("Last TRB in ring at 0x{0:?} was not Link: {1:?}")]
    MissingLink(GuestAddr, TrbType),
    #[error("Tried to construct Command Descriptor from multiple TRBs")]
    CommandDescriptorSize,
    #[error("Tried to construct Event Descriptor from multiple TRBs")]
    EventDescriptorSize,
    #[error("Guest defined a segmented ring larger than allowed maximum size")]
    SegmentedRingTooLarge,
    #[error("Failed reading TRB from guest memory")]
    FailedReadingTRB,
    #[error("Incomplete TD: no more TRBs in cycle to complete chain: {0:?}")]
    IncompleteWorkItem(Vec<TransferRequestBlock>),
    #[error("Event Ring full when trying to enqueue {0:?}")]
    EventRingFull(TransferRequestBlock),
    #[error("Event Ring Segment Table of size {1} cannot be read from address {0:?}")]
    EventRingSegmentTableLocationInvalid(GuestAddr, usize),
    #[error("Event Ring Segment Table Entry has invalid size: {0:?}")]
    InvalidEventRingSegmentSize(EventRingSegment),
}
pub type Result<T> = core::result::Result<T, Error>;

pub trait WorkItem: Sized + IntoIterator<Item = TransferRequestBlock> {
    fn try_from_trb_iter(
        trbs: impl IntoIterator<Item = TransferRequestBlock>,
    ) -> Result<Self>;
}

pub type TransferRing = ConsumerRing<TransferDescriptor>;
pub type CommandRing = ConsumerRing<CommandDescriptor>;

pub struct ConsumerRing<T: WorkItem> {
    addr: GuestAddr,
    shadow_copy: Vec<TransferRequestBlock>,
    dequeue_index: usize,
    consumer_cycle_state: bool,
    _ghost: PhantomData<T>,
}

/// See xHCI 1.2 section 4.14 "Managing Transfer Rings"
impl<T: WorkItem> ConsumerRing<T> {
    fn new(addr: GuestAddr, num_elem: usize) -> Self {
        // TODO: bound size?
        Self {
            addr,
            shadow_copy: vec![TransferRequestBlock::default(); num_elem],
            dequeue_index: 0,
            consumer_cycle_state: true,
            _ghost: PhantomData,
        }
    }
    #[cfg(test)]
    fn new_synthetic(shadow_copy: Vec<TransferRequestBlock>) -> Self {
        Self {
            addr: GuestAddr(0),
            shadow_copy,
            dequeue_index: 0,
            consumer_cycle_state: true,
            _ghost: PhantomData,
        }
    }

    fn queue_advance(&mut self) {
        self.dequeue_index = self.queue_next_index()
    }
    fn queue_next_index(&mut self) -> usize {
        (self.dequeue_index + 1) % self.shadow_copy.len()
    }

    /// xHCI 1.2 sect 4.9.2: When a Transfer Ring is enabled or reset,
    /// the xHC initializes its copies of the Enqueue and Dequeue Pointers
    /// with the value of the Endpoint/Stream Context TR Dequeue Pointer field.
    fn reset(&mut self, tr_dequeue_pointer: GuestAddr) {
        let index = (tr_dequeue_pointer.0 - self.addr.0) as usize
            / size_of::<TransferRequestBlock>();
        self.dequeue_index = index;
    }

    // xHCI 1.2 sect 4.9: "TRB Rings may be larger than a Page,
    // however they shall not cross a 64K byte boundary."
    // xHCI 1.2 sect 4.11.5.1: "The Ring Segment Pointer field in a Link TRB
    // is not required to point to the beginning of a physical memory page."
    // (They *are* required to be at least 16-byte aligned, i.e. sizeof::<TRB>())
    fn update_from_guest(&mut self, memctx: &mut MemCtx) -> Result<()> {
        let mut new_shadow =
            Vec::<TransferRequestBlock>::with_capacity(self.shadow_copy.len());
        let mut addr = self.addr;

        // arbitrary upper limit: if a ring is larger than this, assume
        // something may be trying to attack us from a compromised guest
        let mut trb_count = 0;
        const UPPER_LIMIT: usize =
            1024 * 1024 * 1024 / size_of::<TransferRequestBlock>();

        loop {
            if let Some(val) = memctx.read(addr) {
                new_shadow.push(val);
                trb_count += 1;
                if trb_count >= UPPER_LIMIT {
                    return Err(Error::SegmentedRingTooLarge);
                }
                if val.control.trb_type() == TrbType::Link {
                    // xHCI 1.2 figure 6-38
                    addr = GuestAddr(val.parameter & !15);
                    if addr == self.addr {
                        break;
                    }
                } else {
                    addr = addr.offset::<TransferRequestBlock>(1);
                }
            } else {
                return Err(Error::FailedReadingTRB);
            }
        }

        // xHCI 1.2 sect 4.9.2.1: The last TRB in a Ring Segment is always a Link TRB.
        let last_trb_type = new_shadow.last().unwrap().control.trb_type();
        if last_trb_type != TrbType::Link {
            Err(Error::MissingLink(self.addr, last_trb_type))
        } else {
            self.shadow_copy = new_shadow;
            Ok(())
        }
    }

    /// Find the first transfer-related TRB, if one exists.
    /// (See xHCI 1.2 sect 4.9.2)
    fn dequeue_trb(&mut self) -> Option<TransferRequestBlock> {
        let start_index = self.dequeue_index;
        loop {
            let trb = self.shadow_copy[self.dequeue_index];
            // cycle bit transition - found enqueue pointer
            if trb.control.cycle() != self.consumer_cycle_state {
                return None;
            }
            // xHCI 1.2 figure 4-7
            if trb.control.trb_type() == TrbType::Link {
                if unsafe { trb.control.link.toggle_cycle() } {
                    // xHCI 1.2 figure 4-8
                    self.consumer_cycle_state = !self.consumer_cycle_state;
                }
                self.queue_advance();
                // failsafe - in case of full circuit of matching cycle bits
                // without a toggle_cycle occurring
                if self.dequeue_index == start_index {
                    return None;
                }
            } else {
                // TODO: do we skip, e.g., NoOp TRBs?
                return Some(trb);
            }
        }
    }

    fn dequeue_work_item(&mut self) -> Option<Result<T>> {
        let mut trbs = vec![self.dequeue_trb()?];
        while trbs.last().unwrap().control.chain_bit().unwrap_or(false) {
            // TODO: do we need to consider chain bits of link trb's this would skip?
            if let Some(trb) = self.dequeue_trb() {
                trbs.push(trb);
            } else {
                // TODO: we need more TRBs for this work item that aren't here yet!
                return Some(Err(Error::IncompleteWorkItem(trbs)));
            }
        }
        Some(T::try_from_trb_iter(trbs))
    }
}

pub struct EventRing {
    /// EREP.
    enqueue_pointer: GuestAddr,

    /// xHCI 1.2 sect 4.9.4: software writes the ERDP register to inform
    /// the xHC it has completed processing TRBs up to and including the
    /// TRB pointed to by ERDP.
    dequeue_pointer: GuestAddr,

    /// ESRTE's.
    segment_table: Vec<EventRingSegment>,
    /// ESRT Count.
    segment_table_index: usize,
    /// PCS.
    producer_cycle_state: bool,
}

impl EventRing {
    fn new(
        erstba: GuestAddr,
        erstsz: usize,
        erdp: GuestAddr,
        memctx: &mut MemCtx,
    ) -> Result<Self> {
        let mut x = Self {
            enqueue_pointer: GuestAddr(0),
            dequeue_pointer: erdp,
            segment_table: Vec::new(),
            segment_table_index: 0,
            producer_cycle_state: true,
        };
        x.update_segment_table(erstba, erstsz, memctx)?;
        x.enqueue_pointer = x.segment_table[0].base_address;
        Ok(x)
    }

    // xHCI 1.2 sect 4.9.4.1: ERST entries are not allowed to be modified by
    // software when HCHalted = 0
    fn update_segment_table(
        &mut self,
        erstba: GuestAddr,
        erstsz: usize,
        memctx: &mut MemCtx,
    ) -> Result<()> {
        let many = memctx.read_many(erstba, erstsz).ok_or(
            Error::EventRingSegmentTableLocationInvalid(erstba, erstsz),
        )?;
        self.segment_table = many
            .map(|mut erste: EventRingSegment| {
                // lower bits are reserved
                erste.base_address.0 &= !63;
                if erste.segment_trb_count < 16
                    || erste.segment_trb_count > 4096
                {
                    Err(Error::InvalidEventRingSegmentSize(erste))
                } else {
                    Ok(erste)
                }
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    fn enqueue_trb(
        &mut self,
        mut trb: TransferRequestBlock,
        memctx: &mut MemCtx,
    ) -> core::result::Result<(), TransferRequestBlock> {
        todo!("xHCI 1.2 figure 4-12");

        // if self.last_known_dequeue_index == self.queue_next_index() {
        //     return Err(trb);
        // }

        // trb.control.set_cycle(self.producer_cycle_state);
        // self.shadow_copy[self.enqueue_index] = trb;
        // self.queue_advance();
        // if self.enqueue_index == 0 {
        //     self.producer_cycle_state = !self.producer_cycle_state;
        // }
        Ok(())
    }

    fn enqueue(
        &mut self,
        value: EventDescriptor,
        memctx: &mut MemCtx,
    ) -> Result<()> {
        let mut trbs_iter = value.into_iter();
        let trb = trbs_iter.next().ok_or(Error::EventDescriptorSize)?;
        // xHCI 1.2 sect 4.11.3: Event Descriptors comprised of only one TRB
        if trbs_iter.next().is_some() {
            return Err(Error::EventDescriptorSize);
        }
        self.enqueue_trb(trb, memctx).map_err(Error::EventRingFull)
    }
}

/// xHCI 1.2 sect 6.5
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct EventRingSegment {
    /// Ring Segment Base Address. Lower 6 bits are reserved (addresses are 64-byte aligned).
    base_address: GuestAddr,
    /// Ring Segment Size. Valid values are between 16 and 4096.
    segment_trb_count: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransferRequestBlock {
    /// may be an address or immediate data
    parameter: u64,
    status: TrbStatusField,
    control: TrbControlField,
}

impl core::fmt::Debug for TransferRequestBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TransferRequestBlock {{ parameter: 0x{:x}, control.trb_type: {:?} }}",
            self.parameter,
            self.control.trb_type()
        )?;
        Ok(())
    }
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
#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

pub struct EventDescriptor(TransferRequestBlock);
impl WorkItem for EventDescriptor {
    fn try_from_trb_iter(
        trbs: impl IntoIterator<Item = TransferRequestBlock>,
    ) -> Result<Self> {
        let mut trbs = trbs.into_iter();
        if let Some(trb) = trbs.next() {
            if trbs.next().is_some() {
                Err(Error::EventDescriptorSize)
            } else {
                Ok(Self(trb))
            }
        } else {
            Err(Error::EventDescriptorSize)
        }
    }
}
impl IntoIterator for EventDescriptor {
    type Item = TransferRequestBlock;
    type IntoIter = std::iter::Once<TransferRequestBlock>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0)
    }
}

pub struct CommandDescriptor(TransferRequestBlock);
impl WorkItem for CommandDescriptor {
    fn try_from_trb_iter(
        trbs: impl IntoIterator<Item = TransferRequestBlock>,
    ) -> Result<Self> {
        let mut trbs = trbs.into_iter();
        if let Some(trb) = trbs.next() {
            if trbs.next().is_some() {
                Err(Error::CommandDescriptorSize)
            } else {
                Ok(Self(trb))
            }
        } else {
            Err(Error::CommandDescriptorSize)
        }
    }
}
impl IntoIterator for CommandDescriptor {
    type Item = TransferRequestBlock;
    type IntoIter = std::iter::Once<TransferRequestBlock>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0)
    }
}

pub struct TransferDescriptor {
    trbs: Vec<TransferRequestBlock>,
}
impl WorkItem for TransferDescriptor {
    fn try_from_trb_iter(
        trbs: impl IntoIterator<Item = TransferRequestBlock>,
    ) -> Result<Self> {
        Ok(Self { trbs: trbs.into_iter().collect() })
    }
}
impl IntoIterator for TransferDescriptor {
    type Item = TransferRequestBlock;
    type IntoIter = std::vec::IntoIter<TransferRequestBlock>;

    fn into_iter(self) -> Self::IntoIter {
        self.trbs.into_iter()
    }
}

impl TryFrom<Vec<TransferRequestBlock>> for TransferDescriptor {
    type Error = ();
    fn try_from(
        trbs: Vec<TransferRequestBlock>,
    ) -> core::result::Result<Self, Self::Error> {
        Ok(Self { trbs })
    }
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
    link: TrbControlFieldLink,
}

impl TrbControlField {
    fn trb_type(&self) -> TrbType {
        // all variants are alike in TRB type location
        unsafe { self.normal.trb_type() }
    }

    fn cycle(&self) -> bool {
        // all variants are alike in cycle bit location
        unsafe { self.normal.cycle() }
    }

    fn set_cycle(&mut self, cycle_state: bool) {
        // all variants are alike in cycle bit location
        unsafe { self.normal.set_cycle(cycle_state) }
    }

    fn chain_bit(&self) -> Option<bool> {
        Some(match self.trb_type() {
            TrbType::Normal => unsafe { self.normal.chain_bit() },
            TrbType::DataStage => unsafe { self.data_stage.chain_bit() },
            TrbType::StatusStage => unsafe { self.status_stage.chain_bit() },
            TrbType::Link => unsafe { self.link.chain_bit() },
            _ => return None,
        })
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
    /// Status Stage TRB control fields (xHCI 1.2 table 6-31)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbControlFieldLink(pub u32) {
        /// Used to mark the Enqueue Pointer of the Transfer or Command Ring.
        pub cycle: bool = 0;

        /// Or "TC". If set, the xHC shall toggle its interpretation of the
        /// cycle bit. If claered, the xHC shall continue to the next segment
        /// using its current cycle bit interpretation.
        pub toggle_cycle: bool = 1;

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

        reserved3: u16 = 16..32;
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
                        .with_chain_bit(true) // TODO: do DataStage and StatusStage get chained?
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
                        .with_chain_bit(true)
                        .with_interrupt_on_completion(true)
                        .with_trb_type(TrbType::StatusStage)
                        .with_direction(TrbDirection::Out),
                },
            },
        ];
        let ring = TransferRing::new_synthetic(ring_contents.clone());
        // TODO: read from ring
        let td = TransferDescriptor { trbs: ring_contents };
        assert_eq!(td.transfer_size(), 8);
    }
}

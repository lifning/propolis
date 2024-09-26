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
    IncompleteWorkItem(Vec<Trb>),
    #[error("Incomplete TD: TRBs with chain bit set formed a full ring circuit: {0:?}")]
    IncompleteWorkItemChainCyclic(Vec<Trb>),
    #[error("Event Ring full when trying to enqueue {0:?}")]
    EventRingFull(Trb),
    #[error("Event Ring Segment Table of size {1} cannot be read from address {0:?}")]
    EventRingSegmentTableLocationInvalid(GuestAddr, usize),
    #[error("Event Ring Segment Table Entry has invalid size: {0:?}")]
    InvalidEventRingSegmentSize(EventRingSegment),
}
pub type Result<T> = core::result::Result<T, Error>;
pub enum Never {}

pub trait WorkItem: Sized + IntoIterator<Item = Trb> {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self>;
}

pub type TransferRing = ConsumerRing<TransferDescriptor>;
pub type CommandRing = ConsumerRing<CommandDescriptor>;

pub struct ConsumerRing<T: WorkItem> {
    addr: GuestAddr,
    shadow_copy: Vec<Trb>,
    dequeue_index: usize,
    consumer_cycle_state: bool,
    _ghost: PhantomData<T>,
}

/// See xHCI 1.2 section 4.14 "Managing Transfer Rings"
impl<T: WorkItem> ConsumerRing<T> {
    fn new(addr: GuestAddr) -> Self {
        Self {
            addr,
            shadow_copy: vec![Trb::default()],
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
        let index =
            (tr_dequeue_pointer.0 - self.addr.0) as usize / size_of::<Trb>();
        self.dequeue_index = index;
    }

    // xHCI 1.2 sect 4.9: "TRB Rings may be larger than a Page,
    // however they shall not cross a 64K byte boundary."
    // xHCI 1.2 sect 4.11.5.1: "The Ring Segment Pointer field in a Link TRB
    // is not required to point to the beginning of a physical memory page."
    // (They *are* required to be at least 16-byte aligned, i.e. sizeof::<TRB>())
    fn update_from_guest(&mut self, memctx: &MemCtx) -> Result<()> {
        let mut new_shadow = Vec::<Trb>::with_capacity(self.shadow_copy.len());
        let mut addr = self.addr;

        // arbitrary upper limit: if a ring is larger than this, assume
        // something may be trying to attack us from a compromised guest
        let mut trb_count = 0;
        const UPPER_LIMIT: usize = 1024 * 1024 * 1024 / size_of::<Trb>();

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
                    addr = addr.offset::<Trb>(1);
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
    fn dequeue_trb(&mut self) -> Option<Trb> {
        let start_index = self.dequeue_index;
        loop {
            let trb = self.shadow_copy[self.dequeue_index];
            // cycle bit transition - found enqueue pointer
            if trb.control.cycle() != self.consumer_cycle_state {
                return None;
            }
            self.queue_advance();

            // xHCI 1.2 figure 4-7
            if trb.control.trb_type() == TrbType::Link {
                if unsafe { trb.control.link.toggle_cycle() } {
                    // xHCI 1.2 figure 4-8
                    self.consumer_cycle_state = !self.consumer_cycle_state;
                }
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
        let start_index = self.dequeue_index;
        let mut trbs = vec![self.dequeue_trb()?];
        while trbs.last().unwrap().control.chain_bit().unwrap_or(false) {
            // failsafe - if full circuit of chain bits
            if self.dequeue_index == start_index {
                return Some(Err(Error::IncompleteWorkItemChainCyclic(trbs)));
            }
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
    dequeue_pointer: Option<GuestAddr>,

    /// ESRTE's.
    segment_table: Vec<EventRingSegment>,
    /// "ESRT Count".
    segment_table_index: usize,

    /// "TRB Count".
    segment_remaining_trbs: usize,

    /// PCS.
    producer_cycle_state: bool,
}

impl EventRing {
    fn new(
        erstba: GuestAddr,
        erstsz: usize,
        erdp: GuestAddr,
        memctx: &MemCtx,
    ) -> Result<Self> {
        let mut x = Self {
            enqueue_pointer: GuestAddr(0),
            dequeue_pointer: Some(erdp),
            segment_table: Vec::new(),
            segment_table_index: 0,
            segment_remaining_trbs: 0,
            producer_cycle_state: true,
        };
        x.update_segment_table(erstba, erstsz, memctx)?;
        x.enqueue_pointer = x.segment_table[0].base_address;
        x.segment_remaining_trbs = x.segment_table[0].segment_trb_count;
        Ok(x)
    }

    /// Cache entire segment table. To be called when location (ERSTBA) or
    /// size (ERSTSZ) registers are written, or when host controller is resumed.
    /// (Per xHCI 1.2 sect 4.9.4.1: ERST entries themselves are not allowed
    /// to be modified by software when HCHalted = 0)
    fn update_segment_table(
        &mut self,
        erstba: GuestAddr,
        erstsz: usize,
        memctx: &MemCtx,
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

    /// Must be called when interrupter's ERDP register is written
    fn update_dequeue_pointer(&mut self, erdp: GuestAddr) {
        self.dequeue_pointer = Some(erdp);
    }

    /// Straight translation of xHCI 1.2 figure 4-12.
    fn is_full(&self) -> bool {
        let deq_ptr = self.dequeue_pointer.unwrap();
        if self.segment_remaining_trbs == 1 {
            // check next segment
            self.next_segment().base_address == deq_ptr
        } else {
            // check current segment
            self.enqueue_pointer.offset::<Trb>(1) == deq_ptr
        }
    }

    /// Straight translation of xHCI 1.2 figure 4-12.
    fn next_segment(&self) -> &EventRingSegment {
        &self.segment_table
            [(self.segment_table_index + 1) % self.segment_table.len()]
    }

    /// Straight translation of xHCI 1.2 figure 4-12.
    fn enqueue_trb_unchecked(&mut self, mut trb: Trb, memctx: &MemCtx) {
        trb.control.set_cycle(self.producer_cycle_state);

        memctx.write(self.enqueue_pointer, &trb);
        self.enqueue_pointer.0 += size_of::<Trb>() as u64;
        self.segment_remaining_trbs -= 1;

        if self.segment_remaining_trbs == 0 {
            self.segment_table_index += 1;
            if self.segment_table_index >= self.segment_table.len() {
                self.producer_cycle_state = !self.producer_cycle_state;
                self.segment_table_index = 0;
            }
            let erst_entry = &self.segment_table[self.segment_table_index];
            self.enqueue_pointer = erst_entry.base_address;
            self.segment_remaining_trbs = erst_entry.segment_trb_count;
        }
    }

    /// Straight translation of xHCI 1.2 figure 4-12.
    fn enqueue_trb(
        &mut self,
        mut trb: Trb,
        memctx: &MemCtx,
    ) -> core::result::Result<(), Trb> {
        if self.dequeue_pointer.is_none() {
            // waiting for ERDP write, don't write multiple EventRingFullErrors
            Err(trb)
        } else if self.is_full() {
            let event_ring_full_error = Trb {
                parameter: 0,
                status: TrbStatusField {
                    event: TrbStatusFieldEvent::default().with_completion_code(
                        TrbCompletionCode::EventRingFullError,
                    ),
                },
                control: TrbControlField {
                    normal: TrbControlFieldNormal::default()
                        .with_trb_type(TrbType::HostControllerEvent),
                },
            };
            self.enqueue_trb_unchecked(event_ring_full_error, memctx);
            // must wait until another ERDP write
            self.dequeue_pointer.take();
            Err(trb)
        } else {
            self.enqueue_trb_unchecked(trb, memctx);
            Ok(())
        }
    }

    fn enqueue(
        &mut self,
        value: EventDescriptor,
        memctx: &MemCtx,
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
pub struct Trb {
    /// may be an address or immediate data
    parameter: u64,
    status: TrbStatusField,
    control: TrbControlField,
}

impl core::fmt::Debug for Trb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Trb {{ parameter: 0x{:x}, control.trb_type: {:?} }}",
            self.parameter,
            self.control.trb_type()
        )?;
        Ok(())
    }
}

impl Default for Trb {
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

#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

#[derive(FromRepr, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum TrbCompletionCode {
    Invalid = 0,
    Success = 1,
    DataBufferError = 2,
    BabbleDetectedError = 3,
    UsbTransactionError = 4,
    TrbError = 5,
    StallError = 6,
    ResourceError = 7,
    BandwidthError = 8,
    NoSlotsAvailableError = 9,
    InvalidStreamTypeError = 10,
    SlotNotEnabledError = 11,
    EndpointNotEnabledError = 12,
    ShortPacket = 13,
    RingUnderrun = 14,
    RingOverrun = 15,
    VfEventRingFullError = 16,
    ParameterError = 17,
    BandwidthOverrunError = 18,
    ContextStateError = 19,
    NoPingResponseError = 20,
    EventRingFullError = 21,
    IncompatibleDeviceError = 22,
    MissedServiceError = 23,
    CommandRingStopped = 24,
    CommandAborted = 25,
    Stopped = 26,
    StoppedLengthInvalid = 27,
    StoppedShortPacket = 28,
    MaxExitLatencyTooLarge = 29,
    Reserved30 = 30,
    IsochBufferOverrun = 31,
    EventLostError = 32,
    UndefinedError = 33,
    InvalidStreamIdError = 34,
    SecondaryBandwidthError = 35,
    SplitTransactionError = 36,
    Reserved37 = 37,
    Reserved38 = 38,
    Reserved39 = 39,
    Reserved40 = 40,
    Reserved41 = 41,
    Reserved42 = 42,
    Reserved43 = 43,
    Reserved44 = 44,
    Reserved45 = 45,
    Reserved46 = 46,
    Reserved47 = 47,
    Reserved48 = 48,
    Reserved49 = 49,
    Reserved50 = 50,
    Reserved51 = 51,
    Reserved52 = 52,
    Reserved53 = 53,
    Reserved54 = 54,
    Reserved55 = 55,
    Reserved56 = 56,
    Reserved57 = 57,
    Reserved58 = 58,
    Reserved59 = 59,
    Reserved60 = 60,
    Reserved61 = 61,
    Reserved62 = 62,
    Reserved63 = 63,
    Reserved64 = 64,
    Reserved65 = 65,
    Reserved66 = 66,
    Reserved67 = 67,
    Reserved68 = 68,
    Reserved69 = 69,
    Reserved70 = 70,
    Reserved71 = 71,
    Reserved72 = 72,
    Reserved73 = 73,
    Reserved74 = 74,
    Reserved75 = 75,
    Reserved76 = 76,
    Reserved77 = 77,
    Reserved78 = 78,
    Reserved79 = 79,
    Reserved80 = 80,
    Reserved81 = 81,
    Reserved82 = 82,
    Reserved83 = 83,
    Reserved84 = 84,
    Reserved85 = 85,
    Reserved86 = 86,
    Reserved87 = 87,
    Reserved88 = 88,
    Reserved89 = 89,
    Reserved90 = 90,
    Reserved91 = 91,
    Reserved92 = 92,
    Reserved93 = 93,
    Reserved94 = 94,
    Reserved95 = 95,
    Reserved96 = 96,
    Reserved97 = 97,
    Reserved98 = 98,
    Reserved99 = 99,
    Reserved100 = 100,
    Reserved101 = 101,
    Reserved102 = 102,
    Reserved103 = 103,
    Reserved104 = 104,
    Reserved105 = 105,
    Reserved106 = 106,
    Reserved107 = 107,
    Reserved108 = 108,
    Reserved109 = 109,
    Reserved110 = 110,
    Reserved111 = 111,
    Reserved112 = 112,
    Reserved113 = 113,
    Reserved114 = 114,
    Reserved115 = 115,
    Reserved116 = 116,
    Reserved117 = 117,
    Reserved118 = 118,
    Reserved119 = 119,
    Reserved120 = 120,
    Reserved121 = 121,
    Reserved122 = 122,
    Reserved123 = 123,
    Reserved124 = 124,
    Reserved125 = 125,
    Reserved126 = 126,
    Reserved127 = 127,
    Reserved128 = 128,
    Reserved129 = 129,
    Reserved130 = 130,
    Reserved131 = 131,
    Reserved132 = 132,
    Reserved133 = 133,
    Reserved134 = 134,
    Reserved135 = 135,
    Reserved136 = 136,
    Reserved137 = 137,
    Reserved138 = 138,
    Reserved139 = 139,
    Reserved140 = 140,
    Reserved141 = 141,
    Reserved142 = 142,
    Reserved143 = 143,
    Reserved144 = 144,
    Reserved145 = 145,
    Reserved146 = 146,
    Reserved147 = 147,
    Reserved148 = 148,
    Reserved149 = 149,
    Reserved150 = 150,
    Reserved151 = 151,
    Reserved152 = 152,
    Reserved153 = 153,
    Reserved154 = 154,
    Reserved155 = 155,
    Reserved156 = 156,
    Reserved157 = 157,
    Reserved158 = 158,
    Reserved159 = 159,
    Reserved160 = 160,
    Reserved161 = 161,
    Reserved162 = 162,
    Reserved163 = 163,
    Reserved164 = 164,
    Reserved165 = 165,
    Reserved166 = 166,
    Reserved167 = 167,
    Reserved168 = 168,
    Reserved169 = 169,
    Reserved170 = 170,
    Reserved171 = 171,
    Reserved172 = 172,
    Reserved173 = 173,
    Reserved174 = 174,
    Reserved175 = 175,
    Reserved176 = 176,
    Reserved177 = 177,
    Reserved178 = 178,
    Reserved179 = 179,
    Reserved180 = 180,
    Reserved181 = 181,
    Reserved182 = 182,
    Reserved183 = 183,
    Reserved184 = 184,
    Reserved185 = 185,
    Reserved186 = 186,
    Reserved187 = 187,
    Reserved188 = 188,
    Reserved189 = 189,
    Reserved190 = 190,
    Reserved191 = 191,
    VendorDefinedError192 = 192,
    VendorDefinedError193 = 193,
    VendorDefinedError194 = 194,
    VendorDefinedError195 = 195,
    VendorDefinedError196 = 196,
    VendorDefinedError197 = 197,
    VendorDefinedError198 = 198,
    VendorDefinedError199 = 199,
    VendorDefinedError200 = 200,
    VendorDefinedError201 = 201,
    VendorDefinedError202 = 202,
    VendorDefinedError203 = 203,
    VendorDefinedError204 = 204,
    VendorDefinedError205 = 205,
    VendorDefinedError206 = 206,
    VendorDefinedError207 = 207,
    VendorDefinedError208 = 208,
    VendorDefinedError209 = 209,
    VendorDefinedError210 = 210,
    VendorDefinedError211 = 211,
    VendorDefinedError212 = 212,
    VendorDefinedError213 = 213,
    VendorDefinedError214 = 214,
    VendorDefinedError215 = 215,
    VendorDefinedError216 = 216,
    VendorDefinedError217 = 217,
    VendorDefinedError218 = 218,
    VendorDefinedError219 = 219,
    VendorDefinedError220 = 220,
    VendorDefinedError221 = 221,
    VendorDefinedError222 = 222,
    VendorDefinedError223 = 223,
    VendorDefinedInfo224 = 224,
    VendorDefinedInfo225 = 225,
    VendorDefinedInfo226 = 226,
    VendorDefinedInfo227 = 227,
    VendorDefinedInfo228 = 228,
    VendorDefinedInfo229 = 229,
    VendorDefinedInfo230 = 230,
    VendorDefinedInfo231 = 231,
    VendorDefinedInfo232 = 232,
    VendorDefinedInfo233 = 233,
    VendorDefinedInfo234 = 234,
    VendorDefinedInfo235 = 235,
    VendorDefinedInfo236 = 236,
    VendorDefinedInfo237 = 237,
    VendorDefinedInfo238 = 238,
    VendorDefinedInfo239 = 239,
    VendorDefinedInfo240 = 240,
    VendorDefinedInfo241 = 241,
    VendorDefinedInfo242 = 242,
    VendorDefinedInfo243 = 243,
    VendorDefinedInfo244 = 244,
    VendorDefinedInfo245 = 245,
    VendorDefinedInfo246 = 246,
    VendorDefinedInfo247 = 247,
    VendorDefinedInfo248 = 248,
    VendorDefinedInfo249 = 249,
    VendorDefinedInfo250 = 250,
    VendorDefinedInfo251 = 251,
    VendorDefinedInfo252 = 252,
    VendorDefinedInfo253 = 253,
    VendorDefinedInfo254 = 254,
    VendorDefinedInfo255 = 255,
}

impl From<u8> for TrbCompletionCode {
    fn from(value: u8) -> Self {
        // the field is 8-bits and the entire range is defined in the enum
        unsafe { core::mem::transmute(value) }
    }
}
impl Into<u8> for TrbCompletionCode {
    fn into(self) -> u8 {
        self as u8
    }
}

pub struct EventDescriptor(Trb);
impl WorkItem for EventDescriptor {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self> {
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
    type Item = Trb;
    type IntoIter = std::iter::Once<Trb>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0)
    }
}

pub struct CommandDescriptor(Trb);
impl WorkItem for CommandDescriptor {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self> {
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
    type Item = Trb;
    type IntoIter = std::iter::Once<Trb>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0)
    }
}

#[derive(Debug)]
pub struct TransferDescriptor {
    trbs: Vec<Trb>,
}
impl WorkItem for TransferDescriptor {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self> {
        Ok(Self { trbs: trbs.into_iter().collect() })
    }
}
impl IntoIterator for TransferDescriptor {
    type Item = Trb;
    type IntoIter = std::vec::IntoIter<Trb>;

    fn into_iter(self) -> Self::IntoIter {
        self.trbs.into_iter()
    }
}

impl TryFrom<Vec<Trb>> for TransferDescriptor {
    type Error = Never;
    fn try_from(trbs: Vec<Trb>) -> core::result::Result<Self, Self::Error> {
        Ok(Self { trbs })
    }
}

impl TransferDescriptor {
    /// xHCI 1.2 sect 4.14: The TD Transfer Size is defined by the sum of the
    /// TRB Transfer Length fields in all TRBs that comprise the TD.
    pub fn transfer_size(&self) -> usize {
        self.trbs
            .iter()
            .map(|trb| unsafe { trb.status.transfer.trb_transfer_length() }
                as usize)
            .sum()
    }

    pub fn trb0_type(&self) -> Option<TrbType> {
        self.trbs.first().map(|trb| trb.control.trb_type())
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
                let x = unsafe { trb.status.transfer.trb_transfer_length() };
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

#[derive(Copy, Clone)]
pub union TrbStatusField {
    transfer: TrbStatusFieldTransfer,
    event: TrbStatusFieldEvent,
}
impl Default for TrbStatusField {
    fn default() -> Self {
        Self { transfer: TrbStatusFieldTransfer(0) }
    }
}

bitstruct! {
    /// Representation of the 'status' field of Transfer Request Block (TRB).
    ///
    /// See xHCI 1.2 Section 6.4.1 (Comments are paraphrases thereof)
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbStatusFieldTransfer(pub u32) {
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

bitstruct! {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TrbStatusFieldEvent(pub u32) {
        reserved: u32 = 0..24;
        pub completion_code: TrbCompletionCode = 24..32;
    }
}

#[cfg(test)]
mod tests {
    use crate::vmm::PhysMap;

    use super::*;

    #[test]
    fn test_get_device_descriptor_transfer_ring() {
        let mut phys_map = PhysMap::new_test(16 * 1024);
        phys_map.add_test_mem("guest-ram".to_string(), 0, 16 * 1024);
        let memctx = phys_map.memctx();

        // mimicking pg. 85 of xHCI 1.2
        let ring_segments: &[&[_]] = &[
            &[
                Trb {
                    parameter: 0, // todo!("bmRequestType 0x80, bRequest 6, wValue 0x100, wIndex 0, wLength 8"),
                    status: TrbStatusField {
                        transfer: TrbStatusFieldTransfer::default()
                            .with_td_size(8)
                            .with_interrupter_target(0),
                    },
                    control: TrbControlField {
                        setup_stage: TrbControlFieldSetupStage::default()
                            .with_cycle(true)
                            .with_immediate_data(true)
                            .with_trb_type(TrbType::SetupStage)
                            .with_transfer_type(TrbTransferType::InDataStage),
                    },
                },
                Trb {
                    parameter: 0x123456789abcdef0u64,
                    status: TrbStatusField {
                        transfer: TrbStatusFieldTransfer::default()
                            .with_trb_transfer_length(8),
                    },
                    control: TrbControlField {
                        data_stage: TrbControlFieldDataStage::default()
                            .with_cycle(true)
                            .with_trb_type(TrbType::DataStage)
                            .with_direction(TrbDirection::In),
                    },
                },
                Trb {
                    parameter: 2048,
                    status: TrbStatusField::default(),
                    control: TrbControlField {
                        link: TrbControlFieldLink::default()
                            .with_cycle(true)
                            .with_trb_type(TrbType::Link),
                    },
                },
            ],
            &[
                Trb {
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
                Trb {
                    parameter: 1024,
                    status: TrbStatusField::default(),
                    control: TrbControlField {
                        link: TrbControlFieldLink::default()
                            .with_toggle_cycle(true)
                            .with_trb_type(TrbType::Link),
                    },
                },
            ],
        ];

        for (i, seg) in ring_segments.iter().enumerate() {
            memctx.write_many(GuestAddr((i as u64 + 1) * 1024), seg);
        }

        let mut ring = TransferRing::new(GuestAddr(1024));
        ring.update_from_guest(&memctx).unwrap();

        let setup_td = ring.dequeue_work_item().unwrap().unwrap();
        let data_td = ring.dequeue_work_item().unwrap().unwrap();
        let status_td = ring.dequeue_work_item().unwrap().unwrap();
        assert!(ring.dequeue_work_item().is_none());

        assert_eq!(setup_td.trbs.len(), 1);
        assert_eq!(data_td.trbs.len(), 1);
        assert_eq!(status_td.trbs.len(), 1);

        assert_eq!(setup_td.trb0_type().unwrap(), TrbType::SetupStage);
        assert_eq!(data_td.trb0_type().unwrap(), TrbType::DataStage);
        assert_eq!(status_td.trb0_type().unwrap(), TrbType::StatusStage);

        assert_eq!(data_td.transfer_size(), 8);
    }

    // TODO: test chained TD
    // TODO: test incomplete work items

    #[test]
    fn test_event_ring_enqueue() {
        let mut phys_map = PhysMap::new_test(16 * 1024);
        phys_map.add_test_mem("guest-ram".to_string(), 0, 16 * 1024);
        let memctx = phys_map.memctx();

        let erstba = GuestAddr(0);
        let erstsz = 2;
        let erst_entries = [
            EventRingSegment {
                base_address: GuestAddr(1024),
                segment_trb_count: 16,
            },
            EventRingSegment {
                base_address: GuestAddr(2048),
                segment_trb_count: 16,
            },
        ];

        memctx.write_many(erstba, &erst_entries);

        let erdp = erst_entries[0].base_address;

        let mut ring = EventRing::new(erstba, erstsz, erdp, &memctx).unwrap();

        let mut ed_trb = Trb {
            parameter: 0,
            status: TrbStatusField {
                event: TrbStatusFieldEvent::default()
                    .with_completion_code(TrbCompletionCode::Success),
            },
            control: TrbControlField {
                normal: TrbControlFieldNormal::default()
                    .with_trb_type(TrbType::EventData),
            },
        };
        // enqueue 31 out of 32 (EventRing must leave room for one final
        // event in case of a full ring: the EventRingFullError event!)
        for i in 1..32 {
            ring.enqueue(EventDescriptor(ed_trb), &memctx).unwrap();
            ed_trb.parameter = i;
        }
        ring.enqueue(EventDescriptor(ed_trb), &memctx).unwrap_err();

        // further additions should do nothing until we write a new ERDP
        ring.enqueue(EventDescriptor(ed_trb), &memctx).unwrap_err();

        let mut ring_contents = Vec::new();
        for erste in &erst_entries {
            ring_contents.extend(
                memctx
                    .read_many::<Trb>(
                        erste.base_address,
                        erste.segment_trb_count,
                    )
                    .unwrap(),
            );
        }

        assert_eq!(ring_contents.len(), 32);
        // cycle bits should be set in all these
        for i in 0..31 {
            assert_eq!(ring_contents[i].parameter, i as u64);
            assert_eq!(ring_contents[i].control.trb_type(), TrbType::EventData);
            assert_eq!(ring_contents[i].control.cycle(), true);
            assert_eq!(
                unsafe { ring_contents[i].status.event.completion_code() },
                TrbCompletionCode::Success
            );
        }
        {
            let hce = ring_contents[31];
            assert_eq!(hce.control.cycle(), true);
            assert_eq!(hce.control.trb_type(), TrbType::HostControllerEvent);
            assert_eq!(
                unsafe { hce.status.event.completion_code() },
                TrbCompletionCode::EventRingFullError
            );
        }

        // let's say we (the "software") processed the first 8 events.
        ring.update_dequeue_pointer(
            erst_entries[0].base_address.offset::<Trb>(8),
        );

        // try to enqueue another 8 events!
        for i in 32..39 {
            ed_trb.parameter = i;
            ring.enqueue(EventDescriptor(ed_trb), &memctx).unwrap();
        }
        ring.enqueue(EventDescriptor(ed_trb), &memctx).unwrap_err();

        // check that they've overwritten previous entries appropriately
        ring_contents.clear();
        for erste in &erst_entries {
            ring_contents.extend(
                memctx
                    .read_many::<Trb>(
                        erste.base_address,
                        erste.segment_trb_count,
                    )
                    .unwrap(),
            );
        }

        // cycle bits should be cleared on the new entries
        for i in 0..7 {
            assert_eq!(ring_contents[i].parameter, 32 + i as u64);
            assert_eq!(ring_contents[i].control.trb_type(), TrbType::EventData);
            assert_eq!(ring_contents[i].control.cycle(), false);
            assert_eq!(
                unsafe { ring_contents[i].status.event.completion_code() },
                TrbCompletionCode::Success
            );
        }
        {
            let hce = ring_contents[7];
            assert_eq!(hce.control.cycle(), false);
            assert_eq!(hce.control.trb_type(), TrbType::HostControllerEvent);
            assert_eq!(
                unsafe { hce.status.event.completion_code() },
                TrbCompletionCode::EventRingFullError
            );

            // haven't overwritten this one (only wrote one EventRingFullError)
            let prev = ring_contents[8];
            assert_eq!(prev.parameter, 8);
            assert_eq!(prev.control.cycle(), true);
            assert_eq!(prev.control.trb_type(), TrbType::EventData);
        }

        // TODO: test event ring segment table resizes
    }
}

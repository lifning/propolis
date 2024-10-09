use std::marker::PhantomData;

use crate::common::GuestAddr;
use crate::hw::usb::xhci::bits::ring_data::*;
use crate::vmm::MemCtx;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error in xHC Ring: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("Last TRB in ring at 0x{0:?} was not Link: {1:?}")]
    MissingLink(GuestAddr, TrbType),
    #[error("Tried to construct Command Descriptor from multiple TRBs")]
    CommandDescriptorSize,
    #[error("Guest defined a segmented ring larger than allowed maximum size")]
    SegmentedRingTooLarge,
    #[error("Failed reading TRB from guest memory")]
    FailedReadingTRB,
    #[error("Incomplete TD: no more TRBs in cycle to complete chain: {0:?}")]
    IncompleteWorkItem(Vec<Trb>),
    #[error("Incomplete TD: TRBs with chain bit set formed a full ring circuit: {0:?}")]
    IncompleteWorkItemChainCyclic(Vec<Trb>),
    #[error("TRB Ring Dequeue Pointer was not aligned to size_of<Trb>: {0:?}")]
    InvalidDequeuePointer(GuestAddr),
    #[error("TRB Ring Dequeue Pointer was not contained in any linked TRB segment: {0:?}")]
    NoSegmentContainsDequeuePointer(GuestAddr),
    #[error("Invalid TRB type for a Command Descriptor: {0:?}")]
    InvalidCommandDescriptor(Trb),
}
pub type Result<T> = core::result::Result<T, Error>;
pub enum Never {}

#[derive(Copy, Clone)]
struct SegmentInfo {
    addr: GuestAddr,
    trb_count: usize,
}
// TODO: just put this on GuestRegion tbh
impl SegmentInfo {
    fn contains(&self, ptr: GuestAddr) -> bool {
        ptr.0 >= self.addr.0
            && (ptr.0 as usize)
                < self.addr.0 as usize + self.trb_count * size_of::<Trb>()
    }
}

pub struct ConsumerRing<T: WorkItem> {
    // where the ring *starts*, but note that it may be disjoint via Link TRBs
    start_addr: GuestAddr,
    // it would be great to link_indeces.upper_bound(Bound::Included(x))
    // but alas, unstable API
    // link_indeces: BTreeMap<(usize, SegmentInfo)>,
    link_indices: Vec<(usize, SegmentInfo)>,
    shadow_copy: Vec<Trb>,
    dequeue_index: usize,
    consumer_cycle_state: bool,
    _ghost: PhantomData<T>,
}
pub type TransferRing = ConsumerRing<TransferDescriptor>;
pub type CommandRing = ConsumerRing<CommandDescriptor>;

pub trait WorkItem: Sized + IntoIterator<Item = Trb> {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self>;
}

/// See xHCI 1.2 section 4.14 "Managing Transfer Rings"
impl<T: WorkItem> ConsumerRing<T> {
    pub fn new(addr: GuestAddr, cycle_state: bool) -> Self {
        Self {
            start_addr: addr,
            link_indices: [(0, SegmentInfo { addr, trb_count: 0 })]
                .into_iter()
                .collect(),
            shadow_copy: vec![Trb::default()],
            dequeue_index: 0,
            consumer_cycle_state: cycle_state,
            _ghost: PhantomData,
        }
    }

    fn queue_advance(&mut self) {
        self.dequeue_index = self.queue_next_index()
    }
    fn queue_next_index(&mut self) -> usize {
        (self.dequeue_index + 1) % self.shadow_copy.len()
    }

    /// xHCI 1.2 sects 4.6.10, 6.4.3.9
    pub fn set_dequeue_pointer_and_cycle(
        &mut self,
        deq_ptr: GuestAddr,
        cycle_state: bool,
    ) -> Result<()> {
        if deq_ptr.0 as usize % size_of::<Trb>() != 0 {
            return Err(Error::InvalidDequeuePointer(deq_ptr));
        }
        for (index, region) in self.link_indices.iter() {
            if region.contains(deq_ptr) {
                self.dequeue_index = index
                    + (deq_ptr.0 - region.addr.0) as usize / size_of::<Trb>();
                self.consumer_cycle_state = cycle_state;
                return Ok(());
            }
        }
        Err(Error::NoSegmentContainsDequeuePointer(deq_ptr))
    }

    /// Return the guest address corresponding to the current dequeue pointer.
    pub fn current_dequeue_pointer(&self) -> GuestAddr {
        let mut iter = self.link_indices.iter().copied();
        // always at least has (0, self.addr)
        let (mut index, mut region) = iter.next().unwrap();
        while let Some((next_index, next_region)) = iter.next() {
            if next_index > self.dequeue_index {
                break;
            }
            index = next_index;
            region = next_region;
        }
        region.addr.offset::<Trb>(self.dequeue_index - index)
    }

    /// xHCI 1.2 sect 4.9.2: When a Transfer Ring is enabled or reset,
    /// the xHC initializes its copies of the Enqueue and Dequeue Pointers
    /// with the value of the Endpoint/Stream Context TR Dequeue Pointer field.
    pub fn reset(&mut self, tr_dequeue_pointer: GuestAddr) {
        let index = (tr_dequeue_pointer.0 - self.start_addr.0) as usize
            / size_of::<Trb>();
        self.dequeue_index = index;
    }

    // xHCI 1.2 sect 4.9: "TRB Rings may be larger than a Page,
    // however they shall not cross a 64K byte boundary."
    // xHCI 1.2 sect 4.11.5.1: "The Ring Segment Pointer field in a Link TRB
    // is not required to point to the beginning of a physical memory page."
    // (They *are* required to be at least 16-byte aligned, i.e. sizeof::<TRB>())
    pub fn update_from_guest(&mut self, memctx: &MemCtx) -> Result<()> {
        let mut new_shadow = Vec::<Trb>::with_capacity(self.shadow_copy.len());
        let mut new_link_indeces = Vec::with_capacity(self.link_indices.len());
        let mut addr = self.start_addr;

        // arbitrary upper limit: if a ring is larger than this, assume
        // something may be trying to attack us from a compromised guest
        let mut trb_count = 0;
        const UPPER_LIMIT: usize = 1024 * 1024 * 1024 / size_of::<Trb>();

        new_link_indeces.push((trb_count, SegmentInfo { addr, trb_count: 0 }));

        loop {
            if let Some(val) = memctx.read(addr) {
                new_shadow.push(val);
                trb_count += 1;
                new_link_indeces.last_mut().unwrap().1.trb_count += 1;
                if trb_count >= UPPER_LIMIT {
                    return Err(Error::SegmentedRingTooLarge);
                }
                if val.control.trb_type() == TrbType::Link {
                    // xHCI 1.2 figure 6-38
                    addr = GuestAddr(val.parameter & !15);
                    if addr == self.start_addr {
                        break;
                    } else {
                        new_link_indeces.push((
                            trb_count,
                            SegmentInfo { addr, trb_count: 0 },
                        ));
                    }
                } else {
                    addr = addr.offset::<Trb>(1);
                    if addr == self.start_addr {
                        break;
                    }
                }
            } else {
                return Err(Error::FailedReadingTRB);
            }
        }

        // actually we might've been given an initial pointer that's in the middle of a segment...
        // // xHCI 1.2 sect 4.9.2.1: The last TRB in a Ring Segment is always a Link TRB.
        // let last_trb_type = new_shadow.last().unwrap().control.trb_type();
        // if last_trb_type != TrbType::Link {
        //     Err(Error::MissingLink(self.addr, last_trb_type))
        // } else {
        self.shadow_copy = new_shadow;
        self.link_indices = new_link_indeces;
        Ok(())
        //}
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

    pub fn dequeue_work_item(&mut self) -> Option<Result<T>> {
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

#[derive(Debug)]
pub struct CommandDescriptor(pub Trb);
impl WorkItem for CommandDescriptor {
    fn try_from_trb_iter(trbs: impl IntoIterator<Item = Trb>) -> Result<Self> {
        let mut trbs = trbs.into_iter();
        if let Some(trb) = trbs.next() {
            if trbs.next().is_some() {
                Err(Error::CommandDescriptorSize)
            } else {
                // xHCI 1.2 sect 6.4.3
                match trb.control.trb_type() {
                    TrbType::NoOpCmd
                    | TrbType::EnableSlotCmd
                    | TrbType::DisableSlotCmd
                    | TrbType::AddressDeviceCmd
                    | TrbType::ConfigureEndpointCmd
                    | TrbType::EvaluateContextCmd
                    | TrbType::ResetEndpointCmd
                    | TrbType::StopEndpointCmd
                    | TrbType::SetTRDequeuePointerCmd
                    | TrbType::ResetDeviceCmd
                    | TrbType::ForceEventCmd
                    | TrbType::NegotiateBandwidthCmd
                    | TrbType::SetLatencyToleranceValueCmd
                    | TrbType::GetPortBandwidthCmd
                    | TrbType::ForceHeaderCmd
                    | TrbType::GetExtendedPropertyCmd
                    | TrbType::SetExtendedPropertyCmd => Ok(Self(trb)),
                    _ => Err(Error::InvalidCommandDescriptor(trb)),
                }
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

impl TryInto<CommandInfo> for CommandDescriptor {
    type Error = Error;

    // xHCI 1.2 section 6.4.3
    fn try_into(self) -> Result<CommandInfo> {
        Ok(match self.0.control.trb_type() {
            TrbType::NoOpCmd => CommandInfo::NoOp,
            TrbType::EnableSlotCmd => CommandInfo::EnableSlot {
                slot_type: unsafe { self.0.control.slot_cmd.slot_type() },
            },
            TrbType::DisableSlotCmd => CommandInfo::DisableSlot {
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
            },
            TrbType::AddressDeviceCmd => CommandInfo::AddressDevice {
                input_context_ptr: GuestAddr(self.0.parameter & !0b1111),
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
                block_set_address_request: unsafe {
                    self.0.control.slot_cmd.bit9()
                },
            },
            TrbType::ConfigureEndpointCmd => CommandInfo::ConfigureEndpoint {
                input_context_ptr: GuestAddr(self.0.parameter & !0b1111),
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
                deconfigure: unsafe { self.0.control.slot_cmd.bit9() },
            },
            TrbType::EvaluateContextCmd => CommandInfo::EvaluateContext {
                input_context_ptr: GuestAddr(self.0.parameter & !0b1111),
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
            },
            TrbType::ResetEndpointCmd => CommandInfo::ResetEndpoint {
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
                endpoint_id: unsafe {
                    self.0.control.endpoint_cmd.endpoint_id()
                },
                transfer_state_preserve: unsafe {
                    self.0.control.endpoint_cmd.transfer_state_preserve()
                },
            },
            TrbType::StopEndpointCmd => CommandInfo::StopEndpoint {
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
                endpoint_id: unsafe {
                    self.0.control.endpoint_cmd.endpoint_id()
                },
                suspend: unsafe { self.0.control.endpoint_cmd.suspend() },
            },
            TrbType::SetTRDequeuePointerCmd => unsafe {
                CommandInfo::SetTRDequeuePointer {
                    new_tr_dequeue_ptr: GuestAddr(self.0.parameter & !0b1111),
                    dequeue_cycle_state: (self.0.parameter & 1) != 0,
                    // (streams not implemented)
                    // stream_context_type: ((self.0.parameter >> 1) & 0b111) as u8,
                    // stream_id: self.0.status.command.stream_id(),
                    slot_id: self.0.control.endpoint_cmd.slot_id(),
                    endpoint_id: self.0.control.endpoint_cmd.endpoint_id(),
                }
            },
            TrbType::ResetDeviceCmd => CommandInfo::ResetDevice {
                slot_id: unsafe { self.0.control.slot_cmd.slot_id() },
            },
            // optional normative, ignored by us
            TrbType::ForceEventCmd => CommandInfo::ForceEvent,
            // optional normative, ignored by us
            TrbType::NegotiateBandwidthCmd => CommandInfo::NegotiateBandwidth,
            // optional normative, ignored by us
            TrbType::SetLatencyToleranceValueCmd => {
                CommandInfo::SetLatencyToleranceValue
            }
            // optional
            TrbType::GetPortBandwidthCmd => CommandInfo::GetPortBandwidth {
                port_bandwidth_ctx_ptr: GuestAddr(self.0.parameter & !0b1111),
                hub_slot_id: unsafe {
                    self.0.control.get_port_bw_cmd.hub_slot_id()
                },
                dev_speed: unsafe {
                    self.0.control.get_port_bw_cmd.dev_speed()
                },
            },
            TrbType::ForceHeaderCmd => CommandInfo::ForceHeader {
                packet_type: (self.0.parameter & 0b1_1111) as u8,
                header_info: (self.0.parameter >> 5) as u128
                    | ((unsafe { self.0.status.command_ext.0 } as u128) << 59),
                root_hub_port_number: unsafe {
                    // hack, same bits
                    self.0.control.get_port_bw_cmd.hub_slot_id()
                },
            },
            // optional
            TrbType::GetExtendedPropertyCmd => unsafe {
                CommandInfo::GetExtendedProperty {
                    extended_property_ctx_ptr: GuestAddr(
                        self.0.parameter & !0b1111,
                    ),
                    extended_capability_id: self
                        .0
                        .status
                        .command_ext
                        .extended_capability_id(),
                    command_subtype: self.0.control.ext_props_cmd.subtype(),
                    endpoint_id: self.0.control.ext_props_cmd.endpoint_id(),
                    slot_id: self.0.control.ext_props_cmd.slot_id(),
                }
            },
            // optional
            TrbType::SetExtendedPropertyCmd => unsafe {
                CommandInfo::SetExtendedProperty {
                    extended_capability_id: self
                        .0
                        .status
                        .command_ext
                        .extended_capability_id(),
                    capability_parameter: self
                        .0
                        .status
                        .command_ext
                        .capability_parameter(),
                    command_subtype: self.0.control.ext_props_cmd.subtype(),
                    endpoint_id: self.0.control.ext_props_cmd.endpoint_id(),
                    slot_id: self.0.control.ext_props_cmd.slot_id(),
                }
            },
            _ => unreachable!(),
        })
    }
}

#[derive(Debug)]
pub enum CommandInfo {
    NoOp,
    EnableSlot {
        slot_type: u8,
    },
    DisableSlot {
        slot_id: u8,
    },
    AddressDevice {
        input_context_ptr: GuestAddr,
        slot_id: u8,
        block_set_address_request: bool,
    },
    ConfigureEndpoint {
        input_context_ptr: GuestAddr,
        slot_id: u8,
        deconfigure: bool,
    },
    EvaluateContext {
        input_context_ptr: GuestAddr,
        slot_id: u8,
    },
    ResetEndpoint {
        slot_id: u8,
        endpoint_id: u8,
        transfer_state_preserve: bool,
    },
    StopEndpoint {
        slot_id: u8,
        endpoint_id: u8,
        suspend: bool,
    },
    SetTRDequeuePointer {
        new_tr_dequeue_ptr: GuestAddr,
        dequeue_cycle_state: bool,
        slot_id: u8,
        endpoint_id: u8,
    },
    ResetDevice {
        slot_id: u8,
    },
    ForceEvent,
    NegotiateBandwidth,
    SetLatencyToleranceValue,
    GetPortBandwidth {
        port_bandwidth_ctx_ptr: GuestAddr,
        hub_slot_id: u8,
        dev_speed: u8,
    },
    ForceHeader {
        packet_type: u8,
        header_info: u128,
        root_hub_port_number: u8,
    },
    GetExtendedProperty {
        extended_property_ctx_ptr: GuestAddr,
        extended_capability_id: u16,
        command_subtype: u8,
        endpoint_id: u8,
        slot_id: u8,
    },
    SetExtendedProperty {
        extended_capability_id: u16,
        capability_parameter: u8,
        command_subtype: u8,
        endpoint_id: u8,
        slot_id: u8,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmm::PhysMap;

    #[test]
    fn test_get_device_descriptor_transfer_ring() {
        let mut phys_map = PhysMap::new_test(16 * 1024);
        phys_map.add_test_mem("guest-ram".to_string(), 0, 16 * 1024).unwrap();
        let memctx = phys_map.memctx();

        // mimicking pg. 85 of xHCI 1.2, but with Links thrown in
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
                // link to next ring segment
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
                // link back to first ring segment (with toggle cycle)
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

        let mut ring = TransferRing::new(GuestAddr(1024), true);
        ring.update_from_guest(&memctx).unwrap();

        let setup_td = ring.dequeue_work_item().unwrap().unwrap();

        assert_eq!(
            ring.current_dequeue_pointer(),
            GuestAddr(1024).offset::<Trb>(1)
        );

        let data_td = ring.dequeue_work_item().unwrap().unwrap();

        assert_eq!(
            ring.current_dequeue_pointer(),
            GuestAddr(1024).offset::<Trb>(2)
        );

        // test setting the dequeue pointer
        ring.set_dequeue_pointer_and_cycle(
            GuestAddr(1024).offset::<Trb>(1),
            true,
        )
        .unwrap();

        assert_eq!(
            ring.current_dequeue_pointer(),
            GuestAddr(1024).offset::<Trb>(1)
        );

        let data_td_copy = ring.dequeue_work_item().unwrap().unwrap();

        assert_eq!(data_td.trb0_type(), data_td_copy.trb0_type());

        assert_eq!(
            ring.current_dequeue_pointer(),
            GuestAddr(1024).offset::<Trb>(2)
        );

        let status_td = ring.dequeue_work_item().unwrap().unwrap();

        // skips link trbs
        assert_eq!(
            ring.current_dequeue_pointer(),
            GuestAddr(2048).offset::<Trb>(1)
        );

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
}

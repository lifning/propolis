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
}
pub type Result<T> = core::result::Result<T, Error>;
pub enum Never {}

pub struct ConsumerRing<T: WorkItem> {
    addr: GuestAddr,
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
}

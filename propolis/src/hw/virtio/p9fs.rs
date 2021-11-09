use std::num::NonZeroU16;
use std::sync::Arc;

use crate::common::*;
use crate::dispatch::DispCtx;
use crate::hw::pci;
use crate::util::regmap::RegMap;

use super::bits::*;
use super::pci::{PciVirtio, PciVirtioState};
use super::queue::{VirtQueue, VirtQueues};
use super::VirtioDevice;

use lazy_static::lazy_static;

pub struct PciVirtio9pfs {
    virtio_state: PciVirtioState,
    pci_state: pci::DeviceState,

    source: String,
    target: String,
}

impl PciVirtio9pfs {
    pub fn new(source: String, target: String, queue_size: u16) -> Arc<Self> {
        let queues = VirtQueues::new(
            NonZeroU16::new(queue_size).unwrap(),
            NonZeroU16::new(1).unwrap(),
        );
        let msix_count = Some(1); //guess
        let (virtio_state, pci_state) = PciVirtioState::create(
            queues,
            msix_count,
            VIRTIO_DEV_9P,
            pci::bits::CLASS_STORAGE,
            VIRTIO_9P_CFG_SIZE,
        );
        Arc::new(Self{virtio_state, pci_state, source, target})
    }
}

impl VirtioDevice for PciVirtio9pfs {
    fn cfg_rw(&self, mut rwo: RWOp) {
        P9FS_DEV_REGS.process(&mut rwo, |id, rwo| match rwo {
            RWOp::Read(ro) => {
                match id {
                    BlockReg::TagLen => {
                        ro.write_u16(self.target.len() as u16);
                    }
                    BlockReg::Tag => {
                        ro.write_bytes(self.target.as_bytes());
                    }
                }
            }
            RWOp::Write(_) => { }
        })
    }

    fn get_features(&self) -> u32 { 0 }

    fn set_features(&self, _feat: u32) { }

    fn queue_notify(&self, _vq: &Arc<VirtQueue>, _ctx: &DispCtx) {
        unimplemented!();
    }
}

impl Entity for PciVirtio9pfs {
    fn reset(&self, ctx: &DispCtx) {
        self.virtio_state.reset(self, ctx);
    }
}

impl PciVirtio for PciVirtio9pfs{
    fn virtio_state(&self) -> &PciVirtioState {
        &self.virtio_state
    }
    fn pci_state(&self) -> &pci::DeviceState {
        &self.pci_state
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum BlockReg {
    TagLen,
    Tag,
}

lazy_static! {
    static ref P9FS_DEV_REGS: RegMap<BlockReg> = {
        let layout = [
            (BlockReg::TagLen, 16),
            (BlockReg::Tag, 256),
        ];
        RegMap::create_packed(
            VIRTIO_9P_CFG_SIZE,
            &layout,
            None, //TODO
        )
    };
}

mod bits {
    use std::mem::size_of;

    pub const VIRTIO_9P_MAX_TAG_SIZE: usize = 256;
    pub const VIRTIO_9P_CFG_SIZE: usize = VIRTIO_9P_MAX_TAG_SIZE + size_of::<u16>();
}
use bits::*;

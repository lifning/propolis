use std::num::NonZeroU16;
use std::sync::Arc;

use crate::common::*;
use crate::dispatch::DispCtx;
use crate::hw::pci;
use crate::util::regmap::RegMap;

use super::bits::*;
use super::pci::{PciVirtio, PciVirtioState};
use super::queue::{Chain, VirtQueue, VirtQueues};
use super::VirtioDevice;

use lazy_static::lazy_static;
use rs9p::{Msg, serialize::read_msg};

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

    pub fn handle_req(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) -> Option<Msg> {
        let mem = &ctx.mctx.memctx();

        let mut chain = Chain::with_capacity(1);
        let clen = vq.pop_avail(&mut chain, mem)? as usize;

        //TODO better as uninitialized?
        let mut data: Vec<u8> = vec![0;clen];
        let buf = data.as_mut_slice();
        
        // TODO copy pasta from tail end of Chain::read function. Seemingly
        // cannot use Chain::read as-is because it expects a statically sized
        // type.
        let mut done = 0;
        let total = chain.for_remaining_type(true, |addr, len| {
            let remain = &mut buf[done..];
            if let Some(copied) = mem.read_into(addr, remain, len) {
                let need_more = copied != remain.len();
                done += copied;
                (copied, need_more)
            } else {
                (0, false)
            }
        });

        if total != clen {
            //TODO error msg
            return None
        }

        let mut rdr = std::io::Cursor::new(data);
        match read_msg(&mut rdr) {
            Ok(msg) => {
                println!("ok: ← {:#?}", msg);
                Some(msg)
            }
            Err(_) => {
                //TODO error msg
                println!("err: ← {:?}", rdr.get_ref());
                None
            }
        }
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

    fn queue_notify(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {
        self.handle_req(vq, ctx);
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

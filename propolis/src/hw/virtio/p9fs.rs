use std::num::NonZeroU16;
use std::sync::{Arc, Mutex};
use std::convert::TryInto;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::mem::size_of;
use std::io::{Read, Seek};

use crate::common::*;
use crate::dispatch::DispCtx;
use crate::hw::pci;
use crate::util::regmap::RegMap;
use crate::vmm::MemCtx;

use super::bits::*;
use super::pci::{PciVirtio, PciVirtioState};
use super::queue::{Chain, VirtQueue, VirtQueues};
use super::VirtioDevice;

use lazy_static::lazy_static;
use num_enum::TryFromPrimitive;
use p9ds::proto::{
    self,
    MessageType,
    Tattach, Rattach,
    Twalk, Rwalk,
    Tlopen, Rlopen,
    Treaddir, Rreaddir,
    Tread, Rread,
    Rlerror,
    Version,
    Qid, QidType,
    Dirent,
};
use libc::{
    ENOENT,
    ENOLCK,
    ENOTSUP,
    ERANGE,
    EILSEQ,

    DT_DIR,
    DT_REG,
};
use ipf::WireSize;

pub struct PciVirtio9pfs {
    virtio_state: PciVirtioState,
    pci_state: pci::DeviceState,

    source: String,
    target: String,

    fileserver: Mutex::<Box::<Fileserver>>, 
    msize: u32,
}

impl PciVirtio9pfs {

    pub fn new(source: String, target: String, queue_size: u16) -> Arc<Self> {
        let queues = VirtQueues::new(
            NonZeroU16::new(queue_size).unwrap(),
            NonZeroU16::new(1).unwrap(),
        );
        let msix_count = Some(2); //guess
        let (virtio_state, pci_state) = PciVirtioState::create(
            queues,
            msix_count,
            VIRTIO_DEV_9P,
            pci::bits::CLASS_STORAGE,
            VIRTIO_9P_CFG_SIZE,
        );
        let fileserver = Mutex::new(Box::new(Fileserver{fids: HashMap::new()}));
        let msize = 8192; //default
        Arc::new(Self{
            virtio_state,
            pci_state,
            source,
            target,
            fileserver,
            msize,
        })
    }

    pub fn handle_req(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {
        //println!("handling request");

        let mem = &ctx.mctx.memctx();

        let mut chain = Chain::with_capacity(1);
        let _clen = vq.pop_avail(&mut chain, mem).unwrap() as usize;

        //TODO better as uninitialized?
        //TODO shouldn't clen be 8192? comes in as 16384.... hardcode 8192 for
        //now
        let mut data: Vec<u8> = vec![0;8192];
        let buf = data.as_mut_slice();
        
        // TODO copy pasta from tail end of Chain::read function. Seemingly
        // cannot use Chain::read as-is because it expects a statically sized
        // type.
        let mut done = 0;
        let _total = chain.for_remaining_type(true, |addr, len| {
            let remain = &mut buf[done..];
            if let Some(copied) = mem.read_into(addr, remain, len) {
                let need_more = copied != remain.len();
                done += copied;
                (copied, need_more)
            } else {
                (0, false)
            }
        });

        /*
        if total != clen {
            //TODO error msg
            println!("{} != {}", total, clen);
            return None
        }
        */

        let len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
        let typ = MessageType::try_from_primitive(buf[4]).unwrap();

        match typ {

            MessageType::Tversion =>
                self.handle_version(&data[..len], &mut chain, &mem),

            MessageType::Tattach =>
                self.handle_attach(&data[..len], &mut chain, &mem),

            MessageType::Twalk =>
                self.handle_walk(&data[..len], &mut chain, &mem),

            MessageType::Tlopen =>
                self.handle_open(&data[..len], &mut chain, &mem),

            MessageType::Treaddir =>
                self.handle_readdir(&data[..len], &mut chain, &mem),

            MessageType::Tread =>
                self.handle_read(&data[..len], &mut chain, &mem),
            
            //TODO
            _ => {
                self.write_error(ENOTSUP as u32, &mut chain, &mem);
            }
        };

        vq.push_used(&mut chain, mem, ctx);
    }

    fn write_buf(&self, buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        // more copy pasta from Chain::write b/c like Chain:read a
        // statically sized type is expected.
        let mut done = 0;
        let _total = chain.for_remaining_type(false, |addr, len| {
            let remain = &buf[done..];
            if let Some(copied) = mem.write_from(addr, remain, len) {
                let need_more = copied != remain.len();

                done += copied;
                (copied, need_more)
            } else {
                // Copy failed, so do not attempt anything else
                (0, false)
            }
        });

    }

    fn write_error(&self, ecode: u32, chain: &mut Chain, mem: &MemCtx) {
        let msg = Rlerror::new(ecode);
        let mut out = ipf::to_bytes_le(&msg).unwrap();
        let buf = out.as_mut_slice();
        self.write_buf(buf, chain, mem);
    }

    fn handle_version(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {
        let mut msg: Version = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);
        msg.typ = MessageType::Rversion;
        let mut out = ipf::to_bytes_le(&msg).unwrap();
        let buf = out.as_mut_slice();
        self.write_buf(buf, chain, mem);
    }

    fn handle_attach(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {
        //NOTE: 
        //  - multiple file trees not supported, aname is ignored
        //  - authentication not supported afid is ignored
        //  - users not tracked, uname is ignored

        // deserialize message
        let msg: Tattach = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);

        // grab inode number for qid uniqe file id
        let qpath = match fs::metadata(&self.source) {
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
            Ok(m) => m.ino()
        };

        match self.fileserver.lock() {
            Ok(mut fs) => {
                // check to see if fid is in use
                match fs.fids.get(&msg.fid) {
                    Some(_) => {
                        // The spec says to throw an error here, but in an
                        // effort to support clients who don't explicitly cluck
                        // fids, and considering the fact that we do not support
                        // multiple fs trees, just carry on
                        //return self.write_error(EEXIST as u32, chain, mem);
                    }
                    None => {
                        // create fid entry
                        fs.fids.insert(
                            msg.fid, PathBuf::from(self.source.clone()));
                    }
                };
            }
            Err(_) => {
                return self.write_error(ENOLCK as u32, chain, mem);
            }
        }

        // send response
        let response = Rattach::new(Qid{
            typ: QidType::Dir,
            version: 0,
            path: qpath,
        });
        let mut out = ipf::to_bytes_le(&response).unwrap();
        let buf = out.as_mut_slice();
        self.write_buf(buf, chain, mem);

    }

    fn handle_walk(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        let msg: Twalk = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);

        match self.fileserver.lock() {
            Ok(mut fs) => {

                // check to see if fid exists
                let pathbuf = match fs.fids.get(&msg.fid) {
                    Some(p) => p,
                    None => {
                        return self.write_error(ENOENT as u32, chain, mem); 
                    }
                };

                // create new sub path from referenced fid path and wname
                // elements
                let mut newpath = pathbuf.clone();
                for n in msg.wname {
                    newpath.push(n.value);
                }

                // check that new path is a thing
                let (ino, qt) = match fs::metadata(&newpath) {
                    Err(e) => {
                        let ecode = match e.raw_os_error() {
                            Some(ecode) => ecode,
                            None => 0,
                        };
                        return self.write_error(ecode as u32, chain, mem);
                    }
                    Ok(m) => { 
                        let qt = if m.is_dir() {
                            QidType::Dir
                        } else {
                            QidType::File
                        };
                        (m.ino() , qt)
                    }
                };

                // check to see if newfid is in use
                match fs.fids.get(&msg.newfid) {
                    Some(_) => {
                        // The spec says to throw an error here, but in an
                        // effort to support clients who don't explicitly cluck
                        // fids, and considering the fact that we do not support
                        // multiple fs trees, just carry on
                        //return self.write_error(EEXIST as u32, chain, mem);
                    }
                    None => {}
                };

                // create newfid entry
                fs.fids.insert(msg.newfid, newpath);

                let response = Rwalk::new(vec![Qid{
                    typ: qt,
                    version: 0,
                    path: ino,
                }]);
                let mut out = ipf::to_bytes_le(&response).unwrap();
                let buf = out.as_mut_slice();
                self.write_buf(buf, chain, mem);

            }
            Err(_) => {
                return self.write_error(ENOLCK as u32, chain, mem);
            }
        }

    }

    fn handle_open(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        let msg: Tlopen = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);

        match self.fileserver.lock() {
            Ok(fs) => {

                // check to see if fid exists
                let pathbuf = match fs.fids.get(&msg.fid) {
                    Some(p) => p,
                    None => {
                        return self.write_error(ENOENT as u32, chain, mem); 
                    }
                };

                // check that fid path is a thing
                let (ino, qt) = match fs::metadata(&pathbuf) {
                    Err(e) => {
                        let ecode = match e.raw_os_error() {
                            Some(ecode) => ecode,
                            None => 0,
                        };
                        return self.write_error(ecode as u32, chain, mem);
                    }
                    Ok(m) => { 
                        let qt = if m.is_dir() {
                            QidType::Dir
                        } else {
                            QidType::File
                        };
                        (m.ino() , qt)
                    }
                };

                let response = Rlopen::new(Qid{
                    typ: qt,
                    version: 0,
                    path: ino,
                }, 0);

                let mut out = ipf::to_bytes_le(&response).unwrap();
                let buf = out.as_mut_slice();
                self.write_buf(buf, chain, mem);

            }
            Err(_) => {
                return self.write_error(ENOLCK as u32, chain, mem);
            }
        }
    }

    fn handle_readdir(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        let msg: Treaddir = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);

        // get the path for the requested fid
        let pathbuf = match self.fileserver.lock() {
            Ok(fs) => {
                match fs.fids.get(&msg.fid) {
                    Some(p) => p.clone(),
                    None => {
                        return self.write_error(ENOENT as u32, chain, mem); 
                    }
                }
            }
            Err(_) => {
                return self.write_error(ENOLCK as u32, chain, mem);
            }
        };

        // read the directory at the provided path
        let mut dir = match fs::read_dir(&pathbuf) {
            Ok(r) => match r.collect::<Result<Vec::<fs::DirEntry>, _>>() {
                Ok(d) => d,
                Err(e) => {
                    let ecode = match e.raw_os_error() {
                        Some(ecode) => ecode,
                        None => 0,
                    };
                    return self.write_error(ecode as u32, chain, mem);
                }
            }
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
        };

        //println!("{} dir entries under {}", 
        //    dir.len(), pathbuf.as_path().display());

        // bail with out of range error if offset is greater than entries
        if (dir.len() as u64) < msg.offset {
            return self.write_error(ERANGE as u32, chain, mem);
        }

        // need to sort to ensure consistent offsets
        dir.sort_by(|a, b| a.path().cmp(&b.path()));

        let mut space_left = self.msize as usize
            - size_of::<u32>()          // Rreaddir.size
            - size_of::<MessageType>()  // Rreaddir.typ
            - size_of::<u16>()          // Rreaddir.tag
            - size_of::<u32>();         // Rreaddir.data.len

        let mut entries: Vec<proto::Dirent> = Vec::new();

        let mut offset = 0;
        for de in &dir[msg.offset as usize..] {

            let metadata = match de.metadata() {
                Ok(m) => m,
                Err(e) => {
                    let ecode = match e.raw_os_error() {
                        Some(ecode) => ecode,
                        None => 0,
                    };
                    return self.write_error(ecode as u32, chain, mem);
                }
            };

            let (typ, ftyp) = if metadata.is_dir() {
                (QidType::Dir, DT_DIR)
            } else {
                (QidType::File, DT_REG)
            };

            let qid = Qid{
                typ,
                version: 0,
                path: metadata.ino(),
            };

            let name = match de.file_name().into_string() {
                Ok(n) => n,
                Err(_) => {
                    // getting a bit esoteric with our error codes here...
                    return self.write_error(EILSEQ as u32, chain, mem);
                }
            };

            let dirent = Dirent{
                qid,
                offset,
                typ: ftyp,
                name,
            };

            if space_left <= dirent.wire_size() {
                break;
            }

            space_left -= dirent.wire_size();
            entries.push(dirent);
            offset += 1;
        }

        let response = Rreaddir::new(entries);
        //println!("→ {:#?}", &response);
        let mut out = ipf::to_bytes_le(&response).unwrap();
        let buf = out.as_mut_slice();
        self.write_buf(buf, chain, mem);

    }

    fn handle_read(&self, msg_buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        let msg: Tread = ipf::from_bytes_le(&msg_buf).unwrap();
        //println!("← {:#?}", msg);

        // get the path for the requested fid
        let pathbuf = match self.fileserver.lock() {
            Ok(fs) => {
                match fs.fids.get(&msg.fid) {
                    Some(p) => p.clone(),
                    None => {
                        return self.write_error(ENOENT as u32, chain, mem); 
                    }
                }
            }
            Err(_) => {
                return self.write_error(ENOLCK as u32, chain, mem);
            }
        };

        // read the file at the provided path
        let mut file = match fs::OpenOptions::new().read(true).open(pathbuf) {
            Ok(f) => f,
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
        };
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
        };

        // bail with empty response if offset is greater than file size
        if metadata.len() < msg.offset {
            let response = Rread::new(Vec::new());
            let mut out = ipf::to_bytes_le(&response).unwrap();
            let buf = out.as_mut_slice();
            return self.write_buf(buf, chain, mem);
        }

        match file.seek(std::io::SeekFrom::Start(msg.offset)){
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
            Ok(_) => {},
        }

        let space_left = self.msize as usize
            - size_of::<u32>()          // Rread.size
            - size_of::<MessageType>()  // Rread.typ
            - size_of::<u16>()          // Rread.tag
            - size_of::<u32>();         // Rread.data.len

        let buflen = std::cmp::min(
            space_left,
            (metadata.len() - msg.offset) as usize,
        ) as usize;

        let mut content: Vec<u8> = vec![0;buflen];

        match file.read_exact(content.as_mut_slice()) {
            Err(e) => {
                let ecode = match e.raw_os_error() {
                    Some(ecode) => ecode,
                    None => 0,
                };
                return self.write_error(ecode as u32, chain, mem);
            }
            Ok(()) => {},
        }

        let response = Rread::new(content);
        let mut out = ipf::to_bytes_le(&response).unwrap();
        let buf = out.as_mut_slice();
        self.write_buf(buf, chain, mem);

    }

}

impl VirtioDevice for PciVirtio9pfs {
    fn cfg_rw(&self, mut rwo: RWOp) {
        P9FS_DEV_REGS.process(&mut rwo, |id, rwo| match rwo {
            RWOp::Read(ro) => {
                match id {
                    P9fsReg::TagLen => {
                        //println!("read taglen");
                        ro.write_u16(self.target.len() as u16);
                    }
                    P9fsReg::Tag => {
                        //println!("read tag");
                        let mut bs = [0;256];
                        for (i, x) in self.target.chars().enumerate() {
                            bs[i] = x as u8;
                        }
                        ro.write_bytes(&bs);
                        ro.fill(0);
                    }
                }
            }
            RWOp::Write(_) => { }
        })
    }

    fn get_features(&self) -> u32 { VIRTIO_9P_F_MOUNT_TAG }

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
enum P9fsReg {
    TagLen,
    Tag,
}

lazy_static! {
    static ref P9FS_DEV_REGS: RegMap<P9fsReg> = {
        let layout = [
            (P9fsReg::TagLen, 2),
            (P9fsReg::Tag, 256),
        ];
        RegMap::create_packed(VIRTIO_9P_CFG_SIZE, &layout, None)
    };
}

struct Fileserver {
    fids: HashMap::<u32, PathBuf>,
}


mod bits {
    use std::mem::size_of;

    // features
    pub const VIRTIO_9P_F_MOUNT_TAG: u32 = 0x1;

    pub const VIRTIO_9P_MAX_TAG_SIZE: usize = 256;
    pub const VIRTIO_9P_CFG_SIZE: usize =
        VIRTIO_9P_MAX_TAG_SIZE + size_of::<u16>();
}
use bits::*;

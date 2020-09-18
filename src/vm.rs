use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::os::raw::c_void;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;

use bhyve_api;
use bhyve_api::{vm_entry_cmds, vm_reg_name, SEG_ACCESS_P, SEG_ACCESS_S};
use libc;

use crate::exits::*;

pub fn create_vm(name: &str) -> Result<VmCtx> {
    let ctl = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_EXCL)
        .open(bhyve_api::VMM_CTL_PATH)?;
    let namestr = CString::new(name).or_else(|_x| Err(Error::from_raw_os_error(libc::EINVAL)))?;
    let nameptr = namestr.as_ptr();
    let ctlfd = ctl.as_raw_fd();

    let res = unsafe { libc::ioctl(ctlfd, bhyve_api::VMM_CREATE_VM, nameptr) };
    if res != 0 {
        let err = Error::last_os_error();
        if err.kind() != ErrorKind::AlreadyExists {
            return Err(err);
        }
        // try to nuke(!) the existing vm
        let res = unsafe { libc::ioctl(ctlfd, bhyve_api::VMM_DESTROY_VM, nameptr) };
        if res != 0 {
            let err = Error::last_os_error();
            if err.kind() != ErrorKind::NotFound {
                return Err(err);
            }
        }
        // attempt to create in its presumed absence
        let res = unsafe { libc::ioctl(ctlfd, bhyve_api::VMM_CREATE_VM, nameptr) };
        if res != 0 {
            return Err(Error::last_os_error());
        }
    }

    let mut vmpath = PathBuf::from(bhyve_api::VMM_PATH_PREFIX);
    vmpath.push(name);

    let fp = OpenOptions::new().write(true).read(true).open(vmpath)?;
    Ok(VmCtx {
        hdl: Arc::new(VmHdl { inner: fp }),
    })
}

#[repr(u8)]
#[allow(non_camel_case_types)]
enum VmMemsegs {
    SEG_LOWMEM,
    SEG_BOOTROM,
}

pub struct VmHdl {
    inner: File
}
impl VmHdl {
    fn fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
    pub fn ioctl<T>(&self, cmd: i32, data: *mut T) -> Result<i32> {
        let res = unsafe { libc::ioctl(self.fd(), cmd, data) };
        if res == -1 {
            Err(Error::last_os_error())
        } else {
            Ok(res)
        }
    }
}

pub struct VmCtx {
    hdl: Arc<VmHdl>,
}

impl VmCtx {
    pub fn setup_memory(&mut self, size: u64) -> Result<()> {
        let segid = VmMemsegs::SEG_LOWMEM as i32;
        let mut seg = bhyve_api::vm_memseg {
            segid,
            len: size as usize,
            name: [0u8; bhyve_api::SEG_NAME_LEN],
        };
        self.hdl.ioctl(bhyve_api::VM_ALLOC_MEMSEG, &mut seg)?;
        self.map_memseg(segid, 0, size as usize, 0, bhyve_api::PROT_ALL)
    }

    pub fn setup_bootrom(&mut self, len: usize) -> Result<()> {
        let segid = VmMemsegs::SEG_BOOTROM as i32;
        let mut seg = bhyve_api::vm_memseg {
            segid,
            len,
            name: [0u8; bhyve_api::SEG_NAME_LEN],
        };

        let mut name = &mut seg.name[..];
        name.write("bootrom".as_bytes())?;
        self.hdl.ioctl(bhyve_api::VM_ALLOC_MEMSEG, &mut seg)?;

        // map the bootrom so the first instruction lines up at 0xfffffff0
        let gpa = 0x1_0000_0000 - len as u64;
        self.map_memseg(
            segid,
            gpa,
            len,
            0,
            bhyve_api::PROT_READ | bhyve_api::PROT_EXEC,
        )?;
        Ok(())
    }

    pub fn populate_bootrom(&mut self, input: &mut File, len: usize) -> Result<()> {
        let mut devoff = bhyve_api::vm_devmem_offset {
            segid: VmMemsegs::SEG_BOOTROM as i32,
            offset: 0,
        };
        // find the devmem offset
        self.hdl.ioctl(bhyve_api::VM_DEVMEM_GETOFFSET, &mut devoff)?;
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.hdl.fd(),
                devoff.offset,
            )
        };
        if ptr.is_null() {
            return Err(Error::last_os_error());
        }
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, len) };

        let res = match input.read(buf) {
            Ok(n) if n == len => Ok(()),
            Ok(_) => {
                // TODO: handle short read
                Ok(())
            }
            Err(e) => Err(e),
        };

        unsafe {
            libc::munmap(ptr, len as usize);
        }

        res
    }

    pub fn vcpu(&self, id: i32) -> VcpuCtx {
        assert!(id >= 0 && id < bhyve_api::VM_MAXCPU as i32);
        VcpuCtx {
            hdl: self.get_hdl(),
            id,
        }
    }

    pub fn get_hdl(&self) -> Arc<VmHdl> {
        self.hdl.clone()
    }

    fn map_memseg(&mut self, id: i32, gpa: u64, len: usize, off: u64, prot: u8) -> Result<()> {
        assert!(off <= i64::MAX as u64);
        let mut map = bhyve_api::vm_memmap {
            gpa,
            segid: id,
            segoff: off as i64,
            len,
            prot: prot as i32,
            flags: 0,
        };
        self.hdl.ioctl(bhyve_api::VM_MMAP_MEMSEG, &mut map)?;
        Ok(())
    }
}

pub struct VcpuCtx {
    hdl: Arc<VmHdl>,
    id: i32,
}

impl VcpuCtx {
    pub fn set_reg(&mut self, reg: bhyve_api::vm_reg_name, val: u64) -> Result<()> {
        let mut regcmd = bhyve_api::vm_register {
            cpuid: self.id,
            regnum: reg as i32,
            regval: val,
        };

        self.hdl.ioctl(bhyve_api::VM_SET_REGISTER, &mut regcmd)?;
        Ok(())
    }
    pub fn set_segreg(
        &mut self,
        reg: bhyve_api::vm_reg_name,
        seg: &bhyve_api::seg_desc,
    ) -> Result<()> {
        let mut desc = bhyve_api::vm_seg_desc {
            cpuid: self.id,
            regnum: reg as i32,
            desc: *seg,
        };

        self.hdl
            .ioctl(bhyve_api::VM_SET_SEGMENT_DESCRIPTOR, &mut desc)?;
        Ok(())
    }
    pub fn reboot_state(&mut self) -> Result<()> {
        self.set_reg(vm_reg_name::VM_REG_GUEST_CR0, 0x6000_0010)?;
        self.set_reg(vm_reg_name::VM_REG_GUEST_RFLAGS, 0x0000_0002)?;

        let cs_desc = bhyve_api::seg_desc {
            base: 0xffff_0000,
            limit: 0xffff,
            // Present, R/W, Accessed
            access: SEG_ACCESS_P | SEG_ACCESS_S | 0x3,
        };
        self.set_segreg(vm_reg_name::VM_REG_GUEST_CS, &cs_desc)?;
        self.set_reg(vm_reg_name::VM_REG_GUEST_CS, 0xf000)?;

        let data_desc = bhyve_api::seg_desc {
            base: 0x0000_0000,
            limit: 0xffff,
            // Present, R/W, Accessed
            access: SEG_ACCESS_P | SEG_ACCESS_S | 0x3,
        };
        let data_segs = [
            vm_reg_name::VM_REG_GUEST_ES,
            vm_reg_name::VM_REG_GUEST_SS,
            vm_reg_name::VM_REG_GUEST_DS,
            vm_reg_name::VM_REG_GUEST_FS,
            vm_reg_name::VM_REG_GUEST_GS,
        ];
        for seg in &data_segs {
            self.set_segreg(*seg, &data_desc)?;
            self.set_reg(*seg, 0)?;
        }

        let gidtr_desc = bhyve_api::seg_desc {
            base: 0x0000_0000,
            limit: 0xffff,
            access: 0,
        };
        self.set_segreg(vm_reg_name::VM_REG_GUEST_GDTR, &gidtr_desc)?;
        self.set_segreg(vm_reg_name::VM_REG_GUEST_IDTR, &gidtr_desc)?;

        let ldtr_desc = bhyve_api::seg_desc {
            base: 0x0000_0000,
            limit: 0xffff,
            // LDT present
            access: SEG_ACCESS_P | 0b0010,
        };
        self.set_segreg(vm_reg_name::VM_REG_GUEST_LDTR, &ldtr_desc)?;

        let tr_desc = bhyve_api::seg_desc {
            base: 0x0000_0000,
            limit: 0xffff,
            // TSS32 busy
            access: SEG_ACCESS_P | 0b1011,
        };
        self.set_segreg(vm_reg_name::VM_REG_GUEST_TR, &tr_desc)?;

        Ok(())
    }
    pub fn activate(&mut self) -> Result<()> {
        let mut cpu = self.id;

        self.hdl.ioctl(bhyve_api::VM_ACTIVATE_CPU, &mut cpu)?;
        Ok(())
    }
    pub fn run(&mut self, entry: &VmEntry) -> Result<VmExit> {
        let mut exit: bhyve_api::vm_exit = Default::default();
        let mut entry = entry.to_raw(self.id, &mut exit);
        match self.hdl.ioctl(bhyve_api::VM_RUN, &mut entry) {
            Err(e) => {
                return Err(e);
            }
            Ok(_) => {}
        }
        Ok(VmExit::from(&exit))
    }
}

pub enum VmEntry {
    Run,
}
impl VmEntry {
    fn to_raw(&self, cpuid: i32, exit_ptr: *mut bhyve_api::vm_exit) -> bhyve_api::vm_entry {
        let raw_cmd = match self {
            VmEntry::Run => vm_entry_cmds::VEC_DEFAULT,
        };
        bhyve_api::vm_entry {
            cpuid,
            cmd: raw_cmd as u32,
            u: Default::default(),
            exit_data: exit_ptr as *mut c_void,
        }
    }
}

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use super::bits::*;
use super::{INTxPin, PciEndpoint};
use crate::dispatch::DispCtx;
use crate::intr_pins::{IntrPin, IsaPin};
use crate::types::*;
use crate::util::regmap::{Flags, RegMap};
use crate::util::self_arc::*;

use byteorder::{ByteOrder, LE};
use lazy_static::lazy_static;

enum CfgReg {
    Std,
    Custom,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum StdCfgReg {
    VendorId,
    DeviceId,
    Command,
    Status,
    RevisionId,
    ProgIf,
    Subclass,
    Class,
    CacheLineSize,
    LatencyTimer,
    HeaderType,
    Bist,
    Bar(BarN),
    CardbusPtr,
    SubVendorId,
    SubDeviceId,
    ExpansionRomAddr,
    CapPtr,
    Reserved,
    IntrLine,
    IntrPin,
    MinGrant,
    MaxLatency,
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BarN {
    BAR0 = 0,
    BAR1,
    BAR2,
    BAR3,
    BAR4,
    BAR5,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
enum BarDefine {
    Pio(u16),
    Mmio(u32),
    Mmio64(u64),
    Mmio64High,
}

lazy_static! {
    static ref STD_CFG_MAP: RegMap<StdCfgReg> = {
        let layout = [
            (StdCfgReg::VendorId, OFF_CFG_VENDORID, 2),
            (StdCfgReg::DeviceId, OFF_CFG_DEVICEID, 2),
            (StdCfgReg::Command, OFF_CFG_COMMAND, 2),
            (StdCfgReg::Status, OFF_CFG_STATUS, 2),
            (StdCfgReg::RevisionId, OFF_CFG_REVISIONID, 1),
            (StdCfgReg::ProgIf, OFF_CFG_PROGIF, 1),
            (StdCfgReg::Subclass, OFF_CFG_SUBCLASS, 1),
            (StdCfgReg::Class, OFF_CFG_CLASS, 1),
            (StdCfgReg::CacheLineSize, OFF_CFG_CACHELINESZ, 1),
            (StdCfgReg::LatencyTimer, OFF_CFG_LATENCYTIMER, 1),
            (StdCfgReg::HeaderType, OFF_CFG_HEADERTYPE, 1),
            (StdCfgReg::Bist, OFF_CFG_BIST, 1),
            (StdCfgReg::Bar(BarN::BAR0), OFF_CFG_BAR0, 4),
            (StdCfgReg::Bar(BarN::BAR1), OFF_CFG_BAR1, 4),
            (StdCfgReg::Bar(BarN::BAR2), OFF_CFG_BAR2, 4),
            (StdCfgReg::Bar(BarN::BAR3), OFF_CFG_BAR3, 4),
            (StdCfgReg::Bar(BarN::BAR4), OFF_CFG_BAR4, 4),
            (StdCfgReg::Bar(BarN::BAR5), OFF_CFG_BAR5, 4),
            (StdCfgReg::CardbusPtr, OFF_CFG_CARDBUSPTR, 4),
            (StdCfgReg::SubVendorId, OFF_CFG_SUBVENDORID, 2),
            (StdCfgReg::SubDeviceId, OFF_CFG_SUBDEVICEID, 2),
            (StdCfgReg::ExpansionRomAddr, OFF_CFG_EXPROMADDR, 4),
            (StdCfgReg::CapPtr, OFF_CFG_CAPPTR, 1),
            // Reserved bytes between CapPtr and IntrLine [0x35-0x3c)
            (
                StdCfgReg::Reserved,
                OFF_CFG_RESERVED,
                OFF_CFG_INTRLINE - OFF_CFG_RESERVED,
            ),
            (StdCfgReg::IntrLine, OFF_CFG_INTRLINE, 1),
            (StdCfgReg::IntrPin, OFF_CFG_INTRPIN, 1),
            (StdCfgReg::MinGrant, OFF_CFG_MINGRANT, 1),
            (StdCfgReg::MaxLatency, OFF_CFG_MAXLATENCY, 1),
        ];
        let mut map = RegMap::new(LEN_CFG_STD);
        for reg in layout.iter() {
            let (id, off, size) = (reg.0, reg.1 as usize, reg.2 as usize);
            let flags = match id {
                StdCfgReg::Reserved => {
                    // The reserved section is empty, so the register does not
                    // need a buffer padded to its own size for reads or writes.
                    Flags::NO_READ_EXTEND | Flags::NO_WRITE_EXTEND
                }
                _ => Flags::DEFAULT,
            };
            map.define_with_flags(off, size, id, flags);
        }
        map
    };
}

pub struct Ident {
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub sub_vendor_id: u16,
    pub sub_device_id: u16,
}

#[derive(Default)]
struct State {
    reg_command: u16,
    reg_intr_line: u8,
    reg_intr_pin: u8,

    lintr_pin: Option<IsaPin>,
}

#[derive(Copy, Clone)]
struct BarEntry {
    define: Option<BarDefine>,
    addr: u64,
}
impl Default for BarEntry {
    fn default() -> Self {
        Self { define: None, addr: 0 }
    }
}

struct Bars {
    entries: [BarEntry; 6],
}

impl Bars {
    fn new() -> Self {
        Self { entries: [Default::default(); 6] }
    }
    fn read(&self, bar: BarN) -> u32 {
        let idx = bar as usize;
        let ent = &self.entries[idx];
        if ent.define.is_none() {
            return 0;
        }
        match ent.define.as_ref().unwrap() {
            BarDefine::Pio(_) => ent.addr as u32 | BAR_TYPE_IO,
            BarDefine::Mmio(_) => ent.addr as u32 | BAR_TYPE_MEM,
            BarDefine::Mmio64(_) => ent.addr as u32 | BAR_TYPE_MEM64,
            BarDefine::Mmio64High => {
                assert!(idx > 0);
                let prev = self.entries[idx - 1];
                (prev.addr >> 32) as u32
            }
        }
    }
    fn write(&mut self, bar: BarN, val: u32) {
        let idx = bar as usize;
        if self.entries[idx].define.is_none() {
            return;
        }
        let mut ent = &mut self.entries[idx];
        let old = match ent.define.as_ref().unwrap() {
            BarDefine::Pio(size) => {
                let mask = !(size - 1) as u32;
                let old = ent.addr;
                ent.addr = (val & mask) as u64;
                old
            }
            BarDefine::Mmio(size) => {
                let mask = !(size - 1);
                let old = ent.addr;
                ent.addr = (val & mask) as u64;
                old
            }
            BarDefine::Mmio64(size) => {
                let old = ent.addr;
                let mask = !(size - 1) as u32;
                let low = old as u32 & mask;
                ent.addr = (old & (0xffffffff << 32)) | low as u64;
                old
            }
            BarDefine::Mmio64High => {
                assert!(idx > 0);
                ent = &mut self.entries[idx - 1];
                let size = match ent.define.as_ref().unwrap() {
                    BarDefine::Mmio64(sz) => sz,
                    _ => panic!(),
                };
                let mask = !(size - 1);
                let old = ent.addr;
                ent.addr = ((val as u64) << 32) & mask | (old & 0xffffffff);
                old
            }
        };
        println!(
            "bar write {:x?} {:x} -> {:x}",
            *ent.define.as_ref().unwrap(),
            old,
            ent.addr
        );
    }
}

pub struct DeviceInst<I: Send> {
    ident: Ident,
    cfg_space: RegMap<CfgReg>,

    state: Mutex<State>,
    bars: Mutex<Bars>,

    sa_cell: SelfArcCell<Self>,

    inner: I,
}

impl<I: Device> DeviceInst<I> {
    fn new(ident: Ident, cfg_space: RegMap<CfgReg>, bars: Bars, i: I) -> Self {
        Self {
            ident,
            cfg_space,
            state: Mutex::new(State {
                reg_intr_line: 0xff,
                ..Default::default()
            }),
            bars: Mutex::new(bars),
            sa_cell: SelfArcCell::new(),
            inner: i,
        }
    }
    pub fn with_inner<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&I) -> T,
    {
        f(&self.inner)
    }

    fn cfg_map_read(&self, id: &CfgReg, ro: &mut ReadOp) {
        match id {
            CfgReg::Std => {
                STD_CFG_MAP
                    .with_ctx(self, Self::cfg_std_read, Self::cfg_std_write)
                    .read(ro);
            }
            CfgReg::Custom => self.inner.cfg_read(ro),
        }
    }
    fn cfg_map_write(&self, id: &CfgReg, wo: &WriteOp) {
        match id {
            CfgReg::Std => {
                STD_CFG_MAP
                    .with_ctx(self, Self::cfg_std_read, Self::cfg_std_write)
                    .write(wo);
            }
            CfgReg::Custom => self.inner.cfg_write(wo),
        }
    }
    fn cfg_std_read(&self, id: &StdCfgReg, ro: &mut ReadOp) {
        assert!(ro.offset == 0 || *id == StdCfgReg::Reserved);

        let buf = &mut ro.buf;
        match id {
            StdCfgReg::VendorId => LE::write_u16(buf, self.ident.vendor_id),
            StdCfgReg::DeviceId => LE::write_u16(buf, self.ident.device_id),
            StdCfgReg::Class => buf[0] = self.ident.class,
            StdCfgReg::Subclass => buf[0] = self.ident.subclass,
            StdCfgReg::SubVendorId => {
                LE::write_u16(buf, self.ident.sub_vendor_id)
            }
            StdCfgReg::SubDeviceId => {
                LE::write_u16(buf, self.ident.sub_device_id)
            }

            StdCfgReg::Command => {
                LE::write_u16(buf, self.state.lock().unwrap().reg_command)
            }
            StdCfgReg::IntrLine => {
                buf[0] = self.state.lock().unwrap().reg_intr_line
            }
            StdCfgReg::IntrPin => {
                buf[0] = self.state.lock().unwrap().reg_intr_pin
            }
            StdCfgReg::Bar(bar) => {
                LE::write_u32(buf, self.bars.lock().unwrap().read(*bar))
            }
            StdCfgReg::ExpansionRomAddr => {
                // no rom for now
                LE::write_u32(buf, 0);
            }
            StdCfgReg::Reserved => {
                buf.iter_mut().for_each(|b| *b = 0);
            }
            _ => {
                println!("Unhandled read {:?}", id);
                buf.iter_mut().for_each(|b| *b = 0);
            }
        }
    }
    fn cfg_std_write(&self, id: &StdCfgReg, wo: &WriteOp) {
        assert!(wo.offset == 0 || *id == StdCfgReg::Reserved);

        let buf = wo.buf;
        match id {
            StdCfgReg::Command => {
                let val = LE::read_u16(buf);
                // XXX: wire up change handling
                self.state.lock().unwrap().reg_command = val & REG_MASK_CMD;
            }
            StdCfgReg::IntrLine => {
                self.state.lock().unwrap().reg_intr_line = buf[0];
            }
            StdCfgReg::Bar(bar) => {
                let val = LE::read_u32(buf);
                self.bars.lock().unwrap().write(*bar, val);
            }
            StdCfgReg::Reserved => {}
            _ => {
                println!("Unhandled write {:?}", id);
                // discard all other writes
            }
        }
    }
}

impl<I: Device> PciEndpoint for DeviceInst<I> {
    fn cfg_read(&self, ro: &mut ReadOp) {
        self.cfg_space
            .with_ctx(self, Self::cfg_map_read, Self::cfg_map_write)
            .read(ro);
    }

    fn cfg_write(&self, wo: &WriteOp) {
        self.cfg_space
            .with_ctx(self, Self::cfg_map_read, Self::cfg_map_write)
            .write(wo);
    }
    fn attach(&self, lintr: Option<(INTxPin, IsaPin)>) {
        let mut state = self.state.lock().unwrap();
        if let Some((intx, isa_pin)) = lintr {
            state.reg_intr_pin = intx as u8;
            state.reg_intr_line = isa_pin.get_pin();
            state.lintr_pin = Some(isa_pin);
        }
    }
}

impl<I: Sized + Send> SelfArc for DeviceInst<I> {
    fn self_arc_cell(&self) -> &SelfArcCell<Self> {
        &self.sa_cell
    }
}

pub struct DeviceCtx<'a, 'b> {
    state: &'a Mutex<State>,
    dctx: &'b DispCtx,
}
impl<'a, 'b> DeviceCtx<'a, 'b> {
    fn new(state: &'a Mutex<State>, dctx: &'b DispCtx) -> Self {
        Self { state, dctx }
    }

    pub fn set_lintr(&self, level: bool) {
        let mut state = self.state.lock().unwrap();
        if state.reg_intr_pin == 0 {
            return;
        }
        // XXX: heed INTxDIS
        let pin = state.lintr_pin.as_mut().unwrap();
        if level {
            pin.assert();
        } else {
            pin.deassert();
        }
    }
}

pub trait Device: Send + Sync {
    fn bar_read(&self, bar: BarN, ro: &mut ReadOp) {
        unimplemented!("BAR read ({:?} @ {:x})", bar, ro.offset)
    }
    fn bar_write(&self, bar: BarN, wo: &WriteOp) {
        unimplemented!("BAR write ({:?} @ {:x})", bar, wo.offset)
    }

    fn cfg_read(&self, ro: &mut ReadOp) {
        unimplemented!("CFG read @ {:x}", ro.offset)
    }
    fn cfg_write(&self, wo: &WriteOp) {
        unimplemented!("CFG write @ {:x}", wo.offset)
    }
    // TODO
    // fn cap_read(&self);
    // fn cap_write(&self);
}

pub struct Builder<I> {
    ident: Ident,
    need_lintr: bool,
    bars: [Option<BarDefine>; 6],
    cfgmap: RegMap<CfgReg>,
    _phantom: PhantomData<I>,
}

impl<I: Device> Builder<I> {
    pub fn new(ident: Ident) -> Self {
        let mut cfgmap = RegMap::new_with_flags(
            LEN_CFG,
            Flags::NO_READ_EXTEND | Flags::NO_WRITE_EXTEND,
        );
        cfgmap.define(0, LEN_CFG_STD, CfgReg::Std);
        Self {
            ident,
            need_lintr: false,
            bars: [None; 6],
            cfgmap,
            _phantom: PhantomData,
        }
    }

    pub fn add_bar_io(mut self, bar: BarN, size: u16) -> Self {
        assert!(size.is_power_of_two());
        assert!(size >= 4);

        let idx = bar as usize;
        assert!(self.bars[idx].is_none());

        self.bars[idx] = Some(BarDefine::Pio(size));
        self
    }
    pub fn add_bar_mmio(mut self, bar: BarN, size: u32) -> Self {
        assert!(size.is_power_of_two());
        assert!(size >= 16);

        let idx = bar as usize;
        assert!(self.bars[idx].is_none());

        self.bars[idx] = Some(BarDefine::Mmio(size));
        self
    }
    pub fn add_bar_mmio64(mut self, bar: BarN, size: u64) -> Self {
        assert!(size.is_power_of_two());
        assert!(size >= 16);

        let idx = bar as usize;
        assert!(idx != 6);
        assert!(self.bars[idx].is_none());
        assert!(self.bars[idx + 1].is_none());

        self.bars[idx] = Some(BarDefine::Mmio64(size));
        self.bars[idx + 1] = Some(BarDefine::Mmio64High);
        self
    }
    pub fn add_lintr(mut self) -> Self {
        self.need_lintr = true;
        self
    }
    pub fn add_custom_cfg(mut self, offset: u8, len: u8) -> Self {
        self.cfgmap.define(offset as usize, len as usize, CfgReg::Custom);
        self
    }

    fn generate_bars(&self) -> Bars {
        let mut bars = Bars::new();
        for (idx, ent) in self.bars.iter().enumerate() {
            bars.entries[idx].define = *ent;
        }
        bars
    }

    pub fn finish(self, inner: I) -> Arc<DeviceInst<I>> {
        let bars = self.generate_bars();
        let mut inst =
            Arc::new(DeviceInst::new(self.ident, self.cfgmap, bars, inner));
        SelfArc::self_arc_init(&mut inst);
        inst
    }
}

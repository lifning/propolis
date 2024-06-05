// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeMap;
use std::num::NonZeroU16;
use std::sync::{Arc, Mutex};

use crate::common::*;
use crate::hw::ids::pci::VENDOR_OXIDE;
use crate::hw::pci;
use crate::migrate::*;
use crate::util::regmap::RegMap;

use super::bits::*;
use super::pci::{PciVirtio, PciVirtioState};
use super::queue::{VirtQueue, VirtQueues};
use super::VirtioDevice;
use bits::*;

use lazy_static::lazy_static;

/// Device IDs used when VIRTIO_INPUT_CFG_ID_DEVIDS is queried.
/// (virtio 1.3 sect 5.8.4)
#[repr(C, packed)]
pub struct VirtioInputDevIds {
    bus_type: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

/// Information about the value and range of the axis specified by subselect
/// when VIRTIO_INPUT_CFG_ABS_INFO is queried.
/// (virtio 1.3 sect 5.8.4)
#[repr(C, packed)]
pub struct VirtioInputAbsInfo {
    min: i32,
    max: i32,
    fuzz: i32,
    flat: i32,
    res: i32,
}

pub enum VirtioInputDev {
    Tablet(VirtioTablet),
}

pub struct VirtioTablet {
    supported_events: BTreeMap<u16, Vec<u16>>,
    properties: Vec<u16>,
    //vnc_pointer_recv: !,
}

impl VirtioTablet {
    pub fn new() -> Self {
        Self {
            supported_events: [
                // we'll have up to 8 mouse buttons' state given to us by VNC
                // (RFC 6143 sect 7.5.5). two are scroll wheel EV_REL events.
                (
                    EV_KEY,
                    vec![
                        BTN_LEFT,
                        BTN_MIDDLE,
                        BTN_RIGHT,
                        BTN_FORWARD,
                        BTN_BACK,
                        BTN_TASK,
                    ],
                ),
                (EV_ABS, vec![ABS_X, ABS_Y]),
                // wheel +1/-1 emitted for btns 4 (up) and 5 (down) respectively
                (EV_REL, vec![REL_WHEEL]),
            ]
            .into_iter()
            .collect(),
            // https://www.kernel.org/doc/html/latest/input/event-codes.html#tablets
            properties: vec![INPUT_PROP_POINTER, INPUT_PROP_DIRECT],
            //vnc_pointer_recv: todo!(),
        }
    }
}

pub trait VirtioInputDevice {
    fn name(&self) -> &str;
    fn dev_ids(&self) -> VirtioInputDevIds;
    fn properties(&self) -> &[u16];
    fn supported_events(&self) -> &BTreeMap<u16, Vec<u16>>;
    fn abs_info(&self, axis: u8) -> VirtioInputAbsInfo;
}

impl VirtioInputDevice for VirtioTablet {
    fn name(&self) -> &str {
        "Oxide VirtIO pointer tablet"
    }
    fn dev_ids(&self) -> VirtioInputDevIds {
        VirtioInputDevIds {
            bus_type: BUS_PCI,
            vendor: VENDOR_OXIDE,
            product: 0x7AB1,
            version: 0,
        }
    }
    fn properties(&self) -> &[u16] {
        &self.properties
    }
    fn supported_events(&self) -> &BTreeMap<u16, Vec<u16>> {
        &self.supported_events
    }
    fn abs_info(&self, _axis: u8) -> VirtioInputAbsInfo {
        // TODO care about _axis and answer accordingly
        VirtioInputAbsInfo { min: 0, max: 0x8000, fuzz: 0, flat: 0, res: 1 }
    }
}

fn bitmapify(iter: impl IntoIterator<Item = u16>) -> [u8; 128] {
    let mut bitmap = [0u8; 128];
    for x in iter.into_iter() {
        let i = x as usize / 8;
        bitmap[i] |= 1 << (x % 8);
    }
    bitmap
}

struct Selection {
    sel: u8,
    subsel: u8,
}

pub struct PciVirtioInput {
    virtio_state: PciVirtioState,
    pci_state: pci::DeviceState,
    selection: Mutex<Selection>,
    device: Box<dyn VirtioInputDevice + Send + Sync>,
    // TODO: proper wrapping for device
}
impl PciVirtioInput {
    pub fn new(
        queue_size: u16,
        device: Box<dyn VirtioInputDevice + Send + Sync>,
    ) -> Arc<Self> {
        // eventq + statusq (virtio 1.3 sect 5.8.2)
        let queues = VirtQueues::new(
            NonZeroU16::new(queue_size).unwrap(),
            NonZeroU16::new(1).unwrap(), // TODO: is this appropriate statusq size?
        );
        // TODO: is this right?
        // virtio-input only needs two MSI-X entries for its interrupt needs:
        // - device config changes
        // - eventq notification
        let msix_count = Some(2);
        let (virtio_state, pci_state) = PciVirtioState::create(
            queues,
            msix_count,
            VIRTIO_DEV_INPUT,
            VIRTIO_SUB_DEV_INPUT,
            pci::bits::CLASS_INPUT,
            VIRTIO_INPUT_CFG_SIZE,
        );

        Arc::new_cyclic(|weak| Self {
            pci_state,
            virtio_state,
            selection: Mutex::new(Selection {
                sel: VIRTIO_INPUT_CFG_UNSET,
                subsel: 0,
            }),
            device,
        })
    }

    fn input_cfg_read(&self, id: &InputReg, ro: &mut ReadOp) {
        let selection = self.selection.lock().unwrap();
        match id {
            InputReg::Select => ro.write_u8(selection.sel),
            InputReg::Subselect => ro.write_u8(selection.subsel),
            InputReg::Size => {
                // must be size zero if unsupported select and subsel combination
                // (virtio 1.3 sect 5.8.5.2)
                let size = match selection.sel {
                    VIRTIO_INPUT_CFG_UNSET => 0,
                    VIRTIO_INPUT_CFG_ID_NAME => {
                        if selection.subsel == 0 {
                            self.device.name().len()
                        } else {
                            0
                        }
                    }
                    VIRTIO_INPUT_CFG_ID_SERIAL => 0,
                    VIRTIO_INPUT_CFG_ID_DEVIDS => {
                        if selection.subsel == 0 {
                            core::mem::size_of::<VirtioInputDevIds>()
                        } else {
                            0
                        }
                    }
                    VIRTIO_INPUT_CFG_PROP_BITS => {
                        if selection.subsel == 0 {
                            128
                        } else {
                            0
                        }
                    }
                    VIRTIO_INPUT_CFG_EV_BITS => 128,
                    VIRTIO_INPUT_CFG_ABS_INFO => {
                        core::mem::size_of::<VirtioInputAbsInfo>()
                    }
                    _ => 0,
                };
                ro.write_u8(size as u8);
            }
            InputReg::Reserved => ro.fill(0),
            InputReg::Payload => self.write_payload(&selection, ro),
        }
    }

    fn write_payload(&self, selection: &Selection, ro: &mut ReadOp) {
        probes::vioinput_cfg_read!(|| (selection.sel, selection.subsel));
        match selection.sel {
            VIRTIO_INPUT_CFG_UNSET => {}
            VIRTIO_INPUT_CFG_ID_NAME if selection.subsel == 0 => {
                ro.write_bytes(self.device.name().as_bytes())
            }
            VIRTIO_INPUT_CFG_ID_SERIAL => {}
            VIRTIO_INPUT_CFG_ID_DEVIDS => {
                let d = self.device.dev_ids();
                ro.write_u16(d.bus_type);
                ro.write_u16(d.vendor);
                ro.write_u16(d.product);
                ro.write_u16(d.version);
            }
            VIRTIO_INPUT_CFG_PROP_BITS => {
                if selection.subsel == 0 {
                    ro.write_bytes(&bitmapify(
                        self.device.properties().iter().copied(),
                    ));
                }
            }
            VIRTIO_INPUT_CFG_EV_BITS => {
                let events = self.device.supported_events();
                if selection.subsel == 0 {
                    ro.write_bytes(&bitmapify(events.keys().copied()));
                } else if let Some(val) = events.get(&(selection.subsel as u16))
                {
                    ro.write_bytes(&bitmapify(val.iter().copied()));
                } else {
                    ro.fill(0);
                }
            }
            VIRTIO_INPUT_CFG_ABS_INFO => {
                let abs = self.device.abs_info(selection.subsel);
                ro.write_u32(abs.min as u32);
                ro.write_u32(abs.max as u32);
                ro.write_u32(abs.fuzz as u32);
                ro.write_u32(abs.flat as u32);
                ro.write_u32(abs.res as u32);
            }
            _ => {}
        }
    }

    fn input_cfg_write(&self, id: &InputReg, wo: &mut WriteOp) {
        let mut selection = self.selection.lock().unwrap();
        probes::vioinput_cfg_write!(|| (selection.sel, selection.subsel));
        match id {
            InputReg::Select => selection.sel = wo.read_u8(),
            InputReg::Subselect => selection.subsel = wo.read_u8(),
            // drivers must not write to cfg fields other than sel and subsel
            // (virtio 1.3 sect 5.8.5.1)
            _ => {}
        }
    }
}

impl VirtioDevice for PciVirtioInput {
    fn cfg_rw(&self, mut rwo: RWOp) {
        INPUT_DEV_REGS.process(&mut rwo, |id, rwo| match rwo {
            RWOp::Read(ro) => self.input_cfg_read(id, ro),
            RWOp::Write(wo) => self.input_cfg_write(id, wo),
        });
    }
    fn get_features(&self) -> u32 {
        0 // no feature bits (virtio 1.3 section 5.8.3)
    }
    fn set_features(&self, _feat: u32) -> Result<(), ()> {
        Ok(())
    }

    fn queue_notify(&self, _vq: &Arc<VirtQueue>) {
        todo!("queue notify")
    }
}
impl PciVirtio for PciVirtioInput {
    fn virtio_state(&self) -> &PciVirtioState {
        &self.virtio_state
    }
    fn pci_state(&self) -> &pci::DeviceState {
        &self.pci_state
    }
}
impl Lifecycle for PciVirtioInput {
    fn type_name(&self) -> &'static str {
        "pci-virtio-input"
    }
    fn reset(&self) {
        self.virtio_state.reset(self);
    }
    /*
    fn pause(&self) {
        todo!("pause")
    }
    fn resume(&self) {
        todo!("resume")
    }
    fn paused(&self) -> BoxFuture<'static, ()> {
        todo!("paused")
    }
    */
    fn migrate(&self) -> Migrator {
        Migrator::Multi(self)
    }
}
impl MigrateMulti for PciVirtioInput {
    fn export(
        &self,
        output: &mut PayloadOutputs,
        ctx: &MigrateCtx,
    ) -> Result<(), MigrateStateError> {
        <dyn PciVirtio>::export(self, output, ctx)
    }

    fn import(
        &self,
        offer: &mut PayloadOffers,
        ctx: &MigrateCtx,
    ) -> Result<(), MigrateStateError> {
        <dyn PciVirtio>::import(self, offer, ctx)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum InputReg {
    Select,
    Subselect,
    Size,
    Reserved,
    Payload,
}
lazy_static! {
    static ref INPUT_DEV_REGS: RegMap<InputReg> = {
        let layout = [
            (InputReg::Select, 1),
            (InputReg::Subselect, 1),
            (InputReg::Size, 1),
            (InputReg::Reserved, 5),
            (InputReg::Payload, 128),
        ];
        RegMap::create_packed(
            VIRTIO_INPUT_CFG_SIZE,
            &layout,
            Some(InputReg::Reserved),
        )
    };
}

mod bits {
    pub const VIRTIO_INPUT_CFG_UNSET: u8 = 0x00;
    pub const VIRTIO_INPUT_CFG_ID_NAME: u8 = 0x01;
    pub const VIRTIO_INPUT_CFG_ID_SERIAL: u8 = 0x02;
    pub const VIRTIO_INPUT_CFG_ID_DEVIDS: u8 = 0x03;
    pub const VIRTIO_INPUT_CFG_PROP_BITS: u8 = 0x10;
    pub const VIRTIO_INPUT_CFG_EV_BITS: u8 = 0x11;
    pub const VIRTIO_INPUT_CFG_ABS_INFO: u8 = 0x12;

    // sizeof(struct virtio_input_config)
    pub const VIRTIO_INPUT_CFG_SIZE: usize = 8 + 128;

    // linux/input.h
    pub(super) const BUS_PCI: u16 = 1;

    // linux/input-event-codes.h
    pub(super) const INPUT_PROP_POINTER: u16 = 0; // needs a pointer
    pub(super) const INPUT_PROP_DIRECT: u16 = 1; // direct input devices
    pub(super) const EV_KEY: u16 = 1;
    pub(super) const EV_REL: u16 = 2;
    pub(super) const EV_ABS: u16 = 3;
    pub(super) const BTN_LEFT: u16 = 0x110;
    pub(super) const BTN_RIGHT: u16 = 0x111;
    pub(super) const BTN_MIDDLE: u16 = 0x112;
    pub(super) const BTN_FORWARD: u16 = 0x115;
    pub(super) const BTN_BACK: u16 = 0x116;
    pub(super) const BTN_TASK: u16 = 0x117;
    pub(super) const REL_WHEEL: u16 = 8;
    pub(super) const ABS_X: u16 = 0;
    pub(super) const ABS_Y: u16 = 1;
}

#[usdt::provider(provider = "propolis")]
mod probes {
    fn vioinput_cfg_read(sel: u8, subsel: u8) {}
    fn vioinput_cfg_write(sel: u8, subsel: u8) {}
}

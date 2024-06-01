// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::num::NonZeroU16;
use std::sync::Arc;

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

use futures::future::BoxFuture;
use lazy_static::lazy_static;

/// Device IDs used when VIRTIO_INPUT_CFG_ID_DEVIDS is queried.
/// (virtio 1.3 sect 5.8.4)
#[repr(C, packed)]
struct VirtioInputDevIds {
    bus_type: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

/// Information about the value and range of the axis specified by subselect
/// when VIRTIO_INPUT_CFG_ABS_INFO is queried.
/// (virtio 1.3 sect 5.8.4)
#[repr(C, packed)]
struct VirtioInputAbsInfo {
    min: u32,
    max: u32,
    fuzz: u32,
    flat: u32,
    res: u32,
}

pub enum VirtioInputDev {
    Tablet(VirtioTablet),
}

pub struct VirtioTablet {
    event_recv: !,
}

trait VirtioInputDevice {
    fn name(&self) -> &str;
    fn dev_ids(&self) -> VirtioInputDevIds;
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
            product: 0, // TODO
            version: 0,
        }
    }
    fn abs_info(&self, axis: u8) -> VirtioInputAbsInfo {
        VirtioInputAbsInfo { min: 0, max: 0xFF, fuzz: 3, flat: 69, res: 0 }
    }
}

pub struct PciVirtioInput {
    virtio_state: PciVirtioState,
    pci_state: pci::DeviceState,
    select: u8,
    subselect: u8,
    device: Box<dyn VirtioInputDevice>,
    // TODO: proper wrapping for device
}
impl PciVirtioInput {
    pub fn new(
        queue_size: u16,
        device: Box<dyn VirtioInputDevice>,
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
            select: VIRTIO_INPUT_CFG_UNSET,
            subselect: 0,
            device,
        })
    }

    fn input_cfg_read(&self, id: &InputReg, ro: &mut ReadOp) {
        match id {
            InputReg::Select => ro.write_u8(self.select),
            InputReg::Subselect => ro.write_u8(self.subselect),
            InputReg::Size => ro.write_u8(self.selected_payload().len() as u8),
            InputReg::Reserved => ro.fill(0),
            InputReg::Payload => ro.write_bytes(self.selected_payload()),
        }
    }

    fn selected_payload(&self) -> &[u8] {
        match self.select {
            VIRTIO_INPUT_CFG_UNSET => &[],
            VIRTIO_INPUT_CFG_ID_NAME => {
                if self.subselect == 0 {
                    self.device.name()
                } else {
                    // must be size zero if unsupported select and subsel combination
                    // (virtio 1.3 sect 5.8.5.2)
                    &[]
                }
            }
            VIRTIO_INPUT_CFG_ID_SERIAL => b"",
            // safety: reinterpreting a struct that's Copy and repr(C, packed)
            VIRTIO_INPUT_CFG_ID_DEVIDS => unsafe {
                std::slice::from_raw_parts(
                    &self.device.dev_ids() as *const VirtioInputDevIds
                        as *const u8,
                    core::mem::size_of::<VirtioInputDevIds>(),
                )
            },
            VIRTIO_INPUT_CFG_PROP_BITS => {
                if self.subselect == 0 {
                    &[0u8; 128]
                } else {
                    &[]
                }
            }
            VIRTIO_INPUT_CFG_EV_BITS => {
                &[] // TODO
            }
            // safety: reinterpreting a struct that's Copy and repr(C, packed)
            VIRTIO_INPUT_CFG_ABS_INFO => unsafe {
                std::slice::from_raw_parts(
                    &self.device.abs_info(self.subselect)
                        as *const VirtioInputAbsInfo
                        as *const u8,
                    core::mem::size_of::<VirtioInputAbsInfo>(),
                )
            },
            _ => &[],
        }
    }

    fn input_cfg_write(&self, id: &InputReg, wo: &mut WriteOp) {
        match id {
            InputReg::Select => self.select = wo.read_u8(),
            InputReg::Subselect => self.subselect = wo.read_u8(),
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
    fn pause(&self) {
        todo!("pause")
    }
    fn resume(&self) {
        todo!("resume")
    }
    fn paused(&self) -> BoxFuture<'static, ()> {
        Box::pin(todo!())
    }
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
}

#[usdt::provider(provider = "propolis")]
mod probes {
    // TODO
}

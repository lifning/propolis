// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    hw::usb::xhci::rings::consumer::transfer::PointerOrImmediate, vmm::MemCtx,
};

use super::{
    descriptor::*,
    endpoint::{
        control::{ControlRequestInfo, NoClassRequestInfo},
        Endpoint, InAndOut,
    },
    probes,
    requests::{RequestDirection, SetupData},
    Error, Result,
};

/// This is a hard-coded faux-device that purely exists to test the xHCI implementation.
#[derive(Default)]
pub struct NullUsbDevice {
    control_endpoint:
        Endpoint<ControlRequestInfo<NoClassRequestInfo>, InAndOut>,
}

impl NullUsbDevice {
    const MANUFACTURER_NAME_INDEX: StringIndex = StringIndex(1);
    const PRODUCT_NAME_INDEX: StringIndex = StringIndex(2);
    const SERIAL_INDEX: StringIndex = StringIndex(3);
    const CONFIG_NAME_INDEX: StringIndex = StringIndex(4);
    const INTERFACE_NAME_INDEX: StringIndex = StringIndex(5);

    fn device_descriptor() -> DeviceDescriptor {
        DeviceDescriptor {
            usb_version: USB_VER_2_0,
            device_class: ClassCode(0),
            device_subclass: SubclassCode(0),
            device_protocol: ProtocolCode(0),
            max_packet_size_0: MaxSizeZeroEP::_64,
            vendor_id: VendorId(0x1de),
            product_id: ProductId(0xdead),
            device_version: Bcd16(0),
            manufacturer_name: Self::MANUFACTURER_NAME_INDEX,
            product_name: Self::PRODUCT_NAME_INDEX,
            serial: Self::SERIAL_INDEX,
            configurations: vec![Self::config_descriptor()],
            specific_augmentations: vec![],
        }
    }
    fn config_descriptor() -> ConfigurationDescriptor {
        ConfigurationDescriptor {
            interfaces: vec![Self::interface_descriptor()],
            config_value: ConfigurationValue(0),
            configuration_name: Self::CONFIG_NAME_INDEX,
            attributes: ConfigurationAttributes::default(),
            specific_augmentations: vec![],
        }
    }
    fn interface_descriptor() -> InterfaceDescriptor {
        InterfaceDescriptor {
            interface_num: 0,
            alternate_setting: 0,
            endpoints: vec![Self::endpoint_descriptor()],
            class: InterfaceClass(0),
            subclass: InterfaceSubclass(0),
            protocol: InterfaceProtocol(0),
            interface_name: Self::INTERFACE_NAME_INDEX,
            specific_augmentations: vec![],
        }
    }
    fn endpoint_descriptor() -> EndpointDescriptor {
        EndpointDescriptor {
            endpoint_addr: 1,
            direction: None,
            attributes: EndpointAttributes::default(),
            max_packet_size: 64,
            interval: 1,
            specific_augmentations: vec![],
        }
    }
    fn string_descriptor(idx: u8) -> StringDescriptor {
        let s: &str = match StringIndex(idx) {
            Self::MANUFACTURER_NAME_INDEX => "Oxide Computer Company",
            Self::PRODUCT_NAME_INDEX => "Generic USB 2.0 Encabulator",
            Self::SERIAL_INDEX => "9001",
            Self::CONFIG_NAME_INDEX => "MyCoolConfiguration",
            Self::INTERFACE_NAME_INDEX => "MyNotQuiteAsCoolInterface",
            _ => "weird index but ok",
        };
        StringDescriptor { string: s.to_string() }
    }
    fn device_qualifier_descriptor() -> DeviceQualifierDescriptor {
        DeviceQualifierDescriptor {
            usb_version: USB_VER_2_0,
            device_class: ClassCode(0),
            device_subclass: SubclassCode(0),
            device_protocol: ProtocolCode(0),
            max_packet_size_0: MaxSizeZeroEP::_64,
            num_configurations: 0,
        }
    }

    pub fn setup_stage(
        &mut self,
        endpoint_id: u8,
        setup: SetupData,
    ) -> Result<()> {
        if endpoint_id != 1 {
            return Err(Error::InvalidEndpoint(endpoint_id));
        }
        if let Some(req) = self.control_endpoint.setup_stage(setup)? {
            let payload = self.payload_for(req)?;
            self.control_endpoint.set_payload(payload)?;
        }
        Ok(())
    }

    pub fn data_stage(
        &mut self,
        endpoint_id: u8,
        data_buffer: PointerOrImmediate,
        data_direction: RequestDirection,
        memctx: &MemCtx,
    ) -> Result<usize> {
        if endpoint_id != 1 {
            return Err(Error::InvalidEndpoint(endpoint_id));
        }
        self.control_endpoint.data_stage(data_buffer, data_direction, memctx)
    }

    pub fn status_stage(
        &mut self,
        endpoint_id: u8,
        status_direction: RequestDirection,
    ) -> Result<()> {
        if endpoint_id != 1 {
            return Err(Error::InvalidEndpoint(endpoint_id));
        }
        match self.control_endpoint.status_stage(status_direction)? {
            Some((req, _payload)) => match req {
                ControlRequestInfo::SetConfiguration { configuration: _ } => {
                    // TODO: check config value
                    Ok(())
                }
                x => Err(Error::UnimplementedRequestBehavior(format!("{x:?}"))),
            },
            None => Ok(()),
        }
    }

    fn payload_for(
        &self,
        req: ControlRequestInfo<NoClassRequestInfo>,
    ) -> Result<Vec<u8>> {
        Ok(match req {
            ControlRequestInfo::GetDescriptor { descriptor_type, index } => {
                let descriptor: Box<dyn Descriptor> = match descriptor_type {
                    DescriptorType::Device => {
                        Box::new(Self::device_descriptor())
                    }
                    DescriptorType::Configuration => {
                        Box::new(Self::config_descriptor())
                    }
                    DescriptorType::String => {
                        Box::new(Self::string_descriptor(index))
                    }
                    DescriptorType::DeviceQualifier => {
                        Box::new(Self::device_qualifier_descriptor())
                    }
                    x => return Err(Error::UnimplementedDescriptor(x)),
                };
                probes::usb_get_descriptor!(|| (descriptor_type as u8, index));
                // slog::debug!(log, "usb: GET_DESCRIPTOR({descriptor:?})");
                descriptor.serialize().collect()
            }
            ControlRequestInfo::GetStatus => {
                // USB 2.0 sect 9.4.5 - two-byte response where lowest-order
                // bits are 'self powered' and 'remote wakeup'
                let attrib = Self::config_descriptor().attributes;
                (attrib.self_powered() as u16
                    | (attrib.remote_wakeup() as u16 * 2))
                    .to_le_bytes()
                    .to_vec()
            }
            x => {
                return Err(Error::UnimplementedRequestBehavior(format!(
                    "{x:?}"
                )))
            }
        })
    }

    pub fn import(
        &mut self,
        value: &super::migrate::UsbDeviceV1,
    ) -> core::result::Result<(), crate::migrate::MigrateStateError> {
        let super::migrate::UsbDeviceV1 { device_type, endpoints } = value;
        if *device_type != super::migrate::UsbDeviceTypeV1::Null {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                format!("USB device type mismatch {device_type:?} != Null"),
            ));
        }
        if let Some(ep) = endpoints.get(&0) {
            self.control_endpoint.import(ep)?;
        } else {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                format!("USB endpoint 0 missing"),
            ));
        }
        Ok(())
    }

    pub fn export(&self) -> super::migrate::UsbDeviceV1 {
        super::migrate::UsbDeviceV1 {
            device_type: super::migrate::UsbDeviceTypeV1::Null,
            endpoints: [(0, self.control_endpoint.export())]
                .into_iter()
                .collect(),
        }
    }
}

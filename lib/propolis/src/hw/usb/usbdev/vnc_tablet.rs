// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    accessors::MemAccessor,
    hw::usb::xhci::rings::consumer::transfer::PointerOrImmediate, vmm::MemCtx,
};

use super::{
    descriptor::*,
    endpoint::{control::ControlRequestInfo, Endpoint, InAndOut},
    hid::{report::*, *},
    probes,
    requests::{RequestDirection, SetupData},
    Error, Result,
};

pub struct HidTabletUsbDevice {
    control_endpoint: Endpoint<ControlRequestInfo<HIDRequestInfo>, InAndOut>,
    idle_duration_4ms: u8,
    acc_mem: MemAccessor,
}

impl HidTabletUsbDevice {
    const MANUFACTURER_NAME_INDEX: StringIndex = StringIndex(1);
    const PRODUCT_NAME_INDEX: StringIndex = StringIndex(2);
    const SERIAL_INDEX: StringIndex = StringIndex(3);
    const CONFIG_NAME_INDEX: StringIndex = StringIndex(4);
    const INTERFACE_NAME_INDEX: StringIndex = StringIndex(5);

    pub fn new(acc_mem: &MemAccessor) -> Self {
        let acc_mem = acc_mem.child(Some(format!("USB Tablet")));
        Self {
            control_endpoint: Default::default(),
            idle_duration_4ms: 0,
            acc_mem,
        }
    }

    fn device_descriptor() -> DeviceDescriptor {
        DeviceDescriptor {
            usb_version: USB_VER_2_0,
            // NOTE: HID class is specified in the *interface* descriptor
            device_class: ClassCode(0),
            device_subclass: SubclassCode(0),
            device_protocol: ProtocolCode(0),
            max_packet_size_0: MaxSizeZeroEP::_64,
            vendor_id: VendorId(0x1de),
            product_id: ProductId(0x7ab1),
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
            config_value: ConfigurationValue(1),
            configuration_name: Self::CONFIG_NAME_INDEX,
            attributes: ConfigurationAttributes::default(),
            specific_augmentations: vec![],
        }
    }
    fn interface_descriptor() -> InterfaceDescriptor {
        InterfaceDescriptor {
            interface_num: 1,
            alternate_setting: 0,
            endpoints: vec![Self::interrupt_in_endpoint_descriptor()],
            class: InterfaceClass::HID,
            subclass: InterfaceSubclass(0), // no boot interface support
            protocol: InterfaceProtocol(0), // no boot interface support
            interface_name: Self::INTERFACE_NAME_INDEX,
            specific_augmentations: vec![AugmentedDescriptor::HID(
                HIDDescriptor {
                    hid_version: HID_VER_1_11,
                    country_code: CountryCode::International,
                    class_descriptor: vec![Self::report_descriptor()],
                },
            )],
        }
    }
    fn interrupt_in_endpoint_descriptor() -> EndpointDescriptor {
        EndpointDescriptor {
            endpoint_addr: 1,
            direction: Some(RequestDirection::DeviceToHost),
            attributes: EndpointAttributes::default()
                .with_transfer_type(EndpointTransferType::Interrupt),
            max_packet_size: 64,
            interval: 1,
            specific_augmentations: vec![],
        }
    }
    fn string_descriptor(idx: u8) -> StringDescriptor {
        let s: &str = match StringIndex(idx) {
            Self::MANUFACTURER_NAME_INDEX => "Oxide Computer Company",
            Self::PRODUCT_NAME_INDEX => "Absolute Mouse",
            Self::SERIAL_INDEX => "9002",
            Self::CONFIG_NAME_INDEX => "Absolute Mouse Configuration",
            Self::INTERFACE_NAME_INDEX => "Absolute Mouse Interface",
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

    // might be nice to generate this from some nicer builder-pattern thing someday
    // i.e. ReptDesc::new(Mouse).with_buttons(7).with_axes([X, Y], 0..=0x8000, Absolute)
    pub fn report_descriptor() -> ReportDescriptor {
        ReportDescriptor::new(
            HIDReportType::Input,
            vec![
                UsagePage::GenericDesktopControls.item(),
                GenericDesktopUsage::Mouse.item(),
                Collection::Application.items([
                    // ItemTag::ReportID.one_byte(1),
                    GenericDesktopUsage::Pointer.item(),
                    Collection::Physical.items([
                        UsagePage::Button.item(),
                        // VNC mouse button reports are in the form of a one-byte
                        // bitfield, with bit 0 representing 'disabled', so 1..=7
                        // (HID Button values start at 1 for "primary")
                        ItemTag::UsageMinimum.one_byte(1),
                        ItemTag::UsageMaximum.one_byte(7),
                        ItemTag::LogicalMinimum.one_byte(0),
                        ItemTag::LogicalMaximum.one_byte(1),
                        // 7 buttons to report...
                        ItemTag::ReportCount.one_byte(7),
                        ItemTag::ReportSize.one_byte(1),
                        // 1 bit each
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(false) // absolute
                            .input(),
                        // 1 bit padding to round out the byte in the report
                        // (similar to Mouse example in HID 1.11 sect E.10)
                        ItemTag::ReportCount.one_byte(1),
                        ItemTag::ReportSize.one_byte(1),
                        InputOutputFeatureItem(0).with_constant(true).input(),
                        UsagePage::GenericDesktopControls.item(),
                        GenericDesktopUsage::X.item(),
                        GenericDesktopUsage::Y.item(),
                        ItemTag::LogicalMinimum.one_byte(0),
                        ItemTag::LogicalMaximum.two_byte(0x8000u32),
                        // two axes, 16-bits each
                        ItemTag::ReportSize.one_byte(16),
                        ItemTag::ReportCount.one_byte(2),
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(false) // absolute
                            .input(),
                    ]),
                ]),
                /* if absolute-mouse doesn't work, maybe we use Digitizer page
                Part::Item(
                    ItemTag::UsagePage.one_byte(),
                    UsagePage::Digitizers as u32,
                ),
                Part::Item(
                    ItemTag::Usage.one_byte(),
                    DigitizerUsage::Digitizer as u32,
                ),
                Part::Collection(
                    Collection::Application,
                    vec![
                        // TODO report id 2, 3
                    ],
                ),
                */
            ],
        )
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
            eprintln!("in {endpoint_id}: {req:?}");
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
            Some((req, _payload)) => {
                eprintln!("out {endpoint_id}: {req:?}");
                match req {
                    ControlRequestInfo::SetConfiguration {
                        configuration: _,
                    } => {
                        // TODO: check config value
                        Ok(())
                    }
                    ControlRequestInfo::Class(HIDRequestInfo::SetIdle {
                        duration_4ms,
                        report_id: _,
                        interface: _,
                    }) => {
                        self.idle_duration_4ms = duration_4ms;
                        Ok(())
                    }
                    x => Err(Error::UnimplementedRequestBehavior(format!(
                        "{x:?}"
                    ))),
                }
            }
            None => Ok(()),
        }
    }

    fn payload_for(
        &self,
        req: ControlRequestInfo<HIDRequestInfo>,
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
                    DescriptorType::Report => {
                        Box::new(Self::report_descriptor())
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
            ControlRequestInfo::Class(HIDRequestInfo::GetIdle {
                report_id: _,
                interface: _,
            }) => {
                vec![self.idle_duration_4ms]
            }
            x => {
                return Err(Error::UnimplementedRequestBehavior(format!(
                    "{x:?}"
                )))
            }
        })
    }

    pub fn normal(
        &self,
        endpoint_id: u8,
        data_buffer: PointerOrImmediate,
    ) -> Result<()> {
        eprintln!("normal {endpoint_id}: {data_buffer:#x?}");
        if let PointerOrImmediate::Pointer(range) = data_buffer {
            // TODO: store range and write to it later
            self.acc_mem
                .access()
                .unwrap()
                .write_many(range.0, &vec![0u8; range.1]);
            Ok(())
        } else {
            Err(Error::ImmediateParameterForInTransfer)
        }
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

#[cfg(test)]
mod test {
    use crate::hw::usb::usbdev::descriptor::Descriptor;

    #[rustfmt::skip]
    #[test]
    // dual purpose: verify that ReportDescriptor::serialize() does what we want
    // and trips on attempts to change the report format - which we must not do
    // in a world in which we care about live migration!
    fn tablet_descriptor_serialization() {
        let serialized: Vec<u8> =
            super::HidTabletUsbDevice::report_descriptor().serialize().collect();
        // similar to HID 1.11 sect E.10
        assert_eq!(serialized.as_slice(), &[
            5, 1, // usage page (generic desktop)
            9, 2, // usage (mouse)
            0xA1, 1, // collection (application)
                9, 1, // usage (pointer)
                0xA1, 0, // collection (physical)
                    5, 9, // usage page (buttons)
                    0x19, 1, // usage minimum (1)
                    0x29, 7, // usage maximum (7)
                    0x15, 0, // logical minimum (0)
                    0x25, 1, // logical maximum (1)
                    0x95, 7, // report count (7)
                    0x75, 1, // report size (1)
                    0x81, 2, // input (data, variable, absolute), 7 button bits
                    0x95, 1, // report count (1)
                    0x75, 1, // report size (1)
                    0x81, 1, // input (constant), 1 bit padding
                    5, 1, // usage page (generic desktop)
                    9, 0x30, // usage (x)
                    9, 0x31, // usage (y)
                    0x15, 0, // logical minimum (0)
                    0x26, 0, 0x80, // logical maximum (0x8000)
                    0x75, 0x10, // report size (16)
                    0x95, 2, // report count (2)
                    0x81, 2, // input (data, variable, absolute), 2 position shorts (x & y)
                0xC0, // end collection
            0xC0, // end collection
        ])
    }
}

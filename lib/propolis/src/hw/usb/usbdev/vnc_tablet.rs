// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::{Arc, Condvar, Mutex, Weak};

use bitstruct::bitstruct;
use rfb::proto::{MouseButtons, PointerEvent};
use rgb_frame::Spec;

use crate::{
    hw::{
        ids::usb::{PROPOLIS_USB_TABLET_DEV_ID, VENDOR_OXIDE},
        // XXX: some of this is abstraction leakage while figuring things out
        usb::xhci::{
            bits::device_context::EndpointContext,
            controller::XhciPortWakeHandle,
            device_slots::SlotId,
            port::PortId,
            rings::{
                consumer::transfer::{PointerOrImmediate, TDNormal},
                producer::event::EventInfo,
            },
        },
    },
    vmm::MemCtx,
};

use super::{
    descriptor::*,
    endpoint::{
        control::ControlRequestInfo,
        interrupt::{InterruptInData, InterruptInEndpoint},
        Endpoint, InAndOut,
    },
    hid::{report::*, *},
    probes,
    requests::{RequestDirection, SetupData},
    Error, Result, UsbDevice,
};

const REPORT_SIZE: usize = 7;

#[derive(Default)]
pub struct HIDTabletReport {
    // for control-endpoint Get_Report requests
    last_data: [u8; REPORT_SIZE],
    // XXX: can we lose this
    slot_id: Option<SlotId>,
    // where the unanswered transfer lives
    xfer_dataref: Option<Weak<(Mutex<InterruptInData>, Condvar)>>,
}

bitstruct! {
    struct HIDMouseButtons(pub u8) {
        pub left: bool = 0;
        pub right: bool = 1;
        pub middle: bool = 2;
    }
}

impl HIDTabletReport {
    pub fn pointer_event(&mut self, pe: PointerEvent, spec: Spec) {
        // div: spec.width and spec.height are NonZeroUsize
        let x = (pe.position.x as usize * 0x7fff / spec.width) as u16;
        let y = (pe.position.y as usize * 0x7fff / spec.height) as u16;
        // remap VNC button IDs to HID
        let mouse_left = pe.pressed.intersects(MouseButtons::LEFT);
        let mouse_middle = pe.pressed.intersects(MouseButtons::MIDDLE);
        let mouse_right = pe.pressed.intersects(MouseButtons::RIGHT);
        let scroll_up = pe.pressed.intersects(MouseButtons::SCROLL_A);
        let scroll_down = pe.pressed.intersects(MouseButtons::SCROLL_B);
        let scroll_left = pe.pressed.intersects(MouseButtons::SCROLL_C);
        let scroll_right = pe.pressed.intersects(MouseButtons::SCROLL_D);

        let button_bits = HIDMouseButtons(0)
            .with_left(mouse_left)
            .with_middle(mouse_middle)
            .with_right(mouse_right)
            .0;
        let vert_wheel = scroll_up as i8 - scroll_down as i8;
        let horiz_wheel = scroll_right as i8 - scroll_left as i8;

        let mut data = [0; REPORT_SIZE];
        for (dst, src) in data.iter_mut().zip(
            // would be nice if this filtered through the same construct that
            // generates the ReportDescriptor, should we create such a thing
            [button_bits]
                .into_iter()
                .chain(u16::to_le_bytes(x))
                .chain(u16::to_le_bytes(y))
                .chain([vert_wheel as u8, horiz_wheel as u8]),
        ) {
            *dst = src;
        }
        self.last_data = data;

        if let Some(dataref) = &self.xfer_dataref {
            if let Some(dataref) = dataref.upgrade() {
                dataref.0.lock().unwrap().set_payload(data.to_vec());
                dataref.1.notify_one();
            }
        }
    }

    fn set_ep_data(
        &mut self,
        ep_data: Weak<(Mutex<InterruptInData>, Condvar)>,
    ) {
        self.xfer_dataref = Some(ep_data);
    }
}

pub struct HIDTabletDevice {
    control_endpoint: Endpoint<ControlRequestInfo<HIDRequestInfo>, InAndOut>,
    interrupt_endpoint: Option<InterruptInEndpoint>,
    idle_duration_4ms: u8,
    report: Arc<Mutex<HIDTabletReport>>,
    port_wake_hdl: Arc<XhciPortWakeHandle>,
}

impl HIDTabletDevice {
    const MANUFACTURER_NAME_INDEX: StringIndex = StringIndex(1);
    const PRODUCT_NAME_INDEX: StringIndex = StringIndex(2);
    const SERIAL_INDEX: StringIndex = StringIndex(3);
    const CONFIG_NAME_INDEX: StringIndex = StringIndex(4);
    const INTERFACE_NAME_INDEX: StringIndex = StringIndex(5);

    pub fn new(
        report: Arc<Mutex<HIDTabletReport>>,
        port_wake_hdl: Arc<XhciPortWakeHandle>,
    ) -> Box<Self> {
        Box::new(Self {
            control_endpoint: Default::default(),
            interrupt_endpoint: None,
            idle_duration_4ms: 0,
            report,
            port_wake_hdl,
        })
    }

    fn device_descriptor() -> DeviceDescriptor {
        DeviceDescriptor {
            usb_version: USB_VER_2_0,
            // NOTE: HID class is specified in the *interface* descriptor
            device_class: ClassCode(0),
            device_subclass: SubclassCode(0),
            device_protocol: ProtocolCode(0),
            max_packet_size_0: MaxSizeZeroEP::_64,
            vendor_id: VendorId(VENDOR_OXIDE),
            product_id: ProductId(PROPOLIS_USB_TABLET_DEV_ID),
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
            Self::PRODUCT_NAME_INDEX => "Propolis HID Tablet",
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
            num_configurations: 1,
        }
    }

    // might be nice to generate this from some nicer builder-pattern thing someday
    // i.e. ReptDesc::new(Mouse).with_buttons(5).with_axes([X, Y], 0..=0x7fff, Absolute)
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
                        // HID Button values are, for a righty-mouse mapping,
                        // left, right, middle
                        ItemTag::UsageMinimum.one_byte(1),
                        ItemTag::UsageMaximum.one_byte(5),
                        ItemTag::LogicalMinimum.one_byte(0),
                        ItemTag::LogicalMaximum.one_byte(1),
                        // 5 buttons to report...
                        ItemTag::ReportCount.one_byte(5),
                        ItemTag::ReportSize.one_byte(1),
                        // 1 bit each
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(false) // absolute
                            .input(),
                        // 3 bit padding to round out the byte in the report
                        // (similar to Mouse example in HID 1.11 sect E.10)
                        ItemTag::ReportCount.one_byte(1),
                        ItemTag::ReportSize.one_byte(3),
                        InputOutputFeatureItem(0).with_constant(true).input(),
                        UsagePage::GenericDesktopControls.item(),
                        GenericDesktopUsage::X.item(),
                        GenericDesktopUsage::Y.item(),
                        ItemTag::LogicalMinimum.one_byte(0),
                        ItemTag::LogicalMaximum.two_byte(0x7fffu32),
                        // two axes, 16-bits each
                        ItemTag::ReportSize.one_byte(16),
                        ItemTag::ReportCount.one_byte(2),
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(false) // absolute
                            .input(),
                        // normal vertical scroll wheel
                        UsagePage::GenericDesktopControls.item(),
                        GenericDesktopUsage::Wheel.item(),
                        ItemTag::LogicalMinimum.one_byte(-127i8 as u32),
                        ItemTag::LogicalMaximum.one_byte(127),
                        ItemTag::ReportSize.one_byte(8),
                        ItemTag::ReportCount.one_byte(1),
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(true) // absolute
                            .input(),
                        // horizontal scroll wheel is a bit more obscure
                        UsagePage::Consumer.item(),
                        ConsumerUsage::ACPan.item(),
                        ItemTag::LogicalMinimum.one_byte(-127i8 as u32),
                        ItemTag::LogicalMaximum.one_byte(127),
                        ItemTag::ReportSize.one_byte(8),
                        ItemTag::ReportCount.one_byte(1),
                        InputOutputFeatureItem(0)
                            .with_constant(false) // data
                            .with_variable(true) // variable
                            .with_relative(true) // absolute
                            .input(),
                    ]),
                ]),
            ],
        )
    }

    // TODO: not alloc unnecessarily
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
            ControlRequestInfo::Class(HIDRequestInfo::GetReport {
                report_type: HIDReportType::Input,
                report_id: _,
                interface: _,
            }) => {
                let report = self.report.lock().unwrap();
                report.last_data.to_vec()
            }
            x => {
                return Err(Error::UnimplementedRequestBehavior(format!(
                    "{x:?}"
                )))
            }
        })
    }
}

impl UsbDevice for HIDTabletDevice {
    fn setup_stage(&mut self, endpoint_id: u8, setup: SetupData) -> Result<()> {
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

    fn data_stage(
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

    fn configure_endpoint(
        &mut self,
        endpoint_id: u8,
        ep_ctx: &EndpointContext,
    ) {
        if endpoint_id == 3 {
            let interrupt_in_endpoint = InterruptInEndpoint::new(
                ep_ctx.interval_as_duration(),
                Arc::downgrade(&self.port_wake_hdl),
            );
            // XXX ugly
            self.report
                .lock()
                .unwrap()
                .set_ep_data(interrupt_in_endpoint.data_ref());
            self.interrupt_endpoint = Some(interrupt_in_endpoint);
        } else {
            eprintln!("wat");
        }
    }

    fn normal(
        &mut self,
        slot_id: SlotId,
        endpoint_id: u8,
        normal_td: TDNormal,
    ) -> Result<Option<EventInfo>> {
        // eprintln!("normal {endpoint_id}: {normal_td:x?}");
        if let Some(ep) = &self.interrupt_endpoint {
            ep.normal(slot_id, endpoint_id, normal_td);
        }
        Ok(None)
    }

    fn status_stage(
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
                        // TODO: error if not 0
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

    fn set_address(&self, slot_id: SlotId, _port_id: PortId) {
        self.report.lock().unwrap().slot_id = Some(slot_id);
    }

    fn import(
        &mut self,
        value: &super::migrate::UsbDeviceV1,
    ) -> core::result::Result<(), crate::migrate::MigrateStateError> {
        let super::migrate::UsbDeviceV1 { device_type, endpoints } = value;
        let super::migrate::UsbDeviceTypeV1::Tablet(..) = device_type else {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                format!("USB device type mismatch {device_type:?} != Tablet"),
            ));
        };
        if let Some(ep) = endpoints.get(&0) {
            self.control_endpoint.import(ep)?;
        } else {
            return Err(crate::migrate::MigrateStateError::ImportFailed(
                format!("USB endpoint 0 missing"),
            ));
        }
        Ok(())
    }

    fn export(
        &self,
    ) -> core::result::Result<
        super::migrate::UsbDeviceV1,
        crate::migrate::MigrateStateError,
    > {
        Ok(super::migrate::UsbDeviceV1 {
            device_type: super::migrate::UsbDeviceTypeV1::Null,
            endpoints: [(0, self.control_endpoint.export())]
                .into_iter()
                .collect(),
        })
    }
}

pub mod migrate {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct TabletDeviceV1 {}
}

#[cfg(test)]
mod test {
    use crate::hw::usb::usbdev::descriptor::Descriptor;

    #[rustfmt::skip]
    #[test]
    // dual purpose: verify that ReportDescriptor::serialize() does what we want
    // and trips on attempts to change the report format for this device in
    // particular - which we must not do since we care about live migration!
    fn tablet_descriptor_serialization() {
        let serialized: Vec<u8> =
            super::HIDTabletDevice::report_descriptor().serialize().collect();
        // similar to HID 1.11 sect E.10, but absolute x/y, and with scroll wheels
         assert_eq!(serialized.as_slice(), &[
            5, 1, // usage page (generic desktop)
            9, 2, // usage (mouse)
            0xA1, 1, // collection (application)
                9, 1, // usage (pointer)
                0xA1, 0, // collection (physical)
                    5, 9, // usage page (buttons)
                        0x19, 1, // usage minimum (1)
                        0x29, 5, // usage maximum (5)
                        0x15, 0, // logical minimum (0)
                        0x25, 1, // logical maximum (1)
                        0x95, 5, // report count (5)
                        0x75, 1, // report size (1)
                        0x81, 2, // input (data, variable, absolute), 5 button bits
                        0x95, 1, // report count (1)
                        0x75, 3, // report size (3)
                        0x81, 1, // input (constant), 3 bit padding
                    5, 1, // usage page (generic desktop)
                        9, 0x30, // usage (x)
                        9, 0x31, // usage (y)
                        0x15, 0, // logical minimum (0)
                        0x26, 0xff, 0x7f, // logical maximum (0x7fff)
                        0x75, 0x10, // report size (16)
                        0x95, 2, // report count (2)
                        0x81, 2, // input (data, variable, absolute), 2 position shorts (x & y)
                    5, 1, // usage page (generic desktop)
                        9, 0x38, // usage (wheel)
                        0x15, 0x81, // logical minimum (-127)
                        0x25, 0x7f, // logical maximum (127)
                        0x75, 8, // report size (8)
                        0x95, 1, // report count (1)
                        0x81, 6, // input (data, variable, relative)
                    5, 0x0C, // usage page (consumer devices)
                        0x0A, 0x38, 0x02, // usage (application controls: pan)
                        0x15, 0x81, // logical minimum (-127)
                        0x25, 0x7f, // logical maximum (127)
                        0x75, 8, // report size (8)
                        0x95, 1, // report count (1)
                        0x81, 6, // input (data, variable, relative)
                0xC0, // end collection
            0xC0, // end collection
        ])
    }
}

use crate::hw::usb;

pub struct HidTablet {
    devinfo: usb::DeviceInfo,
}

impl usb::Device for HidTablet {
    fn device_info(&self) -> &DeviceInfo {
        &self.devinfo
    }

    fn poll_interrupt(&self) -> Option<usb::Interrupt> {
        match self.vnc_pointer_channel.recv() {
            Some(event) => {
                Some(usb::Interrupt { /* ... */ })
            }
            None => None,
        }
    }

    fn handle_request(&self, req: usb::Request) -> Response {
        todo!()
    }
}

impl HidTablet {
    fn new() -> Self {
        let en_us = |s: &str| {
            [(usb::LanguageId::EnglishUS, s.to_string())].into_iter().collect()
        };
        Self {
            // ...
            devinfo: usb::DeviceInfo {
                manufacturer_name: en_us("Oxide Computer Company"),
                product_name: en_us("Virtual HID Tablet"),
                serial_number: "1337".to_string(),
                usb_version: usb::USB_VER_1_0,
                device_version: Bcd16::from_raw(0x0100u16),
                vendor_id: usb::VendorId(VENDOR_OXIDE),
                product_id: usb::ProductId(0x7AB1u16),
                // (note: for HID, class is provided in the InterfaceDescriptor,
                // rather than the DeviceDescriptor; do the same here)
                class: usb::ClassCode::None,
                subclass: usb::SubclassCode::None,
                configurations: [(
                    usb::ConfigValue(0u8),
                    ConfigurationInfo {
                        // second value in InterfaceNum is bAlternateSetting
                        interfaces: [(
                            usb::InterfaceNum(0u8, 0u8),
                            InterfaceInfo {
                                endpoints: vec![EndpointInfo {
                                    direction: usb::EndpointDir::In,
                                    transfer_type: usb::TransferType::Interrupt,
                                    interval: 10u8,
                                }],
                                class:
                                    usb::InterfaceClass::HumanInterfaceDevice,
                                subclass: usb::InterfaceSubclass::None,
                                protocol: usb::InterfaceProtocol::None,
                                description: en_us("Absolute-position pointer"),
                            },
                        )]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                )]
                .into_iter()
                .collect(),
                specific_descriptors: vec![
                    // in an enum so we know where to insert it in the response to get device descriptor
                    ClassSpecificDescriptor::HumanInterfaceDevice(
                        HidDescriptor::new_tablet(),
                    ),
                ],
            },
        }
    }
}

//! USB Emulation

use std::collections::HashMap;

use descriptors::*;
use value_types::*;

pub mod usbdev;
pub mod xhci;

pub mod value_types {
    use std::{convert::Infallible, string::FromUtf16Error};

    pub struct Bcd16(u16);
    impl Bcd16 {
        pub fn from_raw(hex: u16) -> Self {
            Self(hex)
        }
        pub fn try_from_decimal(
            dec: u16,
        ) -> Result<Self, std::num::IntErrorKind> {
            if dec > 10_000 {
                Err(std::num::IntErrorKind::PosOverflow)
            } else {
                let ones = dec % 10;
                let tens = (dec / 10) % 10;
                let hundreds = (dec / 100) % 10;
                let thousands = dec / 1000;
                Ok(Self(
                    ones | (tens << 4) | (hundreds << 8) | (thousands << 12),
                ))
            }
        }
    }

    pub struct DescriptorType(pub u8);
    pub struct ClassCode(pub u8);
    pub struct SubclassCode(pub u8);
    pub struct ProtocolCode(pub u8);
    pub struct InterfaceClass(pub u8);
    pub struct InterfaceSubclass(pub u8);
    pub struct InterfaceProtocol(pub u8);

    #[repr(u8)]
    pub enum MaxSizeZeroEP {
        _8 = 8,
        _16 = 16,
        _32 = 32,
        _64 = 64,
    }
    pub struct VendorId(pub u16);
    pub struct ProductId(pub u16);
    pub struct StringIndex(pub u8);
    pub struct ConfigurationValue(pub u8);

    /// USB Language Identifiers, version 1.0.
    /// (see also Universal Serial Bus HID Usage Tables, section 3.6: HID LANGIDs)
    #[repr(u16)]
    #[derive(PartialEq, Eq)]
    pub enum LanguageId {
        EnglishUS = 0x409,
        HIDUsageDataDescriptor = 0x0FF | (0x01 << 10),
        HIDVendor1 = 0x0FF | (0x3c << 10),
        HIDVendor2 = 0x0FF | (0x3d << 10),
        HIDVendor3 = 0x0FF | (0x3e << 10),
        HIDVendor4 = 0x0FF | (0x3f << 10),
    }

    pub struct Bitmap8(u8);
    impl Bitmap8 {
        // TODO
    }

    pub struct CountryCode(pub u8);

    pub struct Utf16String(Vec<u16>);

    impl TryInto<String> for &Utf16String {
        type Error = FromUtf16Error;
        fn try_into(self) -> Result<String, Self::Error> {
            // TODO: specifically LE https://github.com/rust-lang/rust/issues/116258
            String::from_utf16(&self.0)
        }
    }
    impl std::str::FromStr for Utf16String {
        type Err = Infallible;
        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Ok(Self(value.encode_utf16().collect()))
        }
    }

    pub enum EndpointDir {
        In,
        Out,
    }
}

struct DeviceInfo {
    pub manufacturer_name: HashMap<LanguageId, String>,
    pub product_name: HashMap<LanguageId, String>,
    pub serial_number: String,
    pub usb_version: Bcd16,
    pub device_version: Bcd16,
    pub vendor_id: VendorId,
    pub product_id: ProductId,
    pub class: ClassCode,
    pub subclass: SubclassCode,
    pub configurations: HashMap<ConfigurationValue, ConfigurationInfo>,
    pub specific_descriptors: Vec<ClassSpecificDescriptor>,
}

impl Into<DeviceDescriptor> for &DeviceInfo {
    fn into(self) -> DeviceDescriptor {
        todo!()
    }
}

struct ConfigurationInfo {
    pub interfaces: HashMap<InterfaceNum, InterfaceInfo>,
    pub bus_powered: bool,
    pub self_powered: bool,
    pub remote_wakeup: bool,
    pub max_power: u8,
}

struct InterfaceInfo {
    pub alternate_setting: u8,
    pub endpoints: Vec<EndpointInfo>,
    pub class: InterfaceClass,
    pub subclass: InterfaceSubclass,
    pub protocol: InterfaceProtocol,
    pub description: HashMap<LanguageId, String>,
}

struct EndpointInfo {
    pub direction: EndpointDir,
    pub transfer_type: TransferType,
    pub interval: u8,
}

enum ClassSpecificDescriptor {
    Hid(HidDescriptor),
}

trait Device {
    fn device_info(&self) -> &DeviceInfo;
    // default trait impl of `get_descriptor` can be overridden,
    // if a device has a specific need to
    fn get_descriptor(&self, typ: GetDescriptorType) -> Box<dyn Descriptor> {
        match typ {
            GetDescriptorType::Device => {
                Box::new(DeviceDescriptor::from(self.device_info()))
            }
            GetDescriptorType::String(index, language) => match index {
                MANUFACTURER => Box::new(StringDescriptor::from(
                    self.device_info.manufacturer_name.get(language),
                )),
                // ...
            }, // ...
        }
    }
    fn handle_request(&self, req: Request) -> Response;
    fn poll_interrupt(&self) -> Option<Interrupt> {
        None
    }
}

// TODO: del
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod descriptors {
    use super::value_types::*;

    pub trait Descriptor {
        fn descriptor_type(&self) -> DescriptorType;
        fn serialize(&self) -> () {
            todo!()
        }
    }
    impl Descriptor for DeviceDescriptor {
        // const bLength: u8 = 18;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(1u8)
        }
    }

    pub struct DeviceDescriptor {
        bcdUSB: Bcd16,                  // u16
        bDeviceClass: ClassCode,        // u8
        bDeviceSubClass: SubclassCode,  // u8
        bDeviceProtocol: ProtocolCode,  // u8
        bMaxPacketSize0: MaxSizeZeroEP, // repr(u8) enum. values: 8, 16, 32, 64
        idVendor: VendorId,             // u16
        idProduct: ProductId,           // u16
        bcdDevice: Bcd16,               // u16
        iManufacturer: StringIndex,     // u8
        iProduct: StringIndex,          // u8
        iSerial: StringIndex,           // u8

        configurations: Vec<ConfigurationDescriptor>, // .len() as u8 = bNumConfigurations
        // class- or vendor-
        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for ConfigurationDescriptor {
        // const bLength: u8 = 9;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(2u8)
        }
    }
    pub struct ConfigurationDescriptor {
        // wTotalLength (u16) is calculated based on serialization of:
        interfaces: Vec<InterfaceDescriptor>, // .len() as u8 = bNumInterfaces

        bConfigurationValue: ConfigurationValue, // u8
        iConfiguration: StringIndex,             // u8
        bmAttributes: Bitmap8,                   // u8

        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for InterfaceDescriptor {
        // const bLength: u8 = 9;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(4u8)
        }
    }

    pub struct InterfaceDescriptor {
        bInterfaceNumber: u8,
        bAlternateSetting: u8,

        endpoints: Vec<EndpointDescriptor>, // .len() as u8 = bNumEndpoints

        bInterfaceClass: InterfaceClass, // u8
        bInterfaceSubClass: InterfaceSubclass, // u8
        bInterfaceProtocol: InterfaceProtocol, // u8,
        iInterface: StringIndex,         // u8

        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for HidDescriptor {
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(33u8)
        }
    }

    pub struct HidDescriptor {
        bLength: u8,               // dependent on bNumDescriptors
        bcdHID: Bcd16,             // u16
        bCountryCode: CountryCode, // u8

        class_descriptor: Vec<Box<dyn Descriptor>>, // .len() as u8 = bNumDescriptors
                                                    // followed by bDescriptorType (u8), wDescriptorLength (u16) for each at serialization time.
    }

    impl Descriptor for EndpointDescriptor {
        // const bLength: u8 = 7;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(5u8)
        }
    }
    pub struct EndpointDescriptor {
        bEndpointAddress: u8,
        bmAttributes: Bitmap8, // u8
        wMaxPacketSize: u16,
        bInterval: u8,

        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for StringDescriptor {
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(3u8)
        }
    }

    pub struct StringDescriptor {
        bLength: u8,
        bString: Utf16String,
    }

    impl Descriptor for StringLanguageIdentifierDescriptor {
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(3u8)
        }
    }

    // special-case for GET_DESCRIPTOR(String, 0)
    struct StringLanguageIdentifierDescriptor {
        bLength: u8,
        wLANGID: Vec<LanguageId>, // [u16]
    }
}

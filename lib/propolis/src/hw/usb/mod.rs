//! USB Emulation

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use descriptors::*;
use value_types::*;

pub mod usbdev;
pub mod xhci;

pub mod value_types {
    use bitstruct::bitstruct;
    use strum::FromRepr;

    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
        pub fn into_decimal(&self) -> u16 {
            let ones = self.0 & 0xF;
            let tens = (self.0 >> 4) & 0xF;
            let hundreds = (self.0 >> 8) & 0xF;
            let thousands = self.0 >> 12;
            ones + tens * 10 + hundreds * 100 + thousands * 1000
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DescriptorType(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ClassCode(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct SubclassCode(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ProtocolCode(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InterfaceClass(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InterfaceSubclass(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InterfaceProtocol(pub u8);

    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(u8)]
    pub enum MaxSizeZeroEP {
        _8 = 8,
        _16 = 16,
        _32 = 32,
        _64 = 64,
    }
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct VendorId(pub u16);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ProductId(pub u16);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct StringIndex(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct ConfigurationValue(pub u8);
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InterfaceNumber(pub u8);

    /// USB Language Identifiers, version 1.0.
    /// (see also Universal Serial Bus HID Usage Tables, section 3.6: HID LANGIDs)
    #[repr(u16)]
    #[derive(PartialEq, Eq, Hash)]
    pub enum LanguageId {
        EnglishUS = 0x409,
        HIDUsageDataDescriptor = 0x0FF | (0x01 << 10),
        HIDVendor1 = 0x0FF | (0x3c << 10),
        HIDVendor2 = 0x0FF | (0x3d << 10),
        HIDVendor3 = 0x0FF | (0x3e << 10),
        HIDVendor4 = 0x0FF | (0x3f << 10),
    }

    pub struct CountryCode(pub u8);

    pub struct Utf16String(Vec<u16>);
    impl TryInto<String> for &Utf16String {
        type Error = std::string::FromUtf16Error;
        fn try_into(self) -> Result<String, Self::Error> {
            // TODO: specifically LE https://github.com/rust-lang/rust/issues/116258
            String::from_utf16(&self.0)
        }
    }
    impl std::str::FromStr for Utf16String {
        type Err = std::convert::Infallible;
        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Ok(Self(value.encode_utf16().collect()))
        }
    }

    #[repr(u8)]
    #[derive(FromRepr)]
    pub enum EndpointDir {
        Out = 0,
        In = 1,
    }
    impl From<bool> for EndpointDir {
        fn from(value: bool) -> Self {
            // unwrap: bool as u8 is always 0 or 1
            Self::from_repr(value as u8).unwrap()
        }
    }
    impl Into<bool> for EndpointDir {
        fn into(self) -> bool {
            self as u8 != 0
        }
    }

    #[repr(u8)]
    #[derive(FromRepr)]
    pub enum TransferType {
        Configuration = 0,
        Isochronous = 1,
        Bulk = 2,
        Interrupt = 3,
    }
    impl From<u8> for TransferType {
        fn from(value: u8) -> Self {
            Self::from_repr(value)
                .expect("must be converted from a two-bit bitstruct field")
        }
    }
    impl Into<u8> for TransferType {
        fn into(self) -> u8 {
            self as u8
        }
    }

    bitstruct! {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct BitmapCfgDescAttributes(pub u8) {
            reserved: u8 = 0..5;
            pub remote_wakeup: bool = 5;
            pub self_powered: bool = 6;
            pub bus_powered: bool = 7;
        }
    }

    bitstruct! {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct BitmapEndptDescAddress(pub u8) {
            pub endpoint_number: u8 = 0..4;
            reserved: u8 = 4..7;
            pub endpoint_direction: EndpointDir = 7;
        }
    }
    bitstruct! {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct BitmapEndptDescAttributes(pub u8) {
            pub transfer_type: TransferType = 0..2;
            reserved: u8 = 2..8;
        }
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
    pub configurations: BTreeMap<ConfigurationValue, ConfigurationInfo>,
    pub specific_descriptors: Vec<ClassSpecificDescriptor>,
}

struct ConfigurationInfo {
    pub interfaces: BTreeMap<InterfaceNumber, InterfaceInfo>,
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
    fn get_descriptor(
        &self,
        typ: GetDescriptorType,
    ) -> Option<Box<dyn Descriptor>> {
        match typ {
            GetDescriptorType::Device => {
                Some(Box::new(DeviceDescriptor::from(self.device_info())))
            }
            GetDescriptorType::String(index, language) => match index {
                MANUFACTURER => self
                    .device_info()
                    .manufacturer_name
                    .get(language)
                    .map(|s| {
                        // inlining this variable breaks type-check.
                        // turbofish `Box::<dyn Descriptor>::new()` doesn't seem to help.
                        let boxed_dyn: Box<dyn Descriptor> =
                            Box::new(StringDescriptor::from_str(s).unwrap());
                        boxed_dyn
                    }), // ...
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
    use super::{value_types::*, ConfigurationInfo, DeviceInfo, InterfaceInfo};

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
        bcdUSB: Bcd16,
        bDeviceClass: ClassCode,
        bDeviceSubClass: SubclassCode,
        bDeviceProtocol: ProtocolCode,
        bMaxPacketSize0: MaxSizeZeroEP,
        idVendor: VendorId,
        idProduct: ProductId,
        bcdDevice: Bcd16,
        iManufacturer: StringIndex,
        iProduct: StringIndex,
        iSerial: StringIndex,

        /// .len() as u8 = bNumConfigurations
        configurations: Vec<ConfigurationDescriptor>,
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
        /// .len() as u8 = bNumInterfaces
        interfaces: Vec<InterfaceDescriptor>,

        bConfigurationValue: ConfigurationValue,
        iConfiguration: StringIndex,
        bmAttributes: BitmapCfgDescAttributes,

        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for InterfaceDescriptor {
        // const bLength: u8 = 9;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(4u8)
        }
    }

    pub struct InterfaceDescriptor {
        bInterfaceNumber: InterfaceNumber,
        bAlternateSetting: u8,

        endpoints: Vec<EndpointDescriptor>, // .len() as u8 = bNumEndpoints

        bInterfaceClass: InterfaceClass,
        bInterfaceSubClass: InterfaceSubclass,
        bInterfaceProtocol: InterfaceProtocol,
        iInterface: StringIndex,

        specific_augmentations: Vec<Box<dyn Descriptor>>,
    }

    impl Descriptor for HidDescriptor {
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(33u8)
        }
    }
    pub struct HidDescriptor {
        // bLength: u8, // dependent on bNumDescriptors
        bcdHID: Bcd16,
        bCountryCode: CountryCode,

        /// .len() as u8 = bNumDescriptors
        class_descriptor: Vec<Box<dyn Descriptor>>,
        // followed by bDescriptorType (u8), wDescriptorLength (u16) for each at serialization time.
    }

    impl Descriptor for EndpointDescriptor {
        // const bLength: u8 = 7;
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(5u8)
        }
    }
    pub struct EndpointDescriptor {
        bEndpointAddress: BitmapEndptDescAddress,
        bmAttributes: BitmapEndptDescAttributes,
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
        // bLength: u8,
        bString: Utf16String,
    }

    impl Descriptor for StringLanguageIdentifierDescriptor {
        fn descriptor_type(&self) -> DescriptorType {
            DescriptorType(3u8)
        }
    }

    // special-case for GET_DESCRIPTOR(String, 0)
    pub struct StringLanguageIdentifierDescriptor {
        // bLength: u8,
        wLANGID: Vec<LanguageId>, // [u16]
    }

    impl std::str::FromStr for StringDescriptor {
        type Err = std::convert::Infallible;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Self {
                // unwrap: infallible
                bString: Utf16String::from_str(s).unwrap(),
            })
        }
    }

    impl From<&DeviceInfo> for DeviceDescriptor {
        fn from(value: &DeviceInfo) -> Self {
            let DeviceInfo {
                manufacturer_name,
                product_name,
                serial_number,
                usb_version,
                device_version,
                vendor_id,
                product_id,
                class,
                subclass,
                configurations,
                specific_descriptors,
            } = value;
            let configurations = configurations
                .into_iter()
                .map(ConfigurationDescriptor::from)
                .collect();
            Self {
                bcdUSB: *usb_version,
                bDeviceClass: *class,
                bDeviceSubClass: *subclass,
                bDeviceProtocol: todo!(),
                bMaxPacketSize0: todo!(),
                idVendor: *vendor_id,
                idProduct: *product_id,
                bcdDevice: *device_version,
                iManufacturer: todo!(),
                iProduct: todo!(),
                iSerial: todo!(),
                configurations,
                specific_augmentations: todo!(),
            }
        }
    }

    impl From<(&ConfigurationValue, &ConfigurationInfo)>
        for ConfigurationDescriptor
    {
        fn from(
            (cfg_num, cfg_info): (&ConfigurationValue, &ConfigurationInfo),
        ) -> Self {
            let ConfigurationInfo {
                interfaces,
                bus_powered,
                self_powered,
                remote_wakeup,
                max_power,
            } = cfg_info;
            let interfaces =
                interfaces.into_iter().map(InterfaceDescriptor::from).collect();
            let bmAttributes = BitmapCfgDescAttributes::default()
                .with_self_powered(*self_powered)
                .with_bus_powered(*bus_powered)
                .with_remote_wakeup(*remote_wakeup);
            Self {
                interfaces,
                bConfigurationValue: *cfg_num,
                iConfiguration: todo!(),
                bmAttributes,
                specific_augmentations: todo!(),
            }
        }
    }
    impl From<(&InterfaceNumber, &InterfaceInfo)> for InterfaceDescriptor {
        fn from((if_num, if_info): (&InterfaceNumber, &InterfaceInfo)) -> Self {
            let InterfaceInfo {
                alternate_setting,
                endpoints,
                class,
                subclass,
                protocol,
                description,
            } = if_info;

            Self {
                bInterfaceNumber: *if_num,
                bAlternateSetting: todo!(),
                endpoints: todo!(),
                bInterfaceClass: *class,
                bInterfaceSubClass: *subclass,
                bInterfaceProtocol: *protocol,
                iInterface: todo!(),
                specific_augmentations: todo!(),
            }
        }
    }
}

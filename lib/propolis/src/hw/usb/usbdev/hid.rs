// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strum::FromRepr;

use super::descriptor::{
    AugmentedDescriptor, Bcd16, CountryCode, Descriptor, DescriptorType,
};

/// USB HID 1.11 sect 7.2
#[repr(u8)]
#[derive(FromRepr, Debug)]
pub enum HIDRequest {
    GetReport = 1,
    GetIdle = 2,
    GetProtocol = 3,
    Reserved4 = 4,
    Reserved5 = 5,
    Reserved6 = 6,
    Reserved7 = 7,
    Reserved8 = 8,
    SetReport = 9,
    SetIdle = 10,
    SetProtocol = 11,
}

/// USB HID 1.11 sect 7.2.1
#[derive(FromRepr, Debug)]
#[repr(u8)]
pub enum HIDReportType {
    Input = 1,
    Output = 2,
    Feature = 3,
    // all other values reserved
}

/// USB HID 1.11 sect 7.2.5, 7.2.6
#[derive(FromRepr, Debug)]
#[repr(u8)]
pub enum HIDProtocol {
    Boot = 0,
    Report = 1,
}

#[derive(Debug)]
pub struct HIDDescriptor {
    /// bcdHID. HID standard version.
    pub hid_version: Bcd16,

    /// bCountryCode.
    pub country_code: CountryCode,

    /// bNumDescriptors is the length of this Vec,
    /// which is followed by [bDescriptorType (u8), wDescriptorLength (u16)]
    /// for each descriptor at serialization time.
    pub class_descriptor: Vec<ReportDescriptor>,
}
impl Descriptor for HIDDescriptor {
    /// bLength. Dependent on bNumDescriptors.
    fn length(&self) -> u8 {
        // bDescriptorType + wDescriptorLen for each
        6 + 3 * self.class_descriptor.len() as u8
    }

    /// bDescriptorType. 33 for HID Descriptor.
    fn descriptor_type(&self) -> DescriptorType {
        DescriptorType::HID
    }

    fn serialize(&self) -> Box<dyn Iterator<Item = u8> + '_> {
        Box::new(
            self.header()
                .into_iter() // 0, 1
                .chain(self.hid_version.0.to_le_bytes()) // 2, 3
                .chain([
                    self.country_code as u8,           // 4
                    self.class_descriptor.len() as u8, // 5
                ])
                .chain(self.class_descriptor.iter().flat_map(|x| {
                    [x.report_type].into_iter().chain(x.length.to_le_bytes())
                })),
        )
    }
}

struct ReportDescriptor {
    report_type: u8,
    length: u16,
}
impl ReportDescriptor {
    fn serialize(&self) -> impl Iterator<Item = u8> {
        [todo!()].into_iter()
    }
}

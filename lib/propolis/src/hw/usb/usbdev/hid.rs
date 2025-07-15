// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strum::FromRepr;

/// USB HID 1.11 sect 7.2
#[repr(u8)]
#[derive(FromRepr, Debug)]
pub enum HidRequest {
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
pub enum HidReportType {
    Input = 1,
    Output = 2,
    Feature = 3,
    // all other values reserved
}

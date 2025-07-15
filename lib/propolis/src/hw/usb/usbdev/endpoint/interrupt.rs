// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hw::usb::usbdev::requests::{RequestType, SetupData};

type InInterruptEndpoint = super::Endpoint<InInterruptRequestInfo, super::In>;

pub enum InInterruptRequestInfo {}

impl TryFrom<(SetupData, Vec<u8>)> for InInterruptRequestInfo {
    type Error = super::Error;

    fn try_from(
        _value: (SetupData, Vec<u8>),
    ) -> std::result::Result<Self, Self::Error> {
        todo!()
    }
}

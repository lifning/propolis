// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hw::usb::usbdev::requests::{RequestType, SetupData};

pub enum InInterruptRequestInfo {}

impl TryFrom<SetupData> for InInterruptRequestInfo {
    type Error = super::Error;

    fn try_from(setup: SetupData) -> std::result::Result<Self, Self::Error> {
        todo!()
    }
}

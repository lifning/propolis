// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hw::usb::usbdev::requests::{Request, SetupData, StandardRequest};

pub type ControlEndpoint = super::Endpoint<ControlRequestInfo>;

#[derive(Default)]
pub enum ControlRequestInfo {
    #[default]
    None,
    SetConfiguration {
        /// USB 2.0 sect 9.4.7: The lower byte of the wValue field specifies
        /// the desired configuration. This configuration value must be zero or
        /// match a configuration value from a configuration descriptor. If the
        /// configuration value is zero, the device is placed in its Address
        /// state. The upper byte of the wValue field is reserved.
        configuration: u8,
    },
}

impl<'a> TryFrom<(SetupData, Vec<u8>)> for ControlRequestInfo {
    type Error = super::Error;

    fn try_from(
        value: (SetupData, Vec<u8>),
    ) -> std::result::Result<Self, Self::Error> {
        let (setup, payload) = value;
        match setup.request() {
            Request::Standard(StandardRequest::SetConfiguration) => {
                if !payload.is_empty() {
                    Err(Self::Error::InvalidPayloadForRequest(
                        setup.request(),
                        payload.to_vec(),
                    ))
                } else {
                    Ok(Self::SetConfiguration {
                        configuration: setup.value() as u8,
                    })
                }
            }
            Request::Standard(_) | Request::Other(_) => {
                Err(Self::Error::UnimplementedRequest(setup.request()))
            }
        }
    }
}

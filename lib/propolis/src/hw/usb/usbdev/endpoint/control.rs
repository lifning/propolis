// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use core::fmt::Debug;

use crate::hw::usb::usbdev::{
    descriptor::DescriptorType,
    requests::{RequestType, SetupData, StandardRequest},
    Error,
};

pub type ControlEndpoint =
    super::Endpoint<ControlRequestInfo<NoClassRequestInfo>, super::InAndOut>;

#[derive(Debug)]
pub enum ControlRequestInfo<C>
where
    C: TryFrom<SetupData, Error = Error> + Debug,
{
    GetDescriptor {
        descriptor_type: DescriptorType,
        index: u8,
    },
    GetStatus,
    SetConfiguration {
        /// USB 2.0 sect 9.4.7: The lower byte of the wValue field specifies
        /// the desired configuration. This configuration value must be zero or
        /// match a configuration value from a configuration descriptor. If the
        /// configuration value is zero, the device is placed in its Address
        /// state. The upper byte of the wValue field is reserved.
        configuration: u8,
    },

    Class(C),
}

impl<C> TryFrom<SetupData> for ControlRequestInfo<C>
where
    C: TryFrom<SetupData, Error = Error> + Debug,
{
    type Error = super::Error;

    fn try_from(setup: SetupData) -> std::result::Result<Self, Self::Error> {
        match setup.request_type() {
            RequestType::Standard => {
                match StandardRequest::from_repr(setup.request()) {
                    Some(StandardRequest::GetDescriptor) => {
                        let [desc, index] = setup.value().to_be_bytes();
                        Ok(Self::GetDescriptor {
                            descriptor_type: DescriptorType::from_repr(desc)
                                .ok_or_else(|| {
                                    Error::UnknownDescriptorType(desc)
                                })?,
                            index,
                        })
                    }
                    Some(StandardRequest::GetStatus) => Ok(Self::GetStatus),
                    Some(StandardRequest::SetConfiguration) => {
                        Ok(Self::SetConfiguration {
                            configuration: setup.value() as u8,
                        })
                    }
                    _ => Err(Error::UnimplementedRequestType(
                        setup.request(),
                        setup.request_type(),
                    )),
                }
            }
            RequestType::Class => {
                Ok(ControlRequestInfo::Class(C::try_from(setup)?))
            }
            _ => Err(Error::UnimplementedRequestType(
                setup.request(),
                setup.request_type(),
            )),
        }
    }
}

#[derive(Debug)]
pub struct NoClassRequestInfo;
impl TryFrom<SetupData> for NoClassRequestInfo {
    type Error = super::Error;

    fn try_from(setup: SetupData) -> Result<Self, Self::Error> {
        Err(Error::ClassRequestOnNonClassEndpoint(setup))
    }
}

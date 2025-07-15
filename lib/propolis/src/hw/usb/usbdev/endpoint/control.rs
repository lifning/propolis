// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hw::usb::usbdev::{
    descriptor::DescriptorType,
    hid::{HidReportType, HidRequest},
    requests::{RequestType, SetupData, StandardRequest},
    Error,
};

pub type ControlEndpoint = super::Endpoint<ControlRequestInfo, super::InAndOut>;

#[derive(Debug)]
pub enum ControlRequestInfo {
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
    HidGetReport {
        report_type: HidReportType,
        report_id: u8,
        interface: u16,
    },
}

impl TryFrom<SetupData> for ControlRequestInfo {
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
                    _ => Err(Error::UnimplementedRequest(
                        setup.request(),
                        setup.request_type(),
                    )),
                }
            }
            RequestType::Class => {
                match HidRequest::from_repr(setup.request()) {
                    Some(HidRequest::GetReport) => {
                        let [rtype, report_id] = setup.value().to_be_bytes();
                        if let Some(report_type) =
                            HidReportType::from_repr(rtype)
                        {
                            let interface = setup.index();
                            Ok(Self::HidGetReport {
                                report_type,
                                report_id,
                                interface,
                            })
                        } else {
                            Err(Error::InvalidSetupParamsForRequest(
                                setup.request(),
                                setup.request_type(),
                                setup.value(),
                                setup.index(),
                            ))
                        }
                    }
                    Some(HidRequest::GetIdle)
                    | Some(HidRequest::GetProtocol)
                    | Some(HidRequest::SetReport)
                    | Some(HidRequest::SetIdle)
                    | Some(HidRequest::SetProtocol) => {
                        // TODO
                        Err(Error::UnimplementedRequest(
                            setup.request(),
                            setup.request_type(),
                        ))
                    }
                    Some(HidRequest::Reserved4)
                    | Some(HidRequest::Reserved5)
                    | Some(HidRequest::Reserved6)
                    | Some(HidRequest::Reserved7)
                    | Some(HidRequest::Reserved8)
                    | None => Err(Error::UnimplementedRequest(
                        setup.request(),
                        setup.request_type(),
                    )),
                }
            }
            _ => Err(Error::UnimplementedRequest(
                setup.request(),
                setup.request_type(),
            )),
        }
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hw::usb::usbdev::{
    descriptor::DescriptorType,
    hid::{HidProtocol, HidReportType, HidRequest},
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

    // -- class-specific requests --
    /// Get_Report lets the host read a report through the control endpoint.
    /// USB HID 1.11 sect 7.2.1
    HidGetReport {
        report_type: HidReportType,
        report_id: u8,
        interface: u16,
    },
    /// Set_Report sends a report to the device, possibly setting the state of
    /// input, output, or feature controls.
    /// USB HID 1.11 sect 7.2.2
    HidSetReport {
        report_type: HidReportType,
        report_id: u8,
        interface: u16,
    },
    /// Get_Idle reads the current idle rate for a particular Input report.
    /// (see Set_Idle)
    /// USB HID 1.11 sect 7.2.3
    HidGetIdle {
        report_id: u8,
        interface: u16,
    },
    /// Set_Idle silences a particular report on this endpoint until a new
    /// event occurs or the provided duration passes.
    /// USB HID 1.11 sect 7.2.4
    HidSetIdle {
        /// 0 = indefinite. Other values are in units of 4 milliseconds, e.g.
        /// 1u8 = 4ms, 255u8 = 1.020 seconds.
        duration_4ms: u8,
        report_id: u8,
        interface: u16,
    },
    /// Get_Protocol reads which of the boot or report protocol are active.
    /// USB HID 1.11 sect 7.2.5
    HidGetProtocol {
        interface: u16,
    },
    /// Set_Protocol switches between the boot and report protocols.
    /// USB HID 1.11 sect 7.2.6
    HidSetProtocol {
        protocol: HidProtocol,
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
                // FIXME: this assumes all class requests are HID, need to model class in type system too
                match HidRequest::from_repr(setup.request()) {
                    Some(HidRequest::GetReport) => {
                        let [rtype, report_id] = setup.value().to_be_bytes();
                        if let Some(report_type) =
                            HidReportType::from_repr(rtype)
                        {
                            Ok(Self::HidGetReport {
                                report_type,
                                report_id,
                                interface: setup.index(),
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
                    Some(HidRequest::SetReport) => {
                        let [rtype, report_id] = setup.value().to_be_bytes();
                        if let Some(report_type) =
                            HidReportType::from_repr(rtype)
                        {
                            Ok(Self::HidSetReport {
                                report_type,
                                report_id,
                                interface: setup.index(),
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
                    Some(HidRequest::GetIdle) => {
                        let [0, report_id] = setup.value().to_be_bytes() else {
                            return Err(Error::InvalidSetupParamsForRequest(
                                setup.request(),
                                setup.request_type(),
                                setup.value(),
                                setup.index(),
                            ));
                        };
                        Ok(Self::HidGetIdle {
                            report_id,
                            interface: setup.index(),
                        })
                    }
                    Some(HidRequest::SetIdle) => {
                        let [duration_4ms, report_id] =
                            setup.value().to_be_bytes();
                        Ok(Self::HidSetIdle {
                            duration_4ms,
                            report_id,
                            interface: setup.index(),
                        })
                    }
                    Some(HidRequest::GetProtocol) => {
                        Ok(Self::HidGetProtocol { interface: setup.index() })
                    }
                    Some(HidRequest::SetProtocol) => {
                        if let Some(protocol) =
                            HidProtocol::from_repr(setup.value() as u8)
                        {
                            Ok(Self::HidSetProtocol {
                                protocol,
                                interface: setup.index(),
                            })
                        } else {
                            return Err(Error::InvalidSetupParamsForRequest(
                                setup.request(),
                                setup.request_type(),
                                setup.value(),
                                setup.index(),
                            ));
                        }
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

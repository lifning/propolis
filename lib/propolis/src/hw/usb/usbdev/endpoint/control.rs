// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use crate::{
    common::{GuestData, GuestRegion},
    hw::usb::{
        usbdev::{
            descriptor::DescriptorType,
            requests::{
                RequestDirection, RequestType, SetupData, StandardRequest,
            },
            Error, Result,
        },
        xhci::{
            bits::ring_data::TrbCompletionCode,
            controller::XhciPortWakeHandle,
            device_slots::{EndpointId, SlotId},
            rings::consumer::transfer::{PointerOrImmediate, TransferTrb},
        },
    },
    vmm::MemCtx,
};

#[usdt::provider(provider = "propolis")]
mod probes {
    fn usb_control_xfer_setup(
        slot_id: u8,
        endpoint_id: u8,
        request_type: u8,
        request: u8,
        device_to_host: bool,
    ) {
    }
    fn usb_control_xfer_data(slot_id: u8, endpoint_id: u8, trb_pointer: u64) {}
    fn usb_control_xfer_status(slot_id: u8, endpoint_id: u8, ok: bool) {}
}

pub struct ControlEndpoint<C>
where
    ControlRequestInfo<C>: TryFrom<SetupData, Error = Error>,
    C: TryFrom<SetupData, Error = Error> + Debug,
{
    current_setup: Option<SetupData>,
    payload: Option<Vec<u8>>,
    bytes_transferred: usize,
    slot_id: SlotId,
    endpoint_id: EndpointId,
    port_wake_hdl: Arc<XhciPortWakeHandle>,
    _spooky: PhantomData<C>,
}

impl<C> ControlEndpoint<C>
where
    ControlRequestInfo<C>: TryFrom<SetupData, Error = Error>,
    C: TryFrom<SetupData, Error = Error> + Debug,
{
    pub fn new(
        slot_id: SlotId,
        endpoint_id: EndpointId,
        port_wake_hdl: Arc<XhciPortWakeHandle>,
    ) -> Self {
        Self {
            current_setup: None,
            payload: None,
            bytes_transferred: 0,
            slot_id,
            endpoint_id,
            port_wake_hdl,
            _spooky: PhantomData,
        }
    }

    pub fn new_migrated(
        value: &migrate::ControlEndpointV1,
        port_wake_hdl: Arc<XhciPortWakeHandle>,
    ) -> Self {
        let mut new = Self::new(
            SlotId::from(value.slot_id),
            EndpointId::from(value.endpoint_id),
            port_wake_hdl,
        );
        new.import(&value);
        new
    }

    pub fn set_payload(&mut self, payload: Vec<u8>) -> Result<()> {
        if let Some(setup) = &self.current_setup {
            if setup.direction() == RequestDirection::HostToDevice {
                Err(Error::GavePayloadForOutRequest(setup.request(), payload))
            } else if let Some(existing) = &self.payload {
                Err(Error::GavePayloadTwice(existing.to_owned(), payload))
            } else {
                self.payload = Some(payload);
                Ok(())
            }
        } else {
            Err(Error::GavePayloadBeforeRequest(payload))
        }
    }

    pub fn setup_stage(
        &mut self,
        setup: SetupData,
    ) -> Result<Option<ControlRequestInfo<C>>> {
        self.bytes_transferred = 0;
        self.current_setup = Some(setup);
        self.payload = None;
        let control_request_info = match setup.direction() {
            RequestDirection::DeviceToHost => {
                Some(ControlRequestInfo::try_from(setup)?)
            }
            RequestDirection::HostToDevice => None,
        };
        probes::usb_control_xfer_setup!(|| (
            u8::from(self.slot_id),
            u8::from(self.endpoint_id),
            setup.request_type() as u8,
            setup.request(),
            control_request_info.is_some(),
        ));
        Ok(control_request_info)
    }

    /// Unlike [`setup_stage`] and [`status_stage`], this method puts its own
    /// completion events into the Event Ring for each successful Transfer TRB.
    pub fn data_stage(
        &mut self,
        xfer_trbs: &[TransferTrb],
        data_direction: RequestDirection,
        memctx: &MemCtx,
    ) -> Result<()> {
        if let Some(setup_data) = self.current_setup.as_ref() {
            if data_direction != setup_data.direction() {
                return Err(Error::SetupVsDataDirectionMismatch(
                    setup_data.direction(),
                    data_direction,
                ));
            }
            for trb in xfer_trbs {
                let count = match setup_data.direction() {
                    RequestDirection::DeviceToHost => {
                        if let Some(payload) = &self.payload {
                            let PointerOrImmediate::Pointer(region) =
                                trb.data_buffer()
                            else {
                                return Err(
                                    Error::ImmediateParameterForInTransfer,
                                );
                            };
                            memctx
                                .write_from(
                                    region.0,
                                    &payload[self.bytes_transferred..],
                                    region.1,
                                )
                                .ok_or(Error::DataStageWriteFailed)?
                        } else {
                            return Err(Error::MissingPayloadForInRequest(
                                setup_data.request(),
                            ));
                        }
                    }
                    RequestDirection::HostToDevice => {
                        let payload = self.payload.get_or_insert_default();
                        match trb.data_buffer() {
                            PointerOrImmediate::Pointer(GuestRegion(
                                ptr,
                                len,
                            )) => {
                                payload
                                    .resize(self.bytes_transferred + len, 0u8);
                                memctx
                                    .read_into(
                                        ptr,
                                        &mut GuestData::from(
                                            &mut payload
                                                [self.bytes_transferred..],
                                        ),
                                        len,
                                    )
                                    .ok_or(Error::DataStageReadFailed)?
                            }
                            PointerOrImmediate::Immediate(arr, len) => {
                                payload.extend_from_slice(&arr[..len]);
                                len
                            }
                        }
                    }
                };
                probes::usb_control_xfer_data!(|| (
                    u8::from(self.slot_id),
                    u8::from(self.endpoint_id),
                    trb.trb_pointer().0,
                ));
                self.port_wake_hdl.event_sender.send_completion_events_for_trb(
                    trb,
                    TrbCompletionCode::Success,
                    count,
                    self.slot_id,
                    self.endpoint_id,
                );
                self.bytes_transferred += count;
            }
            Ok(())
        } else {
            Err(Error::NoSetupStageBefore("Data Stage"))
        }
    }

    pub fn status_stage(
        &mut self,
        status_direction: RequestDirection,
    ) -> Result<Option<(ControlRequestInfo<C>, Option<&[u8]>)>> {
        if let Some(setup) = self.current_setup.take() {
            if status_direction == setup.direction() {
                return Err(Error::SetupVsStatusDirectionMatch(
                    status_direction,
                ));
            }

            let result = match setup.direction() {
                RequestDirection::HostToDevice => Some(
                    ControlRequestInfo::try_from(setup)
                        .map(|x| (x, self.payload.as_ref().map(Vec::as_slice))),
                )
                .transpose(),
                RequestDirection::DeviceToHost => Ok(None),
            };

            probes::usb_control_xfer_status!(|| (
                u8::from(self.slot_id),
                u8::from(self.endpoint_id),
                result.is_ok(),
            ));
            self.bytes_transferred = 0;
            result.map_err(From::from)
        } else {
            Err(Error::NoSetupStageBefore("Status Stage"))
        }
    }

    pub fn import(&mut self, value: &migrate::ControlEndpointV1) {
        let migrate::ControlEndpointV1 {
            current_setup,
            payload,
            bytes_transferred,
            slot_id,
            endpoint_id,
        } = value;
        self.current_setup = current_setup.map(|x| SetupData(x));
        self.payload = payload.to_owned();
        self.bytes_transferred = *bytes_transferred;
        self.slot_id = SlotId::from(*slot_id);
        self.endpoint_id = EndpointId::from(*endpoint_id);
    }

    pub fn export(&self) -> super::migrate::EndpointV1 {
        let Self {
            current_setup,
            payload,
            bytes_transferred,
            slot_id,
            endpoint_id,
            _spooky,
            port_wake_hdl: _,
        } = self;
        super::migrate::EndpointV1::Control(migrate::ControlEndpointV1 {
            current_setup: current_setup.as_ref().map(|x| x.0),
            payload: payload.to_owned(),
            bytes_transferred: *bytes_transferred,
            slot_id: u8::from(*slot_id),
            endpoint_id: u8::from(*endpoint_id),
        })
    }
}

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
    type Error = Error;

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
    type Error = Error;

    fn try_from(setup: SetupData) -> Result<Self> {
        Err(Error::ClassRequestOnNonClassEndpoint(setup))
    }
}

pub mod migrate {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct ControlEndpointV1 {
        pub current_setup: Option<u64>,
        pub payload: Option<Vec<u8>>,
        pub bytes_transferred: usize,
        pub slot_id: u8,
        pub endpoint_id: u8,
    }
}

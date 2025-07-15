// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::marker::PhantomData;

use crate::common::{GuestData, GuestRegion};
use crate::hw::usb::xhci::rings::consumer::transfer::PointerOrImmediate;
use crate::vmm::MemCtx;

use super::requests::{RequestDirection, SetupData};
use super::{Error, Result};

pub mod control;
pub mod interrupt;

pub struct In;
pub struct Out;
pub struct InAndOut;

pub trait EndpointRequestDirectionMarker {
    const DIR: Option<RequestDirection>;
}
impl EndpointRequestDirectionMarker for In {
    const DIR: Option<RequestDirection> = Some(RequestDirection::DeviceToHost);
}
impl EndpointRequestDirectionMarker for Out {
    const DIR: Option<RequestDirection> = Some(RequestDirection::HostToDevice);
}
impl EndpointRequestDirectionMarker for InAndOut {
    const DIR: Option<RequestDirection> = None;
}

pub struct Endpoint<T, Dir>
where
    T: TryFrom<SetupData>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    current_setup: Option<SetupData>,
    payload: Option<Vec<u8>>,
    bytes_transferred: usize,
    _spooky: PhantomData<T>,
    _2spooky: PhantomData<Dir>,
}

// #[derive(Default)] wants T: Default and Dir: Default, even as Phantoms
impl<T, Dir> Default for Endpoint<T, Dir>
where
    T: TryFrom<SetupData>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    fn default() -> Self {
        Self {
            current_setup: None,
            payload: None,
            bytes_transferred: 0,
            _spooky: PhantomData,
            _2spooky: PhantomData,
        }
    }
}

impl<T> Endpoint<T, In>
where
    T: TryFrom<SetupData>,
    super::Error: From<T::Error>,
{
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = Some(payload);
    }
}

impl<T> Endpoint<T, InAndOut>
where
    T: TryFrom<SetupData>,
    super::Error: From<T::Error>,
{
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
}

impl<T, Dir> Endpoint<T, Dir>
where
    T: TryFrom<SetupData>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    pub fn setup_stage(&mut self, setup: SetupData) -> Result<Option<T>> {
        if let Some(dir) = Dir::DIR {
            if setup.direction() != dir {
                return Err(Error::EndpointVsSetupDirectionMismatch(
                    dir,
                    setup.direction(),
                ));
            }
        }
        self.bytes_transferred = 0;
        self.current_setup = Some(setup);
        self.payload = None;
        Ok(match setup.direction() {
            RequestDirection::DeviceToHost => Some(T::try_from(setup)?),
            RequestDirection::HostToDevice => None,
        })
    }

    pub fn data_stage(
        &mut self,
        data_buffer: PointerOrImmediate,
        data_direction: RequestDirection,
        memctx: &MemCtx,
    ) -> Result<usize> {
        if let Some(setup_data) = self.current_setup.as_ref() {
            if data_direction != setup_data.direction() {
                return Err(Error::SetupVsDataDirectionMismatch(
                    setup_data.direction(),
                    data_direction,
                ));
            }
            let count = match setup_data.direction() {
                RequestDirection::DeviceToHost => {
                    if let Some(payload) = &self.payload {
                        let PointerOrImmediate::Pointer(region) = data_buffer
                        else {
                            return Err(
                                Error::ImmediateParameterForOutDataStage,
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
                    match data_buffer {
                        PointerOrImmediate::Pointer(GuestRegion(ptr, len)) => {
                            payload.resize(self.bytes_transferred + len, 0u8);
                            memctx
                                .read_into(
                                    ptr,
                                    &mut GuestData::from(
                                        &mut payload[self.bytes_transferred..],
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
            self.bytes_transferred += count;
            Ok(count)
        } else {
            Err(Error::NoSetupStageBefore("Data Stage"))
        }
    }

    pub fn status_stage(
        &mut self,
        status_direction: RequestDirection,
    ) -> Result<Option<(T, Option<&[u8]>)>> {
        if let Some(setup) = self.current_setup.take() {
            if status_direction == setup.direction() {
                return Err(Error::SetupVsStatusDirectionMatch(
                    status_direction,
                ));
            }

            let result = match setup.direction() {
                RequestDirection::HostToDevice => Some(
                    T::try_from(setup)
                        .map(|x| (x, self.payload.as_ref().map(Vec::as_slice))),
                )
                .transpose(),
                RequestDirection::DeviceToHost => Ok(None),
            };

            self.bytes_transferred = 0;
            result.map_err(From::from)
        } else {
            Err(Error::NoSetupStageBefore("Status Stage"))
        }
    }

    pub fn import(
        &mut self,
        value: &migrate::EndpointV1,
    ) -> core::result::Result<(), crate::migrate::MigrateStateError> {
        let migrate::EndpointV1 { current_setup, payload, bytes_transferred } =
            value;
        self.current_setup = current_setup.map(|x| SetupData(x));
        self.payload = payload.to_owned();
        self.bytes_transferred = *bytes_transferred;
        Ok(())
    }

    pub fn export(&self) -> migrate::EndpointV1 {
        let Self {
            current_setup,
            payload,
            bytes_transferred,
            _spooky,
            _2spooky,
        } = self;
        migrate::EndpointV1 {
            current_setup: current_setup.as_ref().map(|x| x.0),
            payload: payload.to_owned(),
            bytes_transferred: *bytes_transferred,
        }
    }
}

pub mod migrate {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct EndpointV1 {
        pub current_setup: Option<u64>,
        pub payload: Option<Vec<u8>>,
        pub bytes_transferred: usize,
    }
}

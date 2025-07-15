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
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    current_setup: Option<SetupData>,
    payload: Vec<u8>,
    bytes_transferred: usize,
    _spooky: PhantomData<T>,
    _2spooky: PhantomData<Dir>,
}

impl<T, Dir> Default for Endpoint<T, Dir>
where
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    fn default() -> Self {
        Self {
            current_setup: None,
            payload: Vec::new(),
            bytes_transferred: 0,
            _spooky: PhantomData,
            _2spooky: PhantomData,
        }
    }
}

impl<T> Endpoint<T, In>
where
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
{
    pub fn setup_stage(
        &mut self,
        setup: SetupData,
        payload: Vec<u8>,
    ) -> Result<()> {
        self.setup_stage_inner(setup, payload)
    }
}

impl<T> Endpoint<T, Out>
where
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
{
    pub fn setup_stage(&mut self, setup: SetupData) -> Result<()> {
        self.setup_stage_inner(setup, Vec::new())
    }
}

impl<T> Endpoint<T, InAndOut>
where
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
{
    pub fn setup_stage(
        &mut self,
        setup: SetupData,
        payload: Option<Vec<u8>>,
    ) -> Result<()> {
        if setup.direction() == RequestDirection::DeviceToHost
            && payload.is_none()
        {
            return Err(Error::MissingPayloadForInRequest(setup.request()));
        }
        self.setup_stage_inner(setup, payload.unwrap_or_default())
    }
}

impl<T, Dir> Endpoint<T, Dir>
where
    T: TryFrom<(SetupData, Vec<u8>)>,
    super::Error: From<T::Error>,
    Dir: EndpointRequestDirectionMarker,
{
    fn setup_stage_inner(
        &mut self,
        setup: SetupData,
        payload: Vec<u8>,
    ) -> Result<()> {
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
        self.payload = payload;
        Ok(())
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
                    let PointerOrImmediate::Pointer(region) = data_buffer
                    else {
                        return Err(Error::ImmediateParameterForOutDataStage);
                    };
                    memctx
                        .write_from(
                            region.0,
                            &self.payload[self.bytes_transferred..],
                            region.1,
                        )
                        .ok_or(Error::DataStageWriteFailed)?
                }
                RequestDirection::HostToDevice => match data_buffer {
                    PointerOrImmediate::Pointer(GuestRegion(ptr, len)) => {
                        self.payload.resize(self.bytes_transferred + len, 0u8);
                        memctx
                            .read_into(
                                ptr,
                                &mut GuestData::from(
                                    &mut self.payload[self.bytes_transferred..],
                                ),
                                len,
                            )
                            .ok_or(Error::DataStageReadFailed)?
                    }
                    PointerOrImmediate::Immediate(arr, len) => {
                        self.payload.extend_from_slice(&arr[..len]);
                        len
                    }
                },
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
    ) -> Result<Option<T>> {
        if let Some(setup) = self.current_setup.take() {
            if status_direction == setup.direction() {
                return Err(Error::SetupVsStatusDirectionMatch(
                    status_direction,
                ));
            }

            let result = match setup.direction() {
                RequestDirection::HostToDevice => {
                    let mut new = Vec::new();
                    core::mem::swap(&mut self.payload, &mut new);
                    Some(T::try_from((setup, new))).transpose()
                }
                RequestDirection::DeviceToHost => Ok(None),
            };

            self.payload.clear();
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
        pub payload: Vec<u8>,
        pub bytes_transferred: usize,
    }
}

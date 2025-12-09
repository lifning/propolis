// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::{Arc, Mutex};

use descriptor::DescriptorType;
use requests::{RequestDirection, RequestType, SetupData};
use vnc_tablet::HIDTabletReport;

use crate::{common::GuestAddr, hw::pci, vmm::MemCtx};

use super::xhci::{
    bits::device_context::EndpointContext,
    controller::XhciPortHandle,
    device_slots::{EndpointId, SlotId},
    port::PortId,
    rings::consumer::transfer::TransferTrb,
};

pub mod descriptor;
pub mod endpoint;
pub mod requests;

pub mod hid;

pub mod demo_state_tracker;
pub mod vnc_tablet;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mismatched Setup Stage and Data Stage transfer direction in transfer: {0:?} != {1:?}")]
    SetupVsDataDirectionMismatch(RequestDirection, RequestDirection),
    #[error("given an immediate for IN transfer")]
    ImmediateParameterForInTransfer,
    #[error("In Data Stage memory write failed")]
    DataStageWriteFailed,
    #[error("Out Data Stage memory read failed")]
    DataStageReadFailed,
    #[error("expected Setup Stage before {0}")]
    NoSetupStageBefore(&'static str),
    #[error("matched Setup Stage and Status Stage transfer direction {0:?}")]
    SetupVsStatusDirectionMatch(RequestDirection),
    #[error("unimplemented {1:?} request {0}")]
    UnimplementedRequestType(u8, RequestType),
    #[error("USB device does not implement {0:?}")]
    UnimplementedRequestBehavior(String),
    #[error("invalid payload for {1:?} request {0}: {2:#x?}")]
    InvalidPayloadForRequest(u8, RequestType, Vec<u8>),
    #[error("unimplemented descriptor type: {0:?}")]
    UnimplementedDescriptor(DescriptorType),
    #[error("unknown descriptor type: {0:#x}")]
    UnknownDescriptorType(u8),
    #[error("missing payload for IN request: {0:?}")]
    MissingPayloadForInRequest(u8),
    #[error("tried to provide payload for OUT request: {0:?}, {1:#x?}")]
    GavePayloadForOutRequest(u8, Vec<u8>),
    #[error("tried to provide payload before request: {0:#x?}")]
    GavePayloadBeforeRequest(Vec<u8>),
    #[error("tried to provide two payloads for one request: {0:#x?}, {1:#x?}")]
    GavePayloadTwice(Vec<u8>, Vec<u8>),
    #[error("invalid Setup Stage parameters for {1:?} request {0}: value {2}, index {3}")]
    InvalidSetupParamsForRequest(u8, RequestType, u16, u16),
    #[error("got a class-specific USB request on endpoint with unspecified class: {0:?}")]
    ClassRequestOnNonClassEndpoint(SetupData),
    #[error("received packet on {0:?} not present on this USB device")]
    InvalidEndpoint(EndpointId),
    #[error("received packet on {0:?} not yet configured on this USB device")]
    EndpointNotConfigured(EndpointId),
    #[error("received Normal TD on interrupt endpoint with IOC unset")]
    NoInterruptOnCompletionOnInterruptTransfer,
}

pub type Result<T> = core::result::Result<T, Error>;

#[usdt::provider(provider = "propolis")]
mod probes {
    fn usb_get_descriptor(descriptor_type: u8, index: u8) {}
}

pub enum UsbDeviceType {
    Null,
    HidTablet,
}

impl UsbDeviceType {
    pub fn create(
        &self,
        hid_report: &Arc<Mutex<HIDTabletReport>>,
        port_wake_hdl: Arc<XhciPortHandle>,
        pci_state: &Arc<pci::DeviceState>,
        log: &slog::Logger,
    ) -> Box<dyn UsbDevice> {
        let log = log.new(slog::o!("component" => "usbdev", "port_id" => port_wake_hdl.port_id().as_raw_id()));
        match self {
            UsbDeviceType::Null => Box::new(
                demo_state_tracker::NullUsbDevice::new(port_wake_hdl, log),
            ),
            UsbDeviceType::HidTablet => {
                Box::new(vnc_tablet::HIDTabletDevice::new(
                    hid_report.clone(),
                    port_wake_hdl,
                    pci_state,
                    log,
                ))
            }
        }
    }

    pub fn create_from_payload(
        payload: &migrate::UsbDeviceV1,
        hid_report: &Arc<Mutex<HIDTabletReport>>,
        port_wake_hdl: Arc<XhciPortHandle>,
        pci_state: &Arc<pci::DeviceState>,
        log: &slog::Logger,
    ) -> core::result::Result<
        Box<dyn UsbDevice>,
        crate::migrate::MigrateStateError,
    > {
        let mut dev = match &payload.device_type {
            migrate::UsbDeviceTypeV1::Null => {
                Self::Null.create(hid_report, port_wake_hdl, pci_state, log)
            }
            // specifics imported below this match
            migrate::UsbDeviceTypeV1::Tablet(_specifics) => Self::HidTablet
                .create(hid_report, port_wake_hdl, pci_state, log),
        };
        dev.import(&payload)?;
        Ok(dev)
    }
}

pub trait UsbDevice: Send + Sync + 'static {
    /// Resets EDTLA (xHCI 1.2 sect 4.11.5.2)
    fn new_transfer_descriptor(&self);
    /// Aborts any in-flight transactions for a Stop or Reset Endpoint Command.
    /// Returns pointer to interrupted TRB, and how many bytes remained.
    fn stop_endpoint(
        &mut self,
        endpoint_id: EndpointId,
    ) -> Result<Option<(GuestAddr, usize)>>;
    /// *Caller* must put an appropriate completion event into the Event Ring.
    fn setup_stage(
        &mut self,
        endpoint_id: EndpointId,
        setup: SetupData,
    ) -> Result<()>;
    /// *Implementor* must insert completion events into the Event Ring for
    /// each successful Transfer TRB.
    /// *Caller* must put a USB Transaction Error event into the Event Ring
    /// when Err(_) is returned.
    fn data_stage(
        &mut self,
        endpoint_id: EndpointId,
        trbs: Vec<TransferTrb>,
        data_direction: RequestDirection,
        memctx: &MemCtx,
    ) -> Result<()>;
    /// *Caller* must put an appropriate completion event into the Event Ring.
    fn status_stage(
        &mut self,
        endpoint_id: EndpointId,
        status_direction: RequestDirection,
    ) -> Result<()>;
    /// *Implementor* must insert completion events into the Event Ring for
    /// each successful Transfer TRB.
    /// *Caller* must put a USB Transaction Error event into the Event Ring
    /// when Err(_) is returned.
    fn normal_transfer(
        &mut self,
        endpoint_id: EndpointId,
        trbs: Vec<TransferTrb>,
    ) -> Result<()>;
    /// Configures the Default Control Endpoint.
    fn set_address(&mut self, slot_id: SlotId, port_id: PortId);
    /// Configures other endpoints.
    fn configure_endpoint(
        &mut self,
        slot_id: SlotId,
        endpoint_id: EndpointId,
        ep_ctx: &EndpointContext,
    );
    fn import(
        &mut self,
        payload: &migrate::UsbDeviceV1,
    ) -> core::result::Result<(), crate::migrate::MigrateStateError>;
    fn export(
        &self,
    ) -> core::result::Result<
        migrate::UsbDeviceV1,
        crate::migrate::MigrateStateError,
    >;
}

pub mod migrate {
    use super::endpoint::migrate::EndpointV1;
    use super::vnc_tablet::migrate::TabletDeviceV1;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum UsbDeviceTypeV1 {
        Null,
        Tablet(TabletDeviceV1),
    }

    #[derive(Serialize, Deserialize)]
    pub struct UsbDeviceV1 {
        pub device_type: UsbDeviceTypeV1,
        pub endpoints: BTreeMap<u8, EndpointV1>,
    }
}

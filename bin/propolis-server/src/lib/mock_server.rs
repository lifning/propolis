//! Implementation of a mock Propolis server

use dropshot::channel;
use dropshot::endpoint;
use dropshot::ApiDescription;
use dropshot::HttpError;
use dropshot::HttpResponseCreated;
use dropshot::HttpResponseOk;
use dropshot::HttpResponseUpdatedNoContent;
use dropshot::RequestContext;
use dropshot::TypedBody;
use dropshot::WebsocketConnection;
use futures::SinkExt;
use slog::{error, info, Logger};
use std::io::{Error as IoError, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{watch, Mutex};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{protocol::Role, Message};
use tokio_tungstenite::WebSocketStream;

#[derive(Debug, Eq, PartialEq, Error)]
pub enum Error {
    #[error("Failed to send simulated state change update through channel")]
    TransitionSendFail,
    #[error("Cannot request any new mock instance state once it is stopped/destroyed/failed")]
    TerminalState,
}

use propolis_client::handmade::api;

use crate::config::Config;
use crate::spec::{SlotType, slot_to_pci_path};

/// ed instance properties
pub struct InstanceContext {
    pub state: api::InstanceState,
    pub generation: u64,
    pub properties: api::InstanceProperties,
    state_watcher_rx: watch::Receiver<api::InstanceStateMonitorResponse>,
    state_watcher_tx: watch::Sender<api::InstanceStateMonitorResponse>,
}

impl InstanceContext {
    pub fn new(properties: api::InstanceProperties) -> Self {
        let (state_watcher_tx, state_watcher_rx) =
            watch::channel(api::InstanceStateMonitorResponse {
                gen: 0,
                state: api::InstanceState::Creating,
            });
        Self {
            state: api::InstanceState::Creating,
            generation: 0,
            properties,
            state_watcher_rx,
            state_watcher_tx,
        }
    }

    /// Updates the state of the mock instance.
    ///
    /// Returns an error if the state transition is invalid.
    pub fn set_target_state(
        &mut self,
        target: api::InstanceStateRequested,
    ) -> Result<(), Error> {
        match self.state {
            api::InstanceState::Stopped | api::InstanceState::Destroyed | api::InstanceState::Failed => {
                // Cannot request any state once the target is halt/destroy
                Err(Error::TerminalState)
            }
            api::InstanceState::Rebooting if matches!(target, api::InstanceStateRequested::Run) => {
                // Requesting a run when already on the road to reboot is an
                // immediate success.
                Ok(())
            }
            _ => match target {
                api::InstanceStateRequested::Run | api::InstanceStateRequested::Reboot => {
                    self.generation += 1;
                    self.state = api::InstanceState::Running;
                    self.state_watcher_tx.send(api::InstanceStateMonitorResponse {
                        gen: self.generation,
                        state: self.state.clone(),
                    }).map_err(|_| Error::TransitionSendFail)
                }
                api::InstanceStateRequested::Stop => {
                    self.state = api::InstanceState::Stopped;
                    Ok(())
                }
                api::InstanceStateRequested::MigrateStart => {
                    unimplemented!("migration not yet implemented")
                }
            }
        }
    }
}

/// Contextual information accessible from mock HTTP callbacks.
pub struct Context {
    instance: Mutex<Option<InstanceContext>>,
    _config: Config,
    log: Logger,
}

impl Context {
    pub fn new(config: Config, log: Logger) -> Self {
        Context { instance: Mutex::new(None), _config: config, log }
    }
}

#[endpoint {
    method = PUT,
    path = "/instance",
}]
async fn instance_ensure(
    rqctx: Arc<RequestContext<Context>>,
    request: TypedBody<api::InstanceEnsureRequest>,
) -> Result<HttpResponseCreated<api::InstanceEnsureResponse>, HttpError> {
    let server_context = rqctx.context();
    let request = request.into_inner();
    let (properties, nics, disks, cloud_init_bytes) = (
        request.properties,
        request.nics,
        request.disks,
        request.cloud_init_bytes,
    );

    // Handle an already-initialized instance
    let mut instance = server_context.instance.lock().await;
    if let Some(instance) = &*instance {
        if instance.properties != properties {
            return Err(HttpError::for_internal_error(
                "Cannot update running server".to_string(),
            ));
        }
        return Ok(HttpResponseCreated(api::InstanceEnsureResponse {
            migrate: None,
        }));
    }

    // Perform some basic validation of the requested properties
    for nic in &nics {
        info!(server_context.log, "Creating NIC: {:#?}", nic);
        slot_to_pci_path(nic.slot, SlotType::Nic).map_err(|e| {
            let err = IoError::new(
                ErrorKind::InvalidData,
                format!("Cannot parse vnic PCI: {}", e),
            );
            HttpError::for_internal_error(format!(
                "Cannot build instance: {}",
                err
            ))
        })?;
    }

    for disk in &disks {
        info!(server_context.log, "Creating Disk: {:#?}", disk);
        slot_to_pci_path(disk.slot, SlotType::Disk).map_err(|e| {
            let err = IoError::new(
                ErrorKind::InvalidData,
                format!("Cannot parse disk PCI: {}", e),
            );
            HttpError::for_internal_error(format!(
                "Cannot build instance: {}",
                err
            ))
        })?;
        info!(server_context.log, "Disk {} created successfully", disk.name);
    }

    if let Some(cloud_init_bytes) = &cloud_init_bytes {
        info!(server_context.log, "Creating cloud-init disk");
        slot_to_pci_path(api::Slot(0), SlotType::CloudInit).map_err(|e| {
            let err = IoError::new(ErrorKind::InvalidData, e.to_string());
            HttpError::for_internal_error(format!(
                "Cannot build instance: {}",
                err
            ))
        })?;
        base64::decode(&cloud_init_bytes).map_err(|e| {
            let err = IoError::new(ErrorKind::InvalidInput, e.to_string());
            HttpError::for_internal_error(format!(
                "Cannot build instance: {}",
                err
            ))
        })?;
        info!(server_context.log, "cloud-init disk created");
    }

    *instance = Some(InstanceContext::new(properties));
    Ok(HttpResponseCreated(api::InstanceEnsureResponse { migrate: None }))
}

#[endpoint {
    method = GET,
    path = "/instance",
}]
async fn instance_get(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseOk<api::InstanceGetResponse>, HttpError> {
    let instance = rqctx.context().instance.lock().await;
    let instance = instance.as_ref().ok_or_else(|| {
        HttpError::for_internal_error(
            "Server not initialized (no instance)".to_string(),
        )
    })?;
    let instance_info = api::Instance {
        properties: instance.properties.clone(),
        state: instance.state.clone(),
        disks: vec![],
        nics: vec![],
    };
    Ok(HttpResponseOk(api::InstanceGetResponse { instance: instance_info }))
}

#[endpoint {
    method = GET,
    path = "/instance/state-monitor",
}]
async fn instance_state_monitor(
    rqctx: Arc<RequestContext<Context>>,
    request: TypedBody<api::InstanceStateMonitorRequest>,
) -> Result<HttpResponseOk<api::InstanceStateMonitorResponse>, HttpError> {
    let (mut state_watcher, gen) = {
        let instance = rqctx.context().instance.lock().await;
        let instance = instance.as_ref().ok_or_else(|| {
            HttpError::for_internal_error(
                "Server not initialized (no instance)".to_string(),
            )
        })?;
        let gen = request.into_inner().gen;
        let state_watcher = instance.state_watcher_rx.clone();
        (state_watcher, gen)
    };

    loop {
        let last = state_watcher.borrow().clone();
        if gen <= last.gen {
            let response = api::InstanceStateMonitorResponse {
                gen: last.gen,
                state: last.state,
            };
            return Ok(HttpResponseOk(response));
        }
        state_watcher.changed().await.unwrap();
    }
}

#[endpoint {
    method = PUT,
    path = "/instance/state",
}]
async fn instance_state_put(
    rqctx: Arc<RequestContext<Context>>,
    request: TypedBody<api::InstanceStateRequested>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let mut instance = rqctx.context().instance.lock().await;
    let instance = instance.as_mut().ok_or_else(|| {
        HttpError::for_internal_error(
            "Server not initialized (no instance)".to_string(),
        )
    })?;
    let requested_state = request.into_inner();
    instance.set_target_state(requested_state).map_err(|err| {
        HttpError::for_internal_error(format!("Failed to transition: {}", err))
    })?;
    Ok(HttpResponseUpdatedNoContent {})
}

#[channel {
    protocol = WEBSOCKETS,
    path = "/instance/serial",
}]
async fn instance_serial(
    _rqctx: Arc<RequestContext<Context>>,
    websock: WebsocketConnection,
) -> dropshot::WebsocketChannelResult {
    let config =
        WebSocketConfig { max_send_queue: Some(4096), ..Default::default() };
    let mut ws_stream = WebSocketStream::from_raw_socket(
        websock.into_inner(),
        Role::Server,
        Some(config),
    )
        .await;

    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        ws_stream.send(Message::binary("asdf\n")).await?;
    }
}

/// Returns a Dropshot [`ApiDescription`] object to launch a mock Propolis
/// server.
pub fn api() -> ApiDescription<Context> {
    let mut api = ApiDescription::new();
    api.register(instance_ensure).unwrap();
    api.register(instance_get).unwrap();
    api.register(instance_state_monitor).unwrap();
    api.register(instance_state_put).unwrap();
    api.register(instance_serial).unwrap();
    api
}

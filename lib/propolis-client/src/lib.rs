// Copyright 2022 Oxide Computer Company
//! A client for the Propolis hypervisor frontend's server API.
//!
//! It is being experimentally migrated to `progenitor` for auto-generation,
//! which is opt-in at present with crate feature `generated`, and additional
//! compatibility impls and re-exports to approximate the former handmade
//! bindings' module layout with crate feature `generated-migration`.
//!
//! Presently, when built with the `generated` flag, the legacy handmade
//! bindings are available in the `handmade` submodule.

#![cfg_attr(
    feature = "generated",
    doc = "This documentation was built with the `generated` feature **on**."
)]
#![cfg_attr(
    not(feature = "generated"),
    doc = "This documentation was built with the `generated` feature **off**."
)]

pub mod instance_spec;

#[cfg(feature = "generated")]
mod generated;
#[cfg(feature = "generated")]
pub use generated::*;

#[cfg(feature = "generated")]
pub mod handmade;
#[cfg(not(feature = "generated"))]
mod handmade;
#[cfg(not(feature = "generated"))]
pub use handmade::*;

#[cfg(feature = "generated-migration")]
pub use types as api;
#[cfg(feature = "generated-migration")]
mod _compat_impls {
    use super::{generated, handmade};

    impl From<handmade::api::DiskRequest> for generated::types::DiskRequest {
        fn from(req: handmade::api::DiskRequest) -> Self {
            let handmade::api::DiskRequest {
                name,
                slot,
                read_only,
                device,
                volume_construction_request,
            } = req;
            Self {
                name,
                slot: slot.into(),
                read_only,
                device,
                volume_construction_request: volume_construction_request.into(),
            }
        }
    }

    impl From<handmade::api::Slot> for generated::types::Slot {
        fn from(slot: handmade::api::Slot) -> Self {
            Self(slot.0)
        }
    }
}

#[cfg(feature = "generated")]
pub mod helpers {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::tungstenite::{Error, Message};
    use tokio_tungstenite::tungstenite::protocol::Role;
    use futures::{future, SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf};

    pub struct PropolisSerialConsoleStream {
        ws: WebSocketStream<reqwest::Upgraded>,
    }

    impl PropolisSerialConsoleStream {
        pub fn new(addr: std::net::SocketAddr, byte_offset: Option<i64>) {

        }

        async fn serial_connect(addr: &std::net::SocketAddr, byte_offset: Option<i64>) -> Result<WebSocketStream<reqwest::Upgraded>> {
            let client = crate::Client::new(&format!("http://{}", addr));
            let mut req = client.instance_serial();

            match byte_offset {
                Some(x) if x >= 0 => req = req.from_start(x as u64),
                Some(x) => req = req.most_recent(-x as u64),
                None => req = req.most_recent(16384),
            }
            let upgraded = req
                .send()
                .await
                .map_err(|e| anyhow!("Failed to upgrade connection: {}", e))?
                .into_inner();
            Ok(WebSocketStream::from_raw_socket(upgraded, Role::Client, None).await)
        }
    }

    impl futures::Stream for PropolisSerialConsoleStream {
        type Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            match self.ws.poll_next() {
                Poll::Ready(Some(Ok(Message::Text(ref json)))) => {
                    match serde_json::from_str(&json)? {
                        crate::handmade::InstanceSerialConsoleControlMessage::Migrating {
                            destination, from_start,
                        } => {
                            self.ws = Self::serial_connect(destination, Some(from_start as i64)).await?;
                        }
                    }
                    Poll::Pending
                }
                x => x,
            }
        }
    }

    async fn serial(
        addr: std::net::SocketAddr,
        byte_offset: Option<i64>,
    ) -> Result<()> {
        let mut ws = PropolisSerialConsoleStream::serial_connect(&addr, byte_offset).await?;

        let x = ws.next().await;
        let (sink, stream) = ws.split();

        loop {
            tokio::select! {
                c = wsrx.recv() => {
                    match c {
                        None => {
                            // channel is closed
                            break;
                        }
                        Some(c) => {
                            ws.send(Message::Binary(c)).await?;
                        },
                    }
                }
                msg = ws.next() => {
                    match msg {
                        Some(Ok(Message::Binary(input))) => {
                            stdout.write_all(&input).await?;
                            stdout.flush().await?;
                        }
                        Some(Ok(Message::Close(..))) | None => break,
                        Some(Ok(Message::Text(json))) => {
                            match serde_json::from_str(&json)? {
                                InstanceSerialConsoleControlMessage::Migrating {
                                    destination, from_start,
                                } => {
                                    ws = serial_connect(&destination, Some(from_start as i64)).await?;
                                }
                            }
                        }
                        _ => continue,
                    }
                }
            }
        }

        Ok(())
    }

}
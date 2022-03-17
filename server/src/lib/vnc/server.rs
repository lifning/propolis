use slog::{error, info, o, Logger};

use tokio::net::{TcpStream, TcpListener};
use std::net::SocketAddr;
use image::{GenericImageView, ImageResult, Rgba, io::Reader as ImageReader};
use std::io::{Read, Write};

use crate::vnc::rfb::{RfbProtoVersion, Message};

#[derive(Debug)]
pub struct RamFb {
    addr: u64,
    width: usize,
    height: usize,
}

impl RamFb {
    pub fn new(addr: u64, width: usize, height: usize) -> Self {
        Self { addr, width, height }
    }
}

enum Framebuffer {
    Uninitialized,
    Initialized(RamFb),
}

pub struct VncServer {
    port: u16,
    fb: Framebuffer,
    log: Logger,
}

impl VncServer {
    pub fn new(port: u16, log: Logger) -> Self {
        VncServer { port, fb: Framebuffer::Uninitialized, log }
    }

    pub fn start(&self) {
        let listen_addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let log = self.log.clone();

        tokio::spawn(async move {
            let listener = TcpListener::bind(listen_addr).await.unwrap();

            loop {
                let (stream, addr) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let conn = VncConnection::new(stream, addr, log);
                    conn.process().await;
                });
            }
        });
    }

    pub fn initialize_fb(&mut self, fb: RamFb) {
        self.fb = Framebuffer::Initialized(fb);
    }

    pub fn shutdown(&self) {
        unimplemented!()
    }
}

struct VncConnection {
    stream: TcpStream,
    addr: SocketAddr,
    log: Logger,
    //state: Rfb::RfbState,
}

impl VncConnection {
    fn new(stream: TcpStream, addr: SocketAddr, log: Logger) -> Self {
        VncConnection {
            stream,
            addr,
            log,
        }
    }

    async fn process(&self) {
        let (reader, mut writer) = self.stream.split();

        info!(self.log, "BEGIN: ProtocolVersion Handshake");
        let server_version = RfbProtoVersion::Rfb38;
        server_version.write_to(&mut writer).await.unwrap();

        let client_version: RfbProtoVersion = RfbProtoVersion::read_to(reader).await.unwrap();
        assert_eq!(server_version, client_version);
        info!(self.log, "END: ProtocolVersion Handshake\n");




    }
}

use slog::{error, info, o, Logger};

use std::net::{TcpStream, TcpListener};
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
            let listener = TcpListener::bind(listen_addr).unwrap();

            loop {
                let (stream, addr) = listener.accept().unwrap();
                let log = log.clone();
                tokio::spawn(async move {
                    let mut conn = VncConnection::new(stream, addr, log);
                    conn.process();
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

    fn process(&mut self) {
        info!(self.log, "BEGIN: ProtocolVersion Handshake");
        let server_version = RfbProtoVersion::Rfb38;
        server_version.write_to(&mut self.stream).unwrap();

        let client_version: RfbProtoVersion = RfbProtoVersion::read_from(&mut self.stream).unwrap();
        assert_eq!(server_version, client_version);
        info!(self.log, "END: ProtocolVersion Handshake\n");
    }
}

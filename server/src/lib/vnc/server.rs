use slog::{error, info, o, Logger};

use image::{io::Reader as ImageReader, GenericImageView, ImageResult, Rgba};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};

use crate::vnc::rfb::{
    ClientInit, Message, ProtoVersion, SecurityResult, SecurityType,
    SecurityTypes, ServerInit, FramebufferUpdate, Rectangle, Encoding,
};

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
        VncConnection { stream, addr, log }
    }

    fn process(&mut self) {
        info!(self.log, "BEGIN: ProtocolVersion Handshake");

        info!(self.log, "tx: ProtocolVersion");
        let server_version = ProtoVersion::Rfb38;
        server_version.write_to(&mut self.stream).unwrap();

        info!(self.log, "rx: ProtocolVersion");
        let client_version: ProtoVersion =
            ProtoVersion::read_from(&mut self.stream).unwrap();
        assert_eq!(server_version, client_version);

        info!(self.log, "END: ProtocolVersion Handshake\n");


        info!(self.log, "BEGIN: Security Handshake");

        info!(self.log, "tx: SecurityTypes");
        let security_types = SecurityTypes(vec![SecurityType::None]);
        security_types.write_to(&mut self.stream).unwrap();

        info!(self.log, "rx: SecurityResult");
        let security_result =
            SecurityResult::read_from(&mut self.stream).unwrap();
        assert_eq!(security_result, SecurityResult::Ok);

        info!(self.log, "END: Security Handshake\n");


        info!(self.log, "BEGIN: Initialization");

        info!(self.log, "rx: ClientInit");
        let client_init = ClientInit::read_from(&mut self.stream).unwrap();
        assert_eq!(client_init, ClientInit::Shared);

        info!(self.log, "tx: ServerInit");
        let server_init = ServerInit::default();
        server_init.write_to(&mut self.stream).unwrap();

        info!(self.log, "END: Initialization\n");

        loop {
            let r = Rectangle::new(0, 0, 1024, 748, Encoding::Raw);
            let fbu = FramebufferUpdate::new(vec![r]);
            fbu.write_to(&mut self.stream).unwrap();
        }
    }
}

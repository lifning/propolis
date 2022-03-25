use propolis::dispatch::AsyncCtx;
use propolis::hw::qemu::ramfb::Config;
use slog::{error, info, o, Logger};

use image::{io::Reader as ImageReader, GenericImageView, ImageResult, Rgba};
use propolis::common::GuestAddr;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::vnc::rfb::{
    ClientInit, Encoding, FramebufferUpdate, Message, ProtoVersion, Rectangle,
    SecurityResult, SecurityType, SecurityTypes, ServerInit,
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
    fb: Arc<Mutex<Framebuffer>>,
    actx: Arc<Mutex<Option<AsyncCtx>>>,
    log: Logger,
}

impl VncServer {
    pub fn new(port: u16, log: Logger) -> Self {
        VncServer {
            port,
            fb: Arc::new(Mutex::new(Framebuffer::Uninitialized)),
            actx: Arc::new(tokio::sync::Mutex::new(None)),
            log,
        }
    }

    pub async fn set_async_ctx(&self, actx: AsyncCtx) {
        let mut locked = self.actx.lock().await;
        *locked = Some(actx);
    }

    pub fn start(&self) {
        let listen_addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let log = self.log.clone();
        let fb = Arc::clone(&self.fb);
        let actx = Arc::clone(&self.actx);
        info!(self.log, "vnc-server: starting...");

        tokio::spawn(async move {
            let listener = TcpListener::bind(listen_addr).unwrap();

            loop {
                let log = log.clone();
                let fb = fb.clone();
                let actx = actx.clone();
                let (stream, addr) = listener.accept().unwrap();
                info!(log, "vnc-server: got connection");
                tokio::spawn(async move {
                    info!(log, "vnc-server: spawned");
                    let mut conn =
                        VncConnection::new(stream, addr, fb, actx, log);
                    conn.process().await;
                })
                .await;
            }
        });
    }

    pub async fn initialize_fb(&mut self, fb: RamFb) {
        if fb.addr != 0 {
            let mut locked = self.fb.lock().await;
            *locked = Framebuffer::Initialized(fb);
        }
    }

    pub fn shutdown(&self) {
        unimplemented!()
    }

    pub async fn update(&mut self, config: &Config, is_valid: bool) {
        if is_valid {
            info!(self.log, "updating framebuffer");
            let (addr, w, h) = config.get_fb_info();
            let fb = RamFb::new(addr, w as usize, h as usize);
            self.initialize_fb(fb).await;
        } else {
            info!(self.log, "invalid config update");
        }
    }
}

struct VncConnection {
    stream: TcpStream,
    addr: SocketAddr,
    fb: Arc<Mutex<Framebuffer>>,
    actx: Arc<tokio::sync::Mutex<Option<AsyncCtx>>>,
    log: Logger,
    //state: Rfb::RfbState,
}

impl VncConnection {
    fn new(
        stream: TcpStream,
        addr: SocketAddr,
        fb: Arc<Mutex<Framebuffer>>,
        actx: Arc<tokio::sync::Mutex<Option<AsyncCtx>>>,
        log: Logger,
    ) -> Self {
        VncConnection { stream, addr, fb, actx, log }
    }

    async fn process(&mut self) {
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

        info!(self.log, "rx: SecurityType");
        let client_sectype = SecurityType::read_from(&mut self.stream).unwrap();
        assert_eq!(client_sectype, SecurityType::None);

        info!(self.log, "tx: SecurityResult");
        let sec_res = SecurityResult::Ok;
        sec_res.write_to(&mut self.stream).unwrap();

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
            let locked = self.fb.lock().await;
            match &*locked {
                Framebuffer::Uninitialized => {
                    info!(self.log, "uninitialized");

                    const len: usize = 1024 * 768 * 4;
                    let mut pixels = vec![0u8; len];
                    for i in 0..len {
                        //if i % 4 == 1 {
                        pixels[i] = 0xff;
                        //}
                    }
                    let r =
                        Rectangle::new(0, 0, 1024, 768, Encoding::Raw, pixels);
                    let fbu = FramebufferUpdate::new(vec![r]);
                    fbu.write_to(&mut self.stream).unwrap();
                }
                Framebuffer::Initialized(fb) => {
                    info!(self.log, "initialized={:?}", fb);

                    let len = fb.height * fb.width * 4;
                    let mut buf = vec![0u8; len];

                    let locked = self.actx.lock().await;
                    let actx = locked.as_ref().unwrap();
                    let memctx = actx.dispctx().await.unwrap().mctx.memctx();
                    let read =
                        memctx.read_into(GuestAddr(fb.addr), &mut buf, len);
                    drop(memctx);

                    assert!(read.is_some());
                    info!(self.log, "read {} bytes from guest", read.unwrap());

                    let r = Rectangle::new(
                        0,
                        0,
                        fb.width as u16,
                        fb.height as u16,
                        Encoding::Raw,
                        buf,
                    );
                    let fbu = FramebufferUpdate::new(vec![r]);
                    fbu.write_to(&mut self.stream).unwrap();

                    //sleep(Duration::from_millis()).await;
                }
            }
        }
    }
}

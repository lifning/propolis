// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use propolis::hw::ps2::ctrl::PS2Ctrl;
use propolis::hw::qemu::ramfb::{FrameSnap, RamFb};
use propolis::hw::usb::usbdev::vnc_tablet::HIDTabletReport;

use futures::StreamExt;
use rfb::encodings::{ConnectionContext, EncodingType};
use rfb::proto::{
    ClientMessage, FramebufferUpdate, FramebufferUpdateRequest, Position,
    ProtoVersion, ProtocolError, Rectangle, Resolution, SecurityType,
    SecurityTypes,
};
use rgb_frame::{FourCC, Frame, Spec};
use slog::{error, trace, Logger};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_util::codec::FramedRead;

/// Arbitrary maximum valid resolution
const MAX_RES: Resolution = Resolution { width: 1920, height: 1200 };
const UNINIT_RES: Resolution = Resolution { width: 800, height: 600 };
const UNINIT_FOURCC: FourCC = FourCC::XR24;
const SERVER_NAME: &str = "propolis-vnc";
/// Frame interval for 30 frames per second limit
const FRAME_US_30FPS: Duration = Duration::from_micros(1_000_000 / 30);

struct Devices {
    keyboard: Arc<PS2Ctrl>,
    tablet: Arc<Mutex<HIDTabletReport>>,
    display: Arc<RamFb>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum FrameKind {
    Valid,
    Generated,
}

#[derive(Default)]
struct State {
    devices: Option<Devices>,
    is_stopped: bool,
}

struct ClientState {
    last_snap: Option<(FrameSnap, FrameKind)>,
    last_snap_client: Option<(FrameSnap, FrameKind)>,
    fbu_req: Option<FramebufferUpdateRequest>,
    encodings: BTreeSet<EncodingType>,
    output_fourcc: FourCC,
    connection_context: ConnectionContext,
    serialize_buffer: Vec<u8>,
}
impl Default for ClientState {
    fn default() -> Self {
        Self {
            last_snap: None,
            last_snap_client: None,
            fbu_req: None,
            encodings: BTreeSet::new(),
            output_fourcc: UNINIT_FOURCC,
            connection_context: ConnectionContext::default(),
            serialize_buffer: Vec::new(),
        }
    }
}
impl ClientState {
    fn preferred_available_encoding(&self) -> EncodingType {
        use EncodingType::*;
        // TightPNG, Tight(JPEG), and JPEG are preferred when available
        // because browser-based clients (i.e. noVNC) can utilize native-code
        // image/png and image/jpeg decoders for better performance than
        // JS implementations of the standard RFC6143-defined encodings.
        for enc in [TightPNG, Tight, JPEG, ZRLE, Zlib, TRLE] {
            if self.encodings.contains(&enc) {
                return enc;
            }
        }
        Raw
    }
}

#[derive(Default)]
pub struct Client {
    hup: Option<oneshot::Sender<()>>,
    id: Option<String>,
}

pub struct VncServer {
    state: Mutex<State>,
    client: Mutex<Client>,
    notify: Notify,
    /// Minimum frame interval
    frame_interval: Duration,
    log: Logger,
}

#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("Invalid FourCC {0}")]
    InvalidFourCC(u32),
    #[error("VNC initialization error {0:?}")]
    InitError(#[from] rfb::server::InitError),
    #[error("VNC server is stopped")]
    ServerStopped,
}

/// Alias trait to cut down on verbosity
pub trait Connection: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Connection
    for rfb::tungstenite::BinaryWs<T>
{
}
impl Connection for tokio::net::TcpStream {}
impl Connection for Box<dyn Connection> {}

impl VncServer {
    pub fn new(log: Logger) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            client: Mutex::new(Client::default()),
            notify: Notify::new(),
            frame_interval: FRAME_US_30FPS,
            log,
        })
    }
    pub fn attach(
        &self,
        ps2: Arc<PS2Ctrl>,
        usb_tablet: Arc<Mutex<HIDTabletReport>>,
        fb: Arc<RamFb>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.devices =
            Some(Devices { keyboard: ps2, tablet: usb_tablet, display: fb });
    }
    pub async fn connect(
        self: &Arc<Self>,
        mut conn: impl Connection,
        client_id: String,
    ) -> Result<(), ConnectError> {
        let (resolution, fourcc) = {
            let state = self.state.lock().unwrap();
            if state.is_stopped {
                return Err(ConnectError::ServerStopped);
            }
            match state.devices.as_ref().map(|devs| devs.display.read_spec()) {
                Some(Ok(spec)) if spec_valid(&spec) => (
                    Resolution {
                        width: spec.width.get() as u16,
                        height: spec.height.get() as u16,
                    },
                    spec.fourcc,
                ),
                _ => (UNINIT_RES, UNINIT_FOURCC),
            }
        };

        let _client_init = rfb::server::initialize(
            &mut conn,
            rfb::server::InitParams {
                version: ProtoVersion::Rfb38,
                // vncviewer won't work without offering VncAuth, even though it
                // doesn't ask to use it.
                sec_types: SecurityTypes(vec![
                    SecurityType::None,
                    SecurityType::VncAuthentication,
                ]),
                name: SERVER_NAME.to_string(),
                resolution,
                format: fourcc.into(),
            },
        )
        .await?;

        let hup_recv = self.replace_client(client_id).await;

        let this = self.clone();
        tokio::spawn(async move {
            if let Err(e) = this.run(conn, hup_recv).await {
                error!(this.log, "VNC error, hanging up: {:?}", e);
            }
            this.hup_client();
        });

        Ok(())
    }

    async fn replace_client(&self, new_id: String) -> oneshot::Receiver<()> {
        let mut client = self.wait_client_gone().await;

        let (send, recv) = oneshot::channel();
        client.id = Some(new_id);
        client.hup = Some(send);

        recv
    }
    fn hup_client(&self) {
        let mut client = self.client.lock().unwrap();
        client.hup.take();
        client.id.take();
        self.notify.notify_one();
    }
    async fn wait_client_gone(&self) -> MutexGuard<'_, Client> {
        loop {
            {
                let mut client = self.client.lock().unwrap();
                // tell any existing client to hang up
                if let Some(hup) = client.hup.take() {
                    let _ = hup.send(());
                }
                // and once it is gone, go on to install ourself as active
                if client.id.is_none() {
                    return client;
                }
                drop(client);
            }

            self.notify.notified().await;
        }
    }

    async fn run(
        &self,
        conn: impl Connection,
        mut close_recv: oneshot::Receiver<()>,
    ) -> Result<(), ProtocolError> {
        let mut decoder =
            FramedRead::new(conn, rfb::proto::ClientMessageDecoder::default());
        let mut cstate: ClientState = Default::default();
        loop {
            tokio::select! {
                biased;

                _ = &mut close_recv => {
                    return Ok(());
                },
                msg = decoder.next() => {
                    let msg = match msg {
                        Some(Err(e)) => {
                            return Err(e);
                        }
                        None => {
                            // Client disconnect
                            return Ok(());
                        }
                        Some(Ok(m)) => m,
                    };
                    self.handle_msg(decoder.get_mut(), msg, &mut cstate).await;
                }
                _ = self.wait_for_next_frame(&mut cstate) => {
                    self.send_fbu(decoder.get_mut(), &mut cstate).await?;
                }
            }
        }
    }

    async fn handle_msg(
        &self,
        _conn: &mut impl Connection,
        msg: ClientMessage,
        cstate: &mut ClientState,
    ) {
        match msg {
            ClientMessage::KeyEvent(ke) => {
                let state = self.state.lock().unwrap();
                trace!(self.log, "VNC key event: {:?}", ke);
                if let Some(devs) = state.devices.as_ref() {
                    devs.keyboard.key_event(ke);
                }
            }
            ClientMessage::PointerEvent(pe) => {
                let state = self.state.lock().unwrap();
                trace!(self.log, "VNC pointer event: {:?}", pe);
                if let Some(devs) = state.devices.as_ref() {
                    if let Ok(spec) = devs.display.read_spec() {
                        devs.tablet.lock().unwrap().pointer_event(pe, spec);
                    }
                }
            }
            ClientMessage::ClientCutText(_) => {
                trace!(self.log, "Ignoring VNC CutText request");
            }
            ClientMessage::FramebufferUpdateRequest(req) => {
                cstate.fbu_req = Some(req);
            }
            ClientMessage::SetPixelFormat(pf) => match (&pf).try_into() {
                Ok(fourcc) => {
                    cstate.output_fourcc = fourcc;
                    // Convert any existing frame to the new format
                    if let Some((snap, _kind)) = cstate.last_snap.as_mut() {
                        snap.frame.convert(fourcc);
                    }
                    if let Some((snap, _kind)) =
                        cstate.last_snap_client.as_mut()
                    {
                        snap.frame.convert(fourcc);
                    }
                }
                Err(e) => {
                    slog::warn!(
                        self.log,
                        "Unhandled SetPixelFormat({:?}): {e}",
                        pf
                    );
                }
            },
            ClientMessage::SetEncodings { encodings, unknown } => {
                cstate.encodings = encodings.into_iter().collect();
                slog::trace!(self.log, "SetEncodings({:?})", cstate.encodings);
                if !unknown.is_empty() {
                    slog::debug!(
                        self.log,
                        "Unrecognized SetEncodings values: {:?}",
                        unknown
                    );
                }
            }
        }
    }

    async fn send_fbu(
        &self,
        conn: &mut impl Connection,
        cstate: &mut ClientState,
    ) -> Result<(), ProtocolError> {
        let (serv_snap, _kind) = cstate.last_snap.as_ref().unwrap();
        let FramebufferUpdateRequest { incremental, position: pos, resolution } =
            *cstate.fbu_req.as_ref().unwrap_or(&FramebufferUpdateRequest {
                incremental: false,
                position: Position { x: 0, y: 0 },
                resolution: Resolution {
                    width: serv_snap.frame.spec().width.get() as u16,
                    height: serv_snap.frame.spec().height.get() as u16,
                },
            });
        let Resolution { width, height } = resolution;
        let fbu = if !incremental || cstate.last_snap_client.is_none() {
            let subframe = serv_snap.frame.subframe(
                &(pos.x as usize..width as usize),
                &(pos.y as usize..height as usize),
            );
            FramebufferUpdate(vec![Rectangle {
                position: pos,
                dimensions: Resolution { width, height },
                data: cstate.preferred_available_encoding().from(subframe),
            }])
        } else {
            // unwrap: !is_none
            let (client_snap, _kind) =
                cstate.last_snap_client.as_ref().unwrap();
            const STEP: usize = 128;
            let mut rectangles = vec![];
            for sub_y in (pos.y..pos.y + height).step_by(STEP) {
                let sub_y = sub_y as usize;
                for sub_x in (pos.x..pos.x + width).step_by(STEP) {
                    let sub_x = sub_x as usize;
                    let x_range = sub_x..sub_x + STEP;
                    let y_range = sub_y..sub_y + STEP;

                    // find first different row
                    let client_rows =
                        client_snap.frame.pixels_of_region(&x_range, &y_range);
                    let serv_rows =
                        serv_snap.frame.pixels_of_region(&x_range, &y_range);

                    let Some(top) = client_rows.zip(serv_rows).position(
                        |(c_row, s_row)| {
                            c_row.zip(s_row).any(|(c_px, s_px)| c_px != s_px)
                        },
                    ) else {
                        continue; // no difference in this subregion
                    };

                    // again, but in reverse (rposition) to get last row
                    let client_rows =
                        client_snap.frame.pixels_of_region(&x_range, &y_range);
                    let serv_rows =
                        serv_snap.frame.pixels_of_region(&x_range, &y_range);
                    let bottom = client_rows
                        .zip(serv_rows)
                        .rposition(|(c_row, s_row)| {
                            c_row.zip(s_row).any(|(c_px, s_px)| c_px != s_px)
                        })
                        // unwrap: we know there's a difference
                        // because we didn't continue; above
                        .unwrap();

                    // (+1 rather than RangeInclusive, because different types)
                    let diff_y_range = sub_y + top..sub_y + bottom + 1;

                    // now get first different column between those
                    let client_rows = client_snap
                        .frame
                        .pixels_of_region(&x_range, &diff_y_range);
                    let serv_rows = serv_snap
                        .frame
                        .pixels_of_region(&x_range, &diff_y_range);
                    let left = client_rows
                        .zip(serv_rows)
                        .filter_map(|(c_row, s_row)| {
                            c_row
                                .zip(s_row)
                                .position(|(c_px, s_px)| c_px != s_px)
                        })
                        .min()
                        // unwrap: as above
                        .unwrap();

                    // and last different column, max(rposition)
                    let client_rows = client_snap
                        .frame
                        .pixels_of_region(&x_range, &diff_y_range);
                    let serv_rows = serv_snap
                        .frame
                        .pixels_of_region(&x_range, &diff_y_range);
                    let right = client_rows
                        .zip(serv_rows)
                        .filter_map(|(c_row, s_row)| {
                            c_row
                                .zip(s_row)
                                .rposition(|(c_px, s_px)| c_px != s_px)
                        })
                        .max()
                        // unwrap: as above
                        .unwrap();

                    let diff_x_range = sub_x + left..sub_x + right + 1;

                    let subframe =
                        serv_snap.frame.subframe(&diff_x_range, &diff_y_range);

                    rectangles.push(Rectangle {
                        position: Position {
                            x: diff_x_range.start as u16,
                            y: diff_y_range.start as u16,
                        },
                        dimensions: Resolution {
                            width: subframe.width() as u16,
                            height: subframe.height() as u16,
                        },
                        // TODO: below a certain subframe.raw_size(), just Raw?
                        data: cstate
                            .preferred_available_encoding()
                            .from(subframe),
                    });
                }
            }
            FramebufferUpdate(rectangles)
        };

        fbu.write_to(
            conn,
            &mut cstate.connection_context,
            &mut cstate.serialize_buffer,
        )
        .await?;
        conn.flush().await?;

        // With the FBU sent, the existing request is fulfilled
        cstate.fbu_req = None;
        // and we now know what the client's screen looks like,
        // for the next frame to diff from as necessary
        cstate.last_snap_client = cstate.last_snap.take();

        Ok(())
    }

    fn update_frame(&self, cstate: &mut ClientState) -> bool {
        let state = self.state.lock().unwrap();

        if let Some(mut new_valid_frame) = state
            .devices
            .as_ref()
            .and_then(|devs| devs.display.read_framebuffer(spec_valid))
        {
            new_valid_frame.frame.convert(cstate.output_fourcc);
            cstate.last_snap = Some((new_valid_frame, FrameKind::Valid));
            true
        } else {
            match cstate.last_snap.as_ref() {
                Some((_, FrameKind::Generated)) => {
                    // Reuse existing generated frame
                    false
                }
                _ => {
                    // Fill out a blank frame if none is already in place
                    cstate.last_snap = Some((
                        blank_frame(cstate.output_fourcc),
                        FrameKind::Generated,
                    ));
                    true
                }
            }
        }
    }
    async fn wait_for_next_frame(&self, cstate: &mut ClientState) {
        if cstate.fbu_req.is_none() {
            // If an update has not been requested, we will wait indefinitely
            futures::future::pending::<()>().await;
        }

        loop {
            let wait_len = match (cstate.last_snap.as_ref())
                .or(cstate.last_snap_client.as_ref())
                .map(|(frame, kind)| (kind, frame.when.elapsed()))
            {
                None | Some((FrameKind::Generated, _)) => {
                    // If there is no previous frame, or the existing frame is a
                    // generated blank, do not delay in attempting an update.
                    if self.update_frame(cstate) {
                        return;
                    }
                    // If the update resulted in no change, wait the default
                    // interval to check again
                    self.frame_interval
                }
                Some((FrameKind::Valid, age)) => {
                    let since_last = age;
                    if since_last >= self.frame_interval {
                        self.update_frame(cstate);
                        return;
                    }
                    self.frame_interval
                        .checked_sub(since_last)
                        .unwrap_or_default()
                }
            };
            sleep(wait_len).await
        }
    }

    pub async fn stop(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.is_stopped = true;
            state.devices = None;
        }

        let _client = self.wait_client_gone().await;
    }
}

/// TCP socket listener for VNC client connections
pub struct TcpSock {
    join_hdl: JoinHandle<()>,
    hup_send: oneshot::Sender<()>,
}
impl TcpSock {
    pub async fn new(
        vnc: Arc<VncServer>,
        addr: SocketAddr,
        log: Logger,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (hup_send, hup_recv) = oneshot::channel::<()>();
        let join_hdl = tokio::spawn(async move {
            Self::run(listener, vnc, hup_recv, log).await;
        });
        Ok(Self { join_hdl, hup_send })
    }
    pub async fn halt(self) {
        let Self { join_hdl, hup_send } = self;

        // Signal the socket listener to hang up, then wait for it to bail
        let _ = hup_send.send(());
        let _ = join_hdl.await;
    }
    async fn run(
        listener: TcpListener,
        vnc: Arc<VncServer>,
        mut hup_recv: oneshot::Receiver<()>,
        log: Logger,
    ) {
        loop {
            tokio::select! {
                biased;

                _ = &mut hup_recv => {
                    return;
                },
                sock_res = listener.accept() => {
                    match sock_res {
                        Ok((sock, addr)) => {
                            let conn_res = vnc.connect(
                                Box::new(sock) as Box<dyn Connection + 'static>,
                                addr.to_string(),
                            )
                            .await;
                            if let Err(e) = conn_res {
                                error!(&log, "Error during VNC connection: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!(&log, "VNC TCP listener error: {:?}", e);
                        }
                    }
                },
            };
        }
    }
}

/// Generate a black "filler" frame of default size/format
fn blank_frame(fourcc: FourCC) -> FrameSnap {
    // Generate a new "filler" frame, if one isn't already in place
    //
    // The default buffer contents are all zeroes, which will be black in any of
    // the currently supported FourCC formats
    FrameSnap {
        frame: Frame::new(Spec::new(
            UNINIT_RES.width as usize,
            UNINIT_RES.height as usize,
            fourcc,
        )),
        when: Instant::now(),
    }
}

/// Check that Spec derived from the framebuffer config is:
/// - Of an appropriate size (not zero or > 1920x1200
fn spec_valid(spec: &Spec) -> bool {
    spec.width.get() < MAX_RES.width as usize
        && spec.height.get() < MAX_RES.height as usize
}

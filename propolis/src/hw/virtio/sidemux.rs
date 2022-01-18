use std::sync::{Arc, Mutex};
use std::num::NonZeroU16;
use std::io::Result;
use std::convert::TryInto;

use crate::hw::pci;
use crate::dispatch::{AsyncCtx, DispCtx};
use crate::instance;
use super::pci::{PciVirtio, PciVirtioState};
use super::queue::{Chain, VirtQueue, VirtQueues};
use super::VirtioDevice;
use crate::util::regmap::RegMap;
use super::bits::*;
use super::viona::{
    bits::VIRTIO_NET_S_LINK_UP,
};
use crate::common::*;
use crate::vmm::MemCtx;

use slog::{Logger, debug, warn, error};
use lazy_static::lazy_static;
use rand::Rng;

/// The sidecar ethertype, also refered to as the service acess point (SAP) by
/// dlpi, is the cue in the ethernet header we use on ingress packet processing
/// to identify host-bound packets from a sidecar.
const SIDECAR_ETHERTYPE: u32 = 0x0901;

/// The fixed size of a sidecar header payload
const SIDECAR_PAYLOAD_SIZE: usize = 16;

/// Size of the sidecar L2 header.
const SIDECAR_HDR_SIZE: usize = 23;

/// MTU (currently assuming 1500) plus the sidecar header size which is 23
const MTU_PLUS_SIDECAR: usize = 1523;

/// Only supporting 48-bit MACs
const ETHERADDRL: usize = 6;

/// Layer 2 ethernet frame size, assuming no vlan tags.
const ETHERNET_FRAME_SIZE: usize = 14;

/// Sidemux is an emulated sidecar switch device.
///
/// It takes packets from a Tofino simulator encapsulated with a layer 2
/// sidecar header. On ingress sidemux looks at each sidecar header to
/// determine which PciVirtioSidemux to send the packet out. Before sending
/// packets to a PciVirtioSidemux device, the sidecar header is removed. On
/// ingress from the guest, Sidemux adds a sidecar header to indicate which
/// port the packet came from and pushes the packet to the Tofino simulator.
///
/// Communications with the simulator are handled through DLPI as the packets
/// carry a custom ethertype.
pub struct Sidemux {

    /// Name data link sidemux will communicate with the Tofino simulator over.
    pub link_name: String,

    /// Switch ports exposed to guest as virtio-net devices.
    pub ports: Vec<Arc::<PciVirtioSidemux>>,

    /// DLPI handle for simulator link
    sim_dh: dlpi::DlpiHandle,

    /// Logging instance
    log: slog::Logger

}

impl Sidemux {

    /// Create a new sidemux device with the specified port radix. Once
    /// activated, a dlpi link will be created on the specified interface for
    /// sidecar L2 traffic handling.
    pub fn new(
        radix: usize,
        link_name: String,
        queue_size: u16,
        log: slog::Logger
    ) -> Result<Arc<Self>> {

        let sim_dh = dlpi::open(&link_name, dlpi::sys::DLPI_RAW)?;
        dlpi::bind(sim_dh, SIDECAR_ETHERTYPE)?;

        let mut rng = rand::thread_rng();

        let mut ports = Vec::new();
        for i in 0..radix {

            // Create a MAC address with the Oxide OUI per RFD 174
            let m = rng.gen_range::<u32, _>(0xf00000..0xffffff).to_le_bytes();
            let mac = [0xa8,0x40,0x25,m[0],m[1],m[2]];
            let log = log.clone();
            ports.push(PciVirtioSidemux::new(i, queue_size, mac, sim_dh, log)?);
        }

        Ok(Arc::new(Sidemux{link_name, ports, sim_dh, log}))

    }

}

impl Entity for Sidemux {

    fn type_name(&self) -> &'static str {
        "sidemux"
    }

    fn reset(&self, _ctx: &DispCtx) { }

    fn state_transition(
        &self,
        next: instance::State,
        _target: Option<instance::State>,
        _ctx: &DispCtx,
    ) {
        match next {
            instance::State::Boot => {
                simulator_io_handler(
                    self.sim_dh,
                    self.ports.clone(),
                    self.log.clone(),
                ).unwrap();
            }
            _ => {}
        }
    }
}


/// PciVirtioSidemuxPort is a PCI device exposed to the guest as a virtio-net
/// device. This device represents a sidecar switch port.
pub struct PciVirtioSidemux {

    /// What switch port index this port is.
    index: usize,

    /// Underlying virtio state
    virtio_state: PciVirtioState,

    /// Underlying PCI device state
    pci_state: pci::DeviceState,

    /// Mac address of this port
    mac_addr: [u8; ETHERADDRL],
    
    /// DLPI handle for simulator link
    sim_dh: dlpi::DlpiHandle,

    /// Logging instance
    log: slog::Logger,

    /// Dispatch context for interacting with guest
    dispatch_context: Mutex::<Option::<AsyncCtx>>
}

impl PciVirtioSidemux {

    pub fn new(
        index: usize,
        queue_size: u16,
        mac_addr: [u8; ETHERADDRL],
        sim_dh: dlpi::DlpiHandle,
        log: slog::Logger,
    ) -> Result<Arc<Self>> {

        let queues = VirtQueues::new(
            NonZeroU16::new(queue_size).unwrap(),
            NonZeroU16::new(2).unwrap(), //TX and RX
        );
        let msix_count = Some(2);
        let (virtio_state, pci_state) = PciVirtioState::create(
            queues,
            msix_count,
            VIRTIO_DEV_NET,
            pci::bits::CLASS_NETWORK,
            VIRTIO_NET_CFG_SIZE,
        );
        let dispatch_context = Mutex::new(None);

        Ok(Arc::new(PciVirtioSidemux{
            index,
            virtio_state,
            pci_state,
            mac_addr,
            sim_dh,
            log,
            dispatch_context,
        }))

    }

    pub fn handle_req(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {

        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        match vq.pop_avail(&mut chain, mem) {
            Some(val) => val as usize,
            None => return,
        };

        // Create a statically allocated buffer to read ethernet frames from the
        // guest into.
        //
        // For reasons unknown to me, there are 10 bytes of leading spacing
        // before the ethernet frame coming from virtio. If we're considering
        // this a layer 1 frame from virtio, which I guess makes some amount of
        // sense, then this would account for the 7 bits of preamble and the 1
        // bit frame delimiter, but we are still left with 2 mystery bytes ....
        let mut frame = [0u8; MTU_PLUS_SIDECAR + 10];

        // skip 10 mystery bytes
        let n = read_buf(mem, &mut chain, &mut frame[..10]);

        // TODO This seems to happen somewhat regularly, and if we push_used
        // after a zero read, we enter a death spiral. So just return.
        if n == 0 {
            return;
        }
        if n < 10 {
            warn!(self.log, "short read ({})!", n);
            vq.push_used(&mut chain, mem, ctx);
            return;
        }

        // read in the ethernet header
        let eth = &mut frame[0..ETHERNET_FRAME_SIZE];
        let n = read_buf(mem, &mut chain, eth);
        if n < ETHERNET_FRAME_SIZE {
            warn!(self.log, "frame from guest too small for Ethernet");
            vq.push_used(&mut chain, mem, ctx);
            return;
        }
        // get orignial ethertype
        let ethertype = u16::from_be_bytes([eth[12], eth[13]]);
        // set ethertype to sidecar
        let b = (SIDECAR_ETHERTYPE as u16).to_be_bytes();
        eth[12] = b[0];
        eth[13] = b[1];

        // create a sidecar header, goes right after ethernet header
        let sc = &mut frame[
            ETHERNET_FRAME_SIZE
            ..
            ETHERNET_FRAME_SIZE+SIDECAR_HDR_SIZE
        ];
        // code
        sc[0] = packet::sidecar::SC_FORWARD_FROM_USERSPACE;
        // egress
        let b = (self.index as u16).to_be_bytes();
        sc[3] = b[0];
        sc[4] = b[1];
        // ethertype
        let b = (ethertype).to_be_bytes();
        sc[5] = b[0];
        sc[6] = b[1];

        // read in payload, goes right after sidecar header
        let payload = &mut frame[
            ETHERNET_FRAME_SIZE+SIDECAR_HDR_SIZE
            ..
        ];
        read_buf(mem, &mut chain, payload);

        // send encapped packet out external port
        match dlpi::send(self.sim_dh, &[], &frame, None) {
            Ok(_) => {},
            Err(e) => {
                error!(self.log, "tx (ext): {}", e);
            }
        };

        vq.push_used(&mut chain, mem, ctx);

    }

    pub fn tx_to_guest(&self, data: &[u8], ctx: &DispCtx) {

        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        let vq = &self.virtio_state.queues[0];
        match vq.pop_avail(&mut chain, mem) {
            Some(_) =>  {}
            None => {
                warn!(self.log, "[tx] pop_avail is none");
                return;
            }
        }

        write_buf(data, &mut chain, mem);
        vq.push_used(&mut chain, mem, ctx);

    }

    fn net_cfg_read(&self, id: &NetReg, ro: &mut ReadOp) {
        match id {
            NetReg::Mac => ro.write_bytes(&self.mac_addr),
            NetReg::Status => {
                // Always report link up
                ro.write_u16(VIRTIO_NET_S_LINK_UP);
            }
            NetReg::MaxVqPairs => {
                // hard-wired to single vq pair for now
                ro.write_u16(1);
            }
        }
    }

}


impl VirtioDevice for  PciVirtioSidemux {

    fn cfg_rw(&self, mut rwo: RWOp) {
        NET_DEV_REGS.process(&mut rwo, |id, rwo| match rwo {
            RWOp::Read(ro) => self.net_cfg_read(id, ro),
            RWOp::Write(_) => {
                //ignore writes
            }
        });
    }

    fn get_features(&self) -> u32 { VIRTIO_NET_F_MAC }

    fn set_features(&self, _feat: u32) { }

    fn queue_notify(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {
        self.handle_req(vq, ctx);
    }

}

impl Entity for PciVirtioSidemux {

    fn type_name(&self) -> &'static str {
        "pci-virtio-sidemux"
    }

    fn reset(&self, ctx: &DispCtx) {
        self.virtio_state.reset(self, ctx);
    }

    fn state_transition(
        &self,
        next: instance::State,
        _target: Option<instance::State>,
        ctx: &DispCtx,
    ) {
        match next {
            instance::State::Boot => {
                match self.dispatch_context.lock() {
                    Ok(mut opt_dc) => {
                        *opt_dc = Some(ctx.async_ctx());
                    }
                    Err(e) => {
                        error!(self.log, "lock dispatch context: {}", e);
                    }
                }
            }
            _ => {}
        }
    }
}

impl PciVirtio for PciVirtioSidemux {

    fn virtio_state(&self) -> &PciVirtioState { &self.virtio_state }

    fn pci_state(&self) -> &pci::DeviceState { &self.pci_state }
}

fn simulator_io_handler(
    dh: dlpi::DlpiHandle,
    ports: Vec::<Arc::<PciVirtioSidemux>>,
    log: Logger,
) -> Result<()> {

    tokio::spawn(async move { handle_simulator_packets(dh, ports, log).await });

    Ok(())

}

async fn handle_simulator_packets(
    dh: dlpi::DlpiHandle,
    ports: Vec::<Arc::<PciVirtioSidemux>>,
    log: Logger,
) {

    let mut src = [0u8; dlpi::sys::DLPI_PHYSADDR_MAX];
    let mut msg = [0u8; MTU_PLUS_SIDECAR];
    loop {
        // receive packet
        let mut recvinfo = dlpi::sys::dlpi_recvinfo_t::default();
        let n = match dlpi::recv_async(
            dh, &mut src, &mut msg, Some(&mut recvinfo)).await {

            Ok((_, n)) => {
                debug!(log, "sim rx: ({})", n);
                n
            }
            Err(e) => {
                error!(log, "sim rx: {}", e);
                continue;
            }


        };
        if n < SIDECAR_HDR_SIZE {
            warn!(log, "packet too small for sidecar header?");
            continue;
        }

        // extract relevant info from sidecar header
        let sch = packet::sidecar::SidecarHdr{
            sc_code: msg[14+0],
            sc_ingress: u16::from_be_bytes([msg[14+1], msg[14+2]]),
            sc_egress: u16::from_be_bytes([msg[14+3], msg[14+4]]),
            sc_ether_type: u16::from_be_bytes([msg[14+5], msg[14+6]]),
            sc_payload: msg[14+7..14+7+SIDECAR_PAYLOAD_SIZE].try_into().unwrap(),
        };

        if sch.sc_code != packet::sidecar::SC_FORWARD_TO_USERSPACE {
            warn!(log, "unk sidecar header code: {}", sch.sc_code);
            continue;
        }
        let port = sch.sc_ingress as usize;
        if port >= ports.len() {
            error!(log, "port out of range {} >= {}", port, ports.len());
            continue;
        }
        debug!(log, "sidecar header: {:#?}", sch);

        // send decapped packet to target port


        let mut full_decapd = vec![0u8; n + 10 - SIDECAR_HDR_SIZE];
        let decapd = &mut full_decapd[10..];
        for i in 0..ETHERNET_FRAME_SIZE {
            decapd[i] = msg[i]
        }
        // replace sidecar ethertype with encapsulated packet ethertype
        decapd[12] = msg[14+5];
        decapd[13] = msg[14+6];
        for i in 0..n-SIDECAR_HDR_SIZE-ETHERNET_FRAME_SIZE {
            decapd[i+ETHERNET_FRAME_SIZE] = msg[i+ETHERNET_FRAME_SIZE+SIDECAR_HDR_SIZE];
        }

        let actx = match ports[port].dispatch_context.lock() {
            Ok(opt_ctx) => {
                match &*opt_ctx {
                    Some(actx) => actx.clone(),
                    None => {
                        warn!(log, "no dispatch context for port {} yet", port);
                        continue;
                    }
                }
            }
            Err(e) => {
                error!(log, "lock context for port {}: {}", port, e);
                continue;
            }
        };

        let ctx = actx.dispctx().await.unwrap();
        ports[port].tx_to_guest(&full_decapd, &ctx);

    }
}

// helper functions to read/write a buffer from/to a guest
pub fn read_buf(mem: &MemCtx, chain: &mut Chain, buf: &mut [u8]) -> usize {

    let mut done = 0;
    chain.for_remaining_type(true, |addr, len| {
        let remain = &mut buf[done..];
        if let Some(copied) = mem.read_into(addr, remain, len) {
            let need_more = copied != remain.len();
            done += copied;
            (copied, need_more)
        } else {
            (0, false)
        }
    })

}
fn write_buf(buf: &[u8], chain: &mut Chain, mem: &MemCtx) -> usize {

    let mut done = 0;
    chain.for_remaining_type(false, |addr, len| {
        let remain = &buf[done..];
        if let Some(copied) = mem.write_from(addr, remain, len) {
            let need_more = copied != remain.len();
            done += copied;
            (copied, need_more)
        } else {
            (0, false)
        }
    })

}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum NetReg {
    Mac,
    Status,
    MaxVqPairs,
}
lazy_static! {
    static ref NET_DEV_REGS: RegMap<NetReg> = {
        let layout = [
            (NetReg::Mac, 6),
            (NetReg::Status, 2),
            (NetReg::MaxVqPairs, 2),
        ];
        RegMap::create_packed(VIRTIO_NET_CFG_SIZE, &layout, None)
    };
}

mod bits {
    pub const VIRTIO_NET_CFG_SIZE: usize = 0xa;
}
use bits::*;

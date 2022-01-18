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
use super::viona::bits::VIRTIO_NET_S_LINK_UP;
use crate::common::*;
use crate::vmm::MemCtx;

use slog::{Logger, debug, warn, error};
use lazy_static::lazy_static;
use rand::Rng;
use pretty_hex::{HexConfig, PrettyHex};

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
const ETHERNET_HDR_SIZE: usize = 14;

/// IPv4 header size (static portion)
const IPV4_HDR_SIZE: usize = 20;

/// IPv6 header size (static portion)
const IPV6_HDR_SIZE: usize = 40;

/// ARP packet size
const ARP_PKT_SIZE: usize = 28;

mod ethertype {
    pub const IPV6: u16 = 0x86dd;
    pub const IPV4: u16 = 0x0800;
    pub const ARP: u16 = 0x0806;
}

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

    fn handle_req(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {

        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        match vq.pop_avail(&mut chain, mem) {
            Some(val) => val as usize,
            None => return,
        };

        // only vq.push_used if we actually read something
        let mut push_used = false;

        // read as many ethernet frames from the guest as we can
        loop {

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

            // check if there is nothing left to read
            if n == 0 {
                break;
            }
            push_used = true;

            if n < 10 {
                warn!(self.log, "short read ({})!", n);
                break;
            }

            // read in the ethernet header
            let eth = &mut frame[0..ETHERNET_HDR_SIZE];
            let n = read_buf(mem, &mut chain, eth);
            if n < ETHERNET_HDR_SIZE {
                warn!(self.log, "frame from guest too small for Ethernet");
                break;
            }
            // get orignial ethertype
            let ethertype = u16::from_be_bytes([eth[12], eth[13]]);
            // set ethertype to sidecar
            let b = (SIDECAR_ETHERTYPE as u16).to_be_bytes();
            eth[12] = b[0];
            eth[13] = b[1];

            // create a sidecar header, goes right after ethernet header
            let sc = &mut frame[
                ETHERNET_HDR_SIZE
                ..
                ETHERNET_HDR_SIZE+SIDECAR_HDR_SIZE
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

            // determine payload buffer size
            let begin = ETHERNET_HDR_SIZE+SIDECAR_HDR_SIZE;
            let payload = match ethertype {

                ethertype::IPV6 => {
                    let end = begin+IPV6_HDR_SIZE;
                    let ipv6 = &mut frame[begin..end];
                    read_buf(mem, &mut chain, ipv6);
                    let payload_len = u16::from_be_bytes([ipv6[4],ipv6[5]]);
                    let begin = end;
                    let len = payload_len as usize;
                    &mut frame[begin..begin+len]
                }

                ethertype::IPV4 => {
                    let end = begin+IPV4_HDR_SIZE;
                    let ipv4 = &mut frame[begin..end];
                    read_buf(mem, &mut chain, ipv4);
                    let remaining =
                        u16::from_be_bytes([ipv4[2], ipv4[3]]) as usize - 
                        IPV4_HDR_SIZE;
                    let begin = end;
                    &mut frame[begin..begin+remaining]
                }

                ethertype::ARP => {
                    let end = begin+ARP_PKT_SIZE;
                    &mut frame[begin..end]
                }

                _ => {
                    debug!(
                        self.log,
                        "it's a bird, it's a plane, it's {}!",
                        ethertype,
                    );
                    let cfg = HexConfig {
                        title: false,
                        ascii: false,
                        width: 4,
                        group: 0,
                        ..HexConfig::default()
                    };
                    debug!(self.log, "\n{:?}\n",
                        (&frame[..ETHERNET_HDR_SIZE]).hex_conf(cfg));

                    // we cannot continue, since we don't know the size of the
                    // current frame.
                    break;
                }

            };

            // read in payload, goes right after sidecar header
            read_buf(mem, &mut chain, payload);

            // send encapped packet out external port
            match dlpi::send(self.sim_dh, &[], &frame, None) {
                Ok(_) => {},
                Err(e) => {
                    error!(self.log, "tx (ext): {}", e);
                }
            };

        }

        if push_used {
            vq.push_used(&mut chain, mem, ctx);
        }

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
        if n < ETHERNET_HDR_SIZE + SIDECAR_HDR_SIZE {
            warn!(log, "packet too small for sidecar encap?");
            continue;
        }

        // get ethernet slice
        let eth_begin = 0;
        let eth_end = eth_begin+ETHERNET_HDR_SIZE;

        // get sidecar header slice
        let sc_begin = eth_end;
        let sc_end = sc_begin+SIDECAR_HDR_SIZE;
        let sc = &msg[sc_begin..sc_end];

        // extract relevant info from sidecar header
        let sch = packet::sidecar::SidecarHdr{
            sc_code: sc[0],
            sc_ingress: u16::from_be_bytes([sc[1], sc[2]]),
            sc_egress: u16::from_be_bytes([sc[3], sc[4]]),
            sc_ether_type: u16::from_be_bytes([sc[5], sc[6]]),
            sc_payload: sc[7..7+SIDECAR_PAYLOAD_SIZE].try_into().unwrap(),
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
        
        // replace sidecar ethertype with encapsulated packet ethertype
        let eth = &mut msg[eth_begin..eth_end]; 
        let b = sch.sc_ether_type.to_be_bytes();
        eth[12] = b[0];
        eth[13] = b[1];

        // Get a VirtioQeue for his PciVirtioSidemux device from it's async
        // dispatch context.
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
        let port = &ports[port];
        let ctx = actx.dispctx().await.unwrap();
        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        let vq = &port.virtio_state.queues[0];
        match vq.pop_avail(&mut chain, mem) {
            Some(_) =>  {}
            None => {
                warn!(port.log, "[tx] pop_avail is none");
                return;
            }
        }


        // write the virtio mystery bytes
        write_buf(&[0u8; 10], &mut chain, mem);

        // write the ethernet header
        write_buf(&eth, &mut chain, mem);

        // get payload slice
        let p_begin = sc_end; 
        let payload = &msg[p_begin..];

        // write payload
        write_buf(&payload, &mut chain, mem);

        vq.push_used(&mut chain, mem, &ctx);

    }
}

// helper functions to read/write a buffer from/to a guest
fn read_buf(mem: &MemCtx, chain: &mut Chain, buf: &mut [u8]) -> usize {

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

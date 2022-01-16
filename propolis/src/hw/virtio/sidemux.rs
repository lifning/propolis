use std::sync::Arc;
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

use slog::{Logger, trace, debug, warn, error};
use lazy_static::lazy_static;
use pretty_hex::*;
use rand::Rng;

/// The sidecar ethertype, also refered to as the service acess point (SAP) by
/// dlpi, is the cue in the ethernet header we use on ingress packet processing
/// to identify host-bound packets from a sidecar.
const SIDECAR_ETHERTYPE: u32 = 0x0901;

/// Size of the sidecar L2 header.
const SIDECAR_HDR_SIZE: usize = 23;

/// MTU (currently assuming 1500) plus the sidecar header size which is 23
const MTU_PLUS_SIDECAR: usize = 1523;

/// Only supporting 48-bit MACs
const ETHERADDRL: usize = 6;

/// Layer 2 ethernet frame size, assuming no vlan tags.
const ETHERNET_FRAME_SIZE: usize = 14;

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

            // per RFD 174
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
        ctx: &DispCtx,
    ) {
        match next {
            instance::State::Boot => {
                simulator_io_handler(
                    self.sim_dh,
                    self.ports.clone(),
                    self.log.clone(),
                    ctx,
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
    log: slog::Logger

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

        Ok(Arc::new(PciVirtioSidemux{
            index, virtio_state, pci_state, mac_addr, sim_dh, log}))

    }

    pub fn handle_req(&self, vq: &Arc<VirtQueue>, ctx: &DispCtx) {

        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        let clen = match vq.pop_avail(&mut chain, mem) {
            Some(val) => val as usize,
            None => return
        };

        //TODO gross, no alloc in handler
        let mut data: Vec<u8> = vec![0;clen];
        let buf = data.as_mut_slice();

        // TODO copy pasta from tail end of Chain::read function. Seemingly
        // cannot use Chain::read as-is because it expects a statically sized
        // type.
        let mut done = 0;
        let total = chain.for_remaining_type(true, |addr, len| {
            let remain = &mut buf[done..];
            if let Some(copied) = mem.read_into(addr, remain, len) {
                let need_more = copied != remain.len();
                done += copied;
                (copied, need_more)
            } else {
                (0, false)
            }
        });

        // for reasons unknown to me, there are 10 bytes of leading spacing
        // before the ethernet frame, 
        if total < 10 + ETHERNET_FRAME_SIZE {
            warn!(self.log, "short frame!");
            return;
        }
        let data = &data[10..];
        let total = total - 10;

        // iterate over ethernet packets, add sidecar header and push to
        // simulator
        //
        // TODO
        //   - assuming that we're not going to get fragmented L2 packets
        //   - assuming there are no vlan tags in the ethernet headers
        //   - assuming IPv6, IPv4 or ARP ethertypes
        let mut i = 0; 
        loop {

            if i >= total { break; }

            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            let len = match ethertype {

                ethertype::IPV6 => {
                    trace!(self.log, "pci: ipv6 packet");
                    let payload_len = u16::from_be_bytes([data[14+4], data[14+5]]);
                    payload_len as usize + 40
                }
                ethertype::IPV4 => {
                    trace!(self.log, "pci: ipv4 packet");
                    let total_len = u16::from_be_bytes([data[14+2], data[14+3]]);
                    total_len as usize
                }
                ethertype::ARP => {
                    trace!(self.log, "pci: arp packet");
                    28
                }
                _ => {
                    trace!(
                        self.log,
                        "it's a bird, it's a plane, it's {}! ({})",
                        ethertype,
                        total
                    );
                    let cfg = HexConfig {
                        title: false,
                        ascii: false,
                        width: 4,
                        group: 0, 
                        ..HexConfig::default() 
                    };
                    trace!(self.log, "\n{:?}\n", data.hex_conf(cfg));
                    break;
                }

            };

            // TODO need to handle fragmentation?
            if i + len > data.len() {
                warn!(self.log, "packet size greater than buffer size: {}+{}={} >= {}", 
                    i, len, i+len, data.len());
                break;
            }

            //TODO gross, even more data path allocation!
            let mut msg: Vec<u8> = vec![0;len+ETHERNET_FRAME_SIZE+SIDECAR_HDR_SIZE];

            // add ethernet header
            for j in 0..ETHERNET_FRAME_SIZE {
                msg[j] = data[i+j];
            }
            i += ETHERNET_FRAME_SIZE;

            // set ethertype to sidecar
            let b = (SIDECAR_ETHERTYPE as u16).to_be_bytes();
            msg[12] = b[0];
            msg[13] = b[1];

            // add sidecar header
            // code
            msg[14] = packet::sidecar::SC_FORWARD_FROM_USERSPACE;
            // TODO: is ingress needed here?
            // egress
            let b = (self.index as u16).to_be_bytes();
            msg[14+3] = b[0];
            msg[14+4] = b[1];
            let b = (ethertype).to_be_bytes();
            msg[14+5] = b[0];
            msg[14+6] = b[1];

            // copy packet into msg buf
            for j in 0..(len as usize) {
                msg[j+ETHERNET_FRAME_SIZE+SIDECAR_HDR_SIZE] = data[i+j]
            }

            i += len;

            // send encapped packet out external port

            // destination MAC, this is actually ignored since the dlpi link is
            // in raw mode.
            let dst = &data[..6];

            match dlpi::send(self.sim_dh, &dst, &msg.as_slice(), None) {
                Ok(_) => {},
                Err(e) => {
                    error!(self.log, "tx[ext]: {}", e);
                    continue;
                }
            };

        }

    }

    fn write_buf(&self, buf: &[u8], chain: &mut Chain, mem: &MemCtx) {

        // more copy pasta from Chain::write b/c like Chain:read a
        // statically sized type is expected.
        let mut done = 0;
        let _total = chain.for_remaining_type(false, |addr, len| {
            let remain = &buf[done..];
            if let Some(copied) = mem.write_from(addr, remain, len) {
                let need_more = copied != remain.len();

                done += copied;
                (copied, need_more)
            } else {
                // Copy failed, so do not attempt anything else
                (0, false)
            }
        });

    }

    pub fn tx_to_guest(&self, data: &[u8], ctx: &DispCtx) {

        let mem = &ctx.mctx.memctx();
        let mut chain = Chain::with_capacity(1);
        //let clen = vq.pop_avail(&mut chain, mem).unwrap() as usize;

        self.write_buf(data, &mut chain, mem);

        todo!();

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
}

impl PciVirtio for PciVirtioSidemux {

    fn virtio_state(&self) -> &PciVirtioState { &self.virtio_state }

    fn pci_state(&self) -> &pci::DeviceState { &self.pci_state }
}

fn simulator_io_handler(
    dh: dlpi::DlpiHandle,
    ports: Vec::<Arc::<PciVirtioSidemux>>,
    log: Logger,
    ctx: &DispCtx,
) -> Result<()> {

    let actx = ctx.async_ctx();
    tokio::spawn(async move { handle_simulator_packets(dh, ports, log, actx).await });

    Ok(())

}

async fn handle_simulator_packets(
    dh: dlpi::DlpiHandle,
    ports: Vec::<Arc::<PciVirtioSidemux>>,
    log: Logger,
    actx: AsyncCtx,
) {

    let mut src = [0u8; dlpi::sys::DLPI_PHYSADDR_MAX];
    let mut msg = [0u8; MTU_PLUS_SIDECAR];
    loop {
        // receive packet
        let mut recvinfo = dlpi::sys::dlpi_recvinfo_t::default();
        let n = match dlpi::recv_async(
            dh, &mut src, &mut msg, Some(&mut recvinfo)).await {

            Ok((_, n)) => {
                debug!(log, "rx: ({})", n);
                n
            }
            Err(e) => {
                error!(log, "rx: {}", e);
                continue;
            }


        };
        if n < SIDECAR_HDR_SIZE {
            warn!(log, "packet too small for sidecar header?");
            continue;
        }

        // extract relevant info from sidecar header
        let sch = packet::sidecar::SidecarHdr{
            sc_code: msg[0],
            sc_ingress: u16::from_be_bytes([msg[1], msg[2]]),
            sc_egress: u16::from_be_bytes([msg[3], msg[4]]),
            sc_ether_type: u16::from_be_bytes([msg[5], msg[6]]),
            sc_payload: msg[7..SIDECAR_HDR_SIZE].try_into().unwrap(),
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

        // send decapped packet to target port

        // XXX DLPI is in raw mode, dst MAC is in the buffer
        //let dst = &recvinfo.destaddr[..recvinfo.destaddrlen as usize];

        // TODO need DispCtx
        let ctx = actx.dispctx().await.unwrap();
        ports[port].tx_to_guest(&msg[SIDECAR_HDR_SIZE..n], &ctx);
    }
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

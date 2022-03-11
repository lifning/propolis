use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};

use image::io::Reader as ImageReader;
use image::GenericImageView;
use image::ImageResult;
use image::Rgba;

use slog::{info, Logger};

pub fn start_vnc_server(log: &Logger) {
    info!(log, "starting vnc server");
    //let addrs = [SocketAddr::from(([127, 0, 0, 1], 9000))];
    let addrs = [SocketAddr::from(([0, 0, 0, 0], 9000))];
    let listener = TcpListener::bind(&addrs[..]).unwrap();
    info!(log, "listening");
    for stream in listener.incoming() {
        info!(log, "incoming");
        handle_vnc_client(log, stream.unwrap());
    }
}

fn handle_vnc_client(log: &Logger, mut stream: TcpStream) {
    // 1. send ProtocolVersion message
    info!(log, "handle vnc client");
    let version = "RFB 003.008\n";
    stream
        .write_all(version.as_bytes())
        .expect("could not write protocol handshake");
    info!(log, "tx: ProtocolVersion={:?}", version);

    // 2. receive protocol selection
    let mut client_proto = [0 as u8; 12];
    stream
        .read_exact(&mut client_proto)
        .expect("could not read protocol version");
    assert!(&client_proto == version.as_bytes());
    // TODO: validate and parse proto
    info!(
        log,
        "rx: client ProtocolVersion={:?}",
        std::str::from_utf8(&client_proto.clone())
            .expect("invalid client protocol version")
    );

    info!(log, "END: ProtocolVersion Handshake\n");
    info!(log, "BEGIN: Security Handshake");

    // 3. send security type message
    let sec_options = [1 as u8; 2]; // none
    stream.write_all(&sec_options).expect("could not write security handshake");
    // TODO: implement type 0, implement other types
    info!(log, "tx: Security Types (none)");

    // 4. receive security type
    let mut sec_type = [0 as u8; 1];
    stream.read_exact(&mut sec_type).expect("could not read security type");
    info!(
        log,
        "rx: Security Type Choice = {:?}",
        if sec_type[0] == 1 { "None" } else { "VNC auth" }
    );

    // 5. send SecurityResult
    if sec_type[0] != 1 {
        let res: [u8; 4] = [0, 0, 0, 1]; // u32, big-endian
        stream
            .write_all(&res)
            .expect("could not write SecurityResult (failure)");
        // TODO: cleanup
        info!(log, "bad security type; exiting");
        return;
    } else {
        //let res: u32 = [0, 0, 0, 0]; // u32, big-endian
        let res = [0, 0, 0, 0]; // u32, big-endian
        stream
            .write_all(&res)
            .expect("could not write SecurityResult (success)");
    }
    info!(log, "END: Security Handshake\n");

    // INITIALIZATION PHASE
    info!(log, "BEGIN: Initialization");

    // 1. receive ClientInit
    let mut client_init = [0 as u8; 1];
    stream.read_exact(&mut client_init).expect("could not read ClientInit");
    info!(
        log,
        "rx: ClientInit={:?}",
        if client_init[0] == 1 { "shared" } else { "exclusive" }
    );
    // TODO: handle disconnects

    // 2. send ServerInit
    let width: [u8; 2] = 1024u16.to_be_bytes();
    let height: [u8; 2] = 768u16.to_be_bytes();
    let name = "jordan's tiny desktop";
    let name_len = name.len() as u32;

    // XXX: values copied from vnc-total-hack
    let bits_per_pixel: u8 = 32;
    let depth: u8 = 24;
    let big_endian_flag: u8 = 0;
    let true_color_flag: u8 = 1;
    let red_max: [u8; 2] = 255u16.to_be_bytes();
    let green_max: [u8; 2] = 255u16.to_be_bytes();
    let blue_max: [u8; 2] = 255u16.to_be_bytes();
    let red_shift: u8 = 16;
    let green_shift: u8 = 8;
    let blue_shift: u8 = 0;

    stream.write_all(&width);
    stream.write_all(&height);

    stream.write_all(&[bits_per_pixel]);
    stream.write_all(&[depth]);
    stream.write_all(&[big_endian_flag]);
    stream.write_all(&[true_color_flag]);
    stream.write_all(&red_max);
    stream.write_all(&green_max);
    stream.write_all(&blue_max);
    stream.write_all(&[red_shift]);
    stream.write_all(&[green_shift]);
    stream.write_all(&[blue_shift]);
    //stream.write_all(&[0, 0, 0]);
    let padding: [u8; 3] = [0, 0, 0];
    stream.write_all(&padding);

    stream.write_all(&name_len.to_be_bytes());
    stream.write_all(name.as_bytes());

    info!(log, "tx: ServerInit");
    info!(log, "END: Initialization\n");
    info!(log, "START: Client To Server Messages");

    loop {
        // send over the splash screen
        stream.write_all(&[0, 0]); // type=0, padding
        stream.write_all(&1u16.to_be_bytes()); // nrectangles

        let w: u16 = 1024;
        let h: u16 = 768;
        stream.write_all(&0u16.to_be_bytes()); // x_pos
        stream.write_all(&0u16.to_be_bytes()); // y_pos
        stream.write_all(&w.to_be_bytes()); // width
        stream.write_all(&h.to_be_bytes()); // height
        stream.write_all(&0i32.to_be_bytes()); // raw encoding

        const len: usize = 1024 * 768 * 4;
        let mut pixels = [0 as u8; len];
        for i in 0..len {
            pixels[i] = 0xff;
        }

        let img = ImageReader::open("oxide.jpg").unwrap().decode().unwrap();
        for (x, y, pixel) in img.pixels() {
            let ux = x as usize;
            let uy = y as usize + 100;
            pixels[uy * (1024 * 4) + ux * 4] = pixel[0];
            pixels[uy * (1024 * 4) + ux * 4 + 1] = pixel[1];
            pixels[uy * (1024 * 4) + ux * 4 + 2] = pixel[2];
            pixels[uy * (1024 * 4) + ux * 4 + 3] = pixel[3];
        }

        stream.write(&pixels).expect("could not write pixels");
        info!(log, "tx: message=FramebufferUpdate");
    }
}

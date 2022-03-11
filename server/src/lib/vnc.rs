use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::{TcpStream, TcpListener};

use image::io::Reader as ImageReader;
use image::ImageResult;
use image::GenericImageView;
use image::Rgba;

pub fn start_vnc_server() {
    println!("starting vnc server");
    //let addrs = [SocketAddr::from(([127, 0, 0, 1], 9000))];
    let addrs = [SocketAddr::from(([0, 0, 0, 0], 9000))];
    let listener = TcpListener::bind(&addrs[..]).unwrap();
    println!("listening");
    for stream in listener.incoming() {
    	println!("incoming");
        handle_vnc_client(stream.unwrap());
    }
}

fn handle_vnc_client(mut stream: TcpStream) {
    // 1. send ProtocolVersion message
    println!("handle vnc client");
    let version = "RFB 003.008\n";
    stream
        .write_all(version.as_bytes())
        .expect("could not write protocol handshake");
    println!("tx: ProtocolVersion={:?}", version);

    // 2. receive protocol selection
    let mut client_proto = [0 as u8; 12];
    stream
        .read_exact(&mut client_proto)
        .expect("could not read protocol version");
    assert!(&client_proto == version.as_bytes());
    // TODO: validate and parse proto
    println!(
        "rx: client ProtocolVersion={:?}",
        std::str::from_utf8(&client_proto.clone())
            .expect("invalid client protocol version")
    );

    println!("END: ProtocolVersion Handshake\n");
    println!("BEGIN: Security Handshake");

    // 3. send security type message
    let sec_options = [1 as u8; 2]; // none
    stream.write_all(&sec_options).expect("could not write security handshake");
    // TODO: implement type 0, implement other types
    println!("tx: Security Types (none)");

    // 4. receive security type
    let mut sec_type = [0 as u8; 1];
    stream.read_exact(&mut sec_type).expect("could not read security type");
    println!(
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
        println!("bad security type; exiting");
        return;
    } else {
        //let res: u32 = [0, 0, 0, 0]; // u32, big-endian
        let res = [0, 0, 0, 0]; // u32, big-endian
        stream
            .write_all(&res)
            .expect("could not write SecurityResult (success)");
    }
    println!("END: Security Handshake\n");

    // INITIALIZATION PHASE
    println!("BEGIN: Initialization");

    // 1. receive ClientInit
    let mut client_init = [0 as u8; 1];
    stream.read_exact(&mut client_init).expect("could not read ClientInit");
    println!(
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

    println!("tx: ServerInit");
    println!("END: Initialization\n");
    println!("START: Client To Server Messages");

    /*
    // send a FramebufferUpdate
    stream.write_all(&[0, 0]); // type=0, padding
    stream.write_all(&1u16.to_be_bytes()); // nrectangles

    let w: u16 = 1024;
    let h: u16 = 768;
    stream.write_all(&0u16.to_be_bytes()); // x_pos
    stream.write_all(&0u16.to_be_bytes()); // y_pos
    stream.write_all(&w.to_be_bytes()); // width
    stream.write_all(&h.to_be_bytes()); // height
    stream.write_all(&0i32.to_be_bytes()); // raw encoding

    const len: usize = 1024 * 768;
    println!("len={}", len);
    let mut pixels = [0 as u8; len];
    for i in 0..len {
        if i % 4 == 0 {
            pixels[i] = 0xf0; // blue
        }
    }
    stream.write(&pixels).expect("could not write pixels");
    println!("tx: message=FramebufferUpdate");
    */

    loop {
/*
        let mut msg_type = [0];
        stream
            .read_exact(&mut msg_type)
            .expect("could not read client message type");

        match msg_type[0] {
            0 => {
                println!("rx: message=SetPixelFormat");
                let mut padding = [0 as u8; 3];
                let mut pixel_fmt = [0 as u8; 16];

                stream
                    .read_exact(&mut padding)
                    .expect("could not read padding");
                stream
                    .read_exact(&mut pixel_fmt)
                    .expect("could not read pixel_fmt");

                println!("padding={:?}, pixel_fmt={:?}", padding, pixel_fmt);
            }
            2 => {
                println!("rx: message=SetEncodings");
                let mut padding: [u8; 1] = [0];
                let mut num_encodings: [u8; 2] = [0, 0];

                stream
                    .read_exact(&mut padding)
                    .expect("could not read padding");
                stream
                    .read_exact(&mut num_encodings)
                    .expect("could not read num_encodings");

                let n = u16::from_be_bytes(num_encodings);
                println!(
                    "padding={:?}, num_encodings={:?}, n={}",
                    padding, num_encodings, n
                );

                for i in 0..n {
                    let mut e = [0, 0, 0, 0];
                    stream.read_exact(&mut e).expect("could not read encoding");
                    //println!("e={:?}", e);
                }
            }
            3 => {
                let mut incremental: [u8; 1] = [0];
                let mut x_pos = [0 as u8; 2];
                let mut y_pos = [0 as u8; 2];
                let mut width = [0 as u8; 2];
                let mut height = [0 as u8; 2];

                stream
                    .read_exact(&mut incremental)
                    .expect("could not read incremental");
                stream.read_exact(&mut x_pos).expect("could not read x_pos");
                stream.read_exact(&mut y_pos).expect("could not read y_pos");
                stream.read_exact(&mut width).expect("could not read width");
                stream.read_exact(&mut height).expect("could not read height");

                println!("rx: message=FramebufferUpdateRequest (incremental={}, x_pos={}, y_pos={}, width={}, height={}", if incremental[0] == 1 { "true" } else { "false" }, u16::from_be_bytes(x_pos), u16::from_be_bytes(y_pos), u16::from_be_bytes(width), u16::from_be_bytes(height));
*/

                // send a FramebufferUpdate
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
                    //if i % 4 == 0 {
                    if i % 4 == 0 {
                        pixels[i] = 0xf0; // blue
                    }
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
                println!("tx: message=FramebufferUpdate");
/*
            }
            4 => {
                println!("rx: message=KeyEvent");
            }
            5 => {
                println!("rx: message=PointerEvent");
            }
            6 => {
                println!("rx: message=ClientCutText");
            }
            _ => {
                println!("invalid message type-{}", msg_type[0]);
            }
        }
*/
    }
}

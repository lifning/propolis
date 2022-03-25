use anyhow::{anyhow, Result};

use std::convert::{From, TryInto};
use std::default::Default;
use std::io::{Read, Write};

use tokio::net::TcpStream;

enum RfbState {}

// TODO: error handling with anyhow
pub trait Message {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self>
    where
        Self: Sized;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}

#[derive(Debug, PartialEq)]
pub enum ProtoVersion {
    Rfb38,
}

impl Message for ProtoVersion {
    fn read_from<R: Read>(reader: &mut R) -> Result<ProtoVersion> {
        let mut client_version = [0; 12];
        reader.read_exact(&mut client_version);

        match &client_version {
            b"RFB 003.008\n" => Ok(ProtoVersion::Rfb38),
            _ => Err(anyhow!("unexpected proto version: {:?}", client_version)),
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            ProtoVersion::Rfb38 => writer.write_all(b"RFB 003.008\n"),
        }?;

        Ok(())
    }
}

#[derive(PartialEq, Debug)]
pub enum SecurityType {
    None,
}

impl Message for SecurityType {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut sec_type = [0; 1];
        reader.read_exact(&mut sec_type)?;

        match &sec_type[0] {
            1 => Ok(SecurityType::None),
            _ => Err(anyhow!("unexpected security type: {}", sec_type[0])),
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            SecurityType::None => writer.write_all(&[1u8]),
        }?;

        Ok(())
    }
}

pub struct SecurityTypes(pub Vec<SecurityType>);

impl Message for SecurityTypes {
    // XXX: this will never be used (unless we write a client -- maybe for tests). Structure differently?
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut ntypes = [0u8];
        reader.read_exact(&mut ntypes)?;

        let mut types = Vec::new();
        let n = ntypes[0];
        for _ in 0..n {
            let sec_type = SecurityType::read_from(reader)?;
            types.push(sec_type);
        }

        Ok(SecurityTypes(types))
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let ntypes = self.0.len();
        writer.write(&[ntypes as u8])?;
        for t in &self.0 {
            t.write_to(writer)?;
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum SecurityResult {
    Ok,
    Failed,
}

impl Message for SecurityResult {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        unimplemented!()
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            SecurityResult::Ok => writer.write_all(&0u32.to_be_bytes()),
            SecurityResult::Failed => writer.write_all(&1u32.to_be_bytes()),
        }?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum ClientInit {
    Exclusive,
    Shared,
}

impl Message for ClientInit {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8];
        reader.read_exact(&mut buf)?;
        match buf[0] {
            0 => Ok(ClientInit::Exclusive),
            1 => Ok(ClientInit::Shared),
            v => Err(anyhow!("invalid ClientInit: {}", v)),
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        unimplemented!()
    }
}

struct PixelFormat {
    bits_per_pixel: u8,
    depth: u8,
    big_endian: bool,
    true_color: bool,
    red_max: u16,
    green_max: u16,
    blue_max: u16,
    red_shift: u8,
    blue_shift: u8,
    green_shift: u8,
}

impl Message for PixelFormat {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        unimplemented!()
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[self.bits_per_pixel])?;
        writer.write_all(&[self.depth])?;

        writer.write_all(if self.big_endian { &[1u8] } else { &[0u8] })?;
        writer.write_all(if self.true_color { &[1u8] } else { &[0u8] })?;

        writer.write_all(&self.red_max.to_be_bytes())?;
        writer.write_all(&self.blue_max.to_be_bytes())?;
        writer.write_all(&self.green_max.to_be_bytes())?;

        writer.write_all(&[self.red_shift])?;
        writer.write_all(&[self.blue_shift])?;
        writer.write_all(&[self.green_shift])?;

        // 3 bytes of padding
        writer.write_all(&[0, 0, 0])?;

        Ok(())
    }
}

impl Default for PixelFormat {
    fn default() -> Self {
        PixelFormat {
            bits_per_pixel: 32,
            depth: 24,
            big_endian: false,
            true_color: true,
            red_max: 255,
            green_max: 255,
            blue_max: 255,
            red_shift: 0,
            blue_shift: 8,
            green_shift: 16,
        }
    }
}

pub struct ServerInit {
    fb_width: u16,
    fb_height: u16,
    pixel_fmt: PixelFormat,
    name: String,
}

impl Message for ServerInit {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        unimplemented!()
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.fb_width.to_be_bytes())?;
        writer.write_all(&self.fb_height.to_be_bytes())?;

        self.pixel_fmt.write_to(writer)?;

        let len: u32 = self.name.len().try_into().unwrap();
        writer.write_all(&len.to_be_bytes())?;
        writer.write_all(self.name.as_bytes())?;

        Ok(())
    }
}

impl Default for ServerInit {
    fn default() -> Self {
        ServerInit {
            fb_width: 1024,
            fb_height: 748,
            pixel_fmt: PixelFormat::default(),
            name: "propolis-vnc-server".to_string(),
        }
    }
}

pub enum ServerMessageType {
    FramebufferUpdate,
    SetColorMapEntries,
    Bell,
    ServerCutText,
}

impl From<ServerMessageType> for u8 {
    fn from(t: ServerMessageType) -> Self {
        match t {
            ServerMessageType::FramebufferUpdate => 0,
            ServerMessageType::SetColorMapEntries => 1,
            ServerMessageType::Bell => 2,
            ServerMessageType::ServerCutText => 3,
        }
    }
}

pub enum ServerMessages {}

// TODO: add others
#[derive(Copy, Clone)]
pub enum Encoding {
    Raw,
}

impl From<Encoding> for i32 {
    fn from(e: Encoding) -> Self {
        match e {
            Encoding::Raw => 0,
        }
    }
}

pub struct Rectangle {
    x_pos: u16,
    y_pos: u16,
    width: u16,
    height: u16,
    encoding: Encoding,
    pixels: Vec<u8>,
}

impl Message for Rectangle {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        unimplemented!()
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.x_pos.to_be_bytes());
        writer.write_all(&self.y_pos.to_be_bytes());
        writer.write_all(&self.width.to_be_bytes());
        writer.write_all(&self.height.to_be_bytes());
        // TODO make this call use generics
        let e: i32 = self.encoding.into();
        writer.write_all(&e.to_be_bytes());

        writer.write_all(&self.pixels)?;

        Ok(())
    }
}

impl Rectangle {
    pub fn new(
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        e: Encoding,
        pixels: Vec<u8>,
    ) -> Self {
        Rectangle {
            x_pos: x,
            y_pos: y,
            width: w,
            height: h,
            encoding: e,
            pixels,
        }
    }
}

pub struct FramebufferUpdate {
    rectangles: Vec<Rectangle>,
}

impl Message for FramebufferUpdate {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        unimplemented!()
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[ServerMessageType::FramebufferUpdate.into()])?;

        // 1 byte of padding
        writer.write_all(&[0])?;

        let num_rectangles: u16 = self.rectangles.len().try_into().unwrap();
        writer.write_all(&num_rectangles.to_be_bytes())?;

        for r in &self.rectangles {
            r.write_to(writer)?;
        }

        Ok(())
    }
}

impl FramebufferUpdate {
    pub fn new(rectangles: Vec<Rectangle>) -> Self {
        FramebufferUpdate { rectangles }
    }
}

struct FramebufferUpdateRequest {
    incremental: bool,
    x_position: u16,
    y_position: u16,
    width: u16,
    height: u16,
}

// TODO: this is not general enough
struct RawEncoding {
    width: u16,
    height: u16,
    bytes_per_pixel: u8,
    pixels: Vec<u32>,
}

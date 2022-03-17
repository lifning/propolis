use anyhow::{anyhow, Result};

use std::io::{Read, Write};
use tokio::net::TcpStream;

enum RfbState {
}

// TODO: error handling with anyhow
pub trait Message {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> where Self: Sized;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}

#[derive(Debug, PartialEq)]
pub enum RfbProtoVersion {
    Rfb38,
}


impl Message for RfbProtoVersion {
    fn read_from<R: Read>(reader: &mut R) -> Result<RfbProtoVersion> {
        let mut client_buf = [0; 12];
        reader.read_exact(&mut client_buf);

        match &client_buf {
            b"RFB 003.008\n" => Ok(RfbProtoVersion::Rfb38),
            _ => Err(anyhow!("unexpected proto version")),
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            RfbProtoVersion::Rfb38 => writer.write_all(b"RFB 003.008\n"),
        }?;

        Ok(())
    }
}


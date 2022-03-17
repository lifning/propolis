use anyhow::{anyhow, Result};

use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

enum RfbState {
}

// TODO: error handling with anyhow
pub trait Message {
    async fn read_from(reader: &mut TcpStream) -> Result<Self> where Self: Sized;
    async fn write_to(&self, writer: &mut TcpStream) -> Result<()>;
}

#[derive(Debug, PartialEq)]
pub enum RfbProtoVersion {
    Rfb38,
}


impl Message for RfbProtoVersion {
    async fn read_from(reader: &mut TcpStream) -> Result<RfbProtoVersion> {
        let mut client_buf = [0; 12];
        reader.read_exact(&mut client_buf).await;

        match &client_buf {
            b"RFB 003.008\n" => Ok(RfbProtoVersion::Rfb38),
            _ => Err(anyhow!("unexpected proto version")),
        }
    }

    async fn write_to(&self, writer: &mut TcpStream) -> Result<()> {
        match self {
            RfbProtoVersion::Rfb38 => writer.write_all(b"RFB 003.008\n").await,
        }?;

        Ok(())
    }
}


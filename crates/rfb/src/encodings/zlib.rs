use super::{Encoding, EncodingType};
use crate::encodings::ConnectionContext;

/// https://vncdotool.readthedocs.io/en/0.8.0/rfbproto.html#zlib-encoding
pub struct ZlibEncoding<'a> {
    frame: rgb_frame::SubFrame<'a>,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for ZlibEncoding<'a> {
    fn from(frame: rgb_frame::SubFrame<'a>) -> Self {
        Self { frame }
    }
}

impl<'a> Encoding for ZlibEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Zlib
    }

    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let in_buf: Vec<u8> = self
            .frame
            .pixels() // conceptually: [[[u8; Bpp]; Width]; Height]
            .flatten() // flatten iterator of rows: [[u8; Bpp]; Width*Height]
            .flatten() // flatten pixels into bytes: [u8; Bpp*Width*Height]
            .copied()
            .collect();
        let mut out_buf = Vec::with_capacity(in_buf.len());

        ctx.zlib
            .compress_vec(&in_buf, &mut out_buf, flate2::FlushCompress::Sync)
            .expect("zlib error");
        Box::new(
            (out_buf.len() as u32).to_be_bytes().into_iter().chain(out_buf),
        )
    }
}

use super::{Encoding, EncodingType};
use crate::encodings::raw::RawEncoding;
use crate::encodings::ConnectionContext;
use crate::proto::ProtocolError;

/// https://vncdotool.readthedocs.io/en/0.8.0/rfbproto.html#zlib-encoding
pub struct ZlibWrappedEncoding<E: Encoding, const ET: i32> {
    pub unc_enc: E,
}

pub type ZlibEncoding<'a> =
    ZlibWrappedEncoding<RawEncoding<'a>, { EncodingType::Zlib as i32 }>;

impl<'a> From<rgb_frame::SubFrame<'a>> for ZlibEncoding<'a> {
    fn from(frame: rgb_frame::SubFrame<'a>) -> Self {
        Self { unc_enc: RawEncoding::from(frame) }
    }
}

impl<E: Encoding, const ET: i32> Encoding for ZlibWrappedEncoding<E, ET> {
    fn get_type(&self) -> EncodingType {
        const { EncodingType::from_repr(ET).unwrap() }
    }

    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>> {
        let in_buf: Vec<u8> = self.unc_enc.encode(ctx)?.collect();

        // flate2 `compress_vec` *requires* target Vec to have enough reserved.
        // https://zlib.net/zlib_tech.html "The worst case choice of parameters
        // can result in an expansion of at most 13.5%, plus eleven bytes."
        let mut out_buf = Vec::with_capacity((in_buf.len() * 135 / 100) + 11);

        // RFC 6143 section 7.7.6:
        // > The server flushes the zlib stream to a byte boundary at the end of
        // > each ZRLE-encoded rectangle.  It need not flush the stream between
        // > tiles within a rectangle.
        ctx.zlib
            .compress_vec(&in_buf, &mut out_buf, flate2::FlushCompress::Sync)
            .map_err(|_| {
                ProtocolError::EncodingError(
                    "zlib compression failed".to_string(),
                )
            })?;
        Ok(Box::new(
            (out_buf.len() as u32).to_be_bytes().into_iter().chain(out_buf),
        ))
    }
}

use super::{Encoding, EncodingType};
use crate::encodings::ConnectionContext;

pub struct ZlibEncoding {
    frame: rgb_frame::Frame,
}

impl Encoding for ZlibEncoding {
    fn get_type(&self) -> EncodingType {
        EncodingType::Zlib
    }

    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let in_buf = self.frame.bytes();
        let mut out_buf = Vec::with_capacity(in_buf.len());

        ctx.zlib
            .compress_vec(in_buf, &mut out_buf, flate2::FlushCompress::Sync)
            .expect("zlib error");
        Box::new(
            (out_buf.len() as u32)
                .to_be_bytes()
                .into_iter()
                .chain(out_buf.into_iter()),
        )
    }
}

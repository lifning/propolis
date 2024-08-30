use crate::proto::PixelFormat;

use super::{
    EncodeContext, Encoding, EncodingType, RawEncoding, RawEncodingRef,
};

pub struct ZlibEncodingRef<'a> {
    raw: RawEncodingRef<'a>,
}

impl<'a> Encoding for ZlibEncodingRef<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Zlib
    }

    fn dimensions(&self) -> (u16, u16) {
        self.raw.dimensions()
    }

    fn pixel_format(&self) -> &PixelFormat {
        self.raw.pixel_format()
    }

    fn encode(
        &self,
        ctx: &mut EncodeContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let in_buf = self.raw.raw_buffer();
        let mut out_buf = Vec::with_capacity(in_buf.len());
        ctx.zlib_cmp
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

pub struct ZlibEncoding {
    raw: RawEncoding,
}

impl Encoding for ZlibEncoding {
    fn get_type(&self) -> EncodingType {
        EncodingType::Zlib
    }

    fn dimensions(&self) -> (u16, u16) {
        self.raw.dimensions()
    }

    fn pixel_format(&self) -> &PixelFormat {
        self.raw.pixel_format()
    }

    fn encode(
        &self,
        ctx: &mut EncodeContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let in_buf = self.raw.raw_buffer();
        let mut out_buf = Vec::with_capacity(in_buf.len());
        ctx.zlib_cmp
            .compress_vec(in_buf, &mut out_buf, flate2::FlushCompress::Sync)
            .expect("zlib error");
        Box::new(out_buf.into_iter())
    }
}

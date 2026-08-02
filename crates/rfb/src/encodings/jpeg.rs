// extension encoding format that's not part of the RFC, but is supported in noVNC:
// https://github.com/novnc/noVNC/blob/7c36fabe/core/decoders/jpeg.js

use crate::encodings::{
    subframe_to_image, ConnectionContext, Encoding, EncodingType,
};

pub struct JPEGEncoding<'a> {
    subframe: rgb_frame::SubFrame<'a>,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for JPEGEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

impl<'a> Encoding for JPEGEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::JPEG
    }

    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>> {
        let mut enc_buf: Vec<u8> = Vec::with_capacity(self.subframe.raw_size());
        let enc = ctx.jpeg_encoder(&mut enc_buf);
        subframe_to_image(enc, &self.subframe)?;

        Ok(Box::new(enc_buf.into_iter()))
    }
}

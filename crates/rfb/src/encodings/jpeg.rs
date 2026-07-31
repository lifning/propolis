// extension encoding format that's not part of the RFC, but is supported in noVNC:
// https://github.com/novnc/noVNC/blob/7c36fabe/core/decoders/jpeg.js

use crate::encodings::{ConnectionContext, Encoding, EncodingType};
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder};

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
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let fourcc = self.subframe.fourcc();
        let width = self.subframe.width();
        let height = self.subframe.height();

        let capacity = width * height * fourcc.bytes_per_pixel().get();
        let mut enc_buf: Vec<u8> = Vec::with_capacity(capacity);
        let mut raw_rgb_buf: Vec<u8> = Vec::with_capacity(capacity);

        let (ri, gi, bi, _) = fourcc.le_idx_rgba();
        raw_rgb_buf.extend(
            self.subframe
                .pixels()
                .flatten()
                .flat_map(|px| [px[ri], px[gi], px[bi]]),
        );

        let enc = JpegEncoder::new(&mut enc_buf);
        enc.write_image(
            &raw_rgb_buf,
            width as u32,
            height as u32,
            // none of the others are supported in image crate's JpegEncoder.
            ExtendedColorType::Rgb8,
        )
        .unwrap();
        Box::new(enc_buf.into_iter())
    }
}

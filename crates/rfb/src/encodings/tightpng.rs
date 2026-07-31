// extension encoding format that's not part of the RFC, but is supported in noVNC:
// https://github.com/novnc/noVNC/blob/7c36fabe/core/decoders/tightpng.js

use crate::encodings::{ConnectionContext, Encoding, EncodingType};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

pub struct TightPNGEncoding<'a> {
    subframe: rgb_frame::SubFrame<'a>,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for TightPNGEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

impl<'a> Encoding for TightPNGEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::TightPNG
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
                .flat_map(|px| [px[ri], px[gi], px[bi], 0xFF]),
        );

        let enc = PngEncoder::new(&mut enc_buf);
        enc.write_image(
            &raw_rgb_buf,
            width as u32,
            height as u32,
            // none of the others are supported in image crate's PngEncoder.
            ExtendedColorType::Rgba8,
        )
        .unwrap();
        // not finding much in the way of authoritative, non-code documentation
        // for this subencoding, but...
        // https://github.com/kanaka/libvncserver/blob/0de0fa498/libvncserver/tight.c#L1894
        Box::new(
            core::iter::once(0xA0) // PNG subencoding of 'Tight' scheme
                .chain(tight_len(enc_buf.len()))
                .chain(enc_buf),
        )
    }
}

// https://github.com/kanaka/libvncserver/blob/0de0fa498/libvncserver/tight.c#L1086
fn tight_len(mut length: usize) -> impl Iterator<Item = u8> + Send {
    core::iter::from_fn(move || {
        if length == 0 {
            return None;
        }
        let val = length;
        length >>= 7;
        if val >= 0x80 {
            Some(0x80 | (val & 0x7f) as u8)
        } else {
            Some(val as u8)
        }
    })
}

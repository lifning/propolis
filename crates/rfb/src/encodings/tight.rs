// extension encoding format that's not part of the RFC, but is described by
// https://libvnc.github.io/doc/html/rfbproto_8h_source.html#l00744
// https://wiki.qemu.org/index.php?title=Features/VNC_Tight_PNG&oldid=6094
// and is supported in noVNC:
// https://github.com/novnc/noVNC/blob/7c36fabe/core/decoders/tight.js
// we only implement the "Fill" (8) and "JPEG" (9) subencodings
// (and "TightPNG" (0xA) in a separate encoding).

use crate::encodings::{
    subframe_to_image, ConnectionContext, Encoding, EncodingType,
};
use crate::proto::ProtocolError;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use itertools::Itertools;

pub struct TightEncoding<'a> {
    subframe: rgb_frame::SubFrame<'a>,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for TightEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

impl<'a> Encoding for TightEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Tight
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>> {
        // https://libvnc.github.io/doc/html/rfbproto_8h_source.html#l00860
        // > -- NOTE 1. If the color depth is 24, and all three color components
        // > are 8-bit wide, then one pixel in Tight encoding is always
        // > represented by three bytes, where the first byte is red component,
        // > the second byte is green component, and the third byte is blue
        // > component of the pixel color value.
        if self.subframe.pixels().flatten().all_equal() {
            let color = self.subframe.pixels().flatten().next().unwrap();
            let (ri, gi, bi, _) = self.subframe.fourcc().le_idx_rgba();
            Ok(Box::new(
                core::iter::once(0x80) // Fill
                    .chain([color[ri], color[gi], color[bi]]),
            ))
        } else {
            let mut enc_buf: Vec<u8> =
                Vec::with_capacity(self.subframe.raw_size());
            let enc = JpegEncoder::new(&mut enc_buf);
            subframe_to_image(enc, &self.subframe)?;

            // https://libvnc.github.io/doc/html/rfbproto_8h_source.html#l00784
            // > 1..3 bytes:  data size (N) in compact representation;
            // > N bytes:     JPEG or PNG image.
            Ok(Box::new(
                core::iter::once(0x90) // JPEG
                    .chain(compact_len(enc_buf.len())?)
                    .chain(enc_buf),
            ))
        }
    }
}

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
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>> {
        let mut enc_buf: Vec<u8> = Vec::with_capacity(self.subframe.raw_size());
        let enc = PngEncoder::new(&mut enc_buf);
        subframe_to_image(enc, &self.subframe)?;

        // https://libvnc.github.io/doc/html/rfbproto_8h_source.html#l00784
        // > 1..3 bytes:  data size (N) in compact representation;
        // > N bytes:     JPEG or PNG image.
        Ok(Box::new(
            core::iter::once(0xA0) // PNG
                .chain(compact_len(enc_buf.len())?)
                .chain(enc_buf),
        ))
    }
}

pub(super) fn compact_len(
    length: usize,
) -> crate::proto::Result<impl Iterator<Item = u8> + Send> {
    use itertools::Either::*;
    // https://libvnc.github.io/doc/html/rfbproto_8h_source.html#l00790
    // > Data size is compactly represented in one, two or three bytes, according
    // > to the following scheme:
    match length {
        // > 0xxxxxxx (for values 0..127)
        0..=127 => Ok(Left(Left(core::iter::once(length as u8)))),
        // > 1xxxxxxx 0yyyyyyy (for values 128..16383)
        128..=16383 => Ok(Left(Right(
            core::iter::once(0x80 | (length & 0x7f) as u8)
                .chain([(length >> 7) as u8]),
        ))),
        // > 1xxxxxxx 1yyyyyyy zzzzzzzz (for values 16384..4194303)
        16384..=4194303 => Ok(Right(
            core::iter::once(0x80 | (length & 0x7f) as u8)
                .chain([0x80 | (length >> 7) as u8])
                .chain([(length >> 14) as u8]),
        )),
        _ => Err(ProtocolError::EncodingError(format!(
            "unrepresentable Tight encoding size: {length}"
        ))),
    }
}

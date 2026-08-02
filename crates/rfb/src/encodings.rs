// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

//! RFC 6143 section 7.7
mod raw;
mod trle;
mod zlib;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use raw::RawEncoding;
use trle::{TRLEncoding, ZRLEncoding};
use zlib::ZlibEncoding;

// non-RFC6143 encodings
mod jpeg;
mod tight;

use jpeg::JPEGEncoding;
use tight::TightEncoding;
use tight::TightPNGEncoding;

pub struct ConnectionContext {
    pub zlib: flate2::Compress,
    /// 10..=100
    jpeg_quality: Option<u8>,
    /// 0..=9
    compression_level: Option<u8>,
}
impl Default for ConnectionContext {
    fn default() -> Self {
        Self {
            zlib: flate2::Compress::new(flate2::Compression::fast(), true),
            jpeg_quality: None,
            compression_level: None,
        }
    }
}
impl ConnectionContext {
    pub fn set_compression_params(&mut self, encodings: &[EncodingType]) {
        use EncodingType::*;
        self.jpeg_quality = None;
        self.compression_level = None;
        for enc in encodings {
            match enc {
                JpegQualityPseudo9 | JpegQualityPseudo8
                | JpegQualityPseudo7 | JpegQualityPseudo6
                | JpegQualityPseudo5 | JpegQualityPseudo4
                | JpegQualityPseudo3 | JpegQualityPseudo2
                | JpegQualityPseudo1 | JpegQualityPseudo0 => {
                    // numerically, mapping this 0..=9 to 1..=100 could be
                    // appropriate, but a 1%-quality JPEG is not very
                    // useful for VNC, so we'll just settle for 10..=100.
                    const JPEG_LOWEST: i32 =
                        EncodingType::JpegQualityPseudo0 as i32;
                    let int_0_9 = ((*enc as i32) - JPEG_LOWEST) as u8;
                    self.jpeg_quality = Some((int_0_9 + 1) * 10);
                }
                CompressLevelPseudo9 | CompressLevelPseudo8
                | CompressLevelPseudo7 | CompressLevelPseudo6
                | CompressLevelPseudo5 | CompressLevelPseudo4
                | CompressLevelPseudo3 | CompressLevelPseudo2
                | CompressLevelPseudo1 | CompressLevelPseudo0 => {
                    const LEVEL_LOWEST: i32 =
                        EncodingType::CompressLevelPseudo0 as i32;
                    let int_0_9 = ((*enc as i32) - LEVEL_LOWEST) as u8;
                    // 0 is likely to actually *inflate* size slightly.
                    // let's assume that's not the user's intent when they
                    // move the slider all the way down hoping for efficiency.
                    self.compression_level = Some(1.max(int_0_9));
                    self.zlib = flate2::Compress::new(
                        flate2::Compression::new(int_0_9 as u32),
                        true,
                    );
                }
                _ => {}
            }
        }
    }
    pub fn jpeg_quality(&self) -> Option<u8> {
        self.jpeg_quality
    }
    fn jpeg_encoder<'a>(
        &self,
        enc_buf: &'a mut Vec<u8>,
    ) -> JpegEncoder<&'a mut Vec<u8>> {
        if let Some(qual) = self.jpeg_quality {
            JpegEncoder::new_with_quality(enc_buf, qual)
        } else {
            JpegEncoder::new(enc_buf)
        }
    }
    fn png_encoder<'a>(
        &self,
        enc_buf: &'a mut Vec<u8>,
    ) -> PngEncoder<&'a mut Vec<u8>> {
        use image::codecs::png::{CompressionType, FilterType};
        if let Some(level) = self.compression_level {
            PngEncoder::new_with_quality(
                enc_buf,
                // frustratingly, these enum variants eventually map to
                // flate2 levels in the 1..9 range... such is life
                match level {
                    0..=4 => CompressionType::Fast,
                    5..=7 => CompressionType::Default,
                    8.. => CompressionType::Best,
                },
                FilterType::Adaptive,
            )
        } else {
            PngEncoder::new(enc_buf)
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    strum::FromRepr,
    strum::EnumString,
    strum::Display,
    strum::VariantNames,
)]
#[repr(i32)]
/// https://www.iana.org/assignments/rfb/rfb.xhtml
pub enum EncodingType {
    Raw = 0,
    CopyRect = 1,
    RRE = 2,
    CoRRE = 4,
    Hextile = 5,
    Zlib = 6,
    Tight = 7,
    ZlibHex = 8,
    TRLE = 15,
    ZRLE = 16,
    ZYWRLE = 17,
    H264 = 20,
    JPEG = 21,
    JRLE = 22,
    VaH264 = 23,
    ZRLE2 = 24,
    OpenH264 = 50,
    JpegQualityPseudo9 = -23,
    JpegQualityPseudo8 = -24,
    JpegQualityPseudo7 = -25,
    JpegQualityPseudo6 = -26,
    JpegQualityPseudo5 = -27,
    JpegQualityPseudo4 = -28,
    JpegQualityPseudo3 = -29,
    JpegQualityPseudo2 = -30,
    JpegQualityPseudo1 = -31,
    JpegQualityPseudo0 = -32,
    DesktopSizePseudo = -223,
    LastRectPseudo = -224,
    CursorPseudo = -239,
    CompressLevelPseudo9 = -247,
    CompressLevelPseudo8 = -248,
    CompressLevelPseudo7 = -249,
    CompressLevelPseudo6 = -250,
    CompressLevelPseudo5 = -251,
    CompressLevelPseudo4 = -252,
    CompressLevelPseudo3 = -253,
    CompressLevelPseudo2 = -254,
    CompressLevelPseudo1 = -255,
    CompressLevelPseudo0 = -256,
    TightPNG = -260,
    ContinuousUpdatesPseudo = -313,
}

impl EncodingType {
    pub fn from<'a>(
        &self,
        subframe: rgb_frame::SubFrame<'a>,
    ) -> Box<dyn Encoding + 'a> {
        use EncodingType::*;
        match self {
            // sends the entire subframe's pixels over the wire uncompressed.
            Raw => Box::new(RawEncoding::from(subframe)),
            // same as Raw, but Zlib-deflated.
            Zlib => Box::new(ZlibEncoding::from(subframe)),
            TRLE => Box::new(TRLEncoding::from(subframe)),
            ZRLE => Box::new(ZRLEncoding::from(subframe)),
            // non-RFC encodings originating from TightVNC, with our impl
            // only producing the simpler JPEG/Fill/PNG special-cases.
            Tight => Box::new(TightEncoding::from(subframe)),
            TightPNG => Box::new(TightPNGEncoding::from(subframe)),
            // a non-RFC encoding whose message data is just a JPEG
            JPEG => Box::new(JPEGEncoding::from(subframe)),
            // not reasonable for us to implement, as we don't have the
            // information a window-manager or GPU would about what regions
            // are being duplicated (without brute-force searching)
            CopyRect => unimplemented!(),
            // RRE, CoRRE, and Hextile are deemed "obsolescent" by the RFC
            RRE | CoRRE => unimplemented!(),
            Hextile | ZlibHex => unimplemented!(),
            // a proprietary, lossy, zlib-wavelet-RLE encoding by Hitachi
            ZYWRLE => unimplemented!(),
            // MPEG license-encumbered and generally not worth it for VNC
            H264 | VaH264 | OpenH264 => unimplemented!(),
            // ???
            JRLE | ZRLE2 => unimplemented!(),
            // not a thing you'd meaningfully encode a subframe with
            DesktopSizePseudo
            | LastRectPseudo
            | CursorPseudo
            | ContinuousUpdatesPseudo => unimplemented!(),
            JpegQualityPseudo9 | JpegQualityPseudo8 | JpegQualityPseudo7
            | JpegQualityPseudo6 | JpegQualityPseudo5 | JpegQualityPseudo4
            | JpegQualityPseudo3 | JpegQualityPseudo2 | JpegQualityPseudo1
            | JpegQualityPseudo0 => unimplemented!(),
            CompressLevelPseudo9 | CompressLevelPseudo8
            | CompressLevelPseudo7 | CompressLevelPseudo6
            | CompressLevelPseudo5 | CompressLevelPseudo4
            | CompressLevelPseudo3 | CompressLevelPseudo2
            | CompressLevelPseudo1 | CompressLevelPseudo0 => unimplemented!(),
        }
    }
}

pub trait Encoding: Send + Sync {
    fn get_type(&self) -> EncodingType;

    /// Transform this encoding from its representation into a byte sequence that can be passed to the client.
    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>>;
}

fn subframe_to_image(
    enc: impl image::ImageEncoder,
    subframe: &rgb_frame::SubFrame<'_>,
) -> crate::proto::Result<()> {
    let capacity = subframe.raw_size();
    let mut raw_rgb_buf: Vec<u8> =
        Vec::with_capacity((capacity * 3).div_ceil(4));

    // none of the Bgr orders are supported in image crate's PngEncoder,
    // and only Rgb8 (no alpha) is supported in its JpegEncoder.
    let (ri, gi, bi, _) = subframe.fourcc().le_idx_rgba();
    raw_rgb_buf.extend(
        subframe.pixels().flatten().flat_map(|px| [px[ri], px[gi], px[bi]]),
    );

    enc.write_image(
        &raw_rgb_buf,
        subframe.width() as u32,
        subframe.height() as u32,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| crate::proto::ProtocolError::EncodingError(e.to_string()))
}

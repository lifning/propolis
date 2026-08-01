// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

//! RFC 6143 section 7.7
mod raw;
mod trle;
mod zlib;

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
    // TODO: client-configured jpeg/png effort params?
}
impl Default for ConnectionContext {
    fn default() -> Self {
        Self { zlib: flate2::Compress::new(flate2::Compression::fast(), true) }
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
    DesktopSizePseudo = -223,
    LastRectPseudo = -224,
    CursorPseudo = -239,
    TightPNG = -260,
    ContinuousUpdatesPseudo = -313,
}

impl EncodingType {
    pub fn from<'a>(
        &self,
        subframe: rgb_frame::SubFrame<'a>,
    ) -> Box<dyn Encoding + 'a> {
        match self {
            // sends the entire subframe's pixels over the wire uncompressed.
            EncodingType::Raw => Box::new(RawEncoding::from(subframe)),
            // same as Raw, but Zlib-deflated.
            EncodingType::Zlib => Box::new(ZlibEncoding::from(subframe)),
            EncodingType::TRLE => Box::new(TRLEncoding::from(subframe)),
            EncodingType::ZRLE => Box::new(ZRLEncoding::from(subframe)),
            // non-RFC encodings originating from TightVNC, with our impl
            // only producing the simpler JPEG/Fill/PNG special-cases.
            EncodingType::Tight => Box::new(TightEncoding::from(subframe)),
            EncodingType::TightPNG => {
                Box::new(TightPNGEncoding::from(subframe))
            }
            // a non-RFC encoding whose message data is just a JPEG
            EncodingType::JPEG => Box::new(JPEGEncoding::from(subframe)),
            // not reasonable for us to implement, as we don't have the
            // information a window-manager or GPU would about what regions
            // are being duplicated (without brute-force searching)
            EncodingType::CopyRect => unimplemented!(),
            // RRE, CoRRE, and Hextile are deemed "obsolescent" by the RFC
            EncodingType::RRE => unimplemented!(),
            EncodingType::CoRRE => unimplemented!(),
            EncodingType::Hextile => unimplemented!(),
            EncodingType::ZlibHex => unimplemented!(),
            // a proprietary, lossy, zlib-wavelet-RLE encoding by Hitachi
            EncodingType::ZYWRLE => unimplemented!(),
            // MPEG license-encumbered and generally not worth it for VNC
            EncodingType::H264 => unimplemented!(),
            EncodingType::VaH264 => unimplemented!(),
            EncodingType::OpenH264 => unimplemented!(),
            // ???
            EncodingType::JRLE => unimplemented!(),
            EncodingType::ZRLE2 => unimplemented!(),
            // not a thing you'd meaningfully encode a subframe with
            EncodingType::DesktopSizePseudo => unimplemented!(),
            EncodingType::LastRectPseudo => unimplemented!(),
            EncodingType::CursorPseudo => unimplemented!(),
            EncodingType::ContinuousUpdatesPseudo => unimplemented!(),
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

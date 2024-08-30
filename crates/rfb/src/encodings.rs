// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

use crate::proto::PixelFormat;
use strum::FromRepr;

mod raw;
mod trle;
mod zlib;

pub use raw::{RawEncoding, RawEncodingRef};
pub use trle::{TRLEncoding, ZRLEncoding};
pub use zlib::{ZlibEncoding, ZlibEncodingRef};

pub struct EncodeContext {
    pub zlib_cmp: flate2::Compress,
}

impl Default for EncodeContext {
    fn default() -> Self {
        Self {
            zlib_cmp: flate2::Compress::new(flate2::Compression::fast(), false),
        }
    }
}

#[derive(Debug, FromRepr, Ord, PartialOrd, Eq, PartialEq)]
#[repr(i32)]
pub enum EncodingType {
    Raw = 0,
    CopyRect = 1,
    RRE = 2,
    CoRRE = 4,
    Hextile = 5,
    Zlib = 6,
    TRLE = 15,
    ZRLE = 16,
    JPEG = 21,
    JRLE = 22,
    ZRLE2 = 24,
    DesktopSizePseudo = -223,
    LastRectPseudo = -224,
    CursorPseudo = -239,
    ContinuousUpdatesPseudo = -313,
}

pub trait Encoding: Send {
    fn get_type(&self) -> EncodingType;

    /// Return the pixel format of this encoding's data.
    fn pixel_format(&self) -> &PixelFormat;

    /// Return the width and height in pixels of the encoded screen region.
    fn dimensions(&self) -> (u16, u16);

    /// Transform this encoding from its representation into a byte sequence that can be passed to the client.
    fn encode(
        &self,
        ctx: &mut EncodeContext,
    ) -> Box<dyn Iterator<Item = u8> + '_>;
}

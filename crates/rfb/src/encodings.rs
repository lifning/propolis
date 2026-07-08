// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

use crate::proto::{PixelFormat, Position, Resolution};

use strum::FromRepr;

mod hextile;
/// Section 7.7.1
mod raw;
mod rre;
mod trle;
mod zlib;

pub use raw::{RawEncoding, RawEncodingRef};

pub struct ConnectionContext {
    pub zlib: flate2::Compress,
}
impl Default for ConnectionContext {
    fn default() -> Self {
        Self { zlib: flate2::Compress::new(flate2::Compression::fast(), false) }
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

pub trait Encoding: Send + Sync {
    fn get_type(&self) -> EncodingType;

    /// Return the width and height in pixels of the encoded screen region.
    fn dimensions(&self) -> (u16, u16);

    /// Return the pixel format of this encoding's data.
    fn pixel_format(&self) -> PixelFormat;

    /// Transform this encoding from its representation into a byte sequence that can be passed to the client.
    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_>;

    /// Translates this encoding type from its current pixel format to the given format.
    fn transform(&self, output: &PixelFormat) -> Box<dyn Encoding>;
}

#[allow(dead_code)]
struct RREncoding {
    background_pixel: Pixel,
    sub_rectangles: Vec<RRESubrectangle>,
}

struct Pixel {
    bytes: Vec<u8>,
}

#[allow(dead_code)]
struct RRESubrectangle {
    pixel: Pixel,
    position: Position,
    dimensions: Resolution,
}

#[allow(dead_code)]
struct HextileEncoding {
    tiles: Vec<Vec<HextileTile>>,
}

#[allow(dead_code)]
enum HextileTile {
    Raw(Vec<u8>),
    Encoded(HextileTileEncoded),
}

#[allow(dead_code)]
struct HextileTileEncoded {
    background: Option<Pixel>,
    foreground: Option<Pixel>,
    // TODO: finish this
}

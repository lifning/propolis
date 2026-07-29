// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

use strum::FromRepr;

mod hextile;
/// Section 7.7.1
mod raw;
mod rre;
mod trle;
mod zlib;

pub use raw::RawEncoding;
pub use trle::{TRLEncoding, ZRLEncoding};
pub use zlib::ZlibEncoding;

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
impl EncodingType {
    pub fn from<'a>(
        &self,
        subframe: rgb_frame::SubFrame<'a>,
    ) -> Box<dyn Encoding + 'a> {
        match self {
            EncodingType::Raw => Box::new(RawEncoding::from(subframe)),
            EncodingType::CopyRect => unimplemented!(),
            EncodingType::RRE => unimplemented!(),
            EncodingType::CoRRE => unimplemented!(),
            EncodingType::Hextile => todo!(),
            EncodingType::Zlib => Box::new(ZlibEncoding::from(subframe)),
            EncodingType::TRLE => Box::new(TRLEncoding::from(subframe)),
            EncodingType::ZRLE => Box::new(ZRLEncoding::from(subframe)),
            EncodingType::JPEG => todo!(),
            EncodingType::JRLE => todo!(),
            EncodingType::ZRLE2 => todo!(),
            EncodingType::DesktopSizePseudo => todo!(),
            EncodingType::LastRectPseudo => todo!(),
            EncodingType::CursorPseudo => todo!(),
            EncodingType::ContinuousUpdatesPseudo => todo!(),
        }
    }
}

pub trait Encoding: Send + Sync {
    fn get_type(&self) -> EncodingType;

    /// Transform this encoding from its representation into a byte sequence that can be passed to the client.
    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_>;
}

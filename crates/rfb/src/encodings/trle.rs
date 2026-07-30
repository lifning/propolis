use std::iter::{from_fn, once};
use std::num::NonZeroUsize;
use std::ops::Range;

use crate::encodings::{ConnectionContext, Encoding, EncodingType};
use crate::proto::PixelFormat;

pub struct RLEncoding<'a, const PX: usize> {
    subframe: rgb_frame::SubFrame<'a>,
}

const TRLE_TILE_PX_SIZE: usize = 16;
const ZRLE_TILE_PX_SIZE: usize = 64;

pub type TRLEncoding<'a> = RLEncoding<'a, TRLE_TILE_PX_SIZE>;
pub struct ZRLEncoding<'a>(RLEncoding<'a, ZRLE_TILE_PX_SIZE>);

impl<'a> Encoding for ZRLEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::ZRLE
    }

    fn encode(
        &self,
        ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        let in_buf = self.0.encode(ctx).collect::<Vec<u8>>();
        let mut out_buf = Vec::with_capacity(in_buf.len());
        ctx.zlib
            .compress_vec(&in_buf, &mut out_buf, flate2::FlushCompress::Sync)
            .expect("zlib error");
        Box::new(
            (out_buf.len() as u32).to_be_bytes().into_iter().chain(out_buf),
        )
    }
}

impl<'a> From<rgb_frame::SubFrame<'a>> for ZRLEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self(RLEncoding::from(subframe))
    }
}

impl<'a, const PX: usize> From<rgb_frame::SubFrame<'a>> for RLEncoding<'a, PX> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

fn tile_ranges<const PX: usize>(
    width_: NonZeroUsize,
    height_: NonZeroUsize,
) -> impl Iterator<Item = (Range<usize>, Range<usize>)> {
    let width = width_.get();
    let height = height_.get();
    // if rect isn't a multiple of TILE_SIZE, we still encode the
    // last partial tile. but if it *is* a multiple of TILE_SIZE,
    // we don't -- hence inclusive range, but minus one before divide
    let last_tile_row = (height - 1) / PX;
    let last_tile_col = (width - 1) / PX;
    (0..=last_tile_row).into_iter().flat_map(move |tile_row_idx| {
        let y_start = tile_row_idx * PX;
        let y_end = height.min((tile_row_idx + 1) * PX);
        (0..=last_tile_col).into_iter().map(move |tile_col_idx| {
            let x_start = tile_col_idx * PX;
            let x_end = width.min((tile_col_idx + 1) * PX);

            (x_start..x_end, y_start..y_end)
        })
    })
}

impl<'a, const PX: usize> RLEncoding<'a, PX> {
    fn encode_tiles(&self) -> impl Iterator<Item = TRLETile<PX>> + '_ {
        use itertools::Either;

        // palette reuse not allowed by ZRLE
        let allow_pal_reuse: bool = PX != ZRLE_TILE_PX_SIZE;

        let subframe = self.subframe;

        let Ok(width) = NonZeroUsize::try_from(subframe.width()) else {
            return Either::Left(std::iter::empty());
        };
        let Ok(height) = NonZeroUsize::try_from(subframe.height()) else {
            return Either::Left(std::iter::empty());
        };
        let pixfmt = PixelFormat::from(subframe.fourcc());

        Either::Right(tile_ranges::<PX>(width, height).map(
            move |(x_range, y_range)| {
                let mut tile_pixels: [[CPixel; PX]; PX] =
                    unsafe { core::mem::zeroed() };

                // re-pack later if palette size allows for 4bpp/2bpp/1bpp
                let mut tile_indeces_opt: Option<Vec<u8>> =
                    Some(Vec::with_capacity(PX * PX));
                let mut palette: Vec<CPixel> = Vec::with_capacity(128);

                for (px_row, row) in
                    subframe.pixels_of_region(&x_range, &y_range).enumerate()
                {
                    for (px_col, pixel) in row.enumerate() {
                        let cpixel = CPixel::from_raw(pixel, &pixfmt);
                        tile_pixels[px_row][px_col] = cpixel;
                        // keep updating indexed-mode tile as long as we
                        // haven't overrun our max colors-per-palette (127)
                        if tile_indeces_opt.is_some() {
                            let index = if let Some(pos) =
                                palette.iter().position(|cpx| *cpx == cpixel)
                            {
                                pos as u8
                            } else if palette.len() < 127 {
                                let pos = palette.len();
                                palette.push(cpixel);
                                pos as u8
                            } else {
                                tile_indeces_opt = None;
                                0
                            };
                            if let Some(tile_indeces) = &mut tile_indeces_opt {
                                tile_indeces.push(index);
                            }
                        }
                    }
                }
                if let Some(tile_indeces) = tile_indeces_opt {
                    let packed_pixels = match palette.len() {
                        0 => unreachable!("at least one pixel must be visited"),
                        1 => {
                            // "0bpp", if you prefer. even works out that way
                            // in the subencoding bytes -- we could set
                            // packed_pixels to an empty vec and it would be
                            // identical.. but let's stay clear to the spec :)
                            return TRLETile::SolidColor {
                                color: palette.pop().unwrap(),
                            };
                        }
                        2 => tile_indeces
                            .chunks(8)
                            .map(|indeces| PackedIndeces::new_1bpp(indeces))
                            .collect(),
                        3..=4 => tile_indeces
                            .chunks(4)
                            .map(|indeces| PackedIndeces::new_2bpp(indeces))
                            .collect(),
                        5..=16 => tile_indeces
                            .chunks(2)
                            .map(|indeces| PackedIndeces::new_4bpp(indeces))
                            .collect(),
                        17..=127 => unsafe {
                            // safety: PackedIndeces is repr(transparent) u8
                            core::mem::transmute::<Vec<u8>, Vec<PackedIndeces>>(
                                tile_indeces,
                            )
                        },
                        128.. => unreachable!("tile_indeces_opt must be None"),
                    };
                    TRLETile::PackedPalette { palette, packed_pixels }
                } else {
                    // TODO: RLE encodings
                    TRLETile::Raw {
                        pixels: tile_pixels,
                        width: x_range.count() as u16,
                        height: y_range.count() as u16,
                    }
                }
            },
        ))
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
struct PackedIndeces(u8);

impl PackedIndeces {
    fn new_4bpp(indeces: &[u8]) -> Self {
        assert!(indeces.len() <= 2);
        let left = indeces.get(0).copied().unwrap_or(0);
        let right = indeces.get(1).copied().unwrap_or(0);
        Self((left << 4) | (right & 0xF))
    }
    fn new_2bpp(indeces: &[u8]) -> Self {
        let mut x = 0;
        assert!(indeces.len() <= 4);
        for (pos, ci) in indeces.iter().copied().enumerate() {
            x |= ((ci << 6) & 0xC0) >> (pos * 2);
        }
        Self(x)
    }
    fn new_1bpp(indeces: &[u8]) -> Self {
        let mut x = 0;
        assert!(indeces.len() <= 8);
        for (pos, ci) in indeces.iter().copied().enumerate() {
            x |= ((ci << 7) & 0x80) >> pos;
        }
        Self(x)
    }
}

// may be able to reuse this for ZRLE? (64px instead of 16px)
#[derive(Clone)]
enum TRLETile<const PX: usize> {
    /// 0
    Raw { pixels: [[CPixel; PX]; PX], width: u16, height: u16 },
    /// 1
    SolidColor { color: CPixel },
    /// 2-16
    PackedPalette { palette: Vec<CPixel>, packed_pixels: Vec<PackedIndeces> },
    /// 127
    PackedPaletteReused { packed_pixels: Vec<PackedIndeces> },
    /// 128
    PlainRLE { runs: Vec<(CPixel, usize)> },
    /// 129
    PaletteRLEReused { runs: Vec<(u8, usize)> },
    /// 130-255
    PaletteRLE { palette: Vec<CPixel>, runs: Vec<(u8, usize)> },
}

fn rle(mut length: usize) -> impl Iterator<Item = u8> + Send {
    from_fn(move || {
        if length == 0 {
            None
        } else if length > 0xFF {
            length -= 0xFF;
            Some(0xFF)
        } else {
            let byte = (length - 1) as u8;
            length = 0;
            Some(byte)
        }
    })
}

fn pal_rle((index, length): (u8, usize)) -> impl Iterator<Item = u8> {
    use itertools::Either::*;
    if length == 1 {
        Left(once(index))
    } else {
        Right(once(index | 0x80).chain(rle(length)))
    }
}

impl<const PX: usize> TRLETile<PX> {
    /// Subencoding of the tile according to RFB 6143 7.7.5.
    /// To the extent possible, this function is a translation of that
    /// section of the RFB RFC from English into chained iterators.
    fn encode(self) -> impl Iterator<Item = u8> {
        use itertools::Either::*;
        match self {
            TRLETile::Raw { pixels, width, height } => {
                Left(Left(Left(once(0u8).chain(
                    pixels.into_iter().take(height as usize).flat_map(
                        move |row| {
                            row.into_iter()
                                .take(width as usize)
                                .flat_map(|c| c.bytes())
                        },
                    ),
                ))))
            }
            TRLETile::SolidColor { color } => {
                Left(Left(Right(once(1u8).chain(color.bytes()))))
            }
            TRLETile::PackedPalette { palette, packed_pixels } => {
                let mut subenc = palette.len() as u8;
                // "17 to 126:  Unused.  (Packed palettes of these sizes
                // would offer no advantage over palette RLE)."
                // HACK: in PaletteRLE, a run-length of one is encoded
                // as the unmodified byte value of the palette index,
                // so we can encode it *as though* it were PackedPalette
                // with an altered subencoding byte.
                if (17..=126).contains(&subenc) {
                    subenc += 128;
                }
                Left(Right(Left(
                    once(subenc)
                        .chain(palette.into_iter().flat_map(|c| c.bytes()))
                        .chain(packed_pixels.into_iter().map(|p| p.0)),
                )))
            }
            TRLETile::PackedPaletteReused { packed_pixels } => {
                Left(Right(Right(
                    once(127u8).chain(packed_pixels.into_iter().map(|p| p.0)),
                )))
            }
            TRLETile::PlainRLE { runs } => {
                Right(Left(Left(once(128).chain(runs.into_iter().flat_map(
                    |(color, length)| color.bytes().chain(rle(length)),
                )))))
            }
            TRLETile::PaletteRLEReused { runs } => Right(Left(Right(
                once(129).chain(runs.into_iter().flat_map(pal_rle)),
            ))),
            TRLETile::PaletteRLE { palette, runs } => Right(Right(
                once(
                    (palette.len() + 128)
                        .try_into()
                        .expect("TRLE tile palette too large!"),
                )
                .chain(palette.into_iter().flat_map(|c| c.bytes()))
                .chain(runs.into_iter().flat_map(pal_rle)),
            )),
        }
    }
}

/// RFC 6143 7.7.5:
/// > TRLE makes use of a new type CPIXEL (compressed pixel).  This is the
/// > same as a PIXEL for the agreed pixel format, except as a special
/// > case, it uses a more compact format if true-color-flag is non-zero,
/// > bits-per-pixel is 32, depth is 24 or less, and all of the bits making
/// > up the red, green, and blue intensities fit in either the least
/// > significant 3 bytes or the most significant 3 bytes.  If all of these
/// > are the case, a CPIXEL is only 3 bytes long, and contains the least
/// > significant or the most significant 3 bytes as appropriate.
/// > bytesPerCPixel is the number of bytes in a CPIXEL.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct CPixel {
    buf: [u8; 4],
    len: u8,
}

enum CPixelTransformType {
    AsIs,
    AppendZero,
    PrependZero,
}

impl CPixel {
    fn bytes(self) -> impl Iterator<Item = u8> {
        self.buf.into_iter().take(self.len as usize)
    }
    fn which_padding(pixfmt: &PixelFormat) -> CPixelTransformType {
        if pixfmt.depth <= 24 && pixfmt.bits_per_pixel == 32 {
            let mask =
                pixfmt.value_mask().expect("colormap not supported in cpixel");
            let should_append = if mask.trailing_zeros() >= 8 {
                false
            } else if mask.leading_zeros() >= 8 {
                true
            } else {
                return CPixelTransformType::AsIs;
            } ^ pixfmt.big_endian;
            if should_append {
                CPixelTransformType::AppendZero
            } else {
                CPixelTransformType::PrependZero
            }
        } else {
            CPixelTransformType::AsIs
        }
    }

    fn from_raw<'a>(raw_bytes: &[u8], pixfmt: &PixelFormat) -> Self {
        let mut start = 0;
        let mut end = pixfmt.bits_per_pixel.div_ceil(8);
        match Self::which_padding(pixfmt) {
            CPixelTransformType::AsIs => (),
            CPixelTransformType::AppendZero => end -= 1,
            CPixelTransformType::PrependZero => start += 1,
        }
        let mut buf = [0u8; 4];
        for (inb, outb) in raw_bytes.iter().zip(buf.iter_mut()) {
            *outb = *inb;
        }
        Self { buf, len: (end - start) as u8 }
    }
}

impl<'a, const PX: usize> Encoding for RLEncoding<'a, PX> {
    fn get_type(&self) -> EncodingType {
        EncodingType::TRLE
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        // TODO: make TRLETile Copy
        Box::new(self.encode_tiles().flat_map(|tile| tile.encode()))
    }
}

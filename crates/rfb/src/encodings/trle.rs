use core::iter::once;
use core::num::NonZeroUsize;
use core::ops::Range;

use crate::encodings::zlib::ZlibWrappedEncoding;
use crate::encodings::{ConnectionContext, Encoding, EncodingType};
use crate::proto::PixelFormat;

const TRLE_PX: usize = 16;
const ZRLE_PX: usize = 64;
const PAL_SIZE: usize = 127;

pub struct RLEncoding<'a, const PX: usize> {
    subframe: rgb_frame::SubFrame<'a>,
}

/// RFC 6143, section 7.7.5
pub type TRLEncoding<'a> = RLEncoding<'a, TRLE_PX>;
/// RFC 6143, section 7.7.6
pub type ZRLEncoding<'a> =
    ZlibWrappedEncoding<RLEncoding<'a, ZRLE_PX>, { EncodingType::ZRLE as i32 }>;

impl<'a> From<rgb_frame::SubFrame<'a>> for ZRLEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { unc_enc: RLEncoding::from(subframe) }
    }
}

impl<'a, const PX: usize> From<rgb_frame::SubFrame<'a>> for RLEncoding<'a, PX> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

impl<'a, const PX: usize> Encoding for RLEncoding<'a, PX> {
    fn get_type(&self) -> EncodingType {
        EncodingType::TRLE
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> crate::proto::Result<Box<dyn Iterator<Item = u8> + '_>> {
        Ok(Box::new(self.encode_tiles().flat_map(|tile| tile.encode())))
    }
}

#[allow(clippy::large_enum_variant)]
enum TRLETile<const PX: usize> {
    /// 0
    Raw { pixels: [[CPixel; PX]; PX], width: u16, height: u16 },
    /// 1
    SolidColor { color: CPixel },
    /// 2-16
    PackedPalette {
        palette: StackVec<CPixel, PAL_SIZE>,
        // XXX: this would be {PX*PX}, but Rust doesn't support doing
        // const expressions with const generics. in practice, PX will
        // only ever be 16 or 64 (and for that matter, almost always 64).
        packed_pixels: StackVec<PackedIndeces, { ZRLE_PX * ZRLE_PX }>,
    },
    // NOTE: not actually doing palette re-use because of non-Zlib-TRLE dis-use
    #[allow(dead_code)]
    /// 127
    PackedPaletteReused {
        packed_pixels: StackVec<PackedIndeces, { ZRLE_PX * ZRLE_PX }>,
    },
    // NOTE: punting on actually utilizing the run-length encoding parts,
    // since if we only use ZRLE we've got Zlib deflating such patterns for us
    #[allow(dead_code)]
    /// 128
    PlainRLE { runs: Vec<(CPixel, usize)> },
    #[allow(dead_code)]
    /// 129
    PaletteRLEReused { runs: Vec<(u8, usize)> },
    #[allow(dead_code)]
    /// 130-255
    PaletteRLE { palette: StackVec<CPixel, PAL_SIZE>, runs: Vec<(u8, usize)> },
}

impl<const PX: usize> TRLETile<PX> {
    /// Subencoding of the tile according to RFB 6143 7.7.5.
    /// To the extent possible, this function is a hand-translation of that
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
                if (17..=127).contains(&subenc) {
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
    (0..=last_tile_row).flat_map(move |tile_row_idx| {
        let y_start = tile_row_idx * PX;
        let y_end = height.min((tile_row_idx + 1) * PX);
        (0..=last_tile_col).map(move |tile_col_idx| {
            let x_start = tile_col_idx * PX;
            let x_end = width.min((tile_col_idx + 1) * PX);

            (x_start..x_end, y_start..y_end)
        })
    })
}

impl<'a, const PX: usize> RLEncoding<'a, PX> {
    fn encode_tiles(&self) -> impl Iterator<Item = TRLETile<PX>> + '_ {
        use itertools::Either;

        // palette reuse is not allowed by ZRLE, so is not implemented here.
        // barely any VNC clients remain that even support non-ZRLE'd TRLE.
        // let allow_pal_reuse: bool = PX != ZRLE_PX;

        let subframe = self.subframe;

        let Ok(width) = NonZeroUsize::try_from(subframe.width()) else {
            return Either::Left(core::iter::empty());
        };
        let Ok(height) = NonZeroUsize::try_from(subframe.height()) else {
            return Either::Left(core::iter::empty());
        };
        let pixfmt = PixelFormat::from(subframe.fourcc());
        let cpxform = CPixelTransformType::from(&pixfmt);

        Either::Right(tile_ranges::<PX>(width, height).map(
            move |(x_range, y_range)| {
                let mut tile_pixels: [[CPixel; PX]; PX] =
                    unsafe { core::mem::zeroed() };

                // re-pack later if palette size allows for 4bpp/2bpp/1bpp
                let mut tile_indeces_opt: Option<
                    StackVec<u8, { ZRLE_PX * ZRLE_PX }>,
                > = Some(StackVec::default());
                // Rust restrictiveness discussed in comment of the
                // TRLETile::PackedPalette enum variant
                const { assert!(PX <= ZRLE_PX) };
                // max palette size is 127, per spec
                let mut palette: StackVec<CPixel, PAL_SIZE> =
                    StackVec::default();

                for (px_row, row) in
                    subframe.pixels_of_region(&x_range, &y_range).enumerate()
                {
                    for (px_col, pixel) in row.enumerate() {
                        let cpixel = CPixel::from_raw(pixel, cpxform);
                        tile_pixels[px_row][px_col] = cpixel;
                        // keep updating indexed-mode tile as long as we
                        // haven't overrun our max colors-per-palette (127)
                        if tile_indeces_opt.is_some() {
                            let index = if let Some(pos) =
                                palette.position(|cpx| *cpx == cpixel)
                            {
                                pos as u8
                            } else if let Ok(pos) = palette.push(cpixel) {
                                pos as u8
                            } else {
                                tile_indeces_opt = None;
                                0
                            };
                            if let Some(tile_indeces) = &mut tile_indeces_opt {
                                // unwrap: above const assert && tile_ranges()
                                // never gives us more than PX-sized ranges
                                tile_indeces.push(index).unwrap();
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
                                // unwrap: palette.len is 1.
                                color: palette.pop().unwrap(),
                            };
                        }
                        // these collect() calls pass through an unwrap in
                        // StackVec::from_iter, but are safe because these
                        // iterators will never have more than PAL_SIZE items.
                        //
                        // .chunks(x_range.len()) is because, per the spec:
                        // > For tiles not a multiple of 8, 4, or 2 pixels wide
                        // > (as appropriate), padding bits are used to
                        // > align each *row* to an exact number of bytes.
                        2 => tile_indeces
                            .chunks(x_range.len())
                            .flat_map(|row| {
                                row.chunks(8).map(PackedIndeces::new_1bpp)
                            })
                            .collect(),
                        3..=4 => tile_indeces
                            .chunks(x_range.len())
                            .flat_map(|row| {
                                row.chunks(4).map(PackedIndeces::new_2bpp)
                            })
                            .collect(),
                        5..=16 => tile_indeces
                            .chunks(x_range.len())
                            .flat_map(|row| {
                                row.chunks(2).map(PackedIndeces::new_4bpp)
                            })
                            .collect(),
                        17..=127 => unsafe {
                            // safety: PackedIndeces is repr(transparent) u8
                            core::mem::transmute::<
                                StackVec<u8, _>,
                                StackVec<PackedIndeces, _>,
                            >(tile_indeces)
                        },
                        128.. => unreachable!("tile_indeces_opt must be None"),
                    };
                    TRLETile::PackedPalette { palette, packed_pixels }
                } else {
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
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
struct CPixel {
    buf: [u8; 3],
    len: u8,
}

#[derive(Copy, Clone)]
enum CPixelTransformType {
    AsIs,
    AppendZero,
    PrependZero,
}

impl From<&PixelFormat> for CPixelTransformType {
    fn from(pixfmt: &PixelFormat) -> Self {
        if pixfmt.depth <= 24 && pixfmt.bits_per_pixel == 32 {
            let mask =
                pixfmt.value_mask().expect("colormap not supported in cpixel");
            let should_append = if mask.trailing_zeros() >= 8 {
                false
            } else if mask.leading_zeros() >= 8 {
                true
            } else {
                return Self::AsIs;
            } ^ pixfmt.big_endian;
            if should_append {
                Self::AppendZero
            } else {
                Self::PrependZero
            }
        } else {
            Self::AsIs
        }
    }
}

impl CPixel {
    fn bytes(self) -> impl Iterator<Item = u8> {
        self.buf.into_iter().take(self.len as usize)
    }

    fn from_raw(raw_bytes: &[u8], transform: CPixelTransformType) -> Self {
        let mut start = 0;
        let mut end = raw_bytes.len();
        match transform {
            CPixelTransformType::AsIs => (),
            CPixelTransformType::AppendZero => end -= 1,
            CPixelTransformType::PrependZero => start += 1,
        }
        assert!(end - start <= 3);
        let mut buf = [0u8; 3];
        for (outb, inb) in buf.iter_mut().zip(&raw_bytes[start..end]) {
            *outb = *inb;
        }
        Self { buf, len: (end - start) as u8 }
    }
}

/// RFC 6143 section 7.7.5, under "2 to 16: Packed palette types":
/// > The packed pixels follow, with each
/// > pixel represented as a bit field yielding a zero-based index into
/// > the palette.  For paletteSize 2, a 1-bit field is used; for
/// > paletteSize 3 or 4, a 2-bit field is used; and for paletteSize
/// > from 5 to 16, a 4-bit field is used.  The bit fields are packed
/// > into bytes, with the most significant bits representing the
/// > leftmost pixel (i.e., big endian).  For tiles not a multiple of 8,
/// > 4, or 2 pixels wide (as appropriate), padding bits are used to
/// > align each row to an exact number of bytes.
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
struct PackedIndeces(u8);

impl PackedIndeces {
    fn new_4bpp(indeces: &[u8]) -> Self {
        assert!(indeces.len() <= 2);
        let left = indeces.first().copied().unwrap_or(0);
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

/// RFC 6143 section 7.7.5, under "128:  Plain RLE":
/// > The length is represented as one or more bytes.
/// > The length is calculated as one more than the
/// > sum of all the bytes representing the length.
/// > Any byte value other than 255 indicates the final byte.
fn rle(mut length: usize) -> impl Iterator<Item = u8> + Send {
    core::iter::from_fn(move || {
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

// just a quick trivial heapless Vec-subset to avoid alloc spam / stick to cache
struct StackVec<T, const CAP: usize>([T; CAP], usize);
impl<T: Copy + Default, const CAP: usize> Default for StackVec<T, CAP> {
    fn default() -> Self {
        Self([T::default(); CAP], 0)
    }
}
impl<T: Default, const CAP: usize> StackVec<T, CAP> {
    fn push(&mut self, val: T) -> Result<usize, ()> {
        let Self(arr, len) = self;
        let pos = *len;
        let cell = arr.get_mut(pos).ok_or(())?;
        *cell = val;
        *len += 1;
        Ok(pos)
    }
    fn pop(&mut self) -> Option<T> {
        let Self(arr, len) = self;
        *len = len.checked_sub(1)?;
        Some(core::mem::take(&mut arr[*len]))
    }
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.0[..self.1].iter()
    }
    fn position(&self, pred: impl FnMut(&T) -> bool) -> Option<usize> {
        self.iter().position(pred)
    }
    fn chunks(&self, chunk_size: usize) -> impl Iterator<Item = &[T]> {
        self.0[..self.1].chunks(chunk_size)
    }
    const fn len(&self) -> usize {
        self.1
    }
}
impl<T, const CAP: usize> IntoIterator for StackVec<T, CAP> {
    type Item = T;
    type IntoIter = core::iter::Take<core::array::IntoIter<T, CAP>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().take(self.1)
    }
}
impl<T: Copy + Default, const CAP: usize> FromIterator<T> for StackVec<T, CAP> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut vec = Self::default();
        for item in iter {
            // unwrap: we only call collect() when we know we can afford to
            vec.push(item)
                .map_err(|_| "insufficient space to collect into StackVec")
                .unwrap();
        }
        vec
    }
}

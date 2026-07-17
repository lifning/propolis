use std::iter::{from_fn, once};

use crate::encodings::{ConnectionContext, Encoding, EncodingType};
use crate::proto::PixelFormat;

pub struct RLEncoding<const PX: usize> {
    tiles: Vec<TRLETile<PX>>,
    width: u16,
    height: u16,
    pixfmt: PixelFormat,
}

const TRLE_TILE_PX_SIZE: usize = 16;
const ZRLE_TILE_PX_SIZE: usize = 64;

pub type TRLEncoding = RLEncoding<TRLE_TILE_PX_SIZE>;
pub struct ZRLEncoding(RLEncoding<ZRLE_TILE_PX_SIZE>);

impl Encoding for ZRLEncoding {
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
            (out_buf.len() as u32)
                .to_be_bytes()
                .into_iter()
                .chain(out_buf.into_iter()),
        )
        // todo!("also disable re-use of palettes in zrle mode")
    }
}

impl From<&rgb_frame::Frame> for ZRLEncoding {
    fn from(frame: &rgb_frame::Frame) -> Self {
        let tiles = from_frame_inner::<ZRLE_TILE_PX_SIZE>(frame);
        Self(RLEncoding {
            tiles,
            width: frame.spec().width.get() as u16,
            height: frame.spec().height.get() as u16,
            pixfmt: PixelFormat::from(frame.spec().fourcc),
        })
    }
}

impl<const PX: usize> RLEncoding<PX> {
    const TILE_PIXEL_SIZE: usize = PX;
}

impl<const PX: usize> From<&rgb_frame::Frame> for RLEncoding<PX> {
    fn from(frame: &rgb_frame::Frame) -> Self {
        let tiles = from_frame_inner::<PX>(frame);
        Self {
            tiles,
            width: frame.spec().width.get() as u16,
            height: frame.spec().height.get() as u16,
            pixfmt: PixelFormat::from(frame.spec().fourcc),
        }
    }
}

fn from_frame_inner<const PX: usize>(
    frame: &rgb_frame::Frame,
) -> Vec<TRLETile<PX>> {
    // palette reuse not allowed by ZRLE
    let allow_pal_reuse: bool = PX != ZRLE_TILE_PX_SIZE;

    let width = frame.spec().width.get();
    let height = frame.spec().height.get();
    let pixfmt = PixelFormat::from(frame.spec().fourcc);

    let bytes_per_px = (pixfmt.bits_per_pixel as usize + 7) / 8;

    let buf = frame.bytes();

    // if rect isn't a multiple of TILE_SIZE, we still encode the
    // last partial tile. but if it *is* a multiple of TILE_SIZE,
    // we don't -- hence inclusive range, but minus one before divide
    let last_tile_row = (height - 1) / PX;
    let last_tile_col = (width - 1) / PX;
    (0..=last_tile_row)
        .into_iter()
        .flat_map(move |tile_row_idx| {
            let y_start = tile_row_idx * PX;
            let y_end = height.min((tile_row_idx + 1) * PX);
            (0..=last_tile_col).into_iter().map(move |tile_col_idx| {
                let x_start = tile_col_idx * PX;
                let x_end = width.min((tile_col_idx + 1) * PX);
                let mut tile_pixels: [[CPixel; PX]; PX] =
                    unsafe { core::mem::zeroed() };
                for y in y_start..y_end {
                    let px_row = y - y_start;
                    for x in x_start..x_end {
                        let px_col = x - x_start;
                        let px_start = (y * width + x) * bytes_per_px;
                        let px_end = (y * width + x + 1) * bytes_per_px;
                        tile_pixels[px_row][px_col] =
                            CPixel::from_raw(&buf[px_start..px_end], &pixfmt);
                    }
                }
                TRLETile::Raw {
                    pixels: tile_pixels,
                    width: (x_end - x_start) as u16,
                    height: (y_end - y_start) as u16,
                }
                // TODO: other encodings
            })
        })
        .collect()
}

#[repr(transparent)]
#[derive(Copy, Clone)]
struct PackedIndeces(u8);

// impl From<&[u8; 2]> for PackedIndeces {
//     fn from(&[left, right]: &[u8; 2]) -> Self {
//         Self((left << 4) | (right & 0xF))
//     }
// }
impl PackedIndeces {
    // fn new<const N: usize>(indeces: &[u8; N]) -> Self {
    //     const BPP: usize = 16 / size_of;
    // }
    fn new_4bpp(&[left, right]: &[u8; 2]) -> Self {
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
        assert!(indeces.len() <= 16);
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

fn pal_rle(
    (index, length): &(u8, usize),
) -> Box<dyn Iterator<Item = u8> + Send> {
    if *length == 1 {
        Box::new(once(*index))
    } else {
        Box::new(once(*index | 0x80).chain(rle(*length)))
    }
}

impl<const PX: usize> TRLETile<PX> {
    /// Subencoding of the tile according to RFB 6143 7.7.5.
    /// To the extent possible, this function is a translation of that
    /// section of the RFB RFC from English into chained iterators.
    fn encode(&self) -> Box<dyn Iterator<Item = u8> + Send + '_> {
        match self {
            TRLETile::Raw { pixels, width, height } => Box::new(
                once(0u8).chain(pixels[..*height as usize].iter().flat_map(
                    |row| row[..*width as usize].iter().flat_map(|c| c.bytes()),
                )),
            ),
            TRLETile::SolidColor { color } => {
                Box::new(once(1u8).chain(color.bytes()))
            }
            TRLETile::PackedPalette { palette, packed_pixels } => Box::new(
                once(palette.len() as u8)
                    .chain(palette.iter().flat_map(|c| c.bytes()))
                    .chain(packed_pixels.iter().map(|p| p.0)),
            ),
            TRLETile::PackedPaletteReused { packed_pixels } => {
                Box::new(once(127u8).chain(packed_pixels.iter().map(|p| p.0)))
            }
            TRLETile::PlainRLE { runs } => {
                Box::new(once(128).chain(runs.iter().flat_map(
                    |(color, length)| color.bytes().chain(rle(*length)),
                )))
            }
            TRLETile::PaletteRLEReused { runs } => {
                Box::new(once(129).chain(runs.iter().flat_map(pal_rle)))
            }
            TRLETile::PaletteRLE { palette, runs } => Box::new(
                once(
                    (palette.len() + 128)
                        .try_into()
                        .expect("TRLE tile palette too large!"),
                )
                .chain(palette.iter().flat_map(|c| c.bytes()))
                .chain(runs.iter().flat_map(pal_rle)),
            ),
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
// TODO: [u8; 4] so we can derive Copy and go fast
#[derive(Copy, Clone)]
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
    fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.buf[..self.len as usize].iter().copied()
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

impl<const PX: usize> Encoding for RLEncoding<PX> {
    fn get_type(&self) -> EncodingType {
        EncodingType::TRLE
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        Box::new(self.tiles.iter().flat_map(|tile| tile.encode()))
    }
}

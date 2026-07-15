use crate::{
    encodings::{ConnectionContext, Encoding, EncodingType, Pixel},
    proto::{PixelFormat, Position, Resolution},
};

struct RREncoding {
    background_pixel: Pixel,
    sub_rectangles: Vec<RRESubrectangle>,
    width: u16,
    height: u16,
    pixfmt: PixelFormat,
}

struct RRESubrectangle {
    pixel: Pixel,
    position: Position,
    dimensions: Resolution,
}

impl Encoding for RREncoding {
    fn get_type(&self) -> EncodingType {
        EncodingType::RRE
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        Box::new(
            (self.sub_rectangles.len() as u32)
                .to_be_bytes()
                .into_iter()
                .chain(self.background_pixel.bytes.iter().copied())
                .chain(self.sub_rectangles.iter().flat_map(|sr| {
                    sr.pixel
                        .bytes
                        .iter()
                        .copied()
                        .chain(sr.position.x.to_be_bytes().into_iter())
                        .chain(sr.position.y.to_be_bytes().into_iter())
                        .chain(sr.dimensions.width.to_be_bytes().into_iter())
                        .chain(sr.dimensions.height.to_be_bytes().into_iter())
                })),
        )
    }
}

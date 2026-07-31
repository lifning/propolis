use crate::encodings::{ConnectionContext, Encoding, EncodingType};

/// RFC 6143, section 7.7.1
pub struct RawEncoding<'a> {
    subframe: rgb_frame::SubFrame<'a>,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for RawEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe }
    }
}

impl<'a> Encoding for RawEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Raw
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        Box::new(
            self.subframe
                .pixels() // conceptually: [[[u8; Bpp]; Width]; Height]
                .flatten() // flatten iterator of rows: [[u8; Bpp]; Width*Height]
                .flatten() // flatten pixels into bytes: [u8; Bpp*Width*Height]
                .copied(),
        )
    }
}

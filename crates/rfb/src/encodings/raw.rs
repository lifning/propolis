use crate::encodings::{ConnectionContext, Encoding, EncodingType};

/// Section 7.7.1
pub struct RawEncoding<'a> {
    frame: &'a rgb_frame::Frame,
    // pub(crate) pixels: Vec<u8>,
    // pub(crate) width: u16,
    // height: u16,
    // pixfmt: PixelFormat,
}

impl<'a> RawEncoding<'a> {
    pub fn new(frame: &'a rgb_frame::Frame) -> Self {
        Self { frame }
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
        Box::new(self.frame.pixels().flatten().copied())
    }
}

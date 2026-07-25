use crate::encodings::{ConnectionContext, Encoding, EncodingType};

/// Section 7.7.1
pub struct RawEncoding<'a> {
    frame: rgb_frame::SubFrame<'a>,
}

impl<'a> RawEncoding<'a> {
    pub fn new(frame: rgb_frame::SubFrame<'a>) -> Self {
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
        Box::new(self.frame.pixels().flatten().flatten().copied())
    }
}

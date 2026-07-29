use crate::{
    encodings::{ConnectionContext, Encoding, EncodingType},
    proto::PixelFormat,
};

#[allow(dead_code)]
struct HextileEncoding<'a> {
    subframe: rgb_frame::SubFrame<'a>,
    // tiles: Vec<Vec<HextileTile>>,
    pixfmt: PixelFormat,
}

impl<'a> From<rgb_frame::SubFrame<'a>> for HextileEncoding<'a> {
    fn from(subframe: rgb_frame::SubFrame<'a>) -> Self {
        Self { subframe, pixfmt: PixelFormat::from(subframe.fourcc()) }
    }
}

bitflags::bitflags! {
    pub struct HextileSubencMask: u8 {
        const RAW = 1 << 0;
        const BACKGROUND_SPECIFIED = 1 << 1;
        const FOREGROUND_SPECIFIED = 1 << 2;
        const ANY_SUBRECTS = 1 << 3;
        const SUBRECTS_COLORED = 1 << 4;
    }
}

impl<'a> Encoding for HextileEncoding<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Hextile
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        todo!()
        // Box::new(self.tiles.iter().flat_map(|tile| {
        //     let subencoding_mask = todo!();
        //     [todo!()].into_iter()
        // }))
    }
}

#[allow(dead_code)]
enum HextileTile {
    Raw(Vec<u8>),
    Encoded(HextileTileEncoded),
}

type Pixel = Vec<u8>;

#[allow(dead_code)]
struct HextileTileEncoded {
    background: Option<Pixel>,
    foreground: Option<Pixel>,
    // TODO: finish this
}

impl HextileTileEncoded {
    fn subenc_mask(&self) -> HextileSubencMask {
        let mut x = HextileSubencMask::empty();
        if self.background.is_some() {
            x |= HextileSubencMask::BACKGROUND_SPECIFIED;
        }
        if self.foreground.is_some() {
            x |= HextileSubencMask::FOREGROUND_SPECIFIED;
        }
        x
    }
}

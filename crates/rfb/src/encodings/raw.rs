use std::{num::NonZeroUsize, sync::Arc};

use futures::{
    stream::{self, BoxStream},
    StreamExt,
};
use rgb_frame::FourCC;

use crate::{
    encodings::{ConnectionContext, Encoding, EncodingType},
    proto::PixelFormat,
};

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

    pub(crate) fn raw_buffer(&self) -> &[u8] {
        self.frame.bytes()
    }
}

impl Encoding for RawEncoding {
    fn get_type(&self) -> EncodingType {
        EncodingType::Raw
    }

    fn dimensions(&self) -> (u16, u16) {
        (self.frame.spec().width.into(), self.frame.spec().height.into())
    }

    fn pixel_format(&self) -> PixelFormat {
        &self.frame.spec().fourcc.into()
    }

    fn encode(
        &self,
        _ctx: &mut ConnectionContext,
    ) -> Box<dyn Iterator<Item = u8> + '_> {
        Box::new(self.pixels.iter().copied())
    }

    fn transform(&self, output: &PixelFormat) -> Box<dyn Encoding> {
        // let fourcc: FourCC = TryInto::try_into(&self.pixfmt).unwrap();
        // rgb_frame::Frame::new_uninit(rgb_frame::Spec {
        //     width: NonZeroUsize::try_from(self.width as usize).unwrap(),
        //     height: NonZeroUsize::try_from(self.height as usize).unwrap(),
        //     // just reuse width for now - we should honestly just stuff the rgb_frame itself into the RawEncoding
        //     stride: NonZeroUsize::try_from(self.width as usize).unwrap(),
        //     fourcc,
        // });
        self.frame.convert(target);
        Box::new(RawEncoding {
            pixels: transform(&self.pixels, &self.pixfmt, &output),
            width: self.width,
            height: self.height,
            pixfmt: output.to_owned(),
        })
    }
}

pub struct RawEncodingRef<'a> {
    pixels: &'a [u8],
    width: u16,
    height: u16,
    pixfmt: PixelFormat,
}

impl<'a> RawEncodingRef<'a> {
    pub fn new(
        pixels: &'a [u8],
        width: u16,
        height: u16,
        pixfmt: &PixelFormat,
    ) -> Self {
        Self { pixels, width, height, pixfmt: pixfmt.clone() }
    }

    // useful for transforming into other encodings
    pub(crate) fn raw_buffer(&self) -> &[u8] {
        &self.pixels
    }
}

impl<'a> Encoding for RawEncodingRef<'a> {
    fn get_type(&self) -> EncodingType {
        EncodingType::Raw
    }

    fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn pixel_format(&self) -> &PixelFormat {
        &self.pixfmt
    }

    async fn encode(&self, _ctx: &mut ConnectionContext) -> BoxStream<u8> {
        stream::iter(self.pixels.iter().copied()).boxed()
    }

    fn transform(&self, output: &PixelFormat) -> Box<dyn Encoding> {
        Box::new(RawEncoding {
            pixels: transform(&self.pixels, &self.pixfmt, &output),
            width: self.width,
            height: self.height,
            pixfmt: output.to_owned(),
        })
    }
}

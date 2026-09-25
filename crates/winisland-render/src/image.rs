use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use skia_safe::{
    AlphaType, ColorType, Data, FilterMode, ImageInfo, MipmapMode, Paint, Rect, SamplingOptions,
    images, surfaces,
};

const MAX_SOURCE_DIMENSION: u32 = 8192;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_DIMENSION: u32 = 1024;

#[derive(Clone)]
pub struct Image {
    inner: skia_safe::Image,
}

impl Image {
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
        limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        reader.limits(limits);

        let decoder = reader.into_decoder().ok()?;
        let (width, height) = decoder.dimensions();
        let pixel_count = u64::from(width).saturating_mul(u64::from(height));
        if width == 0
            || height == 0
            || pixel_count > MAX_SOURCE_PIXELS
            || decoder.total_bytes() > MAX_DECODE_BYTES
        {
            return None;
        }

        let decoded = DynamicImage::from_decoder(decoder).ok()?;
        let decoded = if width.max(height) > MAX_OUTPUT_DIMENSION {
            decoded.thumbnail(MAX_OUTPUT_DIMENSION, MAX_OUTPUT_DIMENSION)
        } else {
            decoded
        };
        let rgba = decoded.into_rgba8();
        let (width, height) = rgba.dimensions();
        Self::from_rgba8(
            i32::try_from(width).ok()?,
            i32::try_from(height).ok()?,
            rgba.as_raw(),
        )
    }

    pub fn from_encoded(bytes: &[u8]) -> Option<Self> {
        skia_safe::Image::from_encoded(Data::new_copy(bytes)).map(|inner| Self { inner })
    }

    pub fn from_rgba8(width: i32, height: i32, rgba: &[u8]) -> Option<Self> {
        Self::from_rgba8_with_alpha(width, height, rgba, AlphaType::Unpremul)
    }

    pub fn from_rgba8_premul(width: i32, height: i32, rgba: &[u8]) -> Option<Self> {
        Self::from_rgba8_with_alpha(width, height, rgba, AlphaType::Premul)
    }

    fn from_rgba8_with_alpha(
        width: i32,
        height: i32,
        rgba: &[u8],
        alpha_type: AlphaType,
    ) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let pixels = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?;
        if rgba.len() < pixels.checked_mul(4)? {
            return None;
        }
        let info = ImageInfo::new((width, height), ColorType::RGBA8888, alpha_type, None);
        images::raster_from_data(&info, Data::new_copy(rgba), info.min_row_bytes())
            .map(|inner| Self { inner })
    }

    pub fn dimensions(&self) -> (i32, i32) {
        (self.inner.width(), self.inner.height())
    }

    pub fn width(&self) -> i32 {
        self.inner.width()
    }

    pub fn height(&self) -> i32 {
        self.inner.height()
    }

    pub fn is_texture_backed(&self) -> bool {
        self.inner.is_texture_backed()
    }

    pub fn unique_id(&self) -> u32 {
        self.inner.unique_id()
    }

    pub fn sample_bgra8(&self, width: i32, height: i32) -> Option<Vec<u8>> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let info = ImageInfo::new(
            (width, height),
            ColorType::BGRA8888,
            AlphaType::Premul,
            None,
        );
        let mut surface = surfaces::raster(&info, None, None)?;
        surface.canvas().draw_image_rect_with_sampling_options(
            &self.inner,
            None,
            Rect::from_wh(width as f32, height as f32),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &Paint::default(),
        );
        let len = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(4)?;
        let mut pixels = vec![0u8; len];
        surface
            .read_pixels(&info, &mut pixels, info.min_row_bytes(), (0, 0))
            .then_some(pixels)
    }

    pub(crate) fn as_skia(&self) -> &skia_safe::Image {
        &self.inner
    }

    pub(crate) fn from_skia(inner: skia_safe::Image) -> Self {
        Self { inner }
    }
}

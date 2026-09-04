use std::io::Cursor;

use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, Limits};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, images};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform,
    ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::Streams::IRandomAccessStreamWithContentType;

const MAX_SOURCE_DIMENSION: u32 = 8192;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_DIMENSION: u32 = 1024;
const MAX_SMTC_SOURCE_DIMENSION: u32 = 32_768;
const MAX_SMTC_SOURCE_PIXELS: u64 = 16 * 1024 * 1024;

pub(crate) fn smtc_thumbnail_requires_compression(
    stream: &IRandomAccessStreamWithContentType,
) -> Option<bool> {
    let decoder = BitmapDecoder::CreateAsync(stream).ok()?.join().ok()?;
    let width = decoder.PixelWidth().ok()?;
    let height = decoder.PixelHeight().ok()?;
    let pixel_count = u64::from(width).saturating_mul(u64::from(height));
    Some(
        width == 0
            || height == 0
            || width > MAX_SOURCE_DIMENSION
            || height > MAX_SOURCE_DIMENSION
            || pixel_count > MAX_SOURCE_PIXELS
            || pixel_count.saturating_mul(4) > MAX_DECODE_BYTES,
    )
}

pub(crate) fn decode_cover_image(data: &Data) -> Option<Image> {
    let mut reader = ImageReader::new(Cursor::new(data.as_bytes()))
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
    let info = ImageInfo::new(
        (i32::try_from(width).ok()?, i32::try_from(height).ok()?),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    images::raster_from_data(&info, Data::new_copy(rgba.as_raw()), info.min_row_bytes())
}

pub(crate) fn compress_smtc_thumbnail(
    stream: &IRandomAccessStreamWithContentType,
    source_size: u64,
) -> Option<Vec<u8>> {
    let decoder = BitmapDecoder::CreateAsync(stream).ok()?.join().ok()?;
    let source_width = decoder.PixelWidth().ok()?;
    let source_height = decoder.PixelHeight().ok()?;
    let source_pixels = u64::from(source_width).saturating_mul(u64::from(source_height));
    if source_width == 0
        || source_height == 0
        || source_width > MAX_SMTC_SOURCE_DIMENSION
        || source_height > MAX_SMTC_SOURCE_DIMENSION
        || source_pixels > MAX_SMTC_SOURCE_PIXELS
    {
        return None;
    }

    let scale = (MAX_OUTPUT_DIMENSION as f64 / f64::from(source_width.max(source_height))).min(1.0);
    let width = (f64::from(source_width) * scale).round().max(1.0) as u32;
    let height = (f64::from(source_height) * scale).round().max(1.0) as u32;
    let transform = BitmapTransform::new().ok()?;
    transform.SetScaledWidth(width).ok()?;
    transform.SetScaledHeight(height).ok()?;
    transform
        .SetInterpolationMode(BitmapInterpolationMode::Fant)
        .ok()?;

    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Rgba8,
            BitmapAlphaMode::Straight,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::ColorManageToSRgb,
        )
        .ok()?
        .join()
        .ok()?
        .DetachPixelData()
        .ok()?;
    if pixels.len() != width as usize * height as usize * 4 {
        return None;
    }

    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .ok()?;
    log::info!(
        "SMTC: compressed thumbnail from {} to {} bytes ({}x{})",
        source_size,
        encoded.len(),
        width,
        height
    );
    Some(encoded)
}

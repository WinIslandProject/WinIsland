use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform,
    ColorManagementMode, ExifOrientationMode,
};
use windows::Media::Control::GlobalSystemMediaTransportControlsSession;
use windows::Storage::Streams::{
    Buffer, DataReader, IRandomAccessStreamWithContentType, InputStreamOptions,
};
use winisland_platform::ThumbnailError;

use super::read_track;

const MAX_THUMBNAIL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_THUMBNAIL_STREAM_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SOURCE_DIMENSION: u32 = 8192;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_DIMENSION: u32 = 1024;
const MAX_SMTC_SOURCE_DIMENSION: u32 = 32_768;
const MAX_SMTC_SOURCE_PIXELS: u64 = 16 * 1024 * 1024;
const THUMBNAIL_UNAVAILABLE: windows::core::HRESULT = windows::core::HRESULT(-1);
const THUMBNAIL_STALE: windows::core::HRESULT = windows::core::HRESULT(-2);
const THUMBNAIL_TOO_LARGE: windows::core::HRESULT = windows::core::HRESULT(-3);
const THUMBNAIL_COMPRESSION_FAILED: windows::core::HRESULT = windows::core::HRESULT(-4);

pub(super) fn load(
    session: &GlobalSystemMediaTransportControlsSession,
    expected_title: &str,
) -> Result<Vec<u8>, ThumbnailError> {
    load_inner(session, expected_title).map_err(|error| match error.code() {
        THUMBNAIL_STALE => ThumbnailError::Stale,
        THUMBNAIL_TOO_LARGE | THUMBNAIL_COMPRESSION_FAILED => ThumbnailError::Terminal,
        _ => ThumbnailError::Backend(error.to_string()),
    })
}

fn load_inner(
    session: &GlobalSystemMediaTransportControlsSession,
    expected_title: &str,
) -> windows::core::Result<Vec<u8>> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    if read_track(&props).title != expected_title {
        return Err(windows::core::Error::new(
            THUMBNAIL_STALE,
            "Stale properties",
        ));
    }
    let stream = props.Thumbnail()?.OpenReadAsync()?.join()?;
    let size = stream.Size()?;
    if size == 0 {
        return Err(windows::core::Error::new(
            THUMBNAIL_UNAVAILABLE,
            "Empty thumbnail",
        ));
    }
    if size > MAX_THUMBNAIL_STREAM_BYTES {
        return Err(windows::core::Error::new(
            THUMBNAIL_TOO_LARGE,
            "Thumbnail exceeds 64 MiB",
        ));
    }
    let requires_compression = requires_compression(&stream).unwrap_or(false);
    stream.Seek(0)?;
    if size > MAX_THUMBNAIL_BYTES || requires_compression {
        return compress(&stream, size).ok_or_else(|| {
            windows::core::Error::new(
                THUMBNAIL_COMPRESSION_FAILED,
                "Failed to compress oversized thumbnail",
            )
        });
    }
    let buffer_size = u32::try_from(size).map_err(|_| {
        windows::core::Error::new(THUMBNAIL_TOO_LARGE, "Thumbnail buffer is too large")
    })?;
    let buffer = Buffer::Create(buffer_size)?;
    let result = stream
        .ReadAsync(&buffer, buffer_size, InputStreamOptions::None)?
        .join()?;
    let reader = DataReader::FromBuffer(&result)?;
    let actual_size = reader.UnconsumedBufferLength()?;
    if actual_size == 0 || actual_size > buffer_size {
        return Err(windows::core::Error::new(
            THUMBNAIL_UNAVAILABLE,
            "Invalid thumbnail length",
        ));
    }
    let mut bytes = vec![0u8; actual_size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

fn requires_compression(stream: &IRandomAccessStreamWithContentType) -> Option<bool> {
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

fn compress(stream: &IRandomAccessStreamWithContentType, source_size: u64) -> Option<Vec<u8>> {
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

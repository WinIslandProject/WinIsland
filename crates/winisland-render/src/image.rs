use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use skia_safe::{AlphaType, ColorType, Data, ImageInfo, images};

const MAX_SOURCE_DIMENSION: u32 = 8192;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_DIMENSION: u32 = 1024;

/// 图像句柄，`Send + Sync + Clone`。
///
/// 线程模型（两段式）：
/// - **解码**在任意线程：`decode` / `from_encoded` / `from_rgba8` 都不需要 GPU 上下文；
/// - **GPU 上传**只在渲染线程：`Renderer::prepare_image`。
///
/// 不变量：不暴露底层句柄，因此调用方无法绕过 `Painter` 绘制。
#[derive(Clone)]
pub struct Image {
    inner: skia_safe::Image,
}

impl Image {
    /// 按封面/缩略图流程解码：`image` crate 解码 + 尺寸与内存上限校验 +
    /// 长边超过 1024 时按 `DynamicImage::thumbnail` 等比缩小 + 非预乘 RGBA8 光栅化。
    ///
    /// 与 `from_encoded` 的区别：本方法产出 `AlphaType::Unpremul` 的光栅图像，
    /// 且缩略图重采样由 `image` crate 完成。两条路径的像素结果不同，**不可互相替代**。
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

    /// 用 Skia 自带的图像编解码器解码，保持编码格式自带的 alpha 类型与像素。
    ///
    /// 用于图标、插件图标等需要与 `skia_safe::Image::from_encoded` 逐像素一致的场景。
    pub fn from_encoded(bytes: &[u8]) -> Option<Self> {
        skia_safe::Image::from_encoded(Data::new_copy(bytes)).map(|inner| Self { inner })
    }

    /// 由紧凑排布的 8 位**非预乘** RGBA 像素构造光栅图像（封面解码流程使用）。
    pub fn from_rgba8(width: i32, height: i32, rgba: &[u8]) -> Option<Self> {
        Self::from_rgba8_with_alpha(width, height, rgba, AlphaType::Unpremul)
    }

    /// 由紧凑排布的 8 位**预乘** RGBA 像素构造光栅图像。
    ///
    /// 与 `from_rgba8` 的唯一区别是 alpha 类型：预乘数据（如"RGB 已乘以 alpha"的白色图标）
    /// 必须走这个入口，否则后端会再做一次解预乘，颜色会变暗。
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

    /// 像素尺寸 `(width, height)`。
    pub fn dimensions(&self) -> (i32, i32) {
        (self.inner.width(), self.inner.height())
    }

    pub fn width(&self) -> i32 {
        self.inner.width()
    }

    pub fn height(&self) -> i32 {
        self.inner.height()
    }

    /// 是否为 GPU 纹理图像（即已经过 `Renderer::prepare_image` 上传）。
    pub fn is_texture_backed(&self) -> bool {
        self.inner.is_texture_backed()
    }

    pub(crate) fn as_skia(&self) -> &skia_safe::Image {
        &self.inner
    }
}

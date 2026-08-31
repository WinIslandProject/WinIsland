use std::cell::RefCell;
use std::time::{Duration, Instant};

use crate::utils::gpu::render_surface;
use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    AlphaType, Color, ColorType, FilterMode, ISize, Image, ImageInfo, MipmapMode, Paint, Rect,
    SamplingOptions, Surface, TileMode,
    gpu::{self, DirectContext},
    image_filters,
};
use windows::Win32::Graphics::Gdi::*;

const GLASS_REFRESH_INTERVAL: Duration = Duration::from_millis(33);
const GLASS_REFRESH_INTERVAL_INTEGRATED: Duration = Duration::from_millis(100);
const GLASS_CAPTURE_DOWNSCALE: u32 = 2;
const GLASS_CAPTURE_DOWNSCALE_INTEGRATED: u32 = 3;

fn glass_refresh_interval() -> Duration {
    if crate::utils::gpu::gpu_profile() == crate::utils::gpu::GpuProfile::Integrated {
        GLASS_REFRESH_INTERVAL_INTEGRATED
    } else {
        GLASS_REFRESH_INTERVAL
    }
}

fn glass_capture_downscale() -> u32 {
    if crate::utils::gpu::gpu_profile() == crate::utils::gpu::GpuProfile::Integrated {
        GLASS_CAPTURE_DOWNSCALE_INTEGRATED
    } else {
        GLASS_CAPTURE_DOWNSCALE
    }
}

#[derive(Clone)]
pub struct GlassBackground {
    pub image: Image,
    pub width: i32,
    pub height: i32,
}

pub struct GlassBackgroundParams {
    pub screen_x: i32,
    pub screen_y: i32,
    pub width: u32,
    pub height: u32,
    pub blur_sigma: f32,
    pub surface_width: u32,
    pub surface_height: u32,
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub monitor_w: u32,
    pub monitor_h: u32,
}

struct GlassCache {
    source_surface: Surface,
    blur_surface: Surface,
    image: Option<GlassBackground>,
    pixels: Vec<u8>,
    timestamp: Instant,
    screen_x: i32,
    screen_y: i32,
    width: u32,
    height: u32,
    surface_width: i32,
    surface_height: i32,
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: u32,
    monitor_h: u32,
    blur_sigma_bits: u32,
}

thread_local! {
    static GLASS_CACHE: RefCell<Option<GlassCache>> = const { RefCell::new(None) };
}

pub fn get_glass_background(
    direct_context: &mut DirectContext,
    params: GlassBackgroundParams,
) -> Option<GlassBackground> {
    let GlassBackgroundParams {
        screen_x,
        screen_y,
        width,
        height,
        blur_sigma,
        surface_width,
        surface_height,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    } = params;
    if width == 0 || height == 0 || monitor_w == 0 || monitor_h == 0 {
        return None;
    }

    let blur_sigma_bits = blur_sigma.to_bits();
    let cached = GLASS_CACHE.with(|cell| {
        let cache = cell.borrow();
        let cache = cache.as_ref()?;
        (cache.timestamp.elapsed() < glass_refresh_interval()
            && cache.screen_x == screen_x
            && cache.screen_y == screen_y
            && cache.width == width
            && cache.height == height
            && cache.monitor_x == monitor_x
            && cache.monitor_y == monitor_y
            && cache.monitor_w == monitor_w
            && cache.monitor_h == monitor_h
            && cache.blur_sigma_bits == blur_sigma_bits)
            .then(|| cache.image.clone())?
    });
    if cached.is_some() {
        return cached;
    }
    let info = capture_image_info(width, height);
    let surface_info = info.with_dimensions(ISize::new(
        div_ceil(surface_width.max(width), glass_capture_downscale()) as i32,
        div_ceil(surface_height.max(height), glass_capture_downscale()) as i32,
    ));
    GLASS_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        let needs_new_surfaces = cache.as_ref().is_none_or(|cache| {
            cache.surface_width < surface_info.width()
                || cache.surface_height < surface_info.height()
        });
        if needs_new_surfaces {
            let (source_surface, blur_surface) = create_surfaces(direct_context, &surface_info)?;
            *cache = Some(GlassCache {
                source_surface,
                blur_surface,
                image: None,
                pixels: Vec::with_capacity(info.compute_byte_size(info.min_row_bytes())),
                timestamp: Instant::now(),
                screen_x,
                screen_y,
                width,
                height,
                surface_width: surface_info.width(),
                surface_height: surface_info.height(),
                monitor_x,
                monitor_y,
                monitor_w,
                monitor_h,
                blur_sigma_bits,
            });
        }
        let cache = cache.as_mut()?;
        // SAFETY: dimensions are non-zero and capture_pixels validates every GDI handle.
        if unsafe {
            !capture_pixels(
                (screen_x, screen_y),
                (width, height),
                (monitor_x, monitor_y, monitor_w, monitor_h),
                &info,
                &mut cache.pixels,
            )
        } {
            return cache.image.clone();
        }
        let image = update_cache(direct_context, cache, &info, blur_sigma)?;
        cache.screen_x = screen_x;
        cache.screen_y = screen_y;
        cache.width = width;
        cache.height = height;
        cache.monitor_x = monitor_x;
        cache.monitor_y = monitor_y;
        cache.monitor_w = monitor_w;
        cache.monitor_h = monitor_h;
        cache.blur_sigma_bits = blur_sigma_bits;
        Some(image)
    })
}

pub fn clear_glass_cache() {
    GLASS_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn create_surfaces(
    direct_context: &mut DirectContext,
    info: &ImageInfo,
) -> Option<(Surface, Surface)> {
    Some((
        render_surface(direct_context, info)?,
        render_surface(direct_context, info)?,
    ))
}

fn update_cache(
    direct_context: &mut DirectContext,
    cache: &mut GlassCache,
    info: &ImageInfo,
    blur_sigma: f32,
) -> Option<GlassBackground> {
    cache.source_surface.canvas().clear(Color::TRANSPARENT);
    if !cache.source_surface.canvas().write_pixels(
        info,
        &cache.pixels,
        info.min_row_bytes(),
        (0, 0),
    ) {
        return None;
    }
    direct_context.flush_and_submit_surface(&mut cache.source_surface, None);
    let source_image = cache.source_surface.make_temporary_image()?;

    cache.image = None;
    cache.blur_surface.canvas().clear(Color::TRANSPARENT);
    let mut blur_paint = Paint::default();
    blur_paint.set_anti_alias(true);
    let scale_x = cache.surface_width as f32 / info.width() as f32;
    let scale_y = cache.surface_height as f32 / info.height() as f32;
    if let Some(filter) = image_filters::blur(
        (
            blur_sigma / glass_capture_downscale() as f32 * scale_x,
            blur_sigma / glass_capture_downscale() as f32 * scale_y,
        ),
        Some(TileMode::Clamp),
        None,
        None,
    ) {
        blur_paint.set_image_filter(filter);
    }
    let source_rect = Rect::from_wh(info.width() as f32, info.height() as f32);
    let destination_rect = Rect::from_wh(cache.surface_width as f32, cache.surface_height as f32);
    cache
        .blur_surface
        .canvas()
        .draw_image_rect_with_sampling_options(
            &source_image,
            Some((&source_rect, SrcRectConstraint::Strict)),
            destination_rect,
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &blur_paint,
        );
    direct_context.flush_and_submit_surface(&mut cache.blur_surface, None);
    let image = cache.blur_surface.make_temporary_image()?;
    gpu::images::get_backend_texture_from_image(&image, false)?;
    let background = GlassBackground {
        image,
        width: cache.surface_width,
        height: cache.surface_height,
    };
    cache.image = Some(background.clone());
    cache.timestamp = Instant::now();
    Some(background)
}

fn capture_image_info(width: u32, height: u32) -> ImageInfo {
    ImageInfo::new(
        ISize::new(
            div_ceil(width, glass_capture_downscale()) as i32,
            div_ceil(height, glass_capture_downscale()) as i32,
        ),
        ColorType::BGRA8888,
        AlphaType::Opaque,
        None,
    )
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor).max(1)
}

unsafe fn capture_pixels(
    origin: (i32, i32),
    size: (u32, u32),
    monitor: (i32, i32, u32, u32),
    info: &ImageInfo,
    pixels: &mut Vec<u8>,
) -> bool {
    let (screen_x, screen_y) = origin;
    let (width, height) = size;
    let (monitor_x, monitor_y, monitor_w, monitor_h) = monitor;
    let cap_w = info.width();
    let cap_h = info.height();
    let width_i32 = width as i32;
    let height_i32 = height as i32;
    let monitor_right = monitor_x.saturating_add(monitor_w as i32);
    let monitor_bottom = monitor_y.saturating_add(monitor_h as i32);
    let left_space = screen_x.saturating_sub(monitor_x);
    let right_space = monitor_right.saturating_sub(screen_x.saturating_add(width_i32));
    let capture_x = if right_space >= width_i32 + 10 {
        screen_x + width_i32 + 10
    } else if left_space >= width_i32 + 10 {
        screen_x - width_i32 - 10
    } else if right_space >= left_space {
        monitor_right.saturating_sub(width_i32).max(monitor_x)
    } else {
        monitor_x
    };
    let max_capture_y = monitor_bottom.saturating_sub(height_i32).max(monitor_y);
    let capture_y = screen_y.clamp(monitor_y, max_capture_y);

    // SAFETY: all GDI resources are checked before use and released in reverse order.
    unsafe {
        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return false;
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            ReleaseDC(None, hdc_screen);
            return false;
        }
        let hbm = CreateCompatibleBitmap(hdc_screen, cap_w, cap_h);
        if hbm.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return false;
        }
        let old = SelectObject(hdc_mem, hbm.into());

        let _ = SetStretchBltMode(hdc_mem, STRETCH_BLT_MODE(HALFTONE.0));
        let _ = StretchBlt(
            hdc_mem,
            0,
            0,
            cap_w,
            cap_h,
            Some(hdc_screen),
            capture_x,
            capture_y,
            width_i32,
            height_i32,
            SRCCOPY,
        );

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = cap_w;
        bmi.bmiHeader.biHeight = -cap_h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        pixels.resize(info.compute_byte_size(info.min_row_bytes()), 0);
        GetDIBits(
            hdc_mem,
            hbm,
            0,
            cap_h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        true
    }
}

use std::cell::RefCell;
use std::time::Instant;

use skia_safe::{
    AlphaType, Color, ColorType, FilterMode, ISize, Image, ImageInfo, MipmapMode, Paint, Rect,
    SamplingOptions, Surface, TileMode,
    gpu::{self, Budgeted, DirectContext, SurfaceOrigin},
    image_filters,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HALFTONE, ReleaseDC, SRCCOPY,
    STRETCH_BLT_MODE, SelectObject, SetStretchBltMode, StretchBlt,
};

use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::get_cached_media_image_with_key;
use crate::utils::gpu::render_surface;

thread_local! {
    static MICA_CACHE: RefCell<Option<MicaCache>> = const { RefCell::new(None) };
    static BLURRED_COVER_CACHE: RefCell<Option<BlurredCoverCache>> = const { RefCell::new(None) };
}

struct BlurredCoverCache {
    cache_key: u64,
    blurred_image: Image,
}

struct MicaCache {
    source_surface: Surface,
    blur_surface: Surface,
    image: Option<Image>,
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: u32,
    monitor_h: u32,
    timestamp: Instant,
}

pub fn disable_mica(hwnd: HWND) {
    // SAFETY: hwnd belongs to the live WinIsland window. Both attributes receive pointers to
    // initialized i32 values that remain valid for the duration of each synchronous call.
    unsafe {
        let value: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &value as *const _ as *const _,
            size_of::<i32>() as u32,
        );
        let value: i32 = 0;
        let attr = DWMWINDOWATTRIBUTE(1029);
        let _ = DwmSetWindowAttribute(
            hwnd,
            attr,
            &value as *const _ as *const _,
            size_of::<i32>() as u32,
        );
    }
}

pub fn get_mica_background(
    direct_context: &mut DirectContext,
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: u32,
    monitor_h: u32,
) -> Option<Image> {
    if monitor_w == 0 || monitor_h == 0 {
        return None;
    }

    let refresh_interval_ms =
        if crate::utils::gpu::gpu_profile() == crate::utils::gpu::GpuProfile::Integrated {
            1500
        } else {
            1000
        };
    let needs_capture = MICA_CACHE.with(|cell| {
        let cache = cell.borrow();
        match cache.as_ref() {
            None => true,
            Some(c) => {
                c.monitor_x != monitor_x
                    || c.monitor_y != monitor_y
                    || c.monitor_w != monitor_w
                    || c.monitor_h != monitor_h
                    || c.timestamp.elapsed().as_millis() >= refresh_interval_ms
            }
        }
    });

    if needs_capture
        && let Some((info, pixels)) =
            capture_mica_pixels(monitor_x, monitor_y, monitor_w, monitor_h)
    {
        MICA_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            let needs_new_surfaces = cache
                .as_ref()
                .is_none_or(|cache| cache.monitor_w != monitor_w || cache.monitor_h != monitor_h);
            if needs_new_surfaces {
                let Some((source_surface, blur_surface)) =
                    create_mica_surfaces(direct_context, &info)
                else {
                    return;
                };
                *cache = Some(MicaCache {
                    source_surface,
                    blur_surface,
                    image: None,
                    monitor_x,
                    monitor_y,
                    monitor_w,
                    monitor_h,
                    timestamp: Instant::now(),
                });
            }
            let Some(cache) = cache.as_mut() else {
                return;
            };
            if update_mica_cache(direct_context, cache, &info, &pixels).is_some() {
                cache.monitor_x = monitor_x;
                cache.monitor_y = monitor_y;
                cache.monitor_w = monitor_w;
                cache.monitor_h = monitor_h;
            }
        });
    }

    MICA_CACHE.with(|cell| {
        let cache = cell.borrow();
        cache.as_ref().and_then(|cache| cache.image.clone())
    })
}

pub fn clear_mica_cache() {
    MICA_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn capture_mica_pixels(
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: u32,
    monitor_h: u32,
) -> Option<(ImageInfo, Vec<u8>)> {
    if monitor_w == 0 || monitor_h == 0 {
        return None;
    }
    let downscale = 8u32;
    let cap_w = (monitor_w / downscale).max(1) as i32;
    let cap_h = (monitor_h / downscale).max(1) as i32;

    // SAFETY: all GDI resources are checked before use and released in reverse order.
    unsafe {
        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return None;
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            ReleaseDC(None, hdc_screen);
            return None;
        }
        let hbm = CreateCompatibleBitmap(hdc_screen, cap_w, cap_h);
        if hbm.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return None;
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
            monitor_x,
            monitor_y,
            monitor_w as i32,
            monitor_h as i32,
            SRCCOPY,
        );

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = cap_w;
        bmi.bmiHeader.biHeight = -cap_h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let pixel_count = (cap_w * cap_h * 4) as usize;
        let mut pixels = vec![0u8; pixel_count];
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

        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }

        let info = ImageInfo::new(
            ISize::new(cap_w, cap_h),
            ColorType::BGRA8888,
            AlphaType::Opaque,
            None,
        );
        Some((info, pixels))
    }
}

fn create_mica_surfaces(
    direct_context: &mut DirectContext,
    info: &ImageInfo,
) -> Option<(Surface, Surface)> {
    Some((
        render_surface(direct_context, info)?,
        render_surface(direct_context, info)?,
    ))
}

fn update_mica_cache(
    direct_context: &mut DirectContext,
    cache: &mut MicaCache,
    info: &ImageInfo,
    pixels: &[u8],
) -> Option<Image> {
    if !cache
        .source_surface
        .canvas()
        .write_pixels(info, pixels, info.min_row_bytes(), (0, 0))
    {
        return None;
    }
    direct_context.flush_and_submit_surface(&mut cache.source_surface, None);
    let source_image = cache.source_surface.image_snapshot();

    cache.image = None;
    cache.blur_surface.canvas().clear(Color::TRANSPARENT);
    let mut blur_paint = Paint::default();
    blur_paint.set_anti_alias(true);
    if let Some(filter) = image_filters::blur((6.0, 6.0), Some(TileMode::Clamp), None, None) {
        blur_paint.set_image_filter(filter);
    }
    cache
        .blur_surface
        .canvas()
        .draw_image(&source_image, (0, 0), Some(&blur_paint));
    direct_context.flush_and_submit_surface(&mut cache.blur_surface, None);
    let image = cache.blur_surface.image_snapshot();
    gpu::images::get_backend_texture_from_image(&image, false)?;
    cache.image = Some(image.clone());
    cache.timestamp = Instant::now();
    Some(image)
}

pub fn get_blurred_cover_background(
    direct_context: &mut DirectContext,
    media: &MediaInfo,
) -> Option<Image> {
    if media.title.is_empty() {
        return None;
    }
    let (image, cache_key) = get_cached_media_image_with_key(media)?;

    let cached = BLURRED_COVER_CACHE.with(|cell| {
        let cache = cell.borrow();
        cache
            .as_ref()
            .filter(|entry| entry.cache_key == cache_key)
            .map(|entry| entry.blurred_image.clone())
    });
    if cached.is_some() {
        return cached;
    }

    let info = ImageInfo::new_n32_premul((64, 64), None);
    let mut downscaled_surface = gpu::surfaces::render_target(
        direct_context,
        Budgeted::Yes,
        &info,
        None,
        Some(SurfaceOrigin::TopLeft),
        None,
        Some(false),
        Some(false),
    )?;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    downscaled_surface
        .canvas()
        .draw_image_rect_with_sampling_options(
            &image,
            None,
            Rect::from_xywh(0.0, 0.0, 64.0, 64.0),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &paint,
        );
    direct_context.flush_and_submit_surface(&mut downscaled_surface, None);
    let downscaled = downscaled_surface.image_snapshot();

    let mut blur_surface = gpu::surfaces::render_target(
        direct_context,
        Budgeted::Yes,
        &info,
        None,
        Some(SurfaceOrigin::TopLeft),
        None,
        Some(false),
        Some(false),
    )?;
    let mut blur_paint = Paint::default();
    blur_paint.set_anti_alias(true);
    if let Some(filter) = image_filters::blur((8.0, 8.0), Some(TileMode::Clamp), None, None) {
        blur_paint.set_image_filter(filter);
    }
    blur_surface
        .canvas()
        .draw_image(&downscaled, (0, 0), Some(&blur_paint));
    direct_context.flush_and_submit_surface(&mut blur_surface, None);
    let blurred_image = blur_surface.image_snapshot();
    gpu::images::get_backend_texture_from_image(&blurred_image, false)?;

    BLURRED_COVER_CACHE.with(|cell| {
        *cell.borrow_mut() = Some(BlurredCoverCache {
            cache_key,
            blurred_image: blurred_image.clone(),
        });
    });

    Some(blurred_image)
}

pub fn clear_blurred_cover_cache() {
    BLURRED_COVER_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

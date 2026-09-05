#![allow(deprecated)]
mod controls;
mod palette;
mod visualizer;

pub use controls::{
    get_next_btn_rect, get_pause_btn_rect, get_prev_btn_rect, get_progress_bar_rect,
    set_progress_dragging, set_progress_hover, snap_progress, trigger_cover_flip,
    trigger_next_click, trigger_pause_click, trigger_prev_click,
};
pub use visualizer::{DrawVisualizerParams, draw_visualizer};

use self::controls::ease_out_back;
use self::palette::get_palette_from_image;
use crate::core::smtc::MediaInfo;
use crate::icons::arrows::draw_arrow_right;
use crate::icons::controls::{draw_control_triangle, draw_pause_button, draw_play_button};
use crate::utils::cover::decode_cover_image;
use crate::utils::font::{DrawTextCachedParams, FontManager};
use crate::utils::physics::Spring;
use crate::utils::scroll::{ScrollDrawParams, ScrollText};
use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    Canvas, Color, FilterMode, FontStyle, Image, MipmapMode, Paint, Point, RRect, Rect,
    SamplingOptions, gpu::DirectContext, image_filters,
};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

const CONTENT_PADDING: f32 = 24.0;
const PAGE_ARROW_RIGHT_INSET: f32 = 7.5;
const PAGE_ARROW_FADE_RATE: f32 = 5.0;
const COVER_SIZE: f32 = 72.0;
const TRACK_TEXT_GAP: f32 = 16.0;
const TRACK_TEXT_RIGHT_INSET: f32 = 70.0;
const TRACK_TITLE_BASELINE_OFFSET: f32 = 26.0;
const PROGRESS_TOP_GAP: f32 = 18.0;
const PROGRESS_TIME_FONT_SCALE: f32 = 0.67;
const DEFAULT_PROGRESS_TIME_FONT_SIZE: f32 = 10.0;
const PROGRESS_TIME_WIDTH: f32 = 36.0;
const PROGRESS_TIME_GAP: f32 = 4.0;
const PROGRESS_START_THRESHOLD: f32 = 0.02;
const PROGRESS_JUMP_THRESHOLD: f32 = 0.3;
const PROGRESS_SMOOTHING: f32 = 0.15;
const PROGRESS_HOVER_SMOOTHING: f32 = 0.18;
const PROGRESS_HOVER_SNAP_THRESHOLD: f32 = 0.005;
const PROGRESS_BAR_HEIGHT: f32 = 5.5;
const PROGRESS_BAR_HOVER_GROWTH: f32 = 3.5;
const PROGRESS_TIME_BASELINE_SCALE: f32 = 0.35;
const PROGRESS_TIME_IDLE_ALPHA: f32 = 0.5;
const PROGRESS_TRACK_ALPHA: f32 = 0.25;
const PLAYBACK_CONTROLS_TOP_GAP: f32 = 42.0;
const SKIP_BUTTON_GAP: f32 = 75.0;
const SKIP_ANIMATION_DURATION_SECS: f32 = 0.5;
const PLAY_STATE_RESPONSE: f32 = 0.18;
pub(super) const PAUSE_CONTROL_PRESS_VELOCITY: f32 = -0.18;
const PAUSE_CONTROL_STIFFNESS: f32 = 0.18;
const PAUSE_CONTROL_DAMPING: f32 = 0.64;
const PAUSE_CONTROL_MIN_SCALE: f32 = 0.8;
const PAUSE_CONTROL_MAX_SCALE: f32 = 1.03;
const PAUSE_CONTROL_BLUR_SCALE: f32 = 16.0;
const PAUSE_CONTROL_MAX_BLUR: f32 = 2.5;
const COLLAPSED_VISUALIZER_INSET: f32 = 17.0;
const EXPANDED_VISUALIZER_INSET: f32 = 37.0;
const VISUALIZER_TITLE_OFFSET: f32 = 4.0;
const VISUALIZER_SMOOTHING: (f32, f32) = (0.6, 0.08);

struct ProgressTextCache {
    elapsed_secs: u32,
    remaining_secs: Option<u32>,
    remaining_initialized: bool,
    elapsed_text: String,
    remaining_text: String,
}

thread_local! {
    static IMG_CACHE: RefCell<Option<(u64, Option<Image>)>> = const { RefCell::new(None) };
    static PROGRESS_SMOOTH: RefCell<f32> = const { RefCell::new(-1.0) };
    static PAUSE_ANIM: RefCell<f32> = const { RefCell::new(0.0) };
    static PAUSE_SPRING: RefCell<Spring> = RefCell::new(Spring::new(1.0));
    static PREV_SKIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static NEXT_SKIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static LOCAL_PLAY_STATE: RefCell<Option<(bool, std::time::Instant)>> = const { RefCell::new(None) };
    static TITLE_SCROLL: RefCell<ScrollText> = RefCell::new(ScrollText::new());
    static ARTIST_SCROLL: RefCell<ScrollText> = RefCell::new(ScrollText::new());
    static COVER_FLIP_ANIM: RefCell<Option<std::time::Instant>> = const { RefCell::new(None) };
    static COVER_FLIP_OLD_IMG: RefCell<Option<Image>> = const { RefCell::new(None) };
    static PROGRESS_HOVER: RefCell<(bool, f32)> = const { RefCell::new((false, 0.0)) };
    static PROGRESS_DRAGGING: RefCell<bool> = const { RefCell::new(false) };
    static COVER_ROTATION: RefCell<f32> = const { RefCell::new(0.0) };
    static PROGRESS_TEXT_CACHE: RefCell<ProgressTextCache> = const { RefCell::new(ProgressTextCache {
        elapsed_secs: u32::MAX,
        remaining_secs: None,
        remaining_initialized: false,
        elapsed_text: String::new(),
        remaining_text: String::new(),
    }) };
}

pub fn draw_text_cached(params: DrawTextCachedParams<'_>) {
    FontManager::global().draw_text_cached(params);
}

pub fn get_cached_media_image(media: &MediaInfo) -> Option<Image> {
    get_cached_media_image_with_key(media).map(|(img, _)| img)
}

fn media_image_key(media: &MediaInfo) -> u64 {
    if media.thumbnail_hash != 0 {
        return media.thumbnail_hash;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    media.title.hash(&mut hasher);
    media.album.hash(&mut hasher);
    hasher.finish()
}

pub fn get_cached_media_image_with_key(media: &MediaInfo) -> Option<(Image, u64)> {
    if media.title.is_empty() {
        clear_cover_cache();
        return None;
    }
    let cache_key = media_image_key(media);

    let mut result: Option<(Image, u64)> = None;
    let mut has_current_image = false;
    IMG_CACHE.with(|cache| {
        let mut cache_mut = cache.borrow_mut();
        if let Some((key, img)) = cache_mut.as_ref()
            && *key == cache_key
        {
            result = img.clone().map(|image| (image, *key));
            has_current_image = true;
            return;
        }
        if let Some(data) = media.thumbnail.as_ref() {
            let image = decode_cover_image(data);
            *cache_mut = Some((cache_key, image.clone()));
            result = image.map(|image| (image, cache_key));
            has_current_image = true;
        }
    });
    if has_current_image {
        let flip_finished = COVER_FLIP_ANIM.with(|cell| {
            cell.borrow()
                .is_none_or(|started| started.elapsed().as_secs_f32() >= 0.6)
        });
        if flip_finished {
            COVER_FLIP_OLD_IMG.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
    if result.is_none() {
        COVER_FLIP_OLD_IMG.with(|cell| {
            if let Some(old_img) = cell.borrow().as_ref() {
                result = Some((old_img.clone(), old_img.unique_id() as u64));
            }
        });
    }
    result
}

pub fn get_media_palette(direct_context: &mut DirectContext, media: &MediaInfo) -> Arc<[Color]> {
    if let Some((img, cache_key)) = get_cached_media_image_with_key(media) {
        get_palette_from_image(direct_context, &img, cache_key)
    } else {
        default_media_palette()
    }
}

pub fn default_media_palette() -> Arc<[Color]> {
    static DEFAULT_PALETTE: OnceLock<Arc<[Color]>> = OnceLock::new();
    DEFAULT_PALETTE
        .get_or_init(|| {
            Arc::from([
                Color::from_rgb(180, 180, 180),
                Color::from_rgb(100, 100, 100),
            ])
        })
        .clone()
}

pub fn clear_cover_cache() {
    IMG_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    COVER_FLIP_OLD_IMG.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub struct DrawMusicPageParams<'a> {
    pub canvas: &'a Canvas,
    pub ox: f32,
    pub oy: f32,
    pub w: f32,
    pub h: f32,
    pub alpha: u8,
    pub media: &'a MediaInfo,
    pub music_active: bool,
    pub available_controls: u32,
    pub view_offset: f32,
    pub scale: f32,
    pub expansion_progress: f32,
    pub viz_h_scale: f32,
    pub use_blur: bool,
    pub font_size: f32,
    pub dt: f32,
    pub text_color: Color,
    pub text_color_sec: Color,
    pub palette: &'a [Color],
}

pub fn draw_music_page(params: DrawMusicPageParams<'_>) -> bool {
    let DrawMusicPageParams {
        canvas,
        ox,
        oy,
        w,
        h,
        alpha,
        media,
        music_active,
        available_controls,
        view_offset,
        scale,
        expansion_progress,
        viz_h_scale,
        use_blur,
        font_size,
        dt,
        text_color,
        text_color_sec,
        palette,
    } = params;

    let arrow_alpha =
        (alpha as f32 * (1.0 - view_offset * PAGE_ARROW_FADE_RATE).clamp(0.0, 1.0)) as u8;
    if arrow_alpha > 0 {
        draw_arrow_right(
            canvas,
            ox + w - PAGE_ARROW_RIGHT_INSET * scale,
            oy + h / 2.0,
            arrow_alpha,
            scale,
            text_color,
        );
    }
    let base_img_size = COVER_SIZE * scale;
    let (img_size, img_x, img_y) = (
        base_img_size,
        ox + CONTENT_PADDING * scale,
        oy + CONTENT_PADDING * scale,
    );
    let pause_t = draw_cover(CoverParams {
        canvas,
        media,
        music_active,
        img_x,
        img_y,
        img_size,
        alpha,
        scale,
        use_blur,
        dt,
        text_color,
    });

    let text_x = img_x + img_size + TRACK_TEXT_GAP * scale;
    let max_text_w = w - (text_x - ox) - TRACK_TEXT_RIGHT_INSET * scale;
    let title_y = img_y + TRACK_TITLE_BASELINE_OFFSET * scale;
    draw_track_text(TrackTextParams {
        canvas,
        media,
        music_active,
        text_x,
        max_text_w,
        title_y,
        alpha,
        font_size,
        scale,
        text_color,
        text_color_sec,
    });

    if music_active {
        let bar_y = img_y + img_size + PROGRESS_TOP_GAP * scale;
        let time_font_size = if font_size > 0.0 {
            font_size * PROGRESS_TIME_FONT_SCALE * scale
        } else {
            DEFAULT_PROGRESS_TIME_FONT_SIZE * scale
        };
        let time_w = PROGRESS_TIME_WIDTH * scale;

        let current_pos_ms = if media.is_playing {
            media
                .position_ms
                .saturating_add(media.last_update.elapsed().as_millis() as u64)
        } else {
            media.position_ms
        };
        let duration_ms = media.effective_duration_ms();
        let current_pos_ms = if duration_ms > 0 {
            current_pos_ms.min(duration_ms)
        } else {
            current_pos_ms
        };
        let raw_progress = if duration_ms > 0 {
            current_pos_ms as f32 / duration_ms as f32
        } else {
            0.0
        };

        let progress = PROGRESS_SMOOTH.with(|cell| {
            let mut smooth = cell.borrow_mut();
            let dragging = PROGRESS_DRAGGING.with(|d| *d.borrow());
            if dragging
                || *smooth < 0.0
                || (*smooth < PROGRESS_START_THRESHOLD && raw_progress > PROGRESS_START_THRESHOLD)
            {
                *smooth = raw_progress;
            } else {
                let diff = (raw_progress - *smooth).abs();
                if diff > PROGRESS_JUMP_THRESHOLD {
                    *smooth = raw_progress;
                } else {
                    *smooth += (raw_progress - *smooth) * PROGRESS_SMOOTHING;
                }
            }
            *smooth
        });

        let elapsed_secs = (current_pos_ms / 1000) as u32;
        let remaining_secs = if duration_ms > 0 {
            Some((duration_ms.saturating_sub(current_pos_ms) / 1000) as u32)
        } else {
            None
        };

        let bar_full_left = ox + CONTENT_PADDING * scale;
        let bar_full_right = ox + w - CONTENT_PADDING * scale;

        let bar_left = bar_full_left + time_w + PROGRESS_TIME_GAP * scale;
        let bar_right = bar_full_right - time_w - PROGRESS_TIME_GAP * scale;
        let bar_total_w = bar_right - bar_left;

        let hover_t = PROGRESS_HOVER.with(|cell| {
            let mut state = cell.borrow_mut();
            let target = if state.0 { 1.0_f32 } else { 0.0 };
            state.1 += (target - state.1) * PROGRESS_HOVER_SMOOTHING;
            if (state.1 - target).abs() < PROGRESS_HOVER_SNAP_THRESHOLD {
                state.1 = target;
            }
            state.1
        });

        let bar_h = (PROGRESS_BAR_HEIGHT + PROGRESS_BAR_HOVER_GROWTH * hover_t) * scale;
        let bar_center_y = bar_y;
        let bar_radius = bar_h / 2.0;

        let text_baseline_y = bar_center_y + time_font_size * PROGRESS_TIME_BASELINE_SCALE;

        let time_alpha_factor =
            PROGRESS_TIME_IDLE_ALPHA + (1.0 - PROGRESS_TIME_IDLE_ALPHA) * hover_t;
        let mut time_paint = Paint::default();
        time_paint.set_anti_alias(true);
        time_paint.set_color(Color::from_argb(
            (alpha as f32 * time_alpha_factor) as u8,
            text_color.r(),
            text_color.g(),
            text_color.b(),
        ));

        PROGRESS_TEXT_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if cache.elapsed_secs != elapsed_secs {
                cache.elapsed_secs = elapsed_secs;
                cache.elapsed_text = format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
            }
            if !cache.remaining_initialized || cache.remaining_secs != remaining_secs {
                cache.remaining_initialized = true;
                cache.remaining_secs = remaining_secs;
                cache.remaining_text = remaining_secs.map_or_else(
                    || "--:--".to_string(),
                    |secs| format!("-{}:{:02}", secs / 60, secs % 60),
                );
            }

            draw_text_cached(DrawTextCachedParams {
                canvas,
                text: &cache.elapsed_text,
                x: bar_full_left,
                y: text_baseline_y,
                size: time_font_size,
                bold: false,
                paint: &time_paint,
            });

            let remaining_w = FontManager::global().measure_text_cached(
                &cache.remaining_text,
                time_font_size,
                FontStyle::normal(),
            );
            draw_text_cached(DrawTextCachedParams {
                canvas,
                text: &cache.remaining_text,
                x: bar_full_right - remaining_w,
                y: text_baseline_y,
                size: time_font_size,
                bold: false,
                paint: &time_paint,
            });
        });

        let mut track_paint = Paint::default();
        track_paint.set_anti_alias(true);
        track_paint.set_color(Color::from_argb(
            (alpha as f32 * PROGRESS_TRACK_ALPHA) as u8,
            text_color.r(),
            text_color.g(),
            text_color.b(),
        ));
        let track_rect = Rect::from_xywh(bar_left, bar_center_y - bar_h / 2.0, bar_total_w, bar_h);
        canvas.draw_round_rect(track_rect, bar_radius, bar_radius, &track_paint);

        let filled_w = (bar_total_w * progress).max(bar_h);
        let mut fill_paint = Paint::default();
        fill_paint.set_anti_alias(true);
        fill_paint.set_color(Color::from_argb(
            alpha,
            text_color.r(),
            text_color.g(),
            text_color.b(),
        ));
        let fill_rect = Rect::from_xywh(bar_left, bar_center_y - bar_h / 2.0, filled_w, bar_h);
        let fill_rrect = RRect::new_rect_radii(
            fill_rect,
            &[
                Point::new(bar_radius, bar_radius),
                Point::new(0.0, 0.0),
                Point::new(0.0, 0.0),
                Point::new(bar_radius, bar_radius),
            ],
        );
        canvas.draw_rrect(fill_rrect, &fill_paint);

        let btn_cx = ox + w / 2.0;
        let btn_cy = bar_center_y + bar_h / 2.0 + PLAYBACK_CONTROLS_TOP_GAP * scale;
        let skip_gap = SKIP_BUTTON_GAP * scale;

        let prev_t = PREV_SKIP_ANIM.with(|cell| {
            let start = *cell.borrow();
            match start {
                Some(s) => {
                    let t = s.elapsed().as_secs_f32() / SKIP_ANIMATION_DURATION_SECS;
                    if t >= 1.0 {
                        *cell.borrow_mut() = None;
                        return None;
                    }
                    Some(t)
                }
                None => None,
            }
        });

        if available_controls & crate::plugin::types::MEDIA_CONTROL_PREVIOUS != 0 {
            draw_skip_button(
                canvas,
                btn_cx - skip_gap,
                btn_cy,
                true,
                prev_t,
                alpha,
                scale,
                use_blur,
                text_color,
            );
        }

        if available_controls & crate::plugin::types::MEDIA_CONTROL_TOGGLE_PLAY != 0 {
            draw_pause_control(
                canvas, btn_cx, btn_cy, pause_t, alpha, scale, use_blur, dt, text_color,
            );
        }

        let next_t = NEXT_SKIP_ANIM.with(|cell| {
            let start = *cell.borrow();
            match start {
                Some(s) => {
                    let t = s.elapsed().as_secs_f32() / SKIP_ANIMATION_DURATION_SECS;
                    if t >= 1.0 {
                        *cell.borrow_mut() = None;
                        return None;
                    }
                    Some(t)
                }
                None => None,
            }
        });

        if available_controls & crate::plugin::types::MEDIA_CONTROL_NEXT != 0 {
            draw_skip_button(
                canvas,
                btn_cx + skip_gap,
                btn_cy,
                false,
                next_t,
                alpha,
                scale,
                use_blur,
                text_color,
            );
        }
    }

    let viz_x_offset = COLLAPSED_VISUALIZER_INSET
        + (EXPANDED_VISUALIZER_INSET - COLLAPSED_VISUALIZER_INSET) * expansion_progress;
    draw_visualizer(DrawVisualizerParams {
        canvas,
        x: ox + w - viz_x_offset * scale,
        y: title_y - VISUALIZER_TITLE_OFFSET * scale,
        alpha,
        is_playing: music_active && media.is_playing,
        palette,
        spectrum: &media.spectrum,
        w_scale: scale,
        h_scale: viz_h_scale,
        smooth_factors: VISUALIZER_SMOOTHING,
    });

    false
}

struct CoverParams<'a> {
    canvas: &'a Canvas,
    media: &'a MediaInfo,
    music_active: bool,
    img_x: f32,
    img_y: f32,
    img_size: f32,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    dt: f32,
    text_color: Color,
}

fn draw_cover(params: CoverParams) -> f32 {
    let CoverParams {
        canvas,
        media,
        music_active,
        img_x,
        img_y,
        img_size,
        alpha,
        scale,
        use_blur,
        dt,
        text_color,
    } = params;

    let image_to_draw = if music_active {
        get_cached_media_image(media)
    } else {
        None
    };

    let mut effective_is_playing = media.is_playing;
    LOCAL_PLAY_STATE.with(|cell| {
        let mut opt = cell.borrow_mut();
        if let Some((opt_val, time)) = *opt {
            if media.is_playing == opt_val || time.elapsed().as_millis() > 2000 {
                *opt = None;
            } else {
                effective_is_playing = opt_val;
            }
        }
    });

    let pause_t = PAUSE_ANIM.with(|cell| {
        let mut v = cell.borrow_mut();
        let target = if effective_is_playing { 1.0_f32 } else { 0.0 };
        let factor = 1.0 - (1.0 - PLAY_STATE_RESPONSE).powf(dt);
        *v += (target - *v) * factor;
        if (*v - target).abs() < 0.003 {
            *v = target;
        }
        *v
    });

    let cover_scale = 0.85 + 0.15 * pause_t;
    let cover_brightness = 0.75 + 0.25 * pause_t;

    let (flip_scale_x, flip_blur_sigma, flip_use_old) = COVER_FLIP_ANIM.with(|cell| {
        let start = *cell.borrow();
        match start {
            Some(s) => {
                let t = (s.elapsed().as_secs_f32() / 0.6).min(1.0);
                if t >= 1.0 {
                    *cell.borrow_mut() = None;
                    (1.0_f32, 0.0_f32, false)
                } else {
                    let eased = if t < 0.5 {
                        let t2 = t * 2.0;
                        t2 * t2 * 0.5
                    } else {
                        let t2 = (t - 0.5) * 2.0;
                        let c1 = 1.2_f32;
                        0.5 + (1.0 + c1 * (t2 - 1.0).powi(2) + (t2 - 1.0).powi(3) * (c1 + 1.0))
                            * 0.5
                    };
                    let cos_val = (eased * std::f32::consts::PI).cos();
                    let sx = cos_val.abs().max(0.02);
                    let blur = (1.0 - cos_val.abs()).powf(0.6) * 8.0 * scale;
                    (sx, blur, cos_val > 0.0)
                }
            }
            None => (1.0, 0.0, false),
        }
    });

    let flip_old_img = if flip_use_old {
        COVER_FLIP_OLD_IMG.with(|cell| cell.borrow().clone())
    } else {
        None
    };

    let cover_img = if flip_use_old {
        flip_old_img.or(image_to_draw.clone())
    } else {
        image_to_draw.clone()
    };

    canvas.save();
    let img_cx = img_x + img_size / 2.0;
    let img_cy = img_y + img_size / 2.0;
    canvas.translate((img_cx, img_cy));

    canvas.scale((cover_scale * flip_scale_x, cover_scale));
    canvas.translate((-img_cx, -img_cy));

    if flip_blur_sigma > 0.1 && use_blur {
        let mut blur_paint = Paint::default();
        blur_paint.set_image_filter(image_filters::blur(
            (flip_blur_sigma, flip_blur_sigma * 0.3),
            None,
            None,
            None,
        ));
        canvas.save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&blur_paint));
    }

    canvas.clip_rrect(
        RRect::new_rect_xy(
            Rect::from_xywh(img_x, img_y, img_size, img_size),
            14.0 * scale,
            14.0 * scale,
        ),
        skia_safe::ClipOp::Intersect,
        true,
    );
    if let Some(img) = cover_img {
        let mut img_paint = Paint::default();
        img_paint.set_anti_alias(true);
        let final_alpha = (alpha as f32 * cover_brightness) / 255.0;
        img_paint.set_alpha_f(final_alpha);
        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        let src_rect = if img_w > 0.0 && img_h > 0.0 {
            let aspect = img_w / img_h;
            let src: Rect = if aspect > 1.0 {
                let crop_w = img_h;
                let offset_x = (img_w - crop_w) / 2.0;
                Rect::from_xywh(offset_x, 0.0, crop_w, img_h)
            } else {
                let crop_h = img_w;
                let offset_y = (img_h - crop_h) / 2.0;
                Rect::from_xywh(0.0, offset_y, img_w, crop_h)
            };
            Some(src)
        } else {
            None
        };
        canvas.draw_image_rect_with_sampling_options(
            &img,
            src_rect.as_ref().map(|r| (r, SrcRectConstraint::Fast)),
            Rect::from_xywh(img_x, img_y, img_size, img_size),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
            &img_paint,
        );
    } else {
        draw_placeholder(canvas, img_x, img_y, img_size, alpha, scale, text_color);
    }
    if flip_blur_sigma > 0.1 && use_blur {
        canvas.restore();
    }
    canvas.restore();

    pause_t
}

struct TrackTextParams<'a> {
    canvas: &'a Canvas,
    media: &'a MediaInfo,
    music_active: bool,
    text_x: f32,
    max_text_w: f32,
    title_y: f32,
    alpha: u8,
    font_size: f32,
    scale: f32,
    text_color: Color,
    text_color_sec: Color,
}

fn draw_track_text(params: TrackTextParams) {
    let TrackTextParams {
        canvas,
        media,
        music_active,
        text_x,
        max_text_w,
        title_y,
        alpha,
        font_size,
        scale,
        text_color,
        text_color_sec,
    } = params;

    let mut text_paint = Paint::default();
    text_paint.set_anti_alias(true);
    let title = if !music_active || media.title.is_empty() {
        "No Music playing"
    } else {
        &media.title
    };
    let artist = if !music_active || media.artist.is_empty() {
        "Unknown Artist"
    } else {
        &media.artist
    };

    text_paint.set_color(Color::from_argb(
        alpha,
        text_color.r(),
        text_color.g(),
        text_color.b(),
    ));
    let title_font_size = if font_size > 0.0 {
        font_size * scale
    } else {
        15.0 * scale
    };
    let title_style = FontStyle::bold();

    TITLE_SCROLL.with(|cell| {
        let mut scroll = cell.borrow_mut();
        scroll.draw(ScrollDrawParams {
            canvas,
            text: title,
            x: text_x,
            y: title_y,
            max_w: max_text_w,
            size: title_font_size,
            style: title_style,
            paint: &text_paint,
            scale,
            render_as_paths: true,
        });
    });

    text_paint.set_color(Color::from_argb(
        (alpha as f32 * 0.6) as u8,
        text_color_sec.r(),
        text_color_sec.g(),
        text_color_sec.b(),
    ));
    let artist_y = title_y + 22.0 * scale;
    let artist_font_size = if font_size > 0.0 {
        font_size * scale
    } else {
        15.0 * scale
    };
    let artist_style = FontStyle::normal();

    ARTIST_SCROLL.with(|cell| {
        let mut scroll = cell.borrow_mut();
        scroll.draw(ScrollDrawParams {
            canvas,
            text: artist,
            x: text_x,
            y: artist_y,
            max_w: max_text_w,
            size: artist_font_size,
            style: artist_style,
            paint: &text_paint,
            scale,
            render_as_paths: true,
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_pause_control(
    canvas: &Canvas,
    btn_cx: f32,
    btn_cy: f32,
    pause_t: f32,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    dt: f32,
    text_color: Color,
) {
    let (pause_s, pause_blur) = PAUSE_SPRING.with(|cell| {
        let mut s = cell.borrow_mut();
        s.update_dt(1.0, PAUSE_CONTROL_STIFFNESS, PAUSE_CONTROL_DAMPING, dt);
        if (s.value - 1.0).abs() < 0.001 && s.velocity.abs() < 0.001 {
            s.value = 1.0;
            s.velocity = 0.0;
        }
        (
            s.value
                .clamp(PAUSE_CONTROL_MIN_SCALE, PAUSE_CONTROL_MAX_SCALE),
            (s.velocity.abs() * PAUSE_CONTROL_BLUR_SCALE * scale)
                .min(PAUSE_CONTROL_MAX_BLUR * scale),
        )
    });

    canvas.save();
    if pause_blur > 0.1 && use_blur {
        let mut blur_paint = Paint::default();
        blur_paint.set_image_filter(image_filters::blur(
            (pause_blur, pause_blur),
            None,
            None,
            None,
        ));
        canvas.save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&blur_paint));
    }
    canvas.translate((btn_cx, btn_cy));
    canvas.scale((pause_s, pause_s));
    let icon_progress = ((pause_t - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    let icon_alpha =
        (alpha as f32 * icon_progress * icon_progress * (3.0 - 2.0 * icon_progress)) as u8;
    if icon_alpha > 0 {
        if pause_t >= 0.5 {
            draw_pause_button(canvas, 0.0, 0.0, icon_alpha, scale, text_color);
        } else {
            draw_play_button(canvas, 0.0, 0.0, icon_alpha, scale, text_color);
        }
    }
    if pause_blur > 0.1 && use_blur {
        canvas.restore();
    }
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_skip_button(
    canvas: &Canvas,
    cx: f32,
    cy: f32,
    mirror: bool,
    anim_t: Option<f32>,
    alpha: u8,
    scale: f32,
    use_blur: bool,
    text_color: Color,
) {
    canvas.save();
    canvas.translate((cx, cy));
    if mirror {
        canvas.scale((-1.0, 1.0));
    }
    if let Some(t) = anim_t {
        let skip_blur = (1.0 - t / 0.3).max(0.0) * 6.0 * scale;
        if skip_blur > 0.1 && use_blur {
            let mut blur_paint = Paint::default();
            blur_paint.set_image_filter(image_filters::blur(
                (skip_blur, skip_blur * 0.3),
                None,
                None,
                None,
            ));
            canvas.save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&blur_paint));
        }

        let shoot_t = (t / 0.25).min(1.0);
        let shoot_x = 10.92 * scale + 22.0 * scale * shoot_t;
        let shoot_alpha = ((alpha as f32) * (1.0 - shoot_t)) as u8;
        if shoot_alpha > 0 {
            draw_control_triangle(canvas, shoot_x, 0.0, shoot_alpha, 0.055, scale, text_color);
        }

        let move_t = (t / 0.55).min(1.0);
        let mid_x = -10.92 * scale + (10.92 * 2.0) * scale * move_t;
        let mid_s = 0.050 + (0.055 - 0.050) * move_t;
        draw_control_triangle(canvas, mid_x, 0.0, alpha, mid_s, scale, text_color);

        let fade_raw = ((t - 0.15) / 0.85).clamp(0.0, 1.0);
        let fade_eased = ease_out_back(fade_raw);
        let new_x = -25.0 * scale + (25.0 - 10.92) * scale * fade_eased;
        let new_alpha = ((alpha as f32) * fade_raw) as u8;
        if new_alpha > 0 {
            draw_control_triangle(canvas, new_x, 0.0, new_alpha, 0.050, scale, text_color);
        }

        if skip_blur > 0.1 && use_blur {
            canvas.restore();
        }
    } else {
        draw_control_triangle(canvas, -10.92 * scale, 0.0, alpha, 0.050, scale, text_color);
        draw_control_triangle(canvas, 10.92 * scale, 0.0, alpha, 0.055, scale, text_color);
    }
    canvas.restore();
}

fn draw_placeholder(
    canvas: &Canvas,
    x: f32,
    y: f32,
    size: f32,
    alpha: u8,
    scale: f32,
    text_color: Color,
) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(
        (alpha as f32 * 0.15) as u8,
        text_color.r(),
        text_color.g(),
        text_color.b(),
    ));
    canvas.draw_round_rect(
        Rect::from_xywh(x, y, size, size),
        14.0 * scale,
        14.0 * scale,
        &paint,
    );

    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    crate::icons::music::draw_music_icon(canvas, cx, cy, alpha, scale * 1.8, text_color);
}

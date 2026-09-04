use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    Canvas, ClipOp, Color, FilterMode, MipmapMode, Paint, RRect, Rect, SamplingOptions,
    image_filters,
};

use crate::core::context::MiniContent;
use crate::core::lyrics::LyricHighlight;
use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::{
    DrawVisualizerParams, draw_text_cached, draw_visualizer, get_cached_media_image,
};
use crate::utils::font::{DrawTextCachedParams, FontManager};

const PENDING_LYRIC_CHANNEL: u8 = 190;
const SECONDARY_LYRIC_SCALE: f32 = 0.85;
const LYRIC_PAIR_GAP_SCALE: f32 = 0.18;
const SECONDARY_LYRIC_CHANNEL: u8 = 176;

pub(crate) fn lyric_font_size(font_size: f32, global_scale: f32) -> f32 {
    if font_size > 0.0 {
        font_size * 0.8 * global_scale
    } else {
        12.0 * global_scale
    }
}

pub(crate) fn lyric_pair_height(font_size: f32, global_scale: f32) -> f32 {
    let primary_size = lyric_font_size(font_size, global_scale);
    primary_size * (1.0 + SECONDARY_LYRIC_SCALE + LYRIC_PAIR_GAP_SCALE) + 8.0 * global_scale
}

pub(super) struct MiniContentParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) content: Option<MiniContent<'a>>,
    pub(super) mini_alpha: f32,
    pub(super) current_w: f32,
    pub(super) global_scale: f32,
    pub(super) media: &'a MediaInfo,
    pub(super) offset_x: f32,
    pub(super) stable_offset_y: f32,
    pub(super) base_h: f32,
    pub(super) palette: &'a [Color],
    pub(super) viz_h_scale: f32,
    pub(super) current_lyric: &'a str,
    pub(super) current_secondary_lyric: &'a str,
    pub(super) old_lyric: &'a str,
    pub(super) old_secondary_lyric: &'a str,
    pub(super) lyric_highlight: Option<LyricHighlight>,
    pub(super) expansion_progress: f32,
    pub(super) font_size: f32,
    pub(super) lyric_scroll_offset: f32,
    pub(super) use_blur: bool,
    pub(super) lyric_transition: f32,
    pub(super) text_color: Color,
}

pub(super) fn draw_mini_content(params: MiniContentParams<'_>) {
    let MiniContentParams {
        canvas,
        content: mini_content,
        mini_alpha: mini_alpha_f,
        current_w,
        global_scale,
        media,
        offset_x,
        stable_offset_y,
        base_h,
        palette,
        viz_h_scale,
        current_lyric,
        current_secondary_lyric,
        old_lyric,
        old_secondary_lyric,
        lyric_highlight,
        expansion_progress,
        font_size,
        lyric_scroll_offset,
        use_blur,
        lyric_transition,
        text_color,
    } = params;
    if mini_alpha_f > 0.01 && current_w > 45.0 * global_scale {
        match mini_content {
            Some(MiniContent::Music) => {
                let alpha = (mini_alpha_f * 255.0) as u8;
                if let Some(image) = get_cached_media_image(media) {
                    let base_size = 18.0 * global_scale;
                    let (size, ix, iy) = (
                        base_size,
                        offset_x + 10.0 * global_scale,
                        stable_offset_y + (base_h - base_size) / 2.0,
                    );
                    let mut paint = Paint::default();
                    paint.set_anti_alias(true);
                    paint.set_alpha_f(alpha as f32 / 255.0);
                    canvas.save();

                    canvas.clip_rrect(
                        RRect::new_rect_xy(
                            Rect::from_xywh(ix, iy, size, size),
                            5.0 * global_scale,
                            5.0 * global_scale,
                        ),
                        ClipOp::Intersect,
                        true,
                    );
                    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
                    let img_w = image.width() as f32;
                    let img_h = image.height() as f32;
                    let src_rect = if img_w > 0.0 && img_h > 0.0 {
                        let aspect = img_w / img_h;
                        let src = if aspect > 1.0 {
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
                        &image,
                        src_rect.as_ref().map(|r| (r, SrcRectConstraint::Fast)),
                        Rect::from_xywh(ix, iy, size, size),
                        sampling,
                        &paint,
                    );
                    canvas.restore();
                }
                let palette = &palette;
                let viz_x = offset_x + current_w - 17.0 * global_scale;
                let viz_y = stable_offset_y + base_h / 2.0;
                draw_visualizer(DrawVisualizerParams {
                    canvas,
                    x: viz_x,
                    y: viz_y,
                    alpha,
                    is_playing: media.is_playing,
                    palette,
                    spectrum: &media.spectrum,
                    w_scale: 0.55 * global_scale,
                    h_scale: viz_h_scale * global_scale,
                    smooth_factors: (0.6, 0.08),
                });

                if !current_lyric.is_empty()
                    || !current_secondary_lyric.is_empty()
                    || !old_lyric.is_empty()
                    || !old_secondary_lyric.is_empty()
                {
                    let lyric_fade_f = (1.0 - expansion_progress * 2.5).clamp(0.0, 1.0);
                    let alpha = (alpha as f32 * lyric_fade_f) as u8;

                    if alpha > 0 {
                        let lyric_font_sz = lyric_font_size(font_size, global_scale);
                        let space_left = offset_x + 30.0 * global_scale;
                        let space_right = offset_x + current_w - 29.0 * global_scale;
                        let available_w = space_right - space_left;
                        let scrolling = lyric_scroll_offset > 0.0;
                        let text_x = if scrolling {
                            space_left - lyric_scroll_offset
                        } else {
                            space_left + available_w / 2.0
                        };
                        let text_centered = !scrolling;

                        canvas.save();
                        let clip_rect =
                            Rect::from_xywh(space_left, stable_offset_y, available_w, base_h);
                        canvas.clip_rect(clip_rect, ClipOp::Intersect, true);

                        if use_blur {
                            if lyric_transition < 1.0 && !old_lyric.is_empty() {
                                let mut text_paint = Paint::default();
                                text_paint.set_anti_alias(true);
                                let fade_alpha = (alpha as f32 * (1.0 - lyric_transition)) as u8;
                                text_paint.set_color(Color::from_argb(
                                    fade_alpha,
                                    text_color.r(),
                                    text_color.g(),
                                    text_color.b(),
                                ));

                                let blur_sigma = lyric_transition * 12.0 * global_scale;
                                if blur_sigma > 0.1 {
                                    text_paint.set_image_filter(image_filters::blur(
                                        (blur_sigma, 0.0),
                                        None,
                                        None,
                                        None,
                                    ));
                                }

                                draw_lyric_pair(LyricPairParams {
                                    canvas,
                                    primary: old_lyric,
                                    secondary: old_secondary_lyric,
                                    anchor_x: text_x,
                                    center_y: stable_offset_y + base_h / 2.0
                                        - 10.0 * global_scale * lyric_transition,
                                    size: lyric_font_sz,
                                    centered: text_centered,
                                    paint: &text_paint,
                                    highlight: None,
                                });
                            }

                            if !current_lyric.is_empty() {
                                let mut text_paint = Paint::default();
                                text_paint.set_anti_alias(true);
                                let fade_alpha = (alpha as f32 * lyric_transition) as u8;
                                text_paint.set_color(Color::from_argb(
                                    fade_alpha,
                                    text_color.r(),
                                    text_color.g(),
                                    text_color.b(),
                                ));

                                let blur_sigma = (1.0 - lyric_transition) * 12.0 * global_scale;
                                if blur_sigma > 0.1 {
                                    text_paint.set_image_filter(image_filters::blur(
                                        (blur_sigma, 0.0),
                                        None,
                                        None,
                                        None,
                                    ));
                                }

                                draw_lyric_pair(LyricPairParams {
                                    canvas,
                                    primary: current_lyric,
                                    secondary: current_secondary_lyric,
                                    anchor_x: text_x,
                                    center_y: stable_offset_y
                                        + base_h / 2.0
                                        + 10.0 * global_scale * (1.0 - lyric_transition),
                                    size: lyric_font_sz,
                                    centered: text_centered,
                                    paint: &text_paint,
                                    highlight: lyric_highlight,
                                });
                            }
                        } else {
                            if lyric_transition < 0.5 && !old_lyric.is_empty() {
                                let mut text_paint = Paint::default();
                                text_paint.set_anti_alias(true);
                                let progress = lyric_transition * 2.0;
                                let fade_alpha = (alpha as f32 * (1.0 - progress)) as u8;
                                text_paint.set_color(Color::from_argb(
                                    fade_alpha,
                                    text_color.r(),
                                    text_color.g(),
                                    text_color.b(),
                                ));
                                draw_lyric_pair(LyricPairParams {
                                    canvas,
                                    primary: old_lyric,
                                    secondary: old_secondary_lyric,
                                    anchor_x: text_x,
                                    center_y: stable_offset_y + base_h / 2.0,
                                    size: lyric_font_sz,
                                    centered: text_centered,
                                    paint: &text_paint,
                                    highlight: None,
                                });
                            } else if lyric_transition >= 0.5 && !current_lyric.is_empty() {
                                let mut text_paint = Paint::default();
                                text_paint.set_anti_alias(true);
                                let progress = (lyric_transition - 0.5) * 2.0;
                                let fade_alpha = (alpha as f32 * progress) as u8;
                                text_paint.set_color(Color::from_argb(
                                    fade_alpha,
                                    text_color.r(),
                                    text_color.g(),
                                    text_color.b(),
                                ));
                                draw_lyric_pair(LyricPairParams {
                                    canvas,
                                    primary: current_lyric,
                                    secondary: current_secondary_lyric,
                                    anchor_x: text_x,
                                    center_y: stable_offset_y + base_h / 2.0,
                                    size: lyric_font_sz,
                                    centered: text_centered,
                                    paint: &text_paint,
                                    highlight: lyric_highlight,
                                });
                            }
                        }
                        canvas.restore();
                    }
                }
            }
            Some(MiniContent::Plugin(ctx)) => {
                let font_sz = if font_size > 0.0 {
                    font_size * 0.7 * global_scale
                } else {
                    11.0 * global_scale
                };
                let alpha = (mini_alpha_f * 255.0) as u8;
                let mut text_paint = Paint::default();
                text_paint.set_anti_alias(true);
                text_paint.set_color(Color::from_argb(
                    alpha,
                    text_color.r(),
                    text_color.g(),
                    text_color.b(),
                ));
                let text_x = offset_x + 20.0 * global_scale;
                let text_w = current_w - 40.0 * global_scale;
                let text_y = stable_offset_y + base_h / 2.0 - font_sz * 0.3;
                canvas.save();
                let clip = Rect::from_xywh(text_x, stable_offset_y, text_w, base_h);
                canvas.clip_rect(clip, ClipOp::Intersect, true);
                draw_text_cached(DrawTextCachedParams {
                    canvas,
                    text: if ctx.compact_text.is_empty() {
                        &ctx.title
                    } else {
                        &ctx.compact_text
                    },
                    x: text_x,
                    y: text_y,
                    size: font_sz,
                    bold: true,
                    paint: &text_paint,
                });
                if !ctx.body.is_empty() {
                    let sec_font_sz = font_sz * 0.8;
                    let mut sec_paint = Paint::default();
                    sec_paint.set_anti_alias(true);
                    sec_paint.set_color(Color::from_argb(
                        (alpha as f32 * 0.7) as u8,
                        text_color.r(),
                        text_color.g(),
                        text_color.b(),
                    ));
                    let sec_y = text_y + font_sz * 1.3;
                    draw_text_cached(DrawTextCachedParams {
                        canvas,
                        text: &ctx.body,
                        x: text_x,
                        y: sec_y,
                        size: sec_font_sz,
                        bold: false,
                        paint: &sec_paint,
                    });
                }
                canvas.restore();
            }
            None => {}
        }
    }
}

struct LyricPairParams<'a> {
    canvas: &'a Canvas,
    primary: &'a str,
    secondary: &'a str,
    anchor_x: f32,
    center_y: f32,
    size: f32,
    centered: bool,
    paint: &'a Paint,
    highlight: Option<LyricHighlight>,
}

fn draw_lyric_pair(params: LyricPairParams<'_>) {
    let LyricPairParams {
        canvas,
        primary,
        secondary,
        anchor_x,
        center_y,
        size,
        centered,
        paint,
        highlight,
    } = params;
    let has_pair = !primary.is_empty() && !secondary.is_empty();
    let secondary_size = size * SECONDARY_LYRIC_SCALE;
    let gap = size * LYRIC_PAIR_GAP_SCALE;
    let stack_height = size + secondary_size + gap;
    let primary_y = if has_pair {
        center_y - stack_height / 2.0 + size * 0.8
    } else {
        center_y + size / 3.0
    };
    let secondary_y = if has_pair {
        center_y + stack_height / 2.0 - secondary_size * 0.2
    } else {
        center_y + secondary_size / 3.0
    };
    let text_x = |text: &str, text_size: f32| {
        if centered {
            let width = FontManager::global().measure_text_cached(
                text,
                text_size,
                skia_safe::FontStyle::normal(),
            );
            anchor_x - width / 2.0
        } else {
            anchor_x
        }
    };

    if !primary.is_empty() {
        draw_highlighted_lyric(
            canvas,
            primary,
            text_x(primary, size),
            primary_y,
            size,
            paint,
            highlight,
        );
    }
    if !secondary.is_empty() {
        let mut secondary_paint = paint.clone();
        let color = paint.color();
        secondary_paint.set_color(Color::from_argb(
            color.a(),
            SECONDARY_LYRIC_CHANNEL,
            SECONDARY_LYRIC_CHANNEL,
            SECONDARY_LYRIC_CHANNEL,
        ));
        draw_text_cached(DrawTextCachedParams {
            canvas,
            text: secondary,
            x: text_x(secondary, secondary_size),
            y: secondary_y,
            size: secondary_size,
            bold: false,
            paint: &secondary_paint,
        });
    }
}

fn draw_highlighted_lyric(
    canvas: &Canvas,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    active_paint: &Paint,
    highlight: Option<LyricHighlight>,
) {
    let Some(highlight) = highlight.filter(|highlight| {
        highlight.start_byte <= highlight.end_byte
            && highlight.end_byte <= text.len()
            && text.is_char_boundary(highlight.start_byte)
            && text.is_char_boundary(highlight.end_byte)
    }) else {
        draw_text_cached(DrawTextCachedParams {
            canvas,
            text,
            x,
            y,
            size,
            bold: false,
            paint: active_paint,
        });
        return;
    };

    let active_color = active_paint.color();
    let mut pending_paint = active_paint.clone();
    pending_paint.set_color(Color::from_argb(
        active_color.a(),
        PENDING_LYRIC_CHANNEL,
        PENDING_LYRIC_CHANNEL,
        PENDING_LYRIC_CHANNEL,
    ));
    draw_text_cached(DrawTextCachedParams {
        canvas,
        text,
        x,
        y,
        size,
        bold: false,
        paint: &pending_paint,
    });

    let font_manager = FontManager::global();
    let style = skia_safe::FontStyle::normal();
    let completed_width =
        font_manager.measure_text_cached(&text[..highlight.start_byte], size, style);
    let active_width = font_manager.measure_text_cached(&text[..highlight.end_byte], size, style);
    let draw_layer = |paint: &Paint, clip_left: f32, clip_right: f32| {
        if clip_right <= clip_left {
            return;
        }
        canvas.save();
        canvas.clip_rect(
            Rect::from_ltrb(clip_left, y - size * 1.5, clip_right, y + size * 0.5),
            ClipOp::Intersect,
            true,
        );
        draw_text_cached(DrawTextCachedParams {
            canvas,
            text,
            x,
            y,
            size,
            bold: false,
            paint,
        });
        canvas.restore();
    };
    draw_layer(active_paint, x, x + completed_width);

    let progress = highlight.progress.clamp(0.0, 1.0);
    let interpolate = |pending: u8, active: u8| {
        (pending as f32 + (active as f32 - pending as f32) * progress).round() as u8
    };
    let mut current_paint = active_paint.clone();
    current_paint.set_color(Color::from_argb(
        active_color.a(),
        interpolate(PENDING_LYRIC_CHANNEL, active_color.r()),
        interpolate(PENDING_LYRIC_CHANNEL, active_color.g()),
        interpolate(PENDING_LYRIC_CHANNEL, active_color.b()),
    ));
    draw_layer(&current_paint, x + completed_width, x + active_width);
}

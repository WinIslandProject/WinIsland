use skia_safe::canvas::SrcRectConstraint;
use skia_safe::{
    Canvas, ClipOp, Color, FilterMode, Image, MipmapMode, Paint, RRect, Rect, SamplingOptions,
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
const MIN_VISIBLE_OPACITY: f32 = 0.01;
const MIN_MINI_CONTENT_WIDTH: f32 = 45.0;
const MINI_COVER_SIZE: f32 = 18.0;
const MINI_COVER_LEFT_INSET: f32 = 10.0;
const MINI_COVER_RADIUS: f32 = 5.0;
const MINI_VISUALIZER_RIGHT_INSET: f32 = 17.0;
const MINI_VISUALIZER_WIDTH_SCALE: f32 = 0.55;
const MINI_VISUALIZER_SMOOTHING: (f32, f32) = (0.6, 0.08);
const LYRIC_LEFT_INSET: f32 = 30.0;
const LYRIC_RIGHT_INSET: f32 = 29.0;
const LYRIC_EXPANSION_FADE_RATE: f32 = 2.5;
const LYRIC_TRANSITION_BLUR_SIGMA: f32 = 12.0;
const LYRIC_TRANSITION_OFFSET: f32 = 10.0;
const MIN_BLUR_SIGMA: f32 = 0.1;
const PLUGIN_FONT_SCALE: f32 = 0.7;
const DEFAULT_PLUGIN_FONT_SIZE: f32 = 11.0;
const PLUGIN_HORIZONTAL_INSET: f32 = 20.0;
const PLUGIN_PRIMARY_BASELINE_OFFSET: f32 = 0.3;
const PLUGIN_SECONDARY_FONT_SCALE: f32 = 0.8;
const PLUGIN_SECONDARY_ALPHA: f32 = 0.7;
const PLUGIN_SECONDARY_LINE_SPACING: f32 = 1.3;

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
    if params.mini_alpha <= MIN_VISIBLE_OPACITY
        || params.current_w <= MIN_MINI_CONTENT_WIDTH * params.global_scale
    {
        return;
    }
    let Some(content) = params.content else {
        return;
    };
    let alpha = scaled_alpha(u8::MAX, params.mini_alpha);
    match content {
        MiniContent::Music => draw_music_content(&params, alpha),
        MiniContent::Plugin(context) => draw_plugin_content(&params, context, alpha),
    }
}

fn draw_music_content(params: &MiniContentParams<'_>, alpha: u8) {
    draw_mini_cover(params, alpha);
    draw_visualizer(DrawVisualizerParams {
        canvas: params.canvas,
        x: params.offset_x + params.current_w - MINI_VISUALIZER_RIGHT_INSET * params.global_scale,
        y: params.stable_offset_y + params.base_h / 2.0,
        alpha,
        is_playing: params.media.is_playing,
        palette: params.palette,
        spectrum: &params.media.spectrum,
        w_scale: MINI_VISUALIZER_WIDTH_SCALE * params.global_scale,
        h_scale: params.viz_h_scale * params.global_scale,
        smooth_factors: MINI_VISUALIZER_SMOOTHING,
    });
    draw_mini_lyrics(params, alpha);
}

fn draw_mini_cover(params: &MiniContentParams<'_>, alpha: u8) {
    let Some(image) = get_cached_media_image(params.media) else {
        return;
    };
    let size = MINI_COVER_SIZE * params.global_scale;
    let x = params.offset_x + MINI_COVER_LEFT_INSET * params.global_scale;
    let y = params.stable_offset_y + (params.base_h - size) / 2.0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(f32::from(alpha) / f32::from(u8::MAX));

    params.canvas.save();
    params.canvas.clip_rrect(
        RRect::new_rect_xy(
            Rect::from_xywh(x, y, size, size),
            MINI_COVER_RADIUS * params.global_scale,
            MINI_COVER_RADIUS * params.global_scale,
        ),
        ClipOp::Intersect,
        true,
    );
    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
    let source_rect = center_crop_rect(&image);
    params.canvas.draw_image_rect_with_sampling_options(
        &image,
        source_rect
            .as_ref()
            .map(|rect| (rect, SrcRectConstraint::Fast)),
        Rect::from_xywh(x, y, size, size),
        sampling,
        &paint,
    );
    params.canvas.restore();
}

fn center_crop_rect(image: &Image) -> Option<Rect> {
    let width = image.width() as f32;
    let height = image.height() as f32;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let edge = width.min(height);
    Some(Rect::from_xywh(
        (width - edge) / 2.0,
        (height - edge) / 2.0,
        edge,
        edge,
    ))
}

fn draw_mini_lyrics(params: &MiniContentParams<'_>, alpha: u8) {
    if !has_lyrics(params) {
        return;
    }
    let lyric_alpha = scaled_alpha(
        alpha,
        (1.0 - params.expansion_progress * LYRIC_EXPANSION_FADE_RATE).clamp(0.0, 1.0),
    );
    if lyric_alpha == 0 {
        return;
    }

    let space_left = params.offset_x + LYRIC_LEFT_INSET * params.global_scale;
    let space_right = params.offset_x + params.current_w - LYRIC_RIGHT_INSET * params.global_scale;
    let available_width = space_right - space_left;
    let scrolling = params.lyric_scroll_offset > 0.0;
    let layout = LyricLayout {
        anchor_x: if scrolling {
            space_left - params.lyric_scroll_offset
        } else {
            space_left + available_width / 2.0
        },
        center_y: params.stable_offset_y + params.base_h / 2.0,
        size: lyric_font_size(params.font_size, params.global_scale),
        centered: !scrolling,
    };

    params.canvas.save();
    params.canvas.clip_rect(
        Rect::from_xywh(
            space_left,
            params.stable_offset_y,
            available_width,
            params.base_h,
        ),
        ClipOp::Intersect,
        true,
    );
    if params.use_blur {
        draw_blurred_lyric_transition(params, layout, lyric_alpha);
    } else {
        draw_crossfade_lyric_transition(params, layout, lyric_alpha);
    }
    params.canvas.restore();
}

fn has_lyrics(params: &MiniContentParams<'_>) -> bool {
    !params.current_lyric.is_empty()
        || !params.current_secondary_lyric.is_empty()
        || !params.old_lyric.is_empty()
        || !params.old_secondary_lyric.is_empty()
}

#[derive(Clone, Copy)]
struct LyricLayout {
    anchor_x: f32,
    center_y: f32,
    size: f32,
    centered: bool,
}

fn draw_blurred_lyric_transition(params: &MiniContentParams<'_>, layout: LyricLayout, alpha: u8) {
    let transition = params.lyric_transition;
    if transition < 1.0 && !params.old_lyric.is_empty() {
        let paint = lyric_paint(
            params.text_color,
            scaled_alpha(alpha, 1.0 - transition),
            transition * LYRIC_TRANSITION_BLUR_SIGMA * params.global_scale,
        );
        draw_lyric_pair(LyricPairParams {
            canvas: params.canvas,
            primary: params.old_lyric,
            secondary: params.old_secondary_lyric,
            anchor_x: layout.anchor_x,
            center_y: layout.center_y - LYRIC_TRANSITION_OFFSET * params.global_scale * transition,
            size: layout.size,
            centered: layout.centered,
            paint: &paint,
            highlight: None,
        });
    }
    if params.current_lyric.is_empty() {
        return;
    }
    let paint = lyric_paint(
        params.text_color,
        scaled_alpha(alpha, transition),
        (1.0 - transition) * LYRIC_TRANSITION_BLUR_SIGMA * params.global_scale,
    );
    draw_lyric_pair(LyricPairParams {
        canvas: params.canvas,
        primary: params.current_lyric,
        secondary: params.current_secondary_lyric,
        anchor_x: layout.anchor_x,
        center_y: layout.center_y
            + LYRIC_TRANSITION_OFFSET * params.global_scale * (1.0 - transition),
        size: layout.size,
        centered: layout.centered,
        paint: &paint,
        highlight: params.lyric_highlight,
    });
}

fn draw_crossfade_lyric_transition(params: &MiniContentParams<'_>, layout: LyricLayout, alpha: u8) {
    let transition = params.lyric_transition;
    let (primary, secondary, fade, highlight) = if transition < 0.5 {
        (
            params.old_lyric,
            params.old_secondary_lyric,
            1.0 - transition * 2.0,
            None,
        )
    } else {
        (
            params.current_lyric,
            params.current_secondary_lyric,
            (transition - 0.5) * 2.0,
            params.lyric_highlight,
        )
    };
    if primary.is_empty() {
        return;
    }
    let paint = lyric_paint(params.text_color, scaled_alpha(alpha, fade), 0.0);
    draw_lyric_pair(LyricPairParams {
        canvas: params.canvas,
        primary,
        secondary,
        anchor_x: layout.anchor_x,
        center_y: layout.center_y,
        size: layout.size,
        centered: layout.centered,
        paint: &paint,
        highlight,
    });
}

fn lyric_paint(color: Color, alpha: u8, blur_sigma: f32) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(alpha, color.r(), color.g(), color.b()));
    if blur_sigma > MIN_BLUR_SIGMA {
        paint.set_image_filter(image_filters::blur((blur_sigma, 0.0), None, None, None));
    }
    paint
}

fn draw_plugin_content(
    params: &MiniContentParams<'_>,
    context: &crate::core::context::PluginContext,
    alpha: u8,
) {
    let font_size = if params.font_size > 0.0 {
        params.font_size * PLUGIN_FONT_SCALE * params.global_scale
    } else {
        DEFAULT_PLUGIN_FONT_SIZE * params.global_scale
    };
    let text_x = params.offset_x + PLUGIN_HORIZONTAL_INSET * params.global_scale;
    let text_width = params.current_w - PLUGIN_HORIZONTAL_INSET * 2.0 * params.global_scale;
    let text_y =
        params.stable_offset_y + params.base_h / 2.0 - font_size * PLUGIN_PRIMARY_BASELINE_OFFSET;
    let text = if context.compact_text.is_empty() {
        &context.title
    } else {
        &context.compact_text
    };
    let paint = lyric_paint(params.text_color, alpha, 0.0);

    params.canvas.save();
    params.canvas.clip_rect(
        Rect::from_xywh(text_x, params.stable_offset_y, text_width, params.base_h),
        ClipOp::Intersect,
        true,
    );
    draw_text_cached(DrawTextCachedParams {
        canvas: params.canvas,
        text,
        x: text_x,
        y: text_y,
        size: font_size,
        bold: true,
        paint: &paint,
    });
    if !context.body.is_empty() {
        let secondary_paint = lyric_paint(
            params.text_color,
            scaled_alpha(alpha, PLUGIN_SECONDARY_ALPHA),
            0.0,
        );
        draw_text_cached(DrawTextCachedParams {
            canvas: params.canvas,
            text: &context.body,
            x: text_x,
            y: text_y + font_size * PLUGIN_SECONDARY_LINE_SPACING,
            size: font_size * PLUGIN_SECONDARY_FONT_SCALE,
            bold: false,
            paint: &secondary_paint,
        });
    }
    params.canvas.restore();
}

fn scaled_alpha(alpha: u8, factor: f32) -> u8 {
    (f32::from(alpha) * factor.clamp(0.0, 1.0)) as u8
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

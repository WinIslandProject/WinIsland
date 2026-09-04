mod background;
mod expanded;
mod mini;

pub(crate) use mini::{
    lyric_font_size as mini_lyric_font_size, lyric_pair_height as mini_lyric_pair_height,
};

use self::background::{BackgroundParams, draw_background};
use self::expanded::{ExpandedContentParams, draw_expanded_content};
use self::mini::{MiniContentParams, draw_mini_content};

use crate::core::config::{CompactWidgetSlot, PluginWidgetSlot, WidgetSlot};
use crate::core::lyrics::LyricHighlight;
use crate::core::smtc::MediaInfo;
use crate::ui::compact::CompactOverlay;
use crate::ui::expanded::music_view::{default_media_palette, get_media_palette};
use crate::utils::shape::g3_rounded_rect_path;
use skia_safe::{ClipOp, Color, Paint, Rect, Surface, gpu::DirectContext, image_filters};

pub struct LayoutParams {
    pub current_w: f32,
    pub current_h: f32,
    pub current_r: f32,
    pub sigmas: (f32, f32),
    pub expansion_progress: f32,
    pub view_offset: f32,
    pub global_scale: f32,
    pub hide_progress: f32,
    pub island_x: f32,
    pub island_y: f32,
    pub stable_island_y: f32,
    pub base_h: f32,
}

pub struct MediaParams<'a> {
    pub media: &'a MediaInfo,
    pub music_active: bool,
    pub available_controls: u32,
}

pub struct LyricsParams<'a> {
    pub current_lyric: &'a str,
    pub current_secondary_lyric: &'a str,
    pub old_lyric: &'a str,
    pub old_secondary_lyric: &'a str,
    pub lyric_highlight: Option<LyricHighlight>,
    pub lyric_transition: f32,
    pub lyric_scroll_offset: f32,
}

pub struct WindowParams {
    pub win_x: i32,
    pub win_y: i32,
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub monitor_w: u32,
    pub monitor_h: u32,
}

pub struct StyleParams<'a> {
    pub island_style: &'a str,
    pub use_blur: bool,
    pub font_size: f32,
    pub lyrics_delay: f64,
    pub dt: f32,
    pub widget_layout: &'a [WidgetSlot],
    pub plugin_widget_layout: &'a [PluginWidgetSlot],
    pub plugin_widgets: &'a crate::core::plugin_widget::WidgetManager,
    pub compact_widget_layout: &'a [CompactWidgetSlot],
}

use crate::core::context::MiniContent;

const MIN_VISIBLE_OPACITY: f32 = 0.01;
const MIN_BLUR_SIGMA: f32 = 0.1;
const MINI_FADE_RATE: f32 = 1.5;
const COLLAPSED_VISUALIZER_HEIGHT_SCALE: f32 = 0.45;
const BORDER_WIDTH: f32 = 1.0;
const BORDER_INSET: f32 = BORDER_WIDTH / 2.0;
const SOLID_STYLE: &str = "default";
const SOLID_BORDER_ALPHA: u8 = 30;
const EFFECT_BORDER_ALPHA: u8 = 40;

pub struct DrawIslandParams<'a> {
    pub layout: LayoutParams,
    pub media: MediaParams<'a>,
    pub lyrics: LyricsParams<'a>,
    pub mini_content: Option<MiniContent<'a>>,
    pub compact_overlay: &'a CompactOverlay,
    pub window: WindowParams,
    pub style: StyleParams<'a>,
}

pub fn draw_island(
    direct_context: &mut DirectContext,
    surface: &mut Surface,
    params: DrawIslandParams<'_>,
) -> bool {
    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);
    let layout = &params.layout;
    let rect = Rect::from_xywh(
        layout.island_x,
        layout.island_y,
        layout.current_w,
        layout.current_h,
    );
    let island_path = g3_rounded_rect_path(rect, layout.current_r);
    let blur_filter = if layout.sigmas.0 > MIN_BLUR_SIGMA || layout.sigmas.1 > MIN_BLUR_SIGMA {
        image_filters::blur(layout.sigmas, None, None, None)
    } else {
        None
    };
    draw_background_layer(canvas, direct_context, &params, rect, &island_path);
    canvas.save();
    canvas.clip_path(&island_path, ClipOp::Intersect, true);

    let compact_overlay_visible = params.compact_overlay.is_visible();
    let expanded_alpha = if compact_overlay_visible {
        0.0
    } else {
        layout.expansion_progress.powi(2).clamp(0.0, 1.0) * (1.0 - layout.hide_progress)
    };
    let mini_alpha = if compact_overlay_visible {
        0.0
    } else {
        (1.0 - layout.expansion_progress * MINI_FADE_RATE).clamp(0.0, 1.0)
            * (1.0 - layout.hide_progress)
    };
    let palette = if expanded_alpha > MIN_VISIBLE_OPACITY || mini_alpha > MIN_VISIBLE_OPACITY {
        get_media_palette(direct_context, params.media.media)
    } else {
        default_media_palette()
    };
    let visualizer_height_scale = COLLAPSED_VISUALIZER_HEIGHT_SCALE
        + (1.0 - COLLAPSED_VISUALIZER_HEIGHT_SCALE) * layout.expansion_progress;
    let widget_animating = draw_expanded_layer(
        canvas,
        blur_filter,
        &params,
        &palette,
        expanded_alpha,
        visualizer_height_scale,
    );
    if compact_overlay_visible {
        params.compact_overlay.draw(
            canvas,
            rect,
            layout.global_scale,
            1.0 - layout.hide_progress,
        );
    } else {
        draw_compact_layer(
            canvas,
            &params,
            &palette,
            mini_alpha,
            visualizer_height_scale,
        );
    }
    canvas.restore();
    draw_island_border(canvas, &params);
    widget_animating
}

fn draw_background_layer(
    canvas: &skia_safe::Canvas,
    direct_context: &mut DirectContext,
    params: &DrawIslandParams<'_>,
    rect: Rect,
    island_path: &skia_safe::Path,
) {
    let layout = &params.layout;
    let window = &params.window;
    draw_background(BackgroundParams {
        canvas,
        direct_context,
        rect,
        island_path,
        island_style: params.style.island_style,
        media: params.media.media,
        win_x: window.win_x,
        win_y: window.win_y,
        offset_x: layout.island_x,
        offset_y: layout.island_y,
        current_w: layout.current_w,
        current_h: layout.current_h,
        global_scale: layout.global_scale,
        monitor_x: window.monitor_x,
        monitor_y: window.monitor_y,
        monitor_w: window.monitor_w,
        monitor_h: window.monitor_h,
    });
}

fn draw_expanded_layer(
    canvas: &skia_safe::Canvas,
    blur_filter: Option<skia_safe::ImageFilter>,
    params: &DrawIslandParams<'_>,
    palette: &[Color],
    alpha: f32,
    visualizer_height_scale: f32,
) -> bool {
    let layout = &params.layout;
    let media = &params.media;
    let style = &params.style;
    draw_expanded_content(ExpandedContentParams {
        canvas,
        blur_filter,
        expanded_alpha: alpha,
        view_offset: layout.view_offset,
        current_w: layout.current_w,
        offset_x: layout.island_x,
        offset_y: layout.island_y,
        current_h: layout.current_h,
        media: media.media,
        music_active: media.music_active,
        available_controls: media.available_controls,
        global_scale: layout.global_scale,
        expansion_progress: layout.expansion_progress,
        viz_h_scale: visualizer_height_scale,
        use_blur: style.use_blur,
        font_size: style.font_size,
        dt: style.dt,
        text_color: Color::WHITE,
        text_color_sec: Color::WHITE,
        palette,
        lyrics_delay: style.lyrics_delay,
        widget_layout: style.widget_layout,
        plugin_widget_layout: style.plugin_widget_layout,
        plugin_widgets: style.plugin_widgets,
    })
}

fn draw_compact_layer(
    canvas: &skia_safe::Canvas,
    params: &DrawIslandParams<'_>,
    palette: &[Color],
    alpha: f32,
    visualizer_height_scale: f32,
) {
    let layout = &params.layout;
    let lyrics = &params.lyrics;
    let style = &params.style;
    let has_mini_content = params.mini_content.is_some();
    let center_occupied = crate::ui::widget::compact::has_center_widget(
        style.compact_widget_layout,
        has_mini_content,
    );
    let visible_mini_content = if center_occupied {
        None
    } else {
        params.mini_content
    };
    let (left_extension, right_extension) = crate::ui::widget::compact::side_extensions(
        style.compact_widget_layout,
        center_occupied || has_mini_content,
        has_mini_content,
    );
    let left_extension = left_extension * layout.global_scale;
    let right_extension = right_extension * layout.global_scale;
    draw_mini_content(MiniContentParams {
        canvas,
        content: visible_mini_content,
        mini_alpha: alpha,
        current_w: (layout.current_w - left_extension - right_extension).max(0.0),
        global_scale: layout.global_scale,
        media: params.media.media,
        offset_x: layout.island_x + left_extension,
        stable_offset_y: layout.stable_island_y,
        base_h: layout.base_h,
        palette,
        viz_h_scale: visualizer_height_scale,
        current_lyric: lyrics.current_lyric,
        current_secondary_lyric: lyrics.current_secondary_lyric,
        old_lyric: lyrics.old_lyric,
        old_secondary_lyric: lyrics.old_secondary_lyric,
        lyric_highlight: lyrics.lyric_highlight,
        expansion_progress: layout.expansion_progress,
        font_size: style.font_size,
        lyric_scroll_offset: lyrics.lyric_scroll_offset,
        use_blur: style.use_blur,
        lyric_transition: lyrics.lyric_transition,
        text_color: Color::WHITE,
    });
    crate::ui::widget::compact::draw(
        canvas,
        style.compact_widget_layout,
        Rect::from_xywh(
            layout.island_x,
            layout.stable_island_y,
            layout.current_w,
            layout.base_h,
        ),
        layout.global_scale,
        (alpha * f32::from(u8::MAX)) as u8,
        has_mini_content,
    );
}

fn draw_island_border(canvas: &skia_safe::Canvas, params: &DrawIslandParams<'_>) {
    let layout = &params.layout;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(skia_safe::PaintStyle::Stroke);
    paint.set_stroke_width(BORDER_WIDTH);
    let alpha = if params.style.island_style == SOLID_STYLE {
        SOLID_BORDER_ALPHA
    } else {
        EFFECT_BORDER_ALPHA
    };
    paint.set_color(Color::from_argb(alpha, u8::MAX, u8::MAX, u8::MAX));
    let border_path = g3_rounded_rect_path(
        Rect::from_xywh(
            layout.island_x + BORDER_INSET,
            layout.island_y + BORDER_INSET,
            (layout.current_w - BORDER_WIDTH).max(0.0),
            (layout.current_h - BORDER_WIDTH).max(0.0),
        ),
        (layout.current_r - BORDER_INSET).max(0.0),
    );
    canvas.draw_path(&border_path, &paint);
}

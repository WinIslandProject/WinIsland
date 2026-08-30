mod background;
mod expanded;
mod mini;

pub(crate) use mini::lyric_font_size as mini_lyric_font_size;

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
    pub old_lyric: &'a str,
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
    let DrawIslandParams {
        layout,
        media,
        lyrics,
        mini_content,
        compact_overlay,
        window,
        style,
    } = params;

    let LayoutParams {
        current_w,
        current_h,
        current_r,
        sigmas,
        expansion_progress,
        view_offset,
        global_scale,
        hide_progress,
        island_x,
        island_y,
        stable_island_y,
        base_h,
    } = layout;
    let MediaParams {
        media,
        music_active,
        available_controls,
    } = media;
    let LyricsParams {
        current_lyric,
        old_lyric,
        lyric_highlight,
        lyric_transition,
        lyric_scroll_offset,
    } = lyrics;
    let WindowParams {
        win_x,
        win_y,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    } = window;
    let StyleParams {
        island_style,
        use_blur,
        font_size,
        lyrics_delay,
        dt,
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        compact_widget_layout,
    } = style;
    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);

    let offset_x = island_x;
    let offset_y = island_y;
    let stable_offset_y = stable_island_y;

    let rect = Rect::from_xywh(offset_x, offset_y, current_w, current_h);
    let island_path = g3_rounded_rect_path(rect, current_r);
    let has_blur = sigmas.0 > 0.1 || sigmas.1 > 0.1;
    let blur_filter = if has_blur {
        image_filters::blur(sigmas, None, None, None)
    } else {
        None
    };

    let text_color = Color::WHITE;
    let text_color_sec = Color::WHITE;

    draw_background(BackgroundParams {
        canvas,
        direct_context,
        rect,
        island_path: &island_path,
        island_style,
        media,
        win_x,
        win_y,
        offset_x,
        offset_y,
        current_w,
        current_h,
        global_scale,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    });
    canvas.save();
    canvas.clip_path(&island_path, ClipOp::Intersect, true);

    let compact_overlay_visible = compact_overlay.is_visible();
    let expanded_alpha_f = if compact_overlay_visible {
        0.0
    } else {
        (expansion_progress.powf(2.0)).clamp(0.0, 1.0) * (1.0 - hide_progress)
    };
    let mini_alpha_f = if compact_overlay_visible {
        0.0
    } else {
        (1.0 - expansion_progress * 1.5).clamp(0.0, 1.0) * (1.0 - hide_progress)
    };

    let palette = if expanded_alpha_f > 0.01 || mini_alpha_f > 0.01 {
        get_media_palette(direct_context, media)
    } else {
        default_media_palette()
    };

    let viz_h_scale = 0.45 + (1.0 - 0.45) * expansion_progress;

    let widget_animating = draw_expanded_content(ExpandedContentParams {
        canvas,
        blur_filter,
        expanded_alpha: expanded_alpha_f,
        view_offset,
        current_w,
        offset_x,
        offset_y,
        current_h,
        media,
        music_active,
        available_controls,
        global_scale,
        expansion_progress,
        viz_h_scale,
        use_blur,
        font_size,
        dt,
        text_color,
        text_color_sec,
        palette: &palette,
        lyrics_delay,
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
    });
    if compact_overlay_visible {
        compact_overlay.draw(canvas, rect, global_scale, 1.0 - hide_progress);
    } else {
        let has_mini_content = mini_content.is_some();
        let center_occupied =
            crate::ui::widget::compact::has_center_widget(compact_widget_layout, has_mini_content);
        let visible_mini_content = if center_occupied { None } else { mini_content };
        let has_center_content = center_occupied || has_mini_content;
        let (left_extension, right_extension) = crate::ui::widget::compact::side_extensions(
            compact_widget_layout,
            has_center_content,
            has_mini_content,
        );
        let left_extension = left_extension * global_scale;
        let right_extension = right_extension * global_scale;
        draw_mini_content(MiniContentParams {
            canvas,
            content: visible_mini_content,
            mini_alpha: mini_alpha_f,
            current_w: (current_w - left_extension - right_extension).max(0.0),
            global_scale,
            media,
            offset_x: offset_x + left_extension,
            stable_offset_y,
            base_h,
            palette: &palette,
            viz_h_scale,
            current_lyric,
            old_lyric,
            lyric_highlight,
            expansion_progress,
            font_size,
            lyric_scroll_offset,
            use_blur,
            lyric_transition,
            text_color,
        });
        crate::ui::widget::compact::draw(
            canvas,
            compact_widget_layout,
            Rect::from_xywh(offset_x, stable_offset_y, current_w, base_h),
            global_scale,
            (mini_alpha_f * 255.0) as u8,
            has_mini_content,
        );
    }
    canvas.restore();

    {
        let mut border_paint = Paint::default();
        border_paint.set_anti_alias(true);
        border_paint.set_style(skia_safe::PaintStyle::Stroke);
        border_paint.set_stroke_width(1.0);
        if island_style == "default" {
            border_paint.set_color(Color::from_argb(30, 255, 255, 255));
        } else {
            border_paint.set_color(Color::from_argb(40, 255, 255, 255));
        }
        let border_path = g3_rounded_rect_path(
            Rect::from_xywh(
                offset_x + 0.5,
                offset_y + 0.5,
                (current_w - 1.0).max(0.0),
                (current_h - 1.0).max(0.0),
            ),
            (current_r - 0.5).max(0.0),
        );
        canvas.draw_path(&border_path, &border_paint);
    }
    widget_animating
}

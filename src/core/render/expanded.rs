use skia_safe::{Canvas, Color, ImageFilter, Paint};

use crate::core::config::{PluginWidgetSlot, WidgetSlot};
use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::{DrawMusicPageParams, draw_music_page};
use crate::ui::expanded::widget_view::draw_widget_page;

pub(super) struct ExpandedContentParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) blur_filter: Option<ImageFilter>,
    pub(super) expanded_alpha: f32,
    pub(super) view_offset: f32,
    pub(super) current_w: f32,
    pub(super) offset_x: f32,
    pub(super) offset_y: f32,
    pub(super) current_h: f32,
    pub(super) media: &'a MediaInfo,
    pub(super) music_active: bool,
    pub(super) available_controls: u32,
    pub(super) global_scale: f32,
    pub(super) expansion_progress: f32,
    pub(super) viz_h_scale: f32,
    pub(super) use_blur: bool,
    pub(super) font_size: f32,
    pub(super) dt: f32,
    pub(super) text_color: Color,
    pub(super) text_color_sec: Color,
    pub(super) palette: &'a [Color],
    pub(super) widget_layout: &'a [WidgetSlot],
    pub(super) plugin_widget_layout: &'a [PluginWidgetSlot],
    pub(super) plugin_widgets: &'a crate::core::plugin_widget::WidgetManager,
}

pub(super) fn draw_expanded_content(params: ExpandedContentParams<'_>) -> bool {
    let ExpandedContentParams {
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
        palette,
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
    } = params;
    let mut widget_animating = false;
    if expanded_alpha_f > 0.01 {
        let music_page_available = music_active;
        let alpha = (expanded_alpha_f * 255.0) as u8;
        canvas.save();
        if let Some(ref filter) = blur_filter {
            let mut layer_paint = Paint::default();
            layer_paint.set_image_filter(filter.clone());
            canvas.save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&layer_paint));
        }

        let visible_view_offset = if music_page_available {
            view_offset
        } else {
            1.0
        };
        let page_shift = visible_view_offset * current_w;

        if music_page_available {
            canvas.save();
            canvas.translate((-page_shift, 0.0));
            draw_music_page(DrawMusicPageParams {
                canvas,
                ox: offset_x,
                oy: offset_y,
                w: current_w,
                h: current_h,
                alpha,
                media,
                music_active,
                available_controls,
                view_offset: visible_view_offset,
                scale: global_scale,
                expansion_progress,
                viz_h_scale: viz_h_scale * global_scale,
                use_blur,
                font_size,
                dt,
                text_color,
                text_color_sec,
                palette,
            });
            canvas.restore();
        }

        if visible_view_offset > 0.0 {
            canvas.save();
            canvas.translate((current_w - page_shift, 0.0));
            let widget_anim = draw_widget_page(
                canvas,
                offset_x,
                offset_y,
                current_w,
                current_h,
                alpha,
                global_scale,
                widget_layout,
                plugin_widget_layout,
                plugin_widgets,
                text_color,
                music_page_available,
            );
            canvas.restore();

            widget_animating = widget_anim;
        }

        if blur_filter.is_some() {
            canvas.restore();
        }
        canvas.restore();
    }
    widget_animating
}

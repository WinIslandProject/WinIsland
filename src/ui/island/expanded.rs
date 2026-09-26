use crate::core::smtc::MediaInfo;
use crate::ui::expanded::music_view::{DrawMusicPageParams, draw_music_page};
use crate::ui::expanded::widget_view::draw_widget_page;
use winisland_core::config::{PluginWidgetSlot, WidgetSlot};
use winisland_render::{BlurSpec, LayerSpec, Painter, Rgba, Vec2};

pub(super) struct ExpandedContentParams<'a> {
    pub(super) painter: Painter<'a>,
    pub(super) blur_filter: Option<BlurSpec>,
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
    pub(super) text_color: Rgba,
    pub(super) text_color_sec: Rgba,
    pub(super) palette: &'a [Rgba],
    pub(super) widget_layout: &'a [WidgetSlot],
    pub(super) plugin_widget_layout: &'a [PluginWidgetSlot],
    pub(super) plugin_widgets: &'a winisland_core::widgets::WidgetManager,
}

pub(super) fn draw_expanded_content(params: ExpandedContentParams<'_>) -> bool {
    let ExpandedContentParams {
        painter,
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
        painter.save();
        if let Some(filter) = blur_filter {
            painter.begin_layer(LayerSpec::Blur(filter));
        }

        let visible_view_offset = if music_page_available {
            view_offset
        } else {
            1.0
        };
        let page_shift = visible_view_offset * current_w;

        if music_page_available && visible_view_offset < 1.0 {
            painter.save();
            painter.translate(Vec2::new(-page_shift, 0.0));
            draw_music_page(DrawMusicPageParams {
                painter,
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
            painter.restore();
        }

        if visible_view_offset > 0.0 {
            painter.save();
            painter.translate(Vec2::new(current_w - page_shift, 0.0));
            let widget_anim = draw_widget_page(
                painter,
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
            painter.restore();

            widget_animating = widget_anim;
        }

        if blur_filter.is_some() {
            painter.restore();
        }
        painter.restore();
    }
    widget_animating
}

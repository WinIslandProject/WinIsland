use crate::icons::arrows::draw_arrow_left;
use crate::ui::widget::expanded::{draw_widget, widget_animates, widget_grid_layout};
use std::collections::HashMap;
use winisland_core::config::{
    PluginWidgetSlot, WIDGET_GRID_SLOTS, WidgetSlot, first_free_anchor, plugin_widget_slot,
    span_cells, widget_footprint,
};
use winisland_core::widgets::WidgetManager;
use winisland_plugin_host::draw::replay::{PreparedFrame, replay};
use winisland_plugin_host::host::PluginHost;
use winisland_render::{Painter, Rect, Rgba};

#[allow(clippy::too_many_arguments)]
pub fn draw_prepared_widget(
    painter: Painter<'_>,
    widget_id: u64,
    frame: &PreparedFrame,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    alpha: u8,
) {
    let save_count = painter.save();
    painter.clip_rect_with_anti_alias(Rect::from_xywh(x, y, width, height), false);
    painter.translate(winisland_render::Vec2::new(x, y));
    painter.scale(winisland_render::Vec2::new(
        width / frame.logical_width,
        height / frame.logical_height,
    ));
    let elapsed = replay(frame, painter, alpha);
    painter.restore_to(save_count);
    if elapsed.as_millis() > 3 {
        log::warn!("Plugin widget {widget_id} replay took {elapsed:?}");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_widget_page(
    painter: Painter<'_>,
    ox: f32,
    oy: f32,
    w: f32,
    h: f32,
    alpha: u8,
    scale: f32,
    widget_layout: &[WidgetSlot],
    plugin_widget_layout: &[PluginWidgetSlot],
    plugin_widgets: &WidgetManager,
    plugin_frames: &HashMap<u64, PreparedFrame>,
    plugin_host: Option<&PluginHost>,
    text_color: Rgba,
    show_page_switcher: bool,
) -> bool {
    let mut animating = false;

    if alpha > 20 {
        let layout = widget_grid_layout(ox, oy, w, h, scale);

        let mut occupied = [false; WIDGET_GRID_SLOTS];
        for slot in 0..WIDGET_GRID_SLOTS {
            let Some(kind) = widget_layout
                .iter()
                .find(|entry| entry.slot == slot)
                .and_then(|entry| entry.widget)
            else {
                continue;
            };

            let (slot_x, slot_y, tile_w, tile_h) = layout.footprint_rect(kind, slot);

            draw_widget(
                painter, kind, slot_x, slot_y, tile_w, tile_h, scale, alpha, text_color,
            );

            for cell in widget_footprint(kind, slot) {
                occupied[cell] = true;
            }

            if widget_animates(kind) {
                animating = true;
            }
        }

        for widget in plugin_widgets.widgets() {
            let span = widget.span();
            let configured_anchor = widget.layout_id().and_then(|id| {
                plugin_widget_slot(plugin_widget_layout, &id).map(|entry| entry.slot)
            });
            let anchor = match configured_anchor {
                Some(anchor) => {
                    let cells = span_cells(anchor, span);
                    (!cells.is_empty() && cells.iter().all(|cell| !occupied[*cell]))
                        .then_some(cells[0])
                }
                None if widget.key.is_none() => first_free_anchor(&occupied, span),
                None => None,
            };
            let Some(anchor) = anchor else {
                continue;
            };
            for cell in span_cells(anchor, span) {
                occupied[cell] = true;
            }
            let (slot_x, slot_y, tile_w, tile_h) = layout.footprint_rect_span(anchor, span);
            if let Some(host) = plugin_host
                && plugin_frames.contains_key(&widget.id)
            {
                let inv_scale = if scale > 0.0 { 1.0 / scale } else { 1.0 };
                let _ =
                    host.set_widget_logical_size(widget.id, tile_w * inv_scale, tile_h * inv_scale);
            }
            if let Some(frame) = plugin_frames.get(&widget.id) {
                draw_prepared_widget(
                    painter, widget.id, frame, slot_x, slot_y, tile_w, tile_h, alpha,
                );
            }
        }
    }

    if show_page_switcher && alpha > 0 {
        draw_arrow_left(
            painter,
            ox + 7.5 * scale,
            oy + h / 2.0,
            alpha,
            scale,
            text_color,
        );
    }

    animating
}

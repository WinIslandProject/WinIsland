use crate::core::config::{
    PluginWidgetSlot, WIDGET_GRID_SLOTS, WidgetSlot, first_free_anchor, plugin_widget_slot,
    span_cells, widget_footprint,
};
use crate::core::plugin_widget::WidgetManager;
use crate::icons::arrows::draw_arrow_left;
use crate::plugin::types::{INTERFACE_VERSION_1, WidgetDrawContextV1};
use crate::ui::widget::expanded::{draw_widget, widget_animates, widget_grid_layout};
use skia_safe::{Canvas, Color, Rect};
use std::ffi::c_void;

#[allow(clippy::too_many_arguments)]
pub fn draw_plugin_widget(
    canvas: &Canvas,
    widget: &crate::core::plugin_widget::PluginWidget,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f32,
    alpha: u8,
) {
    let Some(callback) = crate::plugin::manager::acquire_widget_draw(widget.id) else {
        return;
    };
    let inv_scale = if scale > 0.0 { 1.0 / scale } else { 1.0 };
    let ctx = WidgetDrawContextV1 {
        struct_size: std::mem::size_of::<WidgetDrawContextV1>() as u32,
        version: INTERFACE_VERSION_1,
        width: width * inv_scale,
        height: height * inv_scale,
        scale,
        alpha,
        canvas_handle: canvas as *const Canvas as *mut c_void,
        draw: crate::plugin::manager::draw_api(),
    };
    let save_count = canvas.save();
    canvas.clip_rect(Rect::from_xywh(x, y, width, height), None, false);
    canvas.translate((x, y));
    crate::plugin::manager::reset_draw_transform();
    callback.draw(&ctx);
    canvas.restore_to_count(save_count);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_widget_page(
    canvas: &Canvas,
    ox: f32,
    oy: f32,
    w: f32,
    h: f32,
    alpha: u8,
    scale: f32,
    widget_layout: &[WidgetSlot],
    plugin_widget_layout: &[PluginWidgetSlot],
    plugin_widgets: &WidgetManager,
    text_color: Color,
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
                canvas, kind, slot_x, slot_y, tile_w, tile_h, scale, alpha, text_color,
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
            draw_plugin_widget(canvas, widget, slot_x, slot_y, tile_w, tile_h, scale, alpha);
        }
    }

    if show_page_switcher && alpha > 0 {
        draw_arrow_left(
            canvas,
            ox + 7.5 * scale,
            oy + h / 2.0,
            alpha,
            scale,
            text_color,
        );
    }

    animating
}

use skia_safe::{Canvas, Color, FontStyle, Paint, Point, Rect};

use crate::core::config::{
    CompactWidgetAlignment, CompactWidgetKind, CompactWidgetPosition, CompactWidgetSlot,
    PluginWidgetSlot, WIDGET_GRID_SLOTS, WidgetKind, WidgetSlot, plugin_widget_slot, span_cells,
    widget_footprint,
};
use crate::core::i18n::tr;
use crate::core::plugin_widget::PluginWidget;
use crate::ui::widget::expanded::{
    draw_mini_card, draw_widget_preview as draw_widget_card_preview,
};
use crate::utils::color::SettingsTheme;
use crate::utils::font::{DrawTextCachedParams, FontManager};
use crate::utils::shape::g3_rounded_rect_path;

use super::super::input::{
    COMPACT_WIDGET_ISLAND_PANEL_H, COMPACT_WIDGET_PREVIEW_H, CompactWidgetGridGeom,
    WIDGET_ISLAND_PANEL_H, WIDGET_LIBRARY_HEADER_H, WIDGET_PANEL_GAP, WidgetEditorMode,
    WidgetGridGeom, WidgetSource, compact_widget_grid_geom, compact_widget_library_items,
    widget_delete_button_center, widget_grid_geom, widget_library_items, widget_source_rect,
    widget_source_span,
};
use super::super::items::{CONTENT_PADDING, GROUP_INNER_PAD};

#[derive(Clone, Copy)]
pub(super) struct WidgetPreviewParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) item_y: f32,
    pub(super) width: f32,
    pub(super) content_width: f32,
    pub(super) visible_min_y: f32,
    pub(super) visible_max_y: f32,
    pub(super) island_style: &'a str,
    pub(super) expanded_width: f32,
    pub(super) expanded_height: f32,
    pub(super) base_width: f32,
    pub(super) base_height: f32,
    pub(super) widget_editor_mode: WidgetEditorMode,
    pub(super) widget_layout: &'a [WidgetSlot],
    pub(super) plugin_widget_layout: &'a [PluginWidgetSlot],
    pub(super) plugin_widgets: &'a [PluginWidget],
    pub(super) widget_dragging: Option<&'a WidgetSource>,
    pub(super) widget_drag_hover_slot: Option<usize>,
    pub(super) widget_preview_hover_slot: Option<usize>,
    pub(super) compact_widget_layout: &'a [CompactWidgetSlot],
    pub(super) compact_widget_dragging: Option<CompactWidgetKind>,
    pub(super) compact_widget_drag_hover_slot: Option<CompactWidgetPosition>,
    pub(super) compact_widget_preview_hover_slot: Option<CompactWidgetPosition>,
    pub(super) theme: &'a SettingsTheme,
}

fn draw_panel(canvas: &Canvas, rect: Rect, theme: &SettingsTheme) {
    let mut shadow = Paint::default();
    shadow.set_anti_alias(true);
    shadow.set_color(theme.shadow);
    canvas.draw_round_rect(
        Rect::from_xywh(rect.left, rect.top + 2.0, rect.width(), rect.height()),
        14.0,
        14.0,
        &shadow,
    );

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(theme.group_bg);
    canvas.draw_round_rect(rect, 14.0, 14.0, &paint);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(0.75);
    paint.set_color(theme.group_border);
    canvas.draw_round_rect(
        Rect::from_xywh(
            rect.left + 0.375,
            rect.top + 0.375,
            rect.width() - 0.75,
            rect.height() - 0.75,
        ),
        14.0,
        14.0,
        &paint,
    );
}

fn draw_label(canvas: &Canvas, text: &str, x: f32, y: f32, size: f32, bold: bool, color: Color) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    FontManager::global().draw_text_cached(DrawTextCachedParams {
        canvas,
        text,
        x,
        y,
        size,
        bold,
        paint: &paint,
    });
}

fn draw_centered_label(canvas: &Canvas, text: &str, rect: Rect, size: f32, color: Color) {
    let font_manager = FontManager::global();
    let text_width = font_manager.measure_text_cached(text, size, FontStyle::normal());
    draw_label(
        canvas,
        text,
        rect.center_x() - text_width / 2.0,
        rect.center_y() + size * 0.35,
        size,
        false,
        color,
    );
}

fn draw_island_background(
    canvas: &Canvas,
    rect: Rect,
    island_style: &str,
    theme: &SettingsTheme,
    corner_radius: f32,
) {
    let mut shadow = Paint::default();
    shadow.set_anti_alias(true);
    shadow.set_color(Color::from_argb(72, 0, 0, 0));
    let shadow_path = g3_rounded_rect_path(
        Rect::from_xywh(rect.left, rect.top + 4.0, rect.width(), rect.height()),
        corner_radius,
    );
    canvas.draw_path(&shadow_path, &shadow);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    if island_style == "glass" || island_style == "mica" {
        paint.set_color(Color::from_argb(220, 24, 24, 28));
    } else if island_style == "dynamic" {
        let colors = [Color::from_rgb(18, 12, 36), Color::from_rgb(8, 24, 48)];
        #[allow(deprecated)]
        if let Some(shader) = skia_safe::gradient_shader::linear(
            (
                Point::new(rect.left, rect.top),
                Point::new(rect.right, rect.bottom),
            ),
            &colors[..],
            None,
            skia_safe::TileMode::Clamp,
            None,
            None,
        ) {
            paint.set_shader(Some(shader));
        } else {
            paint.set_color(Color::from_rgb(12, 12, 16));
        }
    } else {
        paint.set_color(Color::from_rgb(10, 10, 10));
    }
    let island_path = g3_rounded_rect_path(rect, corner_radius);
    canvas.draw_path(&island_path, &paint);

    paint.set_shader(None);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(1.0);
    paint.set_color(Color::from_argb(
        if island_style == "glass" || island_style == "mica" {
            52
        } else {
            38
        },
        theme.text_pri.r(),
        theme.text_pri.g(),
        theme.text_pri.b(),
    ));
    canvas.draw_path(&island_path, &paint);
}

fn draw_grid(
    canvas: &Canvas,
    geometry: &WidgetGridGeom,
    dragging: bool,
    drop_cells: &[usize],
    theme: &SettingsTheme,
) {
    let slot_radius = 12.0 * geometry.cap_scale;
    for slot in 0..WIDGET_GRID_SLOTS {
        let (x, y, width, height) = geometry.slot_rect(slot);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(if dragging { 1.0 } else { 0.75 });
        paint.set_color(Color::from_argb(
            if dragging { 52 } else { 24 },
            255,
            255,
            255,
        ));
        canvas.draw_round_rect(
            Rect::from_xywh(x, y, width, height),
            slot_radius,
            slot_radius,
            &paint,
        );
    }

    for slot in drop_cells {
        let (x, y, width, height) = geometry.slot_rect(*slot);
        let rect = Rect::from_xywh(x, y, width, height);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::from_argb(
            100,
            theme.accent.r(),
            theme.accent.g(),
            theme.accent.b(),
        ));
        canvas.draw_round_rect(rect, slot_radius, slot_radius, &paint);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(2.0);
        paint.set_color(theme.accent);
        canvas.draw_round_rect(rect, slot_radius, slot_radius, &paint);
    }
}

fn draw_delete_button(canvas: &Canvas, x: f32, y: f32, scale: f32) {
    let radius = (8.0 * scale).max(7.0);
    let stroke_width = (1.5 * scale).max(1.25);
    let arm = (3.0 * scale).max(2.5);
    draw_delete_button_with_metrics(canvas, x, y, radius, stroke_width, arm);
}

fn draw_compact_delete_button(canvas: &Canvas, x: f32, y: f32, scale: f32) {
    let radius = (3.75 * scale).max(4.0);
    let stroke_width = (0.9 * scale).max(1.0);
    let arm = (1.35 * scale).max(1.5);
    draw_delete_button_with_metrics(canvas, x, y, radius, stroke_width, arm);
}

fn draw_delete_button_with_metrics(
    canvas: &Canvas,
    x: f32,
    y: f32,
    radius: f32,
    stroke_width: f32,
    arm: f32,
) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_rgb(255, 59, 48));
    canvas.draw_circle((x, y), radius, &paint);

    paint.set_color(Color::WHITE);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    canvas.draw_line((x - arm, y - arm), (x + arm, y + arm), &paint);
    canvas.draw_line((x + arm, y - arm), (x - arm, y + arm), &paint);
}

fn draw_library_tile(
    canvas: &Canvas,
    source: &WidgetSource,
    plugin_widgets: &[PluginWidget],
    rect: Rect,
) {
    let preview_rect = Rect::from_xywh(
        rect.left + 7.0,
        rect.top + 6.0,
        rect.width() - 14.0,
        rect.height() - 12.0,
    );
    match source {
        WidgetSource::BuiltIn(kind) => {
            let (preview_width, preview_height) = match kind {
                WidgetKind::Clock => (98.0, 46.0),
                WidgetKind::Calendar => (60.0, 60.0),
                WidgetKind::ResourceUsage => (98.0, 46.0),
                WidgetKind::Settings => (54.0, 54.0),
            };
            draw_mini_card(
                canvas,
                *kind,
                preview_rect.center_x() - preview_width / 2.0,
                preview_rect.center_y() - preview_height / 2.0,
                preview_width,
                preview_height,
            );
        }
        WidgetSource::Plugin(id) => {
            if let Some(widget) = plugin_widgets
                .iter()
                .find(|widget| widget.layout_id().as_ref() == Some(id))
            {
                let span = widget.span();
                let natural_width = span.0 as f32 * 60.0;
                let natural_height = span.1 as f32 * 48.0;
                let scale = (preview_rect.width() / natural_width)
                    .min(preview_rect.height() / natural_height)
                    .min(1.0);
                let width = natural_width * scale;
                let height = natural_height * scale;
                crate::ui::expanded::widget_view::draw_plugin_widget(
                    canvas,
                    widget,
                    preview_rect.center_x() - width / 2.0,
                    preview_rect.center_y() - height / 2.0,
                    width,
                    height,
                    scale,
                    255,
                );
            }
        }
    }
}

pub(super) fn draw_widget_preview(params: WidgetPreviewParams<'_>) {
    match params.widget_editor_mode {
        WidgetEditorMode::Expanded => draw_expanded_widget_preview(params),
        WidgetEditorMode::Compact => draw_compact_widget_preview(params),
    }
}

fn draw_expanded_widget_preview(params: WidgetPreviewParams<'_>) {
    let WidgetPreviewParams {
        canvas,
        item_y,
        width,
        content_width,
        visible_min_y,
        visible_max_y,
        island_style,
        expanded_width,
        expanded_height,
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        widget_dragging,
        widget_drag_hover_slot,
        widget_preview_hover_slot,
        theme,
        ..
    } = params;
    let preview_height = params_item_height(
        plugin_widgets,
        widget_layout,
        plugin_widget_layout,
        widget_dragging,
    );
    let y = item_y + 10.0;
    if y + preview_height < visible_min_y || y > visible_max_y {
        return;
    }

    let panel_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let panel_width = content_width - GROUP_INNER_PAD * 2.0;
    let library_y = y + WIDGET_ISLAND_PANEL_H + WIDGET_PANEL_GAP;
    let library_height = preview_height - WIDGET_ISLAND_PANEL_H - WIDGET_PANEL_GAP;
    draw_panel(
        canvas,
        Rect::from_xywh(panel_x, y, panel_width, WIDGET_ISLAND_PANEL_H),
        theme,
    );
    draw_panel(
        canvas,
        Rect::from_xywh(panel_x, library_y, panel_width, library_height),
        theme,
    );

    draw_label(
        canvas,
        &tr("widget_layout_title"),
        panel_x + 16.0,
        y + 25.0,
        13.0,
        true,
        theme.text_pri,
    );
    draw_label(
        canvas,
        &tr("widget_layout_hint"),
        panel_x + 16.0,
        y + 44.0,
        11.0,
        false,
        theme.text_sec,
    );

    let geometry = widget_grid_geom(item_y, width, expanded_width, expanded_height);
    let island_rect = Rect::from_xywh(
        geometry.cap_x,
        geometry.cap_y,
        geometry.cap_w,
        geometry.cap_h,
    );
    draw_island_background(canvas, island_rect, island_style, theme, 28.0);

    let dragging = widget_dragging.is_some();
    let drop_cells = match (widget_dragging, widget_drag_hover_slot) {
        (Some(source), Some(slot)) => widget_source_span(source, plugin_widgets)
            .map(|span| span_cells(slot, span))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    draw_grid(canvas, &geometry, dragging, &drop_cells, theme);

    for entry in widget_layout {
        let Some(kind) = entry.widget else { continue };
        if widget_dragging == Some(&WidgetSource::BuiltIn(kind)) {
            continue;
        }
        let (x, y, width, height) = geometry.footprint_rect(kind.span(), entry.slot);
        draw_widget_card_preview(
            canvas,
            kind,
            x,
            y,
            width,
            height,
            geometry.cap_scale,
            255,
            Color::WHITE,
        );

        let hovered = widget_preview_hover_slot
            .is_some_and(|slot| widget_footprint(kind, entry.slot).contains(&slot));
        if kind != WidgetKind::Settings && (dragging || hovered) {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_delete_button(canvas, button_x, button_y, geometry.cap_scale);
        }
    }

    for widget in plugin_widgets {
        let Some(id) = widget.layout_id() else {
            continue;
        };
        let Some(entry) = plugin_widget_slot(plugin_widget_layout, &id) else {
            continue;
        };
        if widget_dragging == Some(&WidgetSource::Plugin(id.clone())) {
            continue;
        }
        let (x, y, width, height) = geometry.footprint_rect(widget.span(), entry.slot);
        crate::ui::expanded::widget_view::draw_plugin_widget(
            canvas,
            widget,
            x,
            y,
            width,
            height,
            geometry.cap_scale,
            255,
        );
        let cells = span_cells(entry.slot, widget.span());
        let hovered = widget_preview_hover_slot.is_some_and(|slot| cells.contains(&slot));
        if dragging || hovered {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_delete_button(canvas, button_x, button_y, geometry.cap_scale);
        }
    }

    draw_label(
        canvas,
        &tr("widget_library_title"),
        panel_x + 16.0,
        library_y + 25.0,
        13.0,
        true,
        theme.text_pri,
    );
    draw_label(
        canvas,
        &tr("widget_library_hint"),
        panel_x + 16.0,
        library_y + 43.0,
        11.0,
        false,
        theme.text_sec,
    );

    let source_y = library_y + WIDGET_LIBRARY_HEADER_H;
    let library_items = widget_library_items(
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        widget_dragging,
    );
    if library_items.is_empty() {
        if widget_dragging.is_none() {
            draw_centered_label(
                canvas,
                &tr("widget_library_empty"),
                Rect::from_xywh(
                    panel_x + 12.0,
                    source_y,
                    panel_width - 24.0,
                    library_height - WIDGET_LIBRARY_HEADER_H,
                ),
                12.0,
                theme.text_sec,
            );
        }
    } else {
        for (index, source) in library_items.iter().enumerate() {
            let (x, y, width, height) = widget_source_rect(panel_x, source_y, index);
            let rect = Rect::from_xywh(x, y, width, height);
            draw_library_tile(canvas, source, plugin_widgets, rect);
        }
    }
}

fn draw_compact_grid(
    canvas: &Canvas,
    geometry: &CompactWidgetGridGeom,
    dragging: bool,
    drop_position: Option<CompactWidgetPosition>,
    theme: &SettingsTheme,
) {
    if !dragging {
        return;
    }
    canvas.save();
    let island_path = g3_rounded_rect_path(
        Rect::from_xywh(
            geometry.cap_x,
            geometry.cap_y,
            geometry.cap_w,
            geometry.cap_h,
        ),
        geometry.cap_h / 2.0,
    );
    canvas.clip_path(&island_path, skia_safe::ClipOp::Intersect, true);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    if let Some(position) = drop_position {
        let lane_width = geometry.cap_w / 3.0;
        let lane = match position.alignment {
            CompactWidgetAlignment::Left => 0.0,
            CompactWidgetAlignment::Center => 1.0,
            CompactWidgetAlignment::Right => 2.0,
        };
        paint.set_color(Color::from_argb(
            24,
            theme.accent.r(),
            theme.accent.g(),
            theme.accent.b(),
        ));
        canvas.draw_rect(
            Rect::from_xywh(
                geometry.cap_x + lane_width * lane,
                geometry.cap_y,
                lane_width,
                geometry.cap_h,
            ),
            &paint,
        );
        let indicator_x = geometry.drop_indicator_x(position);
        paint.set_color(theme.accent);
        paint.set_stroke_width((1.5 * geometry.cap_scale).clamp(1.5, 2.5));
        paint.set_stroke_cap(skia_safe::paint::Cap::Round);
        let inset = geometry.cap_h * 0.27;
        canvas.draw_line(
            (indicator_x, geometry.cap_y + inset),
            (indicator_x, geometry.cap_y + geometry.cap_h - inset),
            &paint,
        );
    }
    paint.set_color(Color::from_argb(28, 255, 255, 255));
    paint.set_stroke_width(1.0);
    for boundary in [1.0, 2.0] {
        let x = geometry.cap_x + geometry.cap_w * boundary / 3.0;
        let inset = geometry.cap_h * 0.32;
        canvas.draw_line(
            (x, geometry.cap_y + inset),
            (x, geometry.cap_y + geometry.cap_h - inset),
            &paint,
        );
    }
    canvas.restore();
}

fn draw_compact_library_tile(canvas: &Canvas, widget: CompactWidgetKind, rect: Rect) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(8, 255, 255, 255));
    canvas.draw_round_rect(rect, 12.0, 12.0, &paint);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(0.75);
    paint.set_color(Color::from_argb(24, 255, 255, 255));
    canvas.draw_round_rect(
        Rect::from_xywh(
            rect.left + 0.375,
            rect.top + 0.375,
            rect.width() - 0.75,
            rect.height() - 0.75,
        ),
        12.0,
        12.0,
        &paint,
    );
    let widget_width = crate::ui::widget::compact::widget_width(widget);
    let natural_width = widget_width + 18.0;
    let preview_scale = ((rect.width() - 8.0) / natural_width).min(1.0);
    let preview = Rect::from_xywh(
        rect.center_x() - natural_width * preview_scale / 2.0,
        rect.center_y() - 15.0 * preview_scale,
        natural_width * preview_scale,
        30.0 * preview_scale,
    );
    paint.set_style(skia_safe::paint::Style::Fill);
    paint.set_color(Color::from_rgb(10, 10, 10));
    canvas.draw_round_rect(
        preview,
        preview.height() / 2.0,
        preview.height() / 2.0,
        &paint,
    );
    crate::ui::widget::compact::draw_widget(
        canvas,
        widget,
        Rect::from_xywh(
            preview.left + 9.0 * preview_scale,
            preview.top,
            widget_width * preview_scale,
            preview.height(),
        ),
        preview_scale,
        255,
    );
}

fn draw_compact_widget_preview(params: WidgetPreviewParams<'_>) {
    let WidgetPreviewParams {
        canvas,
        item_y,
        width,
        content_width,
        visible_min_y,
        visible_max_y,
        island_style,
        base_width,
        base_height,
        compact_widget_layout,
        compact_widget_dragging,
        compact_widget_drag_hover_slot,
        compact_widget_preview_hover_slot,
        theme,
        ..
    } = params;
    let preview_height = COMPACT_WIDGET_PREVIEW_H - 20.0;
    let y = item_y + 10.0;
    if y + preview_height < visible_min_y || y > visible_max_y {
        return;
    }

    let panel_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let panel_width = content_width - GROUP_INNER_PAD * 2.0;
    let library_y = y + COMPACT_WIDGET_ISLAND_PANEL_H + WIDGET_PANEL_GAP;
    let library_height = preview_height - COMPACT_WIDGET_ISLAND_PANEL_H - WIDGET_PANEL_GAP;
    draw_panel(
        canvas,
        Rect::from_xywh(panel_x, y, panel_width, COMPACT_WIDGET_ISLAND_PANEL_H),
        theme,
    );
    draw_panel(
        canvas,
        Rect::from_xywh(panel_x, library_y, panel_width, library_height),
        theme,
    );
    draw_label(
        canvas,
        &tr("widget_layout_title"),
        panel_x + 16.0,
        y + 25.0,
        13.0,
        true,
        theme.text_pri,
    );
    draw_label(
        canvas,
        &tr("widget_layout_hint"),
        panel_x + 16.0,
        y + 44.0,
        11.0,
        false,
        theme.text_sec,
    );

    let geometry = compact_widget_grid_geom(
        item_y,
        width,
        base_width,
        base_height,
        compact_widget_layout,
        compact_widget_dragging,
    );
    let island_rect = Rect::from_xywh(
        geometry.cap_x,
        geometry.cap_y,
        geometry.cap_w,
        geometry.cap_h,
    );
    draw_island_background(
        canvas,
        island_rect,
        island_style,
        theme,
        island_rect.height() / 2.0,
    );
    let dragging = compact_widget_dragging.is_some();
    draw_compact_grid(
        canvas,
        &geometry,
        dragging,
        compact_widget_drag_hover_slot,
        theme,
    );

    for entry in compact_widget_layout {
        let Some(widget) = entry.widget else { continue };
        if compact_widget_dragging == Some(widget) {
            continue;
        }
        let Some((x, y, width, height)) = geometry.slot_rect(entry.position()) else {
            continue;
        };
        crate::ui::widget::compact::draw_widget(
            canvas,
            widget,
            Rect::from_xywh(x, y, width, height),
            geometry.cap_scale,
            255,
        );
        if compact_widget_preview_hover_slot == Some(entry.position()) {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_compact_delete_button(canvas, button_x, button_y, geometry.cap_scale);
        }
    }

    draw_label(
        canvas,
        &tr("widget_library_title"),
        panel_x + 16.0,
        library_y + 25.0,
        13.0,
        true,
        theme.text_pri,
    );
    draw_label(
        canvas,
        &tr("widget_library_hint"),
        panel_x + 16.0,
        library_y + 43.0,
        11.0,
        false,
        theme.text_sec,
    );
    let source_y = library_y + WIDGET_LIBRARY_HEADER_H;
    let library_items =
        compact_widget_library_items(compact_widget_layout, compact_widget_dragging);
    if library_items.is_empty() {
        if compact_widget_dragging.is_none() {
            draw_centered_label(
                canvas,
                &tr("widget_library_empty"),
                Rect::from_xywh(
                    panel_x + 12.0,
                    source_y,
                    panel_width - 24.0,
                    library_height - WIDGET_LIBRARY_HEADER_H,
                ),
                12.0,
                theme.text_sec,
            );
        }
    } else {
        for (index, widget) in library_items.into_iter().enumerate() {
            let (x, y, width, height) = widget_source_rect(panel_x, source_y, index);
            draw_compact_library_tile(canvas, widget, Rect::from_xywh(x, y, width, height));
        }
    }
}

fn params_item_height(
    plugin_widgets: &[PluginWidget],
    widget_layout: &[WidgetSlot],
    plugin_widget_layout: &[PluginWidgetSlot],
    dragging: Option<&WidgetSource>,
) -> f32 {
    super::super::input::widget_preview_height(
        widget_library_items(
            widget_layout,
            plugin_widget_layout,
            plugin_widgets,
            dragging,
        )
        .len(),
    ) - 20.0
}

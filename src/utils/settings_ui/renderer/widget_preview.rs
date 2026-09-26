use std::cell::RefCell;
use std::collections::VecDeque;

use winisland_render::{
    GradientStop, Image, ImageOptions, Painter, Path, Point, Radius, RasterSurface, Rect, Rgba,
    Sampling, StrokeCap, StrokeJoin, TileMode, Vec2,
};

use crate::ui::widget::expanded::{
    draw_mini_card, draw_widget_preview as draw_widget_card_preview,
};
use crate::utils::color::SettingsTheme;
use crate::utils::color::settings_color;
use crate::utils::settings_ui::SettingsPainter;
use crate::utils::shape::expanded_island_radius;
use winisland_core::config::{
    CompactWidgetAlignment, CompactWidgetKind, CompactWidgetPosition, CompactWidgetSlot,
    PluginWidgetSlot, WIDGET_GRID_SLOTS, WidgetKind, WidgetSlot, plugin_widget_slot, span_cells,
    widget_footprint,
};
use winisland_core::i18n::tr;
use winisland_core::widgets::PluginWidget;

use super::super::input::{
    COMPACT_WIDGET_ISLAND_PANEL_H, CompactWidgetGridGeom, WIDGET_ISLAND_PANEL_H,
    WIDGET_LIBRARY_HEADER_H, WIDGET_PANEL_GAP, WidgetDropAnimation, WidgetDropTarget,
    WidgetEditorHover, WidgetEditorMode, WidgetEditorSlot, WidgetGridGeom, WidgetSource,
    compact_widget_grid_geom, compact_widget_library_items, compact_widget_preview_height,
    widget_delete_button_center, widget_edit_button_center, widget_grid_geom, widget_library_items,
    widget_source_rect, widget_source_span,
};
use super::super::items::{CONTENT_PADDING, GROUP_INNER_PAD};

#[derive(Clone, Copy)]
pub(super) struct WidgetPreviewParams<'a> {
    pub(super) painter: Painter<'a>,
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
    pub(super) widget_drag_hover_slot: Option<WidgetEditorSlot>,
    pub(super) widget_preview_hover_slot: Option<WidgetEditorSlot>,
    pub(super) compact_widget_layout: &'a [CompactWidgetSlot],
    pub(super) compact_widget_dragging: Option<CompactWidgetKind>,
    pub(super) widget_hover: Option<&'a WidgetEditorHover>,
    pub(super) widget_hover_progress: f32,
    pub(super) widget_drop_animation: Option<&'a WidgetDropAnimation>,
    pub(super) theme: &'a SettingsTheme,
}

fn draw_panel(painter: Painter<'_>, rect: Rect, theme: &SettingsTheme) {
    painter.fill_round_rect(
        Rect::from_xywh(rect.left, rect.top + 2.0, rect.width(), rect.height()),
        Radius::uniform(14.0),
        settings_color(theme.shadow),
    );

    painter.fill_round_rect(rect, Radius::uniform(14.0), settings_color(theme.group_bg));
    painter.stroke_round_rect(
        Rect::from_xywh(
            rect.left + 0.375,
            rect.top + 0.375,
            rect.width() - 0.75,
            rect.height() - 0.75,
        ),
        Radius::uniform(14.0),
        0.75,
        settings_color(theme.group_border),
    );
}

fn draw_label(
    painter: Painter<'_>,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    bold: bool,
    color: Rgba,
) {
    SettingsPainter::new(painter).text(text, (x, y), size, bold, color);
}

fn draw_centered_label(painter: Painter<'_>, text: &str, rect: Rect, size: f32, color: Rgba) {
    SettingsPainter::new(painter).centered_text(
        text,
        (rect.center_x(), rect.center_y() + size * 0.35),
        size,
        false,
        color,
    );
}

fn ease_out_back(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0) - 1.0;
    let overshoot = 1.70158;
    1.0 + (overshoot + 1.0) * progress.powi(3) + overshoot * progress.powi(2)
}

fn begin_card_transform(painter: Painter<'_>, rect: Rect, hover: f32, drop: Option<f32>) {
    let drop_scale = drop.map_or(1.0, |progress| 0.94 + 0.06 * ease_out_back(progress));
    let scale = (1.0 + hover * 0.018) * drop_scale;
    painter.save();
    painter.translate(Vec2::new(rect.center_x(), rect.center_y() - hover * 2.0));
    painter.scale(Vec2::new(scale, scale));
    painter.translate(Vec2::new(-rect.center_x(), -rect.center_y()));
}

fn draw_card_feedback(
    painter: Painter<'_>,
    rect: Rect,
    radius: f32,
    hover: f32,
    drop: Option<f32>,
    theme: &SettingsTheme,
) {
    let drop_glow = drop.map_or(0.0, |progress| 1.0 - progress);
    if hover <= 0.001 && drop_glow <= 0.001 {
        return;
    }
    let intensity = hover.max(drop_glow);
    let accent = settings_color(theme.accent);
    painter.fill_round_rect(
        Rect::from_xywh(rect.left, rect.top + 3.0, rect.width(), rect.height()),
        Radius::uniform(radius),
        Rgba::from_argb((42.0 * intensity) as u8, 0, 0, 0),
    );
    painter.fill_round_rect(
        rect,
        Radius::uniform(radius),
        accent.with_alpha((22.0 * intensity) as u8),
    );
    painter.stroke_round_rect(
        Rect::from_xywh(
            rect.left + 0.5,
            rect.top + 0.5,
            rect.width() - 1.0,
            rect.height() - 1.0,
        ),
        Radius::uniform(radius),
        1.0 + drop_glow,
        accent.with_alpha((110.0 * intensity) as u8),
    );
}

fn draw_library_tile_surface(painter: Painter<'_>, rect: Rect, hover: f32, theme: &SettingsTheme) {
    let accent = settings_color(theme.accent);
    painter.fill_round_rect(
        Rect::from_xywh(rect.left, rect.top + 2.0, rect.width(), rect.height()),
        Radius::uniform(12.0),
        Rgba::from_argb((28.0 * hover) as u8, 0, 0, 0),
    );
    painter.fill_round_rect(
        rect,
        Radius::uniform(12.0),
        settings_color(theme.control_bg),
    );
    if hover > 0.001 {
        painter.fill_round_rect(
            rect,
            Radius::uniform(12.0),
            accent.with_alpha((18.0 * hover) as u8),
        );
    }
    painter.stroke_round_rect(
        Rect::from_xywh(
            rect.left + 0.375,
            rect.top + 0.375,
            rect.width() - 0.75,
            rect.height() - 0.75,
        ),
        Radius::uniform(12.0),
        0.75 + hover * 0.5,
        if hover > 0.001 {
            accent.with_alpha((105.0 * hover) as u8)
        } else {
            settings_color(theme.control_border)
        },
    );
}

#[derive(PartialEq)]
struct PreviewBackgroundKey {
    dimensions: [u32; 4],
    style: String,
    border_color: Rgba,
}

thread_local! {
    static PREVIEW_BACKGROUNDS: RefCell<VecDeque<(PreviewBackgroundKey, Image)>> = const { RefCell::new(VecDeque::new()) };
    static PENCIL_ICON: Option<Image> =
        Image::from_encoded(include_bytes!("../../../../resources/in_app/settings/pencil.png"));
}

fn draw_island_background(
    painter: Painter<'_>,
    rect: Rect,
    island_style: &str,
    theme: &SettingsTheme,
    corner_radius: f32,
) {
    let scale = painter.axis_scale().max(1.0) * 2.0;
    let key = PreviewBackgroundKey {
        dimensions: [
            rect.width().to_bits(),
            rect.height().to_bits(),
            corner_radius.to_bits(),
            scale.to_bits(),
        ],
        style: island_style.to_owned(),
        border_color: settings_color(theme.text_pri),
    };
    let padding = 6.0;
    let bounds = Rect::from_xywh(
        rect.left - padding,
        rect.top - padding,
        rect.width() + padding * 2.0,
        rect.height() + padding * 2.0,
    );
    let image = PREVIEW_BACKGROUNDS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, image)) = cache.iter().find(|(cached_key, _)| *cached_key == key) {
            return Some(image.clone());
        }
        let size = (
            (bounds.width() * scale).ceil() as i32,
            (bounds.height() * scale).ceil() as i32,
        );
        let mut surface = RasterSurface::new(size.0, size.1)?;
        surface.clear(Rgba::TRANSPARENT);
        let raster = surface.painter();
        raster.scale(Vec2::new(
            size.0 as f32 / bounds.width(),
            size.1 as f32 / bounds.height(),
        ));
        draw_island_background_path(
            raster,
            Rect::from_xywh(padding, padding, rect.width(), rect.height()),
            island_style,
            theme,
            corner_radius,
        );
        let image = surface.snapshot();
        cache.push_front((key, image.clone()));
        cache.truncate(4);
        Some(image)
    });
    if let Some(image) = image {
        painter.draw_image(
            &image,
            bounds,
            &ImageOptions::default()
                .with_sampling(Sampling::LinearNone)
                .with_anti_alias(false),
        );
    } else {
        draw_island_background_path(painter, rect, island_style, theme, corner_radius);
    }
}

fn draw_island_background_path(
    painter: Painter<'_>,
    rect: Rect,
    island_style: &str,
    theme: &SettingsTheme,
    corner_radius: f32,
) {
    let shadow_path = Path::continuous_rounded_rect(
        Rect::from_xywh(rect.left, rect.top + 4.0, rect.width(), rect.height()),
        corner_radius,
    );
    painter.fill_path(&shadow_path, Rgba::from_argb(72, 0, 0, 0));

    let island_path = Path::continuous_rounded_rect(rect, corner_radius);
    if island_style == "glass" {
        painter.fill_path(&island_path, Rgba::from_argb(220, 24, 24, 28));
    } else if island_style == "dynamic" {
        let stops = [
            GradientStop {
                offset: 0.0,
                color: Rgba::from_rgb(18, 12, 36),
            },
            GradientStop {
                offset: 1.0,
                color: Rgba::from_rgb(8, 24, 48),
            },
        ];
        let filled = painter.fill_path_with_gradient(
            &island_path,
            Point::new(rect.left, rect.top),
            Point::new(rect.right, rect.bottom),
            &stops,
            TileMode::Clamp,
        );
        if !filled {
            painter.fill_path(&island_path, Rgba::from_rgb(12, 12, 16));
        }
    } else {
        painter.fill_path(&island_path, Rgba::from_rgb(10, 10, 10));
    }

    painter.stroke_path(
        &island_path,
        1.0,
        settings_color(theme.text_pri).with_alpha(if island_style == "glass" { 52 } else { 38 }),
        StrokeCap::Butt,
        StrokeJoin::Miter,
    );
}

fn draw_grid(
    painter: Painter<'_>,
    geometry: &WidgetGridGeom,
    dragging: bool,
    drop_cells: &[usize],
    theme: &SettingsTheme,
) {
    let slot_radius = 12.0 * geometry.cap_scale;
    for slot in 0..WIDGET_GRID_SLOTS {
        let (x, y, width, height) = geometry.slot_rect(slot);
        let path = Path::continuous_rounded_rect(Rect::from_xywh(x, y, width, height), slot_radius);
        painter.stroke_path(
            &path,
            if dragging { 1.0 } else { 0.75 },
            Rgba::from_argb(if dragging { 52 } else { 24 }, 255, 255, 255),
            StrokeCap::Butt,
            StrokeJoin::Miter,
        );
    }

    for slot in drop_cells {
        let (x, y, width, height) = geometry.slot_rect(*slot);
        let rect = Rect::from_xywh(x, y, width, height);
        let path = Path::continuous_rounded_rect(rect, slot_radius);
        painter.fill_path(&path, settings_color(theme.accent).with_alpha(100));
        painter.stroke_path(
            &path,
            2.0,
            settings_color(theme.accent),
            StrokeCap::Butt,
            StrokeJoin::Miter,
        );
    }
}

fn draw_delete_button(painter: Painter<'_>, x: f32, y: f32, scale: f32) {
    let radius = (8.0 * scale).max(7.0);
    let stroke_width = (1.5 * scale).max(1.25);
    let arm = (3.0 * scale).max(2.5);
    draw_delete_button_with_metrics(painter, x, y, radius, stroke_width, arm);
}

fn draw_compact_delete_button(painter: Painter<'_>, x: f32, y: f32, scale: f32) {
    let radius = (3.75 * scale).max(4.0);
    let stroke_width = (0.9 * scale).max(1.0);
    let arm = (1.35 * scale).max(1.5);
    draw_delete_button_with_metrics(painter, x, y, radius, stroke_width, arm);
}

fn draw_edit_button(painter: Painter<'_>, x: f32, y: f32, scale: f32, compact: bool) {
    let radius = if compact {
        (3.75 * scale).max(4.0)
    } else {
        (8.0 * scale).max(7.0)
    };
    painter.fill_circle(Point::new(x, y), radius, Rgba::from_rgb(10, 132, 255));
    let size = radius * 1.18;
    PENCIL_ICON.with(|image| {
        let Some(image) = image else {
            return;
        };
        painter.draw_image(
            image,
            Rect::from_xywh(x - size / 2.0, y - size / 2.0, size, size),
            &ImageOptions::default()
                .with_sampling(Sampling::LinearLinear)
                .with_anti_alias(false),
        );
    });
}

fn draw_delete_button_with_metrics(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    radius: f32,
    stroke_width: f32,
    arm: f32,
) {
    painter.fill_circle(Point::new(x, y), radius, Rgba::from_rgb(255, 59, 48));
    painter.stroke_line(
        Point::new(x - arm, y - arm),
        Point::new(x + arm, y + arm),
        stroke_width,
        Rgba::WHITE,
        StrokeCap::Round,
    );
    painter.stroke_line(
        Point::new(x + arm, y - arm),
        Point::new(x - arm, y + arm),
        stroke_width,
        Rgba::WHITE,
        StrokeCap::Round,
    );
}

fn draw_library_tile(
    painter: Painter<'_>,
    source: &WidgetSource,
    plugin_widgets: &[PluginWidget],
    rect: Rect,
    hover: f32,
    theme: &SettingsTheme,
) {
    begin_card_transform(painter, rect, hover, None);
    draw_library_tile_surface(painter, rect, hover, theme);
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
                painter,
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
                    painter,
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
    painter.restore();
}

pub(super) fn draw_widget_preview(params: WidgetPreviewParams<'_>) {
    match params.widget_editor_mode {
        WidgetEditorMode::Expanded => draw_expanded_widget_preview(params),
        WidgetEditorMode::Compact => draw_compact_widget_preview(params),
    }
}

#[derive(Clone, Copy)]
struct PreviewFrame {
    y: f32,
    panel_x: f32,
    panel_width: f32,
    island_panel_height: f32,
    library_y: f32,
    library_height: f32,
}

impl PreviewFrame {
    fn new(
        params: WidgetPreviewParams<'_>,
        preview_height: f32,
        island_panel_height: f32,
    ) -> Option<Self> {
        let y = params.item_y + 10.0;
        if y + preview_height < params.visible_min_y || y > params.visible_max_y {
            return None;
        }
        let panel_x = CONTENT_PADDING + GROUP_INNER_PAD;
        let panel_width = params.content_width - GROUP_INNER_PAD * 2.0;
        let library_y = y + island_panel_height + WIDGET_PANEL_GAP;
        Some(Self {
            y,
            panel_x,
            panel_width,
            island_panel_height,
            library_y,
            library_height: preview_height - island_panel_height - WIDGET_PANEL_GAP,
        })
    }

    fn draw(self, painter: Painter<'_>, theme: &SettingsTheme) {
        for rect in [
            Rect::from_xywh(
                self.panel_x,
                self.y,
                self.panel_width,
                self.island_panel_height,
            ),
            Rect::from_xywh(
                self.panel_x,
                self.library_y,
                self.panel_width,
                self.library_height,
            ),
        ] {
            draw_panel(painter, rect, theme);
        }
        draw_preview_heading(
            painter,
            self.panel_x,
            self.y,
            ("widget_layout_title", "widget_layout_hint"),
            theme,
        );
        draw_preview_heading(
            painter,
            self.panel_x,
            self.library_y,
            ("widget_library_title", "widget_library_hint"),
            theme,
        );
    }

    fn source_y(self) -> f32 {
        self.library_y + WIDGET_LIBRARY_HEADER_H
    }

    fn draw_empty_library(self, painter: Painter<'_>, dragging: bool, theme: &SettingsTheme) {
        if !dragging {
            draw_centered_label(
                painter,
                &tr("widget_library_empty"),
                Rect::from_xywh(
                    self.panel_x + 12.0,
                    self.source_y(),
                    self.panel_width - 24.0,
                    self.library_height - WIDGET_LIBRARY_HEADER_H,
                ),
                12.0,
                settings_color(theme.text_sec),
            );
        }
    }
}

fn draw_preview_heading(
    painter: Painter<'_>,
    panel_x: f32,
    panel_y: f32,
    keys: (&str, &str),
    theme: &SettingsTheme,
) {
    for (key, offset, size, bold, color) in [
        (keys.0, 25.0, 13.0, true, theme.text_pri),
        (keys.1, 44.0, 11.0, false, theme.text_sec),
    ] {
        draw_label(
            painter,
            &tr(key),
            panel_x + 16.0,
            panel_y + offset,
            size,
            bold,
            settings_color(color),
        );
    }
}

fn draw_expanded_widget_preview(params: WidgetPreviewParams<'_>) {
    let WidgetPreviewParams {
        painter,
        item_y,
        width,
        island_style,
        expanded_width,
        expanded_height,
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        widget_dragging,
        widget_drag_hover_slot,
        widget_preview_hover_slot,
        widget_hover,
        widget_hover_progress,
        widget_drop_animation,
        theme,
        ..
    } = params;
    let widget_drag_hover_slot = widget_drag_hover_slot.and_then(WidgetEditorSlot::expanded);
    let widget_preview_hover_slot = widget_preview_hover_slot.and_then(WidgetEditorSlot::expanded);
    let preview_height = params_item_height(
        plugin_widgets,
        widget_layout,
        plugin_widget_layout,
        widget_dragging,
        width,
    );
    let Some(frame) = PreviewFrame::new(params, preview_height, WIDGET_ISLAND_PANEL_H) else {
        return;
    };
    frame.draw(painter, theme);

    let geometry = widget_grid_geom(item_y, width, expanded_width, expanded_height);
    let island_rect = Rect::from_xywh(
        geometry.cap_x,
        geometry.cap_y,
        geometry.cap_w,
        geometry.cap_h,
    );
    draw_island_background(
        painter,
        island_rect,
        island_style,
        theme,
        expanded_island_radius(geometry.cap_w, geometry.cap_h, geometry.cap_scale),
    );

    let dragging = widget_dragging.is_some();
    let drop_cells = match (widget_dragging, widget_drag_hover_slot) {
        (Some(source), Some(slot)) => widget_source_span(source, plugin_widgets)
            .map(|span| span_cells(slot, span))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    draw_grid(painter, &geometry, dragging, &drop_cells, theme);

    for entry in widget_layout {
        let Some(kind) = entry.widget else { continue };
        if widget_dragging == Some(&WidgetSource::BuiltIn(kind)) {
            continue;
        }
        let (x, y, width, height) = geometry.footprint_rect(kind.span(), entry.slot);
        let footprint = widget_footprint(kind, entry.slot);
        let hover = if widget_hover.is_some_and(|hover| {
            matches!(
                hover,
                WidgetEditorHover::ExpandedWidget(WidgetSource::BuiltIn(candidate))
                    if *candidate == kind
            )
        }) {
            widget_hover_progress
        } else {
            0.0
        };
        let drop = widget_drop_animation.and_then(|animation| {
            matches!(
                &animation.target,
                WidgetDropTarget::Expanded(WidgetSource::BuiltIn(candidate)) if *candidate == kind
            )
            .then_some(animation.progress)
        });
        let rect = Rect::from_xywh(x, y, width, height);
        begin_card_transform(painter, rect, hover, drop);
        draw_card_feedback(painter, rect, 12.0 * geometry.cap_scale, hover, drop, theme);
        draw_widget_card_preview(
            painter,
            kind,
            x,
            y,
            width,
            height,
            geometry.cap_scale,
            255,
            Rgba::WHITE,
        );
        painter.restore();

        let hovered = widget_preview_hover_slot.is_some_and(|slot| footprint.contains(&slot));
        if kind != WidgetKind::Settings && (dragging || hovered) {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_delete_button(painter, button_x, button_y, geometry.cap_scale);
            if kind == WidgetKind::ResourceUsage && hovered && !dragging {
                let (edit_x, edit_y) =
                    widget_edit_button_center(x, y, width, height, geometry.cap_scale);
                draw_edit_button(painter, edit_x, edit_y, geometry.cap_scale, false);
            }
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
        let cells = span_cells(entry.slot, widget.span());
        let hover = if widget_hover.is_some_and(|hover| {
            matches!(
                hover,
                WidgetEditorHover::ExpandedWidget(WidgetSource::Plugin(candidate))
                    if candidate == &id
            )
        }) {
            widget_hover_progress
        } else {
            0.0
        };
        let drop = widget_drop_animation.and_then(|animation| {
            matches!(
                &animation.target,
                WidgetDropTarget::Expanded(WidgetSource::Plugin(candidate)) if candidate == &id
            )
            .then_some(animation.progress)
        });
        let rect = Rect::from_xywh(x, y, width, height);
        begin_card_transform(painter, rect, hover, drop);
        draw_card_feedback(painter, rect, 12.0 * geometry.cap_scale, hover, drop, theme);
        crate::ui::expanded::widget_view::draw_plugin_widget(
            painter,
            widget,
            x,
            y,
            width,
            height,
            geometry.cap_scale,
            255,
        );
        painter.restore();
        let hovered = widget_preview_hover_slot.is_some_and(|slot| cells.contains(&slot));
        if dragging || hovered {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_delete_button(painter, button_x, button_y, geometry.cap_scale);
        }
    }

    let source_y = frame.source_y();
    let library_items = widget_library_items(
        widget_layout,
        plugin_widget_layout,
        plugin_widgets,
        widget_dragging,
    );
    if library_items.is_empty() {
        frame.draw_empty_library(painter, widget_dragging.is_some(), theme);
    } else {
        for (index, source) in library_items.iter().enumerate() {
            let (x, y, width, height) =
                widget_source_rect(frame.panel_x, frame.panel_width, source_y, index);
            let rect = Rect::from_xywh(x, y, width, height);
            let hover = if widget_hover.is_some_and(|hover| {
                matches!(
                    hover,
                    WidgetEditorHover::ExpandedLibrary(candidate) if candidate == source
                )
            }) {
                widget_hover_progress
            } else {
                0.0
            };
            draw_library_tile(painter, source, plugin_widgets, rect, hover, theme);
        }
    }
}

fn draw_compact_grid(
    painter: Painter<'_>,
    geometry: &CompactWidgetGridGeom,
    dragging: bool,
    drop_position: Option<CompactWidgetPosition>,
    theme: &SettingsTheme,
) {
    if !dragging {
        return;
    }
    painter.save();
    let island_path = Path::continuous_rounded_rect(
        Rect::from_xywh(
            geometry.cap_x,
            geometry.cap_y,
            geometry.cap_w,
            geometry.cap_h,
        ),
        geometry.cap_h / 2.0,
    );
    painter.clip_path(&island_path);
    if let Some(position) = drop_position {
        let lane_width = geometry.cap_w / 3.0;
        let lane = match position.alignment {
            CompactWidgetAlignment::Left => 0.0,
            CompactWidgetAlignment::Center => 1.0,
            CompactWidgetAlignment::Right => 2.0,
        };
        painter.fill_rect(
            Rect::from_xywh(
                geometry.cap_x + lane_width * lane,
                geometry.cap_y,
                lane_width,
                geometry.cap_h,
            ),
            settings_color(theme.accent).with_alpha(24),
        );
        let indicator_x = geometry.drop_indicator_x(position);
        let inset = geometry.cap_h * 0.27;
        painter.stroke_line(
            Point::new(indicator_x, geometry.cap_y + inset),
            Point::new(indicator_x, geometry.cap_y + geometry.cap_h - inset),
            (1.5 * geometry.cap_scale).clamp(1.5, 2.5),
            settings_color(theme.accent),
            StrokeCap::Round,
        );
    }
    for boundary in [1.0, 2.0] {
        let x = geometry.cap_x + geometry.cap_w * boundary / 3.0;
        let inset = geometry.cap_h * 0.32;
        painter.stroke_line(
            Point::new(x, geometry.cap_y + inset),
            Point::new(x, geometry.cap_y + geometry.cap_h - inset),
            1.0,
            Rgba::from_argb(28, 255, 255, 255),
            StrokeCap::Round,
        );
    }
    painter.restore();
}

fn draw_compact_library_tile(
    painter: Painter<'_>,
    widget: CompactWidgetKind,
    rect: Rect,
    hover: f32,
    theme: &SettingsTheme,
) {
    begin_card_transform(painter, rect, hover, None);
    draw_library_tile_surface(painter, rect, hover, theme);
    let widget_width = crate::ui::widget::compact::widget_width(widget);
    let natural_width = widget_width + 18.0;
    let preview_scale = ((rect.width() - 8.0) / natural_width).min(1.0);
    let preview = Rect::from_xywh(
        rect.center_x() - natural_width * preview_scale / 2.0,
        rect.center_y() - 15.0 * preview_scale,
        natural_width * preview_scale,
        30.0 * preview_scale,
    );
    painter.fill_round_rect(
        preview,
        Radius::uniform(preview.height() / 2.0),
        Rgba::from_rgb(10, 10, 10),
    );
    crate::ui::widget::compact::draw_widget(
        painter,
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
    painter.restore();
}

fn draw_compact_widget_preview(params: WidgetPreviewParams<'_>) {
    let WidgetPreviewParams {
        painter,
        item_y,
        width,
        island_style,
        base_width,
        base_height,
        compact_widget_layout,
        compact_widget_dragging,
        widget_drag_hover_slot,
        widget_preview_hover_slot,
        widget_hover,
        widget_hover_progress,
        widget_drop_animation,
        theme,
        ..
    } = params;
    let compact_widget_drag_hover_slot = widget_drag_hover_slot.and_then(WidgetEditorSlot::compact);
    let compact_widget_preview_hover_slot =
        widget_preview_hover_slot.and_then(WidgetEditorSlot::compact);
    let library_items =
        compact_widget_library_items(compact_widget_layout, compact_widget_dragging);
    let preview_height = compact_widget_preview_height(library_items.len(), width) - 20.0;
    let Some(frame) = PreviewFrame::new(params, preview_height, COMPACT_WIDGET_ISLAND_PANEL_H)
    else {
        return;
    };
    frame.draw(painter, theme);

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
        painter,
        island_rect,
        island_style,
        theme,
        island_rect.height() / 2.0,
    );
    let dragging = compact_widget_dragging.is_some();
    draw_compact_grid(
        painter,
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
        let hover = if widget_hover.is_some_and(|hover| {
            matches!(hover, WidgetEditorHover::CompactWidget(candidate) if *candidate == widget)
        }) {
            widget_hover_progress
        } else {
            0.0
        };
        let drop = widget_drop_animation.and_then(|animation| {
            matches!(
                &animation.target,
                WidgetDropTarget::Compact(candidate) if *candidate == widget
            )
            .then_some(animation.progress)
        });
        let rect = Rect::from_xywh(x, y, width, height);
        begin_card_transform(painter, rect, hover, drop);
        draw_card_feedback(painter, rect, height / 2.0, hover, drop, theme);
        crate::ui::widget::compact::draw_widget(
            painter,
            widget,
            Rect::from_xywh(x, y, width, height),
            geometry.cap_scale,
            255,
        );
        painter.restore();
        if compact_widget_preview_hover_slot == Some(entry.position()) {
            let (button_x, button_y) =
                widget_delete_button_center(x, y, width, height, geometry.cap_scale);
            draw_compact_delete_button(painter, button_x, button_y, geometry.cap_scale);
            if widget == CompactWidgetKind::ResourceUsage {
                let (edit_x, edit_y) =
                    widget_edit_button_center(x, y, width, height, geometry.cap_scale);
                draw_edit_button(painter, edit_x, edit_y, geometry.cap_scale, true);
            }
        }
    }

    let source_y = frame.source_y();
    if library_items.is_empty() {
        frame.draw_empty_library(painter, compact_widget_dragging.is_some(), theme);
    } else {
        for (index, widget) in library_items.into_iter().enumerate() {
            let (x, y, width, height) =
                widget_source_rect(frame.panel_x, frame.panel_width, source_y, index);
            let hover = if widget_hover.is_some_and(|hover| {
                matches!(hover, WidgetEditorHover::CompactLibrary(candidate) if *candidate == widget)
            }) {
                widget_hover_progress
            } else {
                0.0
            };
            draw_compact_library_tile(
                painter,
                widget,
                Rect::from_xywh(x, y, width, height),
                hover,
                theme,
            );
        }
    }
}

fn params_item_height(
    plugin_widgets: &[PluginWidget],
    widget_layout: &[WidgetSlot],
    plugin_widget_layout: &[PluginWidgetSlot],
    dragging: Option<&WidgetSource>,
    width: f32,
) -> f32 {
    super::super::input::widget_preview_height(
        widget_library_items(
            widget_layout,
            plugin_widget_layout,
            plugin_widgets,
            dragging,
        )
        .len(),
        width,
    ) - 20.0
}

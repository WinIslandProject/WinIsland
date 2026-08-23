mod resource_usage;
mod time;

use crate::core::config::{CompactWidgetAlignment, CompactWidgetKind, CompactWidgetSlot};
use skia_safe::{Canvas, Color, Paint, Rect};

const CONTENT_EDGE_INSET: f32 = 9.0;
const CONTENT_GAP: f32 = 7.0;

pub(crate) fn widget_width(widget: CompactWidgetKind) -> f32 {
    match widget {
        CompactWidgetKind::Time => 48.0,
        CompactWidgetKind::ResourceUsage => 132.0,
    }
}

fn widgets(
    layout: &[CompactWidgetSlot],
    alignment: CompactWidgetAlignment,
) -> impl Iterator<Item = CompactWidgetKind> + '_ {
    layout
        .iter()
        .filter(move |entry| entry.alignment == alignment)
        .filter_map(|entry| entry.widget)
}

fn strip_width(widgets: impl Iterator<Item = CompactWidgetKind>) -> f32 {
    let (count, width) = widgets.fold((0, 0.0), |(count, width), widget| {
        (count + 1, width + widget_width(widget))
    });
    if count == 0 {
        0.0
    } else {
        width + CONTENT_GAP * (count - 1) as f32
    }
}

fn extension_width(widgets: impl Iterator<Item = CompactWidgetKind>) -> f32 {
    let width = strip_width(widgets);
    if width == 0.0 {
        0.0
    } else {
        CONTENT_EDGE_INSET + width + CONTENT_GAP
    }
}

pub(crate) fn has_center_widget(layout: &[CompactWidgetSlot], move_center_aside: bool) -> bool {
    !move_center_aside
        && widgets(layout, CompactWidgetAlignment::Center)
            .next()
            .is_some()
}

pub(crate) fn side_extensions(
    layout: &[CompactWidgetSlot],
    has_center_content: bool,
    move_center_aside: bool,
) -> (f32, f32) {
    if !has_center_content || !move_center_aside {
        return (0.0, 0.0);
    }
    (
        extension_width(widgets(layout, CompactWidgetAlignment::Left)),
        extension_width(
            widgets(layout, CompactWidgetAlignment::Center)
                .chain(widgets(layout, CompactWidgetAlignment::Right)),
        ),
    )
}

fn aligned_layout_width(left_width: f32, center_width: f32, right_width: f32) -> f32 {
    let occupied_groups = [left_width, center_width, right_width]
        .into_iter()
        .filter(|width| *width > 0.0)
        .count();
    if occupied_groups == 0 {
        return 0.0;
    }
    CONTENT_EDGE_INSET * 2.0
        + left_width
        + center_width
        + right_width
        + CONTENT_GAP * occupied_groups.saturating_sub(1) as f32
}

pub(crate) fn alignment_offset(
    layout: &[CompactWidgetSlot],
    total_width: f32,
    alignment: CompactWidgetAlignment,
) -> f32 {
    let left_width = strip_width(widgets(layout, CompactWidgetAlignment::Left));
    let center_width = strip_width(widgets(layout, CompactWidgetAlignment::Center));
    let right_width = strip_width(widgets(layout, CompactWidgetAlignment::Right));
    match alignment {
        CompactWidgetAlignment::Left => CONTENT_EDGE_INSET,
        CompactWidgetAlignment::Right => total_width - CONTENT_EDGE_INSET - right_width,
        CompactWidgetAlignment::Center => {
            let minimum =
                CONTENT_EDGE_INSET + left_width + if left_width > 0.0 { CONTENT_GAP } else { 0.0 };
            let maximum = total_width
                - CONTENT_EDGE_INSET
                - right_width
                - center_width
                - if right_width > 0.0 { CONTENT_GAP } else { 0.0 };
            ((total_width - center_width) / 2.0).clamp(minimum, maximum.max(minimum))
        }
    }
}

pub(crate) fn target_width(
    layout: &[CompactWidgetSlot],
    base_width: f32,
    center_content_width: Option<f32>,
) -> f32 {
    if let Some(center_width) = center_content_width {
        return (center_width
            + extension_width(widgets(layout, CompactWidgetAlignment::Left))
            + extension_width(
                widgets(layout, CompactWidgetAlignment::Center)
                    .chain(widgets(layout, CompactWidgetAlignment::Right)),
            ))
        .max(base_width);
    }
    aligned_layout_width(
        strip_width(widgets(layout, CompactWidgetAlignment::Left)),
        strip_width(widgets(layout, CompactWidgetAlignment::Center)),
        strip_width(widgets(layout, CompactWidgetAlignment::Right)),
    )
    .max(base_width)
}

pub(crate) fn preview_width(
    layout: &[CompactWidgetSlot],
    base_width: f32,
    dragging: Option<CompactWidgetKind>,
) -> f32 {
    let left_width = strip_width(widgets(layout, CompactWidgetAlignment::Left));
    let mut center_width = strip_width(widgets(layout, CompactWidgetAlignment::Center));
    let right_width = strip_width(widgets(layout, CompactWidgetAlignment::Right));
    if let Some(widget) = dragging
        && !layout.iter().any(|entry| entry.widget == Some(widget))
    {
        center_width += widget_width(widget) + if center_width > 0.0 { CONTENT_GAP } else { 0.0 };
    }
    aligned_layout_width(left_width, center_width, right_width).max(base_width)
}

pub(crate) fn draw(
    canvas: &Canvas,
    layout: &[CompactWidgetSlot],
    rect: Rect,
    scale: f32,
    alpha: u8,
    has_mini_content: bool,
) {
    if !layout.iter().any(|entry| entry.widget.is_some()) {
        return;
    }
    if has_mini_content {
        draw_strip(
            canvas,
            widgets(layout, CompactWidgetAlignment::Left),
            rect.left + CONTENT_EDGE_INSET * scale,
            rect,
            scale,
            alpha,
        );
        let right_widgets = widgets(layout, CompactWidgetAlignment::Center)
            .chain(widgets(layout, CompactWidgetAlignment::Right));
        let right_width = strip_width(right_widgets) * scale;
        draw_strip(
            canvas,
            widgets(layout, CompactWidgetAlignment::Center)
                .chain(widgets(layout, CompactWidgetAlignment::Right)),
            rect.right - CONTENT_EDGE_INSET * scale - right_width,
            rect,
            scale,
            alpha,
        );
        draw_content_separators(canvas, layout, rect, scale, alpha);
    } else {
        let logical_width = rect.width() / scale.max(f32::EPSILON);
        draw_strip(
            canvas,
            widgets(layout, CompactWidgetAlignment::Left),
            rect.left
                + alignment_offset(layout, logical_width, CompactWidgetAlignment::Left) * scale,
            rect,
            scale,
            alpha,
        );
        draw_strip(
            canvas,
            widgets(layout, CompactWidgetAlignment::Center),
            rect.left
                + alignment_offset(layout, logical_width, CompactWidgetAlignment::Center) * scale,
            rect,
            scale,
            alpha,
        );
        draw_strip(
            canvas,
            widgets(layout, CompactWidgetAlignment::Right),
            rect.left
                + alignment_offset(layout, logical_width, CompactWidgetAlignment::Right) * scale,
            rect,
            scale,
            alpha,
        );
    }
}

fn draw_strip(
    canvas: &Canvas,
    widgets: impl Iterator<Item = CompactWidgetKind>,
    mut x: f32,
    rect: Rect,
    scale: f32,
    alpha: u8,
) {
    let mut widgets = widgets.peekable();
    while let Some(widget) = widgets.next() {
        let width = widget_width(widget) * scale;
        draw_widget(
            canvas,
            widget,
            Rect::from_xywh(x, rect.top, width, rect.height()),
            scale,
            alpha,
        );
        x += width;
        if widgets.peek().is_some() {
            draw_separator(canvas, x + CONTENT_GAP * scale / 2.0, rect, scale, alpha);
            x += CONTENT_GAP * scale;
        }
    }
}

fn draw_separator(canvas: &Canvas, x: f32, rect: Rect, scale: f32, alpha: u8) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke_width(scale.max(1.0));
    paint.set_color(Color::from_argb((alpha as f32 * 0.14) as u8, 255, 255, 255));
    let top = rect.top + rect.height() * 0.25;
    let bottom = rect.bottom - rect.height() * 0.25;
    canvas.draw_line((x, top), (x, bottom), &paint);
}

fn draw_content_separators(
    canvas: &Canvas,
    layout: &[CompactWidgetSlot],
    rect: Rect,
    scale: f32,
    alpha: u8,
) {
    let left_width = extension_width(widgets(layout, CompactWidgetAlignment::Left));
    let right_width = extension_width(
        widgets(layout, CompactWidgetAlignment::Center)
            .chain(widgets(layout, CompactWidgetAlignment::Right)),
    );
    if left_width > 0.0 {
        draw_separator(
            canvas,
            rect.left + (left_width - CONTENT_GAP / 2.0) * scale,
            rect,
            scale,
            alpha,
        );
    }
    if right_width > 0.0 {
        draw_separator(
            canvas,
            rect.right - (right_width - CONTENT_GAP / 2.0) * scale,
            rect,
            scale,
            alpha,
        );
    }
}

pub(crate) fn draw_widget(
    canvas: &Canvas,
    widget: CompactWidgetKind,
    rect: Rect,
    scale: f32,
    alpha: u8,
) {
    match widget {
        CompactWidgetKind::Time => time::draw(canvas, rect, scale, alpha),
        CompactWidgetKind::ResourceUsage => resource_usage::draw(canvas, rect, scale, alpha),
    }
}

pub(crate) fn next_refresh_delay(layout: &[CompactWidgetSlot]) -> Option<std::time::Duration> {
    let time_delay = layout
        .iter()
        .any(|entry| entry.widget == Some(CompactWidgetKind::Time))
        .then(crate::ui::widget::time_text::until_next_minute);
    let resource_delay = layout
        .iter()
        .any(|entry| entry.widget == Some(CompactWidgetKind::ResourceUsage))
        .then(crate::ui::widget::resource_usage::next_refresh_delay);
    match (time_delay, resource_delay) {
        (Some(time), Some(resource)) => Some(time.min(resource)),
        (Some(delay), None) | (None, Some(delay)) => Some(delay),
        (None, None) => None,
    }
}

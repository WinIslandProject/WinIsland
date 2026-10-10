use crate::ui::widget::compact::resource_usage::draw_metrics as draw_compact_metrics;
use crate::ui::widget::expanded::resource_usage::draw_metric_grid;
use crate::ui::widget::expanded::{draw_widget_rounded_background, widget_grid_layout};
use crate::ui::widget::resource_usage::{
    COMPACT_METRIC_GAP, compact_metric_width, gpu_count, gpu_names, metric_label, metric_visible,
    preview_usage, set_configs,
};
use crate::utils::color::SettingsTheme;
use crate::utils::settings_ui::{ellipsize_text, settings_color};
use winisland_core::config::{
    ResourceMetricConfig, ResourceMetricKind, ResourceMetricStyle, WIDGET_GRID_SLOTS, WidgetKind,
    add_detected_gpu_metrics, default_resource_metrics, place_builtin_widget,
    set_resource_widget_span, span_cells,
};
use winisland_core::i18n::tr;
use winisland_render::text::{DrawTextCachedParams, FontManager};
use winisland_render::{FontStyle, Painter, Path, Point, Radius, Rect, Rgba, StrokeCap, Vec2};

use super::page_order::draw_checkbox;
use super::{PopupState, SettingsApp};
use crate::utils::settings_ui::WidgetEditorMode;

const DIALOG_WIDTH: f32 = 540.0;
const DIALOG_RADIUS: f32 = 14.0;
const DIALOG_PADDING: f32 = 20.0;
const DIALOG_MARGIN: f32 = 14.0;
const GROUP_RADIUS: f32 = 10.0;
const HEADER_BASE: f32 = 56.0;
const HINT_LINE_HEIGHT: f32 = 17.0;
const SECTION_GAP: f32 = 12.0;
const EXPANDED_PREVIEW_HEIGHT: f32 = 156.0;
const COMPACT_PREVIEW_HEIGHT: f32 = 80.0;
const SIZE_COLUMN_WIDTH: f32 = 150.0;
const SIZE_ROW_HEIGHT: f32 = 44.0;
const ROW_HEIGHT: f32 = 44.0;
const FOOTER_HEIGHT: f32 = 64.0;
const HANDLE_WIDTH: f32 = 14.0;
const CHECK_SIZE: f32 = 20.0;
const SWATCH_DIAMETER: f32 = 18.0;
const SEGMENT_WIDTH: f32 = 116.0;
const SEGMENT_HEIGHT: f32 = 24.0;
const POPOVER_SWATCH: f32 = 22.0;
const POPOVER_GAP: f32 = 8.0;
const POPOVER_PADDING: f32 = 10.0;
const POPOVER_COLUMNS: usize = 5;
const ROW_SLIDE_SPEED: f32 = 0.35;
const ROW_ANIM_KEY: u64 = 3_000;
const COLORS: [u32; 10] = [
    0x0a84ff, 0x32bef6, 0x30d158, 0xff9f0a, 0xff453a, 0xff375f, 0xaf52de, 0x64d2ff, 0xffffff,
    0x8e8e93,
];
const SIZE_OPTIONS: [(usize, usize); 8] = [
    (1, 1),
    (2, 1),
    (3, 1),
    (1, 2),
    (2, 2),
    (3, 2),
    (1, 3),
    (2, 3),
];

#[derive(Default)]
pub(crate) struct ResourceEditorState {
    color_popover: Option<usize>,
    drag: Option<RowDrag>,
}

#[derive(Clone)]
struct RowDrag {
    metric: ResourceMetricConfig,
    grab_offset: f32,
    pointer_y: f32,
    original: Vec<ResourceMetricConfig>,
}

#[derive(Clone, Copy)]
enum EditorControl {
    Done,
    Reset,
    SizeDropdown,
    Toggle(usize),
    Style(usize, ResourceMetricStyle),
    Color(usize),
    Row(usize),
    Swatch(u32),
}

struct EditorLayout {
    dialog: Rect,
    preview: Option<Rect>,
    size_row: Option<Rect>,
    size_button: Option<Rect>,
    list: Rect,
    done: Rect,
    reset: Rect,
}

struct RowLayout {
    row: Rect,
    handle: Rect,
    check: Rect,
    swatch: Rect,
    style: Option<Rect>,
    text_left: f32,
    text_right: f32,
}

impl RowLayout {
    fn new(list: Rect, slot: f32, has_style: bool) -> Self {
        let row = Rect::from_xywh(
            list.left,
            list.top + slot * ROW_HEIGHT,
            list.width(),
            ROW_HEIGHT,
        );
        let center_y = row.center_y();
        let handle = Rect::from_xywh(row.left + 12.0, center_y - 10.0, HANDLE_WIDTH, 20.0);
        let check = Rect::from_xywh(
            handle.right + 12.0,
            center_y - CHECK_SIZE / 2.0,
            CHECK_SIZE,
            CHECK_SIZE,
        );
        let swatch = Rect::from_xywh(
            check.right + 12.0,
            center_y - SWATCH_DIAMETER / 2.0,
            SWATCH_DIAMETER,
            SWATCH_DIAMETER,
        );
        let style = has_style.then(|| {
            Rect::from_xywh(
                row.right - 14.0 - SEGMENT_WIDTH,
                center_y - SEGMENT_HEIGHT / 2.0,
                SEGMENT_WIDTH,
                SEGMENT_HEIGHT,
            )
        });
        Self {
            row,
            handle,
            check,
            swatch,
            style,
            text_left: swatch.right + 12.0,
            text_right: style.map_or(row.right - 14.0, |style| style.left - 12.0),
        }
    }

    fn style_segment(&self, style: ResourceMetricStyle) -> Option<Rect> {
        let control = self.style?;
        let half = control.width() / 2.0;
        let left = match style {
            ResourceMetricStyle::Bar => control.left,
            ResourceMetricStyle::Ring => control.left + half,
        };
        Some(Rect::from_xywh(left, control.top, half, control.height()))
    }
}

fn row_key(metric: &ResourceMetricConfig) -> u64 {
    let kind = ResourceMetricKind::ALL
        .iter()
        .position(|kind| *kind == metric.kind)
        .unwrap_or_default() as u64;
    ROW_ANIM_KEY + kind * 256 + u64::from(metric.gpu)
}

fn has_style(metric: &ResourceMetricConfig) -> bool {
    metric.kind != ResourceMetricKind::Network
}

impl SettingsApp {
    fn resource_editor_metrics(&self) -> &[ResourceMetricConfig] {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded | WidgetEditorMode::Pages => &self.config.resource_metrics,
            WidgetEditorMode::Compact => &self.config.compact_resource_metrics,
        }
    }

    fn resource_editor_metrics_mut(&mut self) -> &mut Vec<ResourceMetricConfig> {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded | WidgetEditorMode::Pages => {
                &mut self.config.resource_metrics
            }
            WidgetEditorMode::Compact => &mut self.config.compact_resource_metrics,
        }
    }

    fn resource_editor_expanded(&self) -> bool {
        self.widget_editor_mode != WidgetEditorMode::Compact
    }

    fn resource_editor_rows(&self) -> Vec<usize> {
        let gpu_count = gpu_count();
        self.resource_editor_metrics()
            .iter()
            .enumerate()
            .filter(|(_, metric)| metric_visible(metric, gpu_count))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn open_resource_editor(&mut self) {
        let gpu_count = gpu_names().len();
        let added = add_detected_gpu_metrics(&mut self.config.resource_metrics, gpu_count)
            | add_detected_gpu_metrics(&mut self.config.compact_resource_metrics, gpu_count);
        if added {
            crate::core::persistence::save_config(&self.config);
        }
        self.resource_editor = ResourceEditorState::default();
        self.resource_editor_open = true;
        self.snap_resource_rows();
        self.request_redraw();
    }

    fn close_resource_editor(&mut self) {
        if let Some(drag) = self.resource_editor.drag.take() {
            *self.resource_editor_metrics_mut() = drag.original;
        }
        self.resource_editor = ResourceEditorState::default();
        self.resource_editor_open = false;
        self.request_redraw();
    }

    fn snap_resource_rows(&mut self) {
        let metrics = self.resource_editor_metrics().to_vec();
        for (slot, index) in self.resource_editor_rows().into_iter().enumerate() {
            self.anim.snap(row_key(&metrics[index]), slot as f32);
        }
    }

    fn retarget_resource_rows(&mut self) {
        let metrics = self.resource_editor_metrics().to_vec();
        for (slot, index) in self.resource_editor_rows().into_iter().enumerate() {
            self.anim
                .set_with_speed(row_key(&metrics[index]), slot as f32, ROW_SLIDE_SPEED);
        }
    }

    fn resource_editor_layout(&self) -> EditorLayout {
        let (window_width, window_height) = self.logical_window_size();
        let width = DIALOG_WIDTH.min(window_width - DIALOG_MARGIN * 2.0);
        let rows = self.resource_editor_rows().len() as f32;
        let expanded = self.resource_editor_expanded();
        let preview_height = if expanded {
            EXPANDED_PREVIEW_HEIGHT
        } else {
            COMPACT_PREVIEW_HEIGHT
        };
        let header = self.resource_editor_header_height(width);
        let base = header + rows * ROW_HEIGHT + FOOTER_HEIGHT;
        let available = window_height - DIALOG_MARGIN * 2.0;
        let show_preview = base + preview_height + SECTION_GAP <= available;
        let size_row_height = if expanded && !show_preview {
            SIZE_ROW_HEIGHT + SECTION_GAP
        } else {
            0.0
        };
        let height = if show_preview {
            base + preview_height + SECTION_GAP
        } else {
            base + size_row_height
        }
        .min(available);
        let dialog = Rect::from_xywh(
            (window_width - width) / 2.0,
            (window_height - height) / 2.0,
            width,
            height,
        );
        let inner_left = dialog.left + DIALOG_PADDING;
        let inner_width = dialog.width() - DIALOG_PADDING * 2.0;
        let mut top = dialog.top + header;
        let preview = show_preview.then(|| {
            let rect = Rect::from_xywh(inner_left, top, inner_width, preview_height);
            top += preview_height + SECTION_GAP;
            rect
        });
        let size_row = (size_row_height > 0.0).then(|| {
            let rect = Rect::from_xywh(inner_left, top, inner_width, SIZE_ROW_HEIGHT);
            top += size_row_height;
            rect
        });
        let size_button = match (expanded, preview, size_row) {
            (true, Some(card), _) => Some(Rect::from_xywh(
                card.right - SIZE_COLUMN_WIDTH + 16.0,
                card.center_y() + 2.0,
                SIZE_COLUMN_WIDTH - 32.0,
                26.0,
            )),
            (true, None, Some(row)) => Some(resource_size_button(row)),
            _ => None,
        };
        let list = Rect::from_xywh(inner_left, top, inner_width, rows * ROW_HEIGHT);
        let button_top = dialog.bottom - DIALOG_PADDING - 28.0;
        EditorLayout {
            dialog,
            preview,
            size_row,
            size_button,
            list,
            done: Rect::from_xywh(dialog.right - DIALOG_PADDING - 84.0, button_top, 84.0, 28.0),
            reset: Rect::from_xywh(inner_left, button_top, 108.0, 28.0),
        }
    }

    fn resource_editor_hint_lines(&self, dialog_width: f32) -> Vec<String> {
        FontManager::global().wrap_text(
            &tr("resource_editor_hint"),
            12.0,
            FontStyle::normal(),
            dialog_width - DIALOG_PADDING * 2.0,
        )
    }

    fn resource_editor_header_height(&self, dialog_width: f32) -> f32 {
        HEADER_BASE + self.resource_editor_hint_lines(dialog_width).len() as f32 * HINT_LINE_HEIGHT
    }

    fn resource_row_slot(&self, metric: &ResourceMetricConfig) -> f32 {
        self.anim.try_get(row_key(metric)).unwrap_or_else(|| {
            let metrics = self.resource_editor_metrics();
            self.resource_editor_rows()
                .iter()
                .position(|index| metrics[*index].is_same_metric(metric))
                .unwrap_or_default() as f32
        })
    }

    fn resource_color_popover_rect(&self, layout: &EditorLayout, index: usize) -> Option<Rect> {
        let metric = self.resource_editor_metrics().get(index)?;
        let row = RowLayout::new(
            layout.list,
            self.resource_row_slot(metric),
            has_style(metric),
        );
        let width = POPOVER_COLUMNS as f32 * (POPOVER_SWATCH + POPOVER_GAP) - POPOVER_GAP
            + POPOVER_PADDING * 2.0;
        let rows = COLORS.len().div_ceil(POPOVER_COLUMNS) as f32;
        let height = rows * (POPOVER_SWATCH + POPOVER_GAP) - POPOVER_GAP + POPOVER_PADDING * 2.0;
        let below = row.swatch.bottom + 8.0;
        let top = if below + height <= layout.dialog.bottom - 8.0 {
            below
        } else {
            row.swatch.top - 8.0 - height
        };
        Some(Rect::from_xywh(
            (row.swatch.center_x() - width / 2.0).max(layout.dialog.left + 8.0),
            top,
            width,
            height,
        ))
    }

    fn resource_editor_control(&self, x: f32, y: f32) -> Option<EditorControl> {
        let layout = self.resource_editor_layout();
        let point = Point::new(x, y);
        if let Some(index) = self.resource_editor.color_popover {
            let popover = self.resource_color_popover_rect(&layout, index)?;
            return COLORS
                .iter()
                .enumerate()
                .find(|(position, _)| popover_swatch_rect(popover, *position).contains(point))
                .map(|(_, color)| EditorControl::Swatch(*color));
        }
        if !layout.dialog.contains(point) {
            return None;
        }
        if layout.done.contains(point) {
            return Some(EditorControl::Done);
        }
        if layout.reset.contains(point) {
            return Some(EditorControl::Reset);
        }
        if layout
            .size_button
            .is_some_and(|button| button.contains(point))
        {
            return Some(EditorControl::SizeDropdown);
        }
        let metrics = self.resource_editor_metrics();
        for index in self.resource_editor_rows() {
            let metric = &metrics[index];
            let row = RowLayout::new(
                layout.list,
                self.resource_row_slot(metric),
                has_style(metric),
            );
            if !row.row.contains(point) {
                continue;
            }
            if row.check.inset(-6.0).contains(point) {
                return Some(EditorControl::Toggle(index));
            }
            if row.swatch.inset(-5.0).contains(point) {
                return Some(EditorControl::Color(index));
            }
            for style in [ResourceMetricStyle::Bar, ResourceMetricStyle::Ring] {
                if row
                    .style_segment(style)
                    .is_some_and(|segment| segment.contains(point))
                {
                    return Some(EditorControl::Style(index, style));
                }
            }
            return Some(EditorControl::Row(index));
        }
        None
    }

    pub(crate) fn resource_editor_control_at(&self, x: f32, y: f32) -> bool {
        self.resource_editor.drag.is_some()
            || self
                .resource_editor_control(x, y)
                .is_some_and(|control| !matches!(control, EditorControl::Row(_)))
    }

    pub(crate) fn handle_resource_editor_click(&mut self, x: f32, y: f32) {
        let control = self.resource_editor_control(x, y);
        if self.resource_editor.color_popover.is_some() {
            if let Some(EditorControl::Swatch(color)) = control
                && let Some(index) = self.resource_editor.color_popover
                && let Some(metric) = self.resource_editor_metrics_mut().get_mut(index)
            {
                metric.color = color;
                self.commit_resource_editor();
            }
            self.resource_editor.color_popover = None;
            self.request_redraw();
            return;
        }
        let Some(control) = control else {
            if !self
                .resource_editor_layout()
                .dialog
                .contains(Point::new(x, y))
            {
                self.close_resource_editor();
            }
            return;
        };
        match control {
            EditorControl::Done => {
                self.close_resource_editor();
                return;
            }
            EditorControl::Reset => {
                let mut defaults = default_resource_metrics();
                add_detected_gpu_metrics(&mut defaults, gpu_count());
                *self.resource_editor_metrics_mut() = defaults;
                self.retarget_resource_rows();
            }
            EditorControl::SizeDropdown => {
                self.open_resource_size_popup();
                return;
            }
            EditorControl::Toggle(index) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    metric.enabled = !metric.enabled;
                }
            }
            EditorControl::Style(index, style) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    metric.style = style;
                }
            }
            EditorControl::Color(index) => {
                self.resource_editor.color_popover = Some(index);
                self.request_redraw();
                return;
            }
            EditorControl::Row(index) => {
                let layout = self.resource_editor_layout();
                let metric = self.resource_editor_metrics()[index].clone();
                let row_top = layout.list.top + self.resource_row_slot(&metric) * ROW_HEIGHT;
                self.resource_editor.drag = Some(RowDrag {
                    original: self.resource_editor_metrics().to_vec(),
                    metric,
                    grab_offset: y - row_top,
                    pointer_y: y,
                });
                self.request_redraw();
                return;
            }
            EditorControl::Swatch(_) => return,
        }
        self.commit_resource_editor();
    }

    pub(crate) fn update_resource_editor_drag(&mut self) -> bool {
        let Some(mut drag) = self.resource_editor.drag.clone() else {
            return false;
        };
        let (_, y) = self.logical_mouse_pos;
        drag.pointer_y = y;
        let layout = self.resource_editor_layout();
        let rows = self.resource_editor_rows();
        let slot_count = rows.len();
        let target = (((y - drag.grab_offset - layout.list.top) / ROW_HEIGHT) + 0.5)
            .floor()
            .clamp(0.0, slot_count.saturating_sub(1) as f32) as usize;
        let metrics = self.resource_editor_metrics();
        let current_slot = rows
            .iter()
            .position(|index| metrics[*index].is_same_metric(&drag.metric));
        if let Some(current_slot) = current_slot
            && current_slot != target
        {
            let anchor = metrics[rows[target]].clone();
            let metrics = self.resource_editor_metrics_mut();
            let moved = metrics.remove(rows[current_slot]);
            let anchor_index = metrics
                .iter()
                .position(|metric| metric.is_same_metric(&anchor))
                .unwrap_or(metrics.len());
            let insert = if target > current_slot {
                anchor_index + 1
            } else {
                anchor_index
            };
            metrics.insert(insert.min(metrics.len()), moved);
            self.retarget_resource_rows();
        }
        self.resource_editor.drag = Some(drag);
        true
    }

    pub(crate) fn handle_resource_editor_release(&mut self) -> bool {
        let Some(drag) = self.resource_editor.drag.take() else {
            return false;
        };
        let rows = self.resource_editor_rows();
        let metrics = self.resource_editor_metrics();
        if let Some(slot) = rows
            .iter()
            .position(|index| metrics[*index].is_same_metric(&drag.metric))
        {
            let layout = self.resource_editor_layout();
            let dropped = (drag.pointer_y - drag.grab_offset - layout.list.top) / ROW_HEIGHT;
            self.anim.snap(row_key(&drag.metric), dropped);
            self.anim
                .set_with_speed(row_key(&drag.metric), slot as f32, ROW_SLIDE_SPEED);
        }
        if self.resource_editor_metrics() != drag.original.as_slice() {
            self.commit_resource_editor();
        }
        self.request_redraw();
        true
    }

    pub(crate) fn handle_resource_editor_escape(&mut self) {
        if let Some(drag) = self.resource_editor.drag.take() {
            *self.resource_editor_metrics_mut() = drag.original;
            self.retarget_resource_rows();
        } else if self.resource_editor.color_popover.take().is_none() {
            self.close_resource_editor();
        }
        self.request_redraw();
    }

    fn commit_resource_editor(&mut self) {
        set_configs(
            &self.config.resource_metrics,
            &self.config.compact_resource_metrics,
        );
        crate::core::persistence::save_config(&self.config);
        self.mark_items_dirty();
        self.request_redraw();
    }

    fn open_resource_size_popup(&mut self) {
        let Some(button) = self.resource_editor_layout().size_button else {
            return;
        };
        let selected = SIZE_OPTIONS
            .iter()
            .position(|size| {
                *size
                    == (
                        self.config.resource_widget_columns,
                        self.config.resource_widget_rows,
                    )
            })
            .unwrap_or(0);
        let options: Vec<_> = SIZE_OPTIONS
            .iter()
            .map(|(columns, rows)| format!("{columns} × {rows}"))
            .collect();
        let values: Vec<_> = SIZE_OPTIONS
            .iter()
            .map(|(columns, rows)| format!("{columns}x{rows}"))
            .collect();
        let (window_width, window_height) = self.logical_window_size();
        self.show_popup(PopupState::new(
            select_resource_size,
            button,
            options,
            values,
            selected,
            window_width,
            window_height,
        ));
    }

    fn resize_resource_widget(&mut self, columns: usize, rows: usize) {
        let span = set_resource_widget_span(columns, rows);
        self.config.resource_widget_columns = span.0;
        self.config.resource_widget_rows = span.1;
        let Some(current_anchor) = self.config.widget_layout.iter().find_map(|entry| {
            (entry.widget == Some(WidgetKind::ResourceUsage)).then_some(entry.slot)
        }) else {
            return;
        };
        let settings_slot =
            self.config.widget_layout.iter().find_map(|entry| {
                (entry.widget == Some(WidgetKind::Settings)).then_some(entry.slot)
            });
        let current = span_cells(current_anchor, span)
            .first()
            .copied()
            .unwrap_or(current_anchor);
        let target = std::iter::once(current)
            .chain(0..WIDGET_GRID_SLOTS)
            .find(|candidate| {
                let cells = span_cells(*candidate, span);
                cells.first() == Some(candidate)
                    && !settings_slot.is_some_and(|slot| cells.contains(&slot))
            })
            .unwrap_or(current);
        place_builtin_widget(
            &mut self.config.widget_layout,
            &mut self.config.plugin_widget_layout,
            &self.plugin_widgets,
            WidgetKind::ResourceUsage,
            target,
        );
    }

    pub(crate) fn draw_resource_editor(
        &self,
        painter: Painter<'_>,
        theme: &SettingsTheme,
        win_w: f32,
        win_h: f32,
    ) {
        if !self.resource_editor_open {
            return;
        }
        painter.fill_rect(
            Rect::from_xywh(0.0, 0.0, win_w, win_h),
            Rgba::from_argb(96, 0, 0, 0),
        );
        let layout = self.resource_editor_layout();
        let dialog = layout.dialog;
        draw_sheet(painter, dialog, theme);

        draw_text(
            painter,
            &tr(if self.resource_editor_expanded() {
                "resource_editor_title_expanded"
            } else {
                "resource_editor_title_compact"
            }),
            dialog.left + DIALOG_PADDING,
            dialog.top + 36.0,
            17.0,
            true,
            settings_color(theme.text_pri),
        );
        for (line, hint) in self
            .resource_editor_hint_lines(dialog.width())
            .iter()
            .enumerate()
        {
            draw_text(
                painter,
                hint,
                dialog.left + DIALOG_PADDING,
                dialog.top + 57.0 + line as f32 * HINT_LINE_HEIGHT,
                12.0,
                false,
                settings_color(theme.text_sec),
            );
        }

        if let Some(preview) = layout.preview {
            self.draw_resource_preview(painter, preview, theme);
        }
        if let Some(group) = layout.size_row {
            draw_group(painter, group, theme);
            draw_text(
                painter,
                &tr("resource_size"),
                group.left + 14.0,
                group.center_y() + 4.5,
                13.0,
                false,
                settings_color(theme.text_pri),
            );
        } else if let (Some(card), Some(button)) = (layout.preview, layout.size_button) {
            let divider = card.right - SIZE_COLUMN_WIDTH;
            painter.fill_rect(
                Rect::from_xywh(divider, card.top + 14.0, 1.0, card.height() - 28.0),
                settings_color(theme.separator),
            );
            draw_text(
                painter,
                &tr("resource_size"),
                button.left,
                button.top - 9.0,
                12.0,
                false,
                settings_color(theme.text_sec),
            );
        }
        if let Some(button) = layout.size_button {
            draw_raised_control(painter, button, 7.0, theme);
            draw_text(
                painter,
                &format!(
                    "{} × {}",
                    self.config.resource_widget_columns, self.config.resource_widget_rows
                ),
                button.left + 11.0,
                button.center_y() + 4.0,
                12.0,
                false,
                settings_color(theme.text_pri),
            );
            draw_popup_chevrons(painter, button.right - 14.0, button.center_y(), theme);
        }

        self.draw_resource_rows(painter, &layout, theme);

        let pointer = Point::new(self.logical_mouse_pos.0, self.logical_mouse_pos.1);
        let reset_hovered = layout.reset.contains(pointer) && self.resource_editor.drag.is_none();
        draw_raised_control(painter, layout.reset, 7.0, theme);
        if reset_hovered {
            painter.fill_round_rect(
                layout.reset,
                Radius::uniform(7.0),
                settings_color(theme.text_pri).with_alpha(14),
            );
        }
        draw_centered_text(
            painter,
            &tr("resource_editor_reset"),
            layout.reset,
            12.5,
            false,
            settings_color(theme.text_pri),
        );
        let done_hovered = layout.done.contains(pointer) && self.resource_editor.drag.is_none();
        painter.fill_round_rect(
            layout.done,
            Radius::uniform(7.0),
            settings_color(theme.accent),
        );
        if done_hovered {
            painter.fill_round_rect(
                layout.done,
                Radius::uniform(7.0),
                Rgba::from_argb(28, 255, 255, 255),
            );
        }
        draw_centered_text(
            painter,
            &tr("resource_editor_done"),
            layout.done,
            13.0,
            true,
            Rgba::WHITE,
        );

        if let Some(index) = self.resource_editor.color_popover
            && let Some(popover) = self.resource_color_popover_rect(&layout, index)
        {
            let current = self
                .resource_editor_metrics()
                .get(index)
                .map(|metric| metric.color);
            draw_color_popover(painter, popover, current, pointer, theme);
        }
    }

    fn draw_resource_rows(
        &self,
        painter: Painter<'_>,
        layout: &EditorLayout,
        theme: &SettingsTheme,
    ) {
        let list = layout.list;
        draw_group(painter, list, theme);
        let metrics = self.resource_editor_metrics();
        let rows = self.resource_editor_rows();
        let gpu_count = gpu_count();
        let gpu_names = gpu_names();
        let pointer = Point::new(self.logical_mouse_pos.0, self.logical_mouse_pos.1);
        let dragged = self.resource_editor.drag.as_ref();
        let save_count = painter.save();
        painter.clip_round_rect(list, winisland_render::Radius::uniform(GROUP_RADIUS));
        for (slot, index) in rows.iter().enumerate() {
            let metric = &metrics[*index];
            if dragged.is_some_and(|drag| drag.metric.is_same_metric(metric)) {
                continue;
            }
            let row = RowLayout::new(list, self.resource_row_slot(metric), has_style(metric));
            if slot > 0 {
                painter.fill_rect(
                    Rect::from_xywh(row.text_left, row.row.top, list.right - row.text_left, 1.0),
                    settings_color(theme.separator),
                );
            }
            let hover = self.resource_editor.color_popover.is_none() && dragged.is_none();
            draw_metric_row(
                painter,
                &row,
                metric,
                gpu_count,
                &gpu_names,
                hover.then_some(pointer),
                self.resource_editor.color_popover == Some(*index),
                theme,
            );
        }
        painter.restore_to(save_count);
        if let Some(drag) = dragged
            && let Some(metric) = metrics
                .iter()
                .find(|metric| metric.is_same_metric(&drag.metric))
        {
            let top = (drag.pointer_y - drag.grab_offset)
                .clamp(list.top - 6.0, list.bottom - ROW_HEIGHT + 6.0);
            let row = RowLayout::new(list, (top - list.top) / ROW_HEIGHT, has_style(metric));
            let lifted = row.row.inset(-2.0);
            for (offset, spread, alpha) in [(10.0, 8.0, 40), (3.0, 2.0, 46)] {
                painter.fill_round_rect(
                    Rect::from_xywh(
                        lifted.left - spread,
                        lifted.top + offset - spread,
                        lifted.width() + spread * 2.0,
                        lifted.height() + spread * 2.0,
                    ),
                    Radius::uniform(GROUP_RADIUS + spread),
                    Rgba::from_argb(alpha, 0, 0, 0),
                );
            }
            painter.fill_round_rect(
                lifted,
                Radius::uniform(GROUP_RADIUS),
                settings_color(theme.popup_bg),
            );
            painter.stroke_round_rect(
                lifted,
                Radius::uniform(GROUP_RADIUS),
                1.0,
                settings_color(theme.popup_border),
            );
            draw_metric_row(
                painter, &row, metric, gpu_count, &gpu_names, None, false, theme,
            );
        }
    }

    fn draw_resource_preview(&self, painter: Painter<'_>, card: Rect, theme: &SettingsTheme) {
        draw_group(painter, card, theme);
        let metrics = self.resource_editor_metrics();
        let save_count = painter.save();
        painter.clip_round_rect(card, winisland_render::Radius::uniform(GROUP_RADIUS));
        if self.resource_editor_expanded() {
            let grid = widget_grid_layout(
                0.0,
                0.0,
                self.config.expanded_width,
                self.config.expanded_height,
                1.0,
            );
            let span = (
                self.config.resource_widget_columns.max(1),
                self.config.resource_widget_rows.max(1),
            );
            let (_, _, tile_w, tile_h) = grid.footprint_rect_span(0, span);
            let area = Rect::from_ltrb(
                card.left,
                card.top,
                card.right - SIZE_COLUMN_WIDTH,
                card.bottom,
            );
            let padding = 10.0;
            let fit = ((area.height() - padding * 2.0 - 24.0) / tile_h.max(1.0))
                .min((area.width() - padding * 2.0 - 32.0) / tile_w.max(1.0));
            let tile = Rect::from_xywh(
                area.center_x() - tile_w * fit / 2.0,
                area.center_y() - tile_h * fit / 2.0,
                tile_w * fit,
                tile_h * fit,
            );
            let island = tile.inset(-padding);
            painter.fill_path(
                &Path::continuous_rounded_rect(island, 18.0),
                Rgba::from_rgb(0, 0, 0),
            );
            draw_widget_rounded_background(
                painter,
                tile.left,
                tile.top,
                tile.width(),
                tile.height(),
                fit,
                255,
            );
            draw_metric_grid(painter, tile, fit, 255, Rgba::WHITE, metrics, preview_usage);
        } else {
            let scale = 1.55;
            let gpu_count = gpu_count();
            let content: f32 = metrics
                .iter()
                .filter(|metric| metric.enabled && metric_visible(metric, gpu_count))
                .map(|metric| compact_metric_width(metric, gpu_count) + COMPACT_METRIC_GAP)
                .sum::<f32>()
                .max(44.0)
                * scale;
            let height = 27.0 * scale;
            let width = (content + 30.0 * scale).min(card.width() - 24.0);
            let pill = Rect::from_xywh(
                card.center_x() - width / 2.0,
                card.center_y() - height / 2.0,
                width,
                height,
            );
            painter.fill_path(
                &Path::continuous_rounded_rect(pill, height / 2.0),
                Rgba::from_rgb(0, 0, 0),
            );
            let inner = Rect::from_ltrb(
                pill.left + 14.0 * scale,
                pill.top + 3.0 * scale,
                pill.right - 14.0 * scale,
                pill.bottom - 3.0 * scale,
            );
            draw_compact_metrics(painter, inner, metrics, scale, 255, preview_usage);
        }
        painter.restore_to(save_count);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_metric_row(
    painter: Painter<'_>,
    row: &RowLayout,
    metric: &ResourceMetricConfig,
    gpu_count: usize,
    gpu_names: &[String],
    pointer: Option<Point>,
    popover_open: bool,
    theme: &SettingsTheme,
) {
    let center_y = row.row.center_y();
    let handle_color = settings_color(theme.text_sec).with_alpha(
        if pointer.is_some_and(|pointer| row.row.contains(pointer)) {
            210
        } else {
            140
        },
    );
    for line in 0..3 {
        painter.fill_round_rect(
            Rect::from_xywh(
                row.handle.left,
                center_y - 5.0 + line as f32 * 4.5,
                HANDLE_WIDTH,
                1.6,
            ),
            Radius::uniform(0.8),
            handle_color,
        );
    }
    draw_checkbox(painter, row.check, f32::from(metric.enabled), false, theme);
    let accent = rgb(metric.color);
    let swatch_hovered = pointer.is_some_and(|pointer| row.swatch.inset(-5.0).contains(pointer));
    draw_swatch(
        painter,
        row.swatch,
        accent,
        swatch_hovered || popover_open,
        theme,
    );

    let fonts = FontManager::global();
    let label = metric_label(metric, gpu_count);
    let title_color = settings_color(if metric.enabled {
        theme.text_pri
    } else {
        theme.text_sec
    });
    draw_text(
        painter,
        &label,
        row.text_left,
        center_y - 2.0,
        13.0,
        true,
        title_color,
    );
    let subtitle = match metric.kind {
        ResourceMetricKind::Gpu => gpu_names
            .get(usize::from(metric.gpu))
            .cloned()
            .unwrap_or_else(|| tr("resource_metric_gpu")),
        ResourceMetricKind::Cpu => tr("resource_metric_cpu"),
        ResourceMetricKind::Ram => tr("resource_metric_ram"),
        ResourceMetricKind::Disk => tr("resource_metric_disk"),
        ResourceMetricKind::Network => tr("resource_metric_network"),
    };
    let subtitle = ellipsize_text(
        fonts,
        &subtitle,
        11.0,
        FontStyle::normal(),
        (row.text_right - row.text_left).max(1.0),
    );
    draw_text(
        painter,
        &subtitle,
        row.text_left,
        center_y + 13.0,
        11.0,
        false,
        settings_color(theme.text_sec),
    );
    if let Some(control) = row.style {
        draw_segmented(painter, row, control, metric.style, pointer, theme);
    }
}

fn popover_swatch_rect(popover: Rect, position: usize) -> Rect {
    let column = (position % POPOVER_COLUMNS) as f32;
    let row = (position / POPOVER_COLUMNS) as f32;
    Rect::from_xywh(
        popover.left + POPOVER_PADDING + column * (POPOVER_SWATCH + POPOVER_GAP),
        popover.top + POPOVER_PADDING + row * (POPOVER_SWATCH + POPOVER_GAP),
        POPOVER_SWATCH,
        POPOVER_SWATCH,
    )
}

fn draw_color_popover(
    painter: Painter<'_>,
    popover: Rect,
    current: Option<u32>,
    pointer: Point,
    theme: &SettingsTheme,
) {
    for (offset, spread, alpha) in [(10.0, 8.0, 34), (3.0, 1.0, 40)] {
        painter.fill_round_rect(
            Rect::from_xywh(
                popover.left - spread,
                popover.top + offset - spread,
                popover.width() + spread * 2.0,
                popover.height() + spread * 2.0,
            ),
            Radius::uniform(12.0 + spread),
            Rgba::from_argb(alpha, 0, 0, 0),
        );
    }
    painter.fill_round_rect(
        popover,
        Radius::uniform(12.0),
        settings_color(theme.popup_bg),
    );
    painter.stroke_round_rect(
        popover,
        Radius::uniform(12.0),
        1.0,
        settings_color(theme.popup_border),
    );
    for (position, color) in COLORS.iter().enumerate() {
        let rect = popover_swatch_rect(popover, position);
        let center = Point::new(rect.center_x(), rect.center_y());
        let selected = current == Some(*color);
        let hovered = rect.contains(pointer);
        let radius = rect.width() / 2.0 - if hovered { 0.0 } else { 1.0 };
        painter.fill_circle(center, radius, rgb(*color));
        painter.stroke_circle(
            center,
            radius - 0.5,
            1.0,
            settings_color(theme.text_pri).with_alpha(40),
        );
        if selected {
            painter.stroke_circle(center, radius + 2.5, 2.0, settings_color(theme.accent));
            painter.fill_circle(center, 3.0, Rgba::from_argb(230, 255, 255, 255));
        }
    }
}

fn resource_size_button(group: Rect) -> Rect {
    Rect::from_xywh(
        group.right - 10.0 - 104.0,
        group.center_y() - 12.0,
        104.0,
        24.0,
    )
}

fn select_resource_size(app: &mut SettingsApp, value: &str) {
    let Some((columns, rows)) = value.split_once('x') else {
        return;
    };
    let (Ok(columns), Ok(rows)) = (columns.parse::<usize>(), rows.parse::<usize>()) else {
        return;
    };
    app.resize_resource_widget(columns, rows);
}

fn is_dark(theme: &SettingsTheme) -> bool {
    theme.text_pri.r() > 128
}

fn draw_sheet(painter: Painter<'_>, dialog: Rect, theme: &SettingsTheme) {
    for (offset, spread) in [(14.0, 6.0), (4.0, 1.0)] {
        painter.fill_round_rect(
            Rect::from_xywh(
                dialog.left - spread,
                dialog.top + offset - spread,
                dialog.width() + spread * 2.0,
                dialog.height() + spread * 2.0,
            ),
            Radius::uniform(DIALOG_RADIUS + spread),
            settings_color(theme.shadow),
        );
    }
    painter.fill_round_rect(
        dialog,
        Radius::uniform(DIALOG_RADIUS),
        settings_color(theme.win_bg),
    );
    painter.stroke_round_rect(
        dialog,
        Radius::uniform(DIALOG_RADIUS),
        1.0,
        settings_color(theme.popup_border),
    );
}

fn draw_group(painter: Painter<'_>, rect: Rect, theme: &SettingsTheme) {
    painter.fill_round_rect(
        rect,
        Radius::uniform(GROUP_RADIUS),
        settings_color(theme.group_bg),
    );
    painter.stroke_round_rect(
        rect,
        Radius::uniform(GROUP_RADIUS),
        1.0,
        settings_color(theme.group_border),
    );
}

fn draw_raised_control(painter: Painter<'_>, rect: Rect, radius: f32, theme: &SettingsTheme) {
    painter.fill_round_rect(
        rect.offset(Vec2::new(0.0, 0.5)),
        Radius::uniform(radius),
        settings_color(theme.shadow),
    );
    painter.fill_round_rect(
        rect,
        Radius::uniform(radius),
        if is_dark(theme) {
            settings_color(theme.control_hover)
        } else {
            Rgba::WHITE
        },
    );
    painter.stroke_round_rect(
        rect,
        Radius::uniform(radius),
        0.75,
        settings_color(theme.control_border),
    );
}

fn draw_popup_chevrons(painter: Painter<'_>, x: f32, y: f32, theme: &SettingsTheme) {
    let color = settings_color(theme.text_sec);
    for (offset, up) in [(-3.0, true), (3.0, false)] {
        let direction = if up { -1.0 } else { 1.0 };
        let y = y + offset;
        painter.stroke_line(
            Point::new(x - 3.0, y - 1.5 * direction),
            Point::new(x, y + 1.5 * direction),
            1.4,
            color,
            StrokeCap::Round,
        );
        painter.stroke_line(
            Point::new(x, y + 1.5 * direction),
            Point::new(x + 3.0, y - 1.5 * direction),
            1.4,
            color,
            StrokeCap::Round,
        );
    }
}

fn draw_segmented(
    painter: Painter<'_>,
    row: &RowLayout,
    control: Rect,
    style: ResourceMetricStyle,
    pointer: Option<Point>,
    theme: &SettingsTheme,
) {
    painter.fill_round_rect(
        control,
        Radius::uniform(7.0),
        settings_color(theme.control_bg),
    );
    for (option, key) in [
        (ResourceMetricStyle::Bar, "resource_style_bar"),
        (ResourceMetricStyle::Ring, "resource_style_ring"),
    ] {
        let Some(segment) = row.style_segment(option) else {
            continue;
        };
        let active = option == style;
        if active {
            draw_raised_control(painter, segment.inset(2.0), 5.5, theme);
        } else if pointer.is_some_and(|pointer| segment.contains(pointer)) {
            painter.fill_round_rect(
                segment.inset(2.0),
                Radius::uniform(5.5),
                settings_color(theme.text_pri).with_alpha(16),
            );
        }
        draw_centered_text(
            painter,
            &tr(key),
            segment,
            11.5,
            active,
            settings_color(if active {
                theme.text_pri
            } else {
                theme.text_sec
            }),
        );
    }
}

fn draw_swatch(
    painter: Painter<'_>,
    rect: Rect,
    color: Rgba,
    emphasized: bool,
    theme: &SettingsTheme,
) {
    let center = Point::new(rect.center_x(), rect.center_y());
    let radius = rect.width() / 2.0;
    if emphasized {
        painter.stroke_circle(
            center,
            radius + 3.0,
            1.5,
            settings_color(theme.text_sec).with_alpha(150),
        );
    }
    painter.fill_circle(center, radius, color);
    painter.stroke_circle(
        center,
        radius - 0.5,
        1.0,
        settings_color(theme.text_pri).with_alpha(48),
    );
    painter.stroke_circle(
        center,
        radius - 2.5,
        1.0,
        Rgba::from_argb(70, 255, 255, 255),
    );
}

fn rgb(value: u32) -> Rgba {
    Rgba::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn draw_text(painter: Painter<'_>, text: &str, x: f32, y: f32, size: f32, bold: bool, color: Rgba) {
    FontManager::global().draw_text_cached(DrawTextCachedParams {
        painter,
        text,
        x,
        y,
        size,
        bold,
        color,
        blur: None,
    });
}

fn draw_centered_text(
    painter: Painter<'_>,
    text: &str,
    rect: Rect,
    size: f32,
    bold: bool,
    color: Rgba,
) {
    let width = FontManager::global().measure_text_cached(
        text,
        size,
        if bold {
            FontStyle::bold()
        } else {
            FontStyle::normal()
        },
    );
    draw_text(
        painter,
        text,
        rect.center_x() - width / 2.0,
        rect.center_y() + size * 0.36,
        size,
        bold,
        color,
    );
}

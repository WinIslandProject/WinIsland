use crate::utils::color::SettingsTheme;
use crate::utils::color::rgba_of_paint;
use crate::utils::settings_ui::settings_paint;
use skia_safe::{Canvas, Color, Contains, Paint, Point, Rect};
use winisland_core::config::{
    ResourceMetricKind, ResourceMetricStyle, WIDGET_GRID_SLOTS, WidgetKind, place_builtin_widget,
    set_resource_widget_span, span_cells,
};
use winisland_core::i18n::tr;
use winisland_render::Painter;
use winisland_render::text::{DrawTextCachedParams, FontManager};

use super::{PopupState, SettingsApp};
use crate::utils::settings_ui::WidgetEditorMode;

const DIALOG_WIDTH: f32 = 548.0;
const EXPANDED_DIALOG_HEIGHT: f32 = 526.0;
const COMPACT_DIALOG_HEIGHT: f32 = 496.0;
const EXPANDED_HEADER_HEIGHT: f32 = 122.0;
const COMPACT_HEADER_HEIGHT: f32 = 92.0;
const ROW_HEIGHT: f32 = 72.0;
const ROW_GAP: f32 = 6.0;
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

#[derive(Clone, Copy)]
enum EditorControl {
    Close,
    Toggle(usize),
    Bar(usize),
    Ring(usize),
    Color(usize),
    MoveUp(usize),
    MoveDown(usize),
    SizeDropdown,
}

impl SettingsApp {
    fn resource_editor_metrics(&self) -> &[winisland_core::config::ResourceMetricConfig] {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => &self.config.resource_metrics,
            WidgetEditorMode::Compact => &self.config.compact_resource_metrics,
        }
    }

    fn resource_editor_metrics_mut(
        &mut self,
    ) -> &mut Vec<winisland_core::config::ResourceMetricConfig> {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => &mut self.config.resource_metrics,
            WidgetEditorMode::Compact => &mut self.config.compact_resource_metrics,
        }
    }

    fn resource_editor_header_height(&self) -> f32 {
        match self.widget_editor_mode {
            WidgetEditorMode::Expanded => EXPANDED_HEADER_HEIGHT,
            WidgetEditorMode::Compact => COMPACT_HEADER_HEIGHT,
        }
    }

    fn resource_editor_rect(&self) -> Rect {
        let (window_width, window_height) = self.logical_window_size();
        let width = DIALOG_WIDTH.min(window_width - 28.0);
        let desired_height = match self.widget_editor_mode {
            WidgetEditorMode::Expanded => EXPANDED_DIALOG_HEIGHT,
            WidgetEditorMode::Compact => COMPACT_DIALOG_HEIGHT,
        };
        let height = desired_height.min(window_height - 28.0);
        Rect::from_xywh(
            (window_width - width) / 2.0,
            (window_height - height) / 2.0,
            width,
            height,
        )
    }

    fn resource_editor_control(&self, x: f32, y: f32) -> Option<EditorControl> {
        let dialog = self.resource_editor_rect();
        let point = Point::new(x, y);
        if !dialog.contains(point) {
            return None;
        }
        let close = Rect::from_xywh(dialog.right - 50.0, dialog.top + 24.0, 30.0, 30.0);
        if close.contains(point) {
            return Some(EditorControl::Close);
        }
        if self.widget_editor_mode == WidgetEditorMode::Expanded
            && resource_size_button(dialog).contains(point)
        {
            return Some(EditorControl::SizeDropdown);
        }
        let rows_top = dialog.top + self.resource_editor_header_height();
        for index in 0..self.resource_editor_metrics().len() {
            let row = Rect::from_xywh(
                dialog.left + 20.0,
                rows_top + index as f32 * (ROW_HEIGHT + ROW_GAP),
                dialog.width() - 40.0,
                ROW_HEIGHT,
            );
            if !row.contains(point) {
                continue;
            }
            let center_y = row.center_y();
            let up = Rect::from_xywh(row.left + 10.0, center_y - 13.0, 24.0, 26.0);
            let down = Rect::from_xywh(row.left + 36.0, center_y - 13.0, 24.0, 26.0);
            let toggle = Rect::from_xywh(row.left + 72.0, center_y - 11.0, 38.0, 22.0);
            let style = Rect::from_xywh(row.right - 190.0, center_y - 15.0, 120.0, 30.0);
            let color = Rect::from_xywh(row.right - 48.0, center_y - 16.0, 32.0, 32.0);
            return if up.contains(point) {
                Some(EditorControl::MoveUp(index))
            } else if down.contains(point) {
                Some(EditorControl::MoveDown(index))
            } else if toggle.contains(point) {
                Some(EditorControl::Toggle(index))
            } else if Rect::from_xywh(style.left, style.top, style.width() / 2.0, style.height())
                .contains(point)
            {
                Some(EditorControl::Bar(index))
            } else if style.contains(point) {
                Some(EditorControl::Ring(index))
            } else if color.contains(point) {
                Some(EditorControl::Color(index))
            } else {
                None
            };
        }
        None
    }

    pub(crate) fn resource_editor_control_at(&self, x: f32, y: f32) -> bool {
        self.resource_editor_control(x, y).is_some()
    }

    pub(crate) fn handle_resource_editor_click(&mut self, x: f32, y: f32) {
        let dialog = self.resource_editor_rect();
        let Some(control) = self.resource_editor_control(x, y) else {
            if !dialog.contains(Point::new(x, y)) {
                self.resource_editor_open = false;
                self.request_redraw();
            }
            return;
        };
        match control {
            EditorControl::Close => self.resource_editor_open = false,
            EditorControl::Toggle(index) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    metric.enabled = !metric.enabled;
                }
            }
            EditorControl::Bar(index) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    metric.style = ResourceMetricStyle::Bar;
                }
            }
            EditorControl::Ring(index) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    metric.style = ResourceMetricStyle::Ring;
                }
            }
            EditorControl::Color(index) => {
                if let Some(metric) = self.resource_editor_metrics_mut().get_mut(index) {
                    let current = COLORS.iter().position(|color| *color == metric.color);
                    metric.color =
                        COLORS[current.map_or(0, |position| position + 1) % COLORS.len()];
                }
            }
            EditorControl::MoveUp(index) if index > 0 => {
                self.resource_editor_metrics_mut().swap(index, index - 1);
            }
            EditorControl::MoveDown(index) if index + 1 < self.resource_editor_metrics().len() => {
                self.resource_editor_metrics_mut().swap(index, index + 1);
            }
            EditorControl::MoveUp(_) | EditorControl::MoveDown(_) => {}
            EditorControl::SizeDropdown => self.open_resource_size_popup(),
        }
        crate::ui::widget::resource_usage::set_configs(
            &self.config.resource_metrics,
            &self.config.compact_resource_metrics,
        );
        crate::core::persistence::save_config(&self.config);
        self.mark_items_dirty();
        self.request_redraw();
    }

    fn open_resource_size_popup(&mut self) {
        let dialog = self.resource_editor_rect();
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
            resource_size_button(dialog),
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
        canvas: &Canvas,
        theme: &SettingsTheme,
        win_w: f32,
        win_h: f32,
    ) {
        if !self.resource_editor_open {
            return;
        }
        canvas.draw_rect(
            Rect::from_xywh(0.0, 0.0, win_w, win_h),
            &settings_paint(Color::from_argb(138, 0, 0, 0)),
        );
        let dialog = self.resource_editor_rect();
        canvas.draw_round_rect(
            Rect::from_xywh(
                dialog.left,
                dialog.top + 8.0,
                dialog.width(),
                dialog.height(),
            ),
            20.0,
            20.0,
            &settings_paint(theme.shadow),
        );
        canvas.draw_round_rect(dialog, 20.0, 20.0, &settings_paint(theme.win_bg));
        let mut border = settings_paint(theme.popup_border);
        border.set_style(skia_safe::paint::Style::Stroke);
        border.set_stroke_width(1.0);
        canvas.draw_round_rect(dialog, 20.0, 20.0, &border);

        draw_text(
            canvas,
            &tr(match self.widget_editor_mode {
                WidgetEditorMode::Expanded => "resource_editor_title_expanded",
                WidgetEditorMode::Compact => "resource_editor_title_compact",
            }),
            dialog.left + 20.0,
            dialog.top + 41.0,
            21.0,
            true,
            theme.text_pri,
        );
        draw_text(
            canvas,
            &tr("resource_editor_hint"),
            dialog.left + 20.0,
            dialog.top + 65.0,
            12.0,
            false,
            theme.text_sec,
        );
        if self.widget_editor_mode == WidgetEditorMode::Expanded {
            draw_text(
                canvas,
                &tr("resource_size"),
                dialog.left + 20.0,
                dialog.top + 98.0,
                12.0,
                true,
                theme.text_sec,
            );
            let button = resource_size_button(dialog);
            canvas.draw_round_rect(button, 8.0, 8.0, &settings_paint(theme.control_bg));
            let mut button_border = settings_paint(theme.control_border);
            button_border.set_style(skia_safe::paint::Style::Stroke);
            button_border.set_stroke_width(0.75);
            canvas.draw_round_rect(button, 8.0, 8.0, &button_border);
            draw_text(
                canvas,
                &format!(
                    "{} × {}",
                    self.config.resource_widget_columns, self.config.resource_widget_rows
                ),
                button.left + 12.0,
                button.center_y() + 4.0,
                12.0,
                false,
                theme.text_pri,
            );
            draw_dropdown_arrow(canvas, button.right - 15.0, button.center_y(), theme);
        }
        let close = Rect::from_xywh(dialog.right - 50.0, dialog.top + 24.0, 30.0, 30.0);
        canvas.draw_circle(
            (close.center_x(), close.center_y()),
            15.0,
            &settings_paint(theme.control_bg),
        );
        let mut close_paint = settings_paint(theme.text_sec);
        close_paint.set_style(skia_safe::paint::Style::Stroke);
        close_paint.set_stroke_width(1.8);
        close_paint.set_stroke_cap(skia_safe::paint::Cap::Round);
        canvas.draw_line(
            (close.center_x() - 4.0, close.center_y() - 4.0),
            (close.center_x() + 4.0, close.center_y() + 4.0),
            &close_paint,
        );
        canvas.draw_line(
            (close.center_x() + 4.0, close.center_y() - 4.0),
            (close.center_x() - 4.0, close.center_y() + 4.0),
            &close_paint,
        );

        let rows_top = dialog.top + self.resource_editor_header_height();
        let metrics = self.resource_editor_metrics();
        for (index, metric) in metrics.iter().enumerate() {
            let row = Rect::from_xywh(
                dialog.left + 20.0,
                rows_top + index as f32 * (ROW_HEIGHT + ROW_GAP),
                dialog.width() - 40.0,
                ROW_HEIGHT,
            );
            canvas.draw_round_rect(row, 13.0, 13.0, &settings_paint(theme.group_bg));
            let center_y = row.center_y();
            draw_order_button(
                canvas,
                row.left + 10.0,
                center_y - 13.0,
                true,
                index > 0,
                theme,
            );
            draw_order_button(
                canvas,
                row.left + 36.0,
                center_y - 13.0,
                false,
                index + 1 < metrics.len(),
                theme,
            );
            draw_switch(
                canvas,
                row.left + 72.0,
                center_y - 11.0,
                metric.enabled,
                theme,
            );
            draw_text(
                canvas,
                metric_name(metric.kind),
                row.left + 122.0,
                center_y + 5.0,
                14.0,
                true,
                if metric.enabled {
                    theme.text_pri
                } else {
                    theme.disabled
                },
            );

            let style = Rect::from_xywh(row.right - 190.0, center_y - 15.0, 120.0, 30.0);
            canvas.draw_round_rect(style, 8.0, 8.0, &settings_paint(theme.control_bg));
            let selected = match metric.style {
                ResourceMetricStyle::Bar => {
                    Rect::from_xywh(style.left + 2.0, style.top + 2.0, 58.0, 26.0)
                }
                ResourceMetricStyle::Ring => {
                    Rect::from_xywh(style.left + 60.0, style.top + 2.0, 58.0, 26.0)
                }
            };
            canvas.draw_round_rect(selected, 6.0, 6.0, &settings_paint(theme.accent));
            draw_centered_text(
                canvas,
                &tr("resource_style_bar"),
                Rect::from_xywh(style.left, style.top, 60.0, 30.0),
                11.0,
                metric.style == ResourceMetricStyle::Bar,
                theme,
            );
            draw_centered_text(
                canvas,
                &tr("resource_style_ring"),
                Rect::from_xywh(style.left + 60.0, style.top, 60.0, 30.0),
                11.0,
                metric.style == ResourceMetricStyle::Ring,
                theme,
            );

            let color = rgb(metric.color);
            canvas.draw_circle((row.right - 32.0, center_y), 14.0, &settings_paint(color));
            let mut color_border = settings_paint(Color::from_argb(
                90,
                theme.text_pri.r(),
                theme.text_pri.g(),
                theme.text_pri.b(),
            ));
            color_border.set_style(skia_safe::paint::Style::Stroke);
            color_border.set_stroke_width(1.0);
            canvas.draw_circle((row.right - 32.0, center_y), 14.0, &color_border);
        }
    }
}

fn metric_name(kind: ResourceMetricKind) -> &'static str {
    match kind {
        ResourceMetricKind::Cpu => "CPU",
        ResourceMetricKind::Ram => "RAM",
        ResourceMetricKind::Gpu => "GPU",
        ResourceMetricKind::Network => "NET",
        ResourceMetricKind::Disk => "DISK",
    }
}

fn resource_size_button(dialog: Rect) -> Rect {
    Rect::from_xywh(dialog.right - 170.0, dialog.top + 77.0, 150.0, 32.0)
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

fn draw_dropdown_arrow(canvas: &Canvas, x: f32, y: f32, theme: &SettingsTheme) {
    let mut paint = settings_paint(theme.text_sec);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(1.5);
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    paint.set_stroke_join(skia_safe::paint::Join::Round);
    canvas.draw_line((x - 4.0, y - 2.0), (x, y + 2.0), &paint);
    canvas.draw_line((x, y + 2.0), (x + 4.0, y - 2.0), &paint);
}

fn rgb(value: u32) -> Color {
    Color::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn draw_text(canvas: &Canvas, text: &str, x: f32, y: f32, size: f32, bold: bool, color: Color) {
    let paint = settings_paint(color);
    FontManager::global().draw_text_cached(DrawTextCachedParams {
        painter: Painter::from_canvas(canvas),
        text,
        x,
        y,
        size,
        bold,
        color: rgba_of_paint(&paint),
        blur: None,
    });
}

fn draw_centered_text(
    canvas: &Canvas,
    text: &str,
    rect: Rect,
    size: f32,
    selected: bool,
    theme: &SettingsTheme,
) {
    let fonts = FontManager::global();
    let width = fonts.measure_text_cached(
        text,
        size,
        if selected {
            winisland_render::FontStyle::bold()
        } else {
            winisland_render::FontStyle::normal()
        },
    );
    draw_text(
        canvas,
        text,
        rect.center_x() - width / 2.0,
        rect.center_y() + 4.0,
        size,
        selected,
        if selected {
            theme.selection_text
        } else {
            theme.text_sec
        },
    );
}

fn draw_switch(canvas: &Canvas, x: f32, y: f32, enabled: bool, theme: &SettingsTheme) {
    let rect = Rect::from_xywh(x, y, 38.0, 22.0);
    canvas.draw_round_rect(
        rect,
        11.0,
        11.0,
        &settings_paint(if enabled {
            theme.toggle_on
        } else {
            theme.toggle_off
        }),
    );
    canvas.draw_circle(
        (if enabled { x + 27.0 } else { x + 11.0 }, y + 11.0),
        8.0,
        &settings_paint(Color::WHITE),
    );
}

fn draw_order_button(
    canvas: &Canvas,
    x: f32,
    y: f32,
    up: bool,
    enabled: bool,
    theme: &SettingsTheme,
) {
    let rect = Rect::from_xywh(x, y, 24.0, 26.0);
    canvas.draw_round_rect(
        rect,
        6.0,
        6.0,
        &settings_paint(if enabled {
            theme.control_bg
        } else {
            theme.control_disabled
        }),
    );
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(if enabled {
        theme.text_pri
    } else {
        theme.disabled
    });
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(1.6);
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    paint.set_stroke_join(skia_safe::paint::Join::Round);
    let cy = y + 13.0;
    let direction = if up { -1.0 } else { 1.0 };
    canvas.draw_line(
        (x + 8.0, cy - 3.0 * direction),
        (x + 12.0, cy + 2.0 * direction),
        &paint,
    );
    canvas.draw_line(
        (x + 12.0, cy + 2.0 * direction),
        (x + 16.0, cy - 3.0 * direction),
        &paint,
    );
}

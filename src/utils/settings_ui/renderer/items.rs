use skia_safe::{Canvas, Color, Contains, FontStyle, Paint, Point, Rect};

use crate::utils::color::SettingsTheme;
use crate::utils::font::{DrawTextCachedParams, DrawTextInRectParams, FontManager};

use super::super::items::*;
use super::controls::*;
use super::widget_preview::{WidgetPreviewParams, draw_widget_preview};
use super::{ActiveStepperValue, DrawItemsParams};

struct ItemCtx<'a> {
    canvas: &'a Canvas,
    theme: &'a SettingsTheme,
    content_w: f32,
    width: f32,
    visible_min_y: f32,
    visible_max_y: f32,
    hover_pos: Option<(f32, f32)>,
}

impl ItemCtx<'_> {
    fn row_visible(&self, y: f32, height: f32) -> bool {
        y + height >= self.visible_min_y && y <= self.visible_max_y
    }

    fn hovered(&self, rect: Rect) -> bool {
        self.hover_pos
            .is_some_and(|(x, y)| rect.contains(Point::new(x, y)))
    }
}

#[derive(Default)]
struct GroupRows {
    in_group: bool,
    row_count: usize,
    current_row: usize,
}

fn row_text_color(ctx: &ItemCtx, enabled: bool) -> Color {
    if enabled {
        ctx.theme.text_pri
    } else {
        ctx.theme.text_sec
    }
}

fn draw_row_text(ctx: &ItemCtx, y: f32, height: f32, label: &str, color: Color) -> (f32, bool) {
    let cy = y + height / 2.0;
    let visible = ctx.row_visible(y, height);
    if visible {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(color);
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            canvas: ctx.canvas,
            text: label,
            x: CONTENT_PADDING + GROUP_INNER_PAD,
            y: cy + 5.0,
            size: 13.0,
            bold: false,
            paint: &paint,
        });
    }
    (cy, visible)
}

fn draw_row_stepper(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    value: &str,
    enabled: bool,
    active_stepper_value: &Option<ActiveStepperValue>,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let (cy, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, row_text_color(ctx, enabled));

    let btn_inc_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - STEPPER_BTN_SIZE;
    let value_x = btn_inc_x - STEPPER_GAP - STEPPER_VALUE_W;
    let btn_dec_x = value_x - STEPPER_GAP - STEPPER_BTN_SIZE;
    let btn_y = cy - STEPPER_BTN_SIZE / 2.0;
    if visible {
        let control_x = btn_dec_x;
        let control_w = STEPPER_BTN_SIZE * 2.0 + STEPPER_VALUE_W;
        let mut control = Paint::default();
        control.set_anti_alias(true);
        control.set_color(if enabled {
            theme.control_bg
        } else {
            theme.control_disabled
        });
        canvas.draw_round_rect(
            Rect::from_xywh(control_x, btn_y, control_w, STEPPER_BTN_SIZE),
            POPUP_BTN_R,
            POPUP_BTN_R,
            &control,
        );
        control.set_style(skia_safe::paint::Style::Stroke);
        control.set_stroke_width(0.75);
        control.set_color(theme.control_border);
        canvas.draw_round_rect(
            Rect::from_xywh(
                control_x + 0.375,
                btn_y + 0.375,
                control_w - 0.75,
                STEPPER_BTN_SIZE - 0.75,
            ),
            POPUP_BTN_R,
            POPUP_BTN_R,
            &control,
        );
        control.set_color(theme.separator);
        canvas.draw_line(
            (value_x, btn_y + 4.0),
            (value_x, btn_y + STEPPER_BTN_SIZE - 4.0),
            &control,
        );
        canvas.draw_line(
            (btn_inc_x, btn_y + 4.0),
            (btn_inc_x, btn_y + STEPPER_BTN_SIZE - 4.0),
            &control,
        );
        draw_stepper_btn(
            canvas,
            btn_dec_x,
            btn_y,
            "−",
            enabled,
            theme,
            ctx.hovered(Rect::from_xywh(
                btn_dec_x,
                btn_y,
                STEPPER_BTN_SIZE,
                STEPPER_BTN_SIZE,
            )),
        );
        draw_stepper_btn(
            canvas,
            btn_inc_x,
            btn_y,
            "+",
            enabled,
            theme,
            ctx.hovered(Rect::from_xywh(
                btn_inc_x,
                btn_y,
                STEPPER_BTN_SIZE,
                STEPPER_BTN_SIZE,
            )),
        );
    }

    let val_center = value_x + STEPPER_VALUE_W / 2.0;
    if visible {
        let fm = FontManager::global();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let is_editing = active_stepper_value.as_ref().is_some_and(|input| {
            (input.rect.left - value_x).abs() < 0.5 && (input.rect.top - btn_y).abs() < 0.5
        });
        let display_value = active_stepper_value
            .as_ref()
            .filter(|_| is_editing)
            .map(|input| input.text)
            .unwrap_or(value);
        let show_caret = active_stepper_value
            .as_ref()
            .is_some_and(|input| is_editing && input.show_caret);
        if is_editing {
            let mut input_paint = Paint::default();
            input_paint.set_anti_alias(true);
            input_paint.set_color(theme.card_highlight);
            canvas.draw_round_rect(
                Rect::from_xywh(value_x, btn_y, STEPPER_VALUE_W, STEPPER_BTN_SIZE),
                5.0,
                5.0,
                &input_paint,
            );
            input_paint.set_style(skia_safe::paint::Style::Stroke);
            input_paint.set_stroke_width(1.0);
            input_paint.set_color(theme.accent);
            canvas.draw_round_rect(
                Rect::from_xywh(
                    value_x + 0.5,
                    btn_y + 0.5,
                    STEPPER_VALUE_W - 1.0,
                    STEPPER_BTN_SIZE - 1.0,
                ),
                4.5,
                4.5,
                &input_paint,
            );
        }
        paint.set_color(if enabled {
            theme.text_pri
        } else {
            theme.text_sec
        });
        let val_w = fm.measure_text_cached(display_value, 13.0, FontStyle::normal());
        fm.draw_text_cached(DrawTextCachedParams {
            canvas,
            text: display_value,
            x: val_center - val_w / 2.0,
            y: cy + 5.0,
            size: 13.0,
            bold: false,
            paint: &paint,
        });
        if show_caret {
            let mut caret_paint = Paint::default();
            caret_paint.set_anti_alias(true);
            caret_paint.set_stroke_width(1.0);
            caret_paint.set_color(theme.accent);
            let caret_x = val_center + val_w / 2.0 + 1.5;
            canvas.draw_line(
                (caret_x, btn_y + 5.0),
                (caret_x, btn_y + STEPPER_BTN_SIZE - 5.0),
                &caret_paint,
            );
        }
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_row_switch(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    enabled: bool,
    switch_pos: f32,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let (cy, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, row_text_color(ctx, enabled));

    let toggle_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - TOGGLE_W;
    let toggle_y = cy - TOGGLE_H / 2.0;
    if visible {
        draw_switch(canvas, toggle_x, toggle_y, switch_pos, enabled, theme);
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_row_font_picker(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    btn_label: &str,
    reset_label: &Option<String>,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let (cy, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, theme.text_pri);
    if visible {
        let sel_w: f32 = 72.0;
        let sel_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - sel_w;
        let btn_y = cy - POPUP_BTN_H / 2.0;
        draw_pill_btn(PillBtnParams {
            canvas,
            x: sel_x,
            y: btn_y,
            w: sel_w,
            h: POPUP_BTN_H,
            label: btn_label,
            text_color: theme.text_pri,
            bg_color: theme.card_highlight,
            hover_bg_color: theme.control_hover,
            border_color: theme.control_border,
            hovered: ctx.hovered(Rect::from_xywh(sel_x, btn_y, sel_w, POPUP_BTN_H)),
        });

        if let Some(rl) = reset_label {
            let rst_w: f32 = 72.0;
            let rst_x = sel_x - rst_w - 6.0;
            draw_pill_btn(PillBtnParams {
                canvas,
                x: rst_x,
                y: btn_y,
                w: rst_w,
                h: POPUP_BTN_H,
                label: rl,
                text_color: theme.danger,
                bg_color: theme.card_highlight,
                hover_bg_color: theme.control_hover,
                border_color: theme.control_border,
                hovered: ctx.hovered(Rect::from_xywh(rst_x, btn_y, rst_w, POPUP_BTN_H)),
            });
        }
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

#[allow(clippy::too_many_arguments)]
fn draw_row_folder_picker(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    btn_label: &str,
    clear_label: &Option<String>,
    current_path: &Option<String>,
    enabled: bool,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let has_path = current_path.as_ref().is_some_and(|p| !p.is_empty());
    let row_h = if has_path { 64.0 } else { ROW_HEIGHT };
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let (cy, visible) = draw_row_text(ctx, y, row_h, label, row_text_color(ctx, enabled));

    if visible {
        if let Some(path) = current_path
            && !path.is_empty()
        {
            paint.set_color(theme.text_sec);
            let max_w = content_w - GROUP_INNER_PAD * 2.0 - 140.0;
            let display = truncate_text(fm, path, 11.0, max_w);
            fm.draw_text_cached(DrawTextCachedParams {
                canvas,
                text: &display,
                x: row_x,
                y: cy + 17.0,
                size: 11.0,
                bold: false,
                paint: &paint,
            });
        }

        let label_color = if enabled {
            theme.text_pri
        } else {
            theme.text_sec
        };
        let bg_color = if enabled {
            theme.card_highlight
        } else {
            theme.disabled
        };

        let sel_w: f32 = 72.0;
        let sel_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - sel_w;
        let btn_y = cy - POPUP_BTN_H / 2.0;
        draw_pill_btn(PillBtnParams {
            canvas,
            x: sel_x,
            y: btn_y,
            w: sel_w,
            h: POPUP_BTN_H,
            label: btn_label,
            text_color: label_color,
            bg_color,
            hover_bg_color: theme.control_hover,
            border_color: theme.control_border,
            hovered: enabled && ctx.hovered(Rect::from_xywh(sel_x, btn_y, sel_w, POPUP_BTN_H)),
        });

        if let Some(cl) = clear_label {
            let clr_w: f32 = 72.0;
            let clr_x = sel_x - clr_w - 6.0;
            draw_pill_btn(PillBtnParams {
                canvas,
                x: clr_x,
                y: btn_y,
                w: clr_w,
                h: POPUP_BTN_H,
                label: cl,
                text_color: if enabled {
                    theme.danger
                } else {
                    theme.text_sec
                },
                bg_color,
                hover_bg_color: theme.control_hover,
                border_color: theme.control_border,
                hovered: enabled && ctx.hovered(Rect::from_xywh(clr_x, btn_y, clr_w, POPUP_BTN_H)),
            });
        }
    }

    advance_group_row(ctx, y + row_h, groups, visible);
}

fn draw_row_source_select(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    options: &[(String, bool)],
    enabled: bool,
    active_source_button: Option<Rect>,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let fm = FontManager::global();
    let (cy, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, row_text_color(ctx, enabled));

    let selected_label = options
        .iter()
        .find(|(_, active)| *active)
        .map(|(l, _)| l.as_str())
        .unwrap_or("");

    let btn_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - POPUP_BTN_W;
    let btn_y = cy - POPUP_BTN_H / 2.0;
    let is_open = active_source_button.is_some_and(|button| {
        (button.left - btn_x).abs() < 0.5 && (button.top - btn_y).abs() < 0.5
    });

    if visible {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_color(if enabled {
            if ctx.hovered(Rect::from_xywh(btn_x, btn_y, POPUP_BTN_W, POPUP_BTN_H)) {
                theme.control_hover
            } else {
                theme.control_bg
            }
        } else {
            theme.control_disabled
        });
        canvas.draw_round_rect(
            Rect::from_xywh(btn_x, btn_y, POPUP_BTN_W, POPUP_BTN_H),
            POPUP_BTN_R,
            POPUP_BTN_R,
            &p,
        );
        p.set_color(if is_open {
            theme.accent
        } else {
            theme.control_border
        });
        p.set_style(skia_safe::paint::Style::Stroke);
        p.set_stroke_width(if is_open { 1.25 } else { 0.75 });
        canvas.draw_round_rect(
            Rect::from_xywh(
                btn_x + 0.5,
                btn_y + 0.5,
                POPUP_BTN_W - 1.0,
                POPUP_BTN_H - 1.0,
            ),
            POPUP_BTN_R,
            POPUP_BTN_R,
            &p,
        );
        p.set_style(skia_safe::paint::Style::Fill);

        p.set_color(if enabled {
            theme.text_pri
        } else {
            theme.text_sec
        });
        let text_w = POPUP_BTN_W - 22.0;
        fm.draw_text_in_rect(DrawTextInRectParams {
            canvas,
            text: selected_label,
            x: btn_x + 4.0,
            y: btn_y + 17.0,
            w: text_w,
            size: 13.0,
            bold: false,
            paint: &p,
        });

        let chev_cx = btn_x + POPUP_BTN_W - 12.0;
        let chev_cy = cy;
        let (top_y, bottom_y) = if is_open {
            (chev_cy - 1.5, chev_cy + 1.5)
        } else {
            (chev_cy + 1.5, chev_cy - 1.5)
        };
        let chev_svg = format!(
            "M {} {} L {} {} L {} {}",
            chev_cx - 3.0,
            bottom_y,
            chev_cx,
            top_y,
            chev_cx + 3.0,
            bottom_y,
        );
        p.set_color(if enabled {
            theme.text_sec
        } else {
            theme.disabled
        });
        p.set_style(skia_safe::paint::Style::Stroke);
        p.set_stroke_width(1.5);
        if let Some(chev_path) = skia_safe::Path::from_svg(&chev_svg) {
            canvas.draw_path(&chev_path, &p);
        }
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_row_button(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    btn_label: &str,
    enabled: bool,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let (cy, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, row_text_color(ctx, enabled));
    if visible {
        let label_color = if enabled {
            theme.text_pri
        } else {
            theme.text_sec
        };
        let bg_color = if enabled {
            theme.control_bg
        } else {
            theme.control_disabled
        };

        let btn_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - POPUP_BTN_W;
        draw_pill_btn(PillBtnParams {
            canvas,
            x: btn_x,
            y: cy - POPUP_BTN_H / 2.0,
            w: POPUP_BTN_W,
            h: POPUP_BTN_H,
            label: btn_label,
            text_color: label_color,
            bg_color,
            hover_bg_color: theme.control_hover,
            border_color: theme.control_border,
            hovered: enabled
                && ctx.hovered(Rect::from_xywh(
                    btn_x,
                    cy - POPUP_BTN_H / 2.0,
                    POPUP_BTN_W,
                    POPUP_BTN_H,
                )),
        });
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_row_app_item(
    ctx: &ItemCtx,
    y: f32,
    label: &str,
    active: bool,
    enabled: bool,
    groups: &mut GroupRows,
) {
    let canvas = ctx.canvas;
    let theme = ctx.theme;
    let content_w = ctx.content_w;
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let cy = y + ROW_HEIGHT / 2.0;
    let visible = ctx.row_visible(y, ROW_HEIGHT);

    let check_size = 20.0;
    let check_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - check_size;
    let check_y = cy - check_size / 2.0;

    let mut p = Paint::default();
    p.set_anti_alias(true);
    if visible && active && enabled {
        p.set_color(theme.accent);
        canvas.draw_round_rect(
            Rect::from_xywh(check_x, check_y, check_size, check_size),
            5.0,
            5.0,
            &p,
        );
        p.set_color(Color::WHITE);
        p.set_stroke_width(2.0);
        p.set_style(skia_safe::paint::Style::Stroke);
        let svg = format!(
            "M {} {} L {} {} L {} {}",
            check_x + 5.0,
            check_y + 10.0,
            check_x + 9.0,
            check_y + 14.0,
            check_x + 15.0,
            check_y + 6.0,
        );
        if let Some(path) = skia_safe::Path::from_svg(&svg) {
            canvas.draw_path(&path, &p);
        }
    } else if visible {
        p.set_color(if enabled {
            theme.card_highlight
        } else {
            theme.disabled
        });
        p.set_style(skia_safe::paint::Style::Stroke);
        p.set_stroke_width(1.5);
        canvas.draw_round_rect(
            Rect::from_xywh(check_x, check_y, check_size, check_size),
            5.0,
            5.0,
            &p,
        );
    }

    if visible {
        paint.set_color(if enabled {
            theme.text_pri
        } else {
            theme.text_sec
        });
        let max_label_w = check_x - row_x - 8.0;
        let display = truncate_text(fm, label, 13.0, max_label_w);
        fm.draw_text_cached(DrawTextCachedParams {
            canvas,
            text: &display,
            x: row_x,
            y: cy + 5.0,
            size: 13.0,
            bold: false,
            paint: &paint,
        });
    }

    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_row_label(ctx: &ItemCtx, y: f32, label: &str, groups: &mut GroupRows) {
    let (_, visible) = draw_row_text(ctx, y, ROW_HEIGHT, label, ctx.theme.text_sec);
    advance_group_row(ctx, y + ROW_HEIGHT, groups, visible);
}

fn draw_center_link(ctx: &ItemCtx, y: f32, height: f32, label: &str, color: Color) {
    if !ctx.row_visible(y, height) {
        return;
    }
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    let link_w = fm.measure_text_cached(label, 13.0, FontStyle::normal());
    fm.draw_text_cached(DrawTextCachedParams {
        canvas: ctx.canvas,
        text: label,
        x: ctx.width / 2.0 - link_w / 2.0,
        y: y + 24.0,
        size: 13.0,
        bold: false,
        paint: &paint,
    });
}

fn draw_center_text(ctx: &ItemCtx, y: f32, height: f32, text: &str, size: f32, color: Color) {
    if !ctx.row_visible(y, height) {
        return;
    }
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    let ct_w = fm.measure_text_cached(text, size, FontStyle::normal());
    fm.draw_text_cached(DrawTextCachedParams {
        canvas: ctx.canvas,
        text,
        x: ctx.width / 2.0 - ct_w / 2.0,
        y: y + 22.0,
        size,
        bold: false,
        paint: &paint,
    });
}

fn advance_group_row(ctx: &ItemCtx, sep_y: f32, groups: &mut GroupRows, visible: bool) {
    if !groups.in_group {
        return;
    }
    groups.current_row += 1;
    if groups.current_row < groups.row_count && visible {
        draw_row_separator(ctx.canvas, ctx.theme, ctx.content_w, sep_y);
    }
}

pub fn content_height(items: &[SettingsItem], start_y: f32) -> f32 {
    let mut h = start_y;
    for item in items {
        h += item.height();
    }
    h
}

pub fn draw_items(params: DrawItemsParams<'_>) {
    let canvas = params.canvas;
    let items = params.items;
    let start_y = params.start_y;
    let width = params.width;
    let anims = params.anims;
    let theme = params.theme;
    let visible_min_y = params.visible_min_y;
    let visible_max_y = params.visible_max_y;
    let island_style = params.island_style;
    let expanded_width = params.expanded_width;
    let expanded_height = params.expanded_height;
    let base_width = params.base_width;
    let base_height = params.base_height;
    let widget_editor_mode = params.widget_editor_mode;
    let widget_layout = params.widget_layout;
    let plugin_widget_layout = params.plugin_widget_layout;
    let plugin_widgets = params.plugin_widgets;
    let widget_dragging = params.widget_dragging;
    let widget_drag_hover_slot = params.widget_drag_hover_slot;
    let widget_preview_hover_slot = params.widget_preview_hover_slot;
    let compact_widget_layout = params.compact_widget_layout;
    let compact_widget_dragging = params.compact_widget_dragging;
    let compact_widget_drag_hover_slot = params.compact_widget_drag_hover_slot;
    let compact_widget_preview_hover_slot = params.compact_widget_preview_hover_slot;
    let active_source_button = params.active_source_button;
    let active_stepper_value = params.active_stepper_value;

    let fm = FontManager::global();
    let mut y = start_y;
    let mut switch_idx = 0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut groups = GroupRows::default();
    let content_w = width - CONTENT_PADDING * 2.0;
    let ctx = ItemCtx {
        canvas,
        theme,
        content_w,
        width,
        visible_min_y,
        visible_max_y,
        hover_pos: params.hover_pos,
    };

    let mut i = 0;
    while i < items.len() {
        let item = &items[i];
        if y > visible_max_y + 120.0 {
            break;
        }
        match item {
            SettingsItem::SectionHeader { label } => {
                let h = item.height();
                if y + h >= visible_min_y && y <= visible_max_y {
                    paint.set_color(theme.text_pri);
                    fm.draw_text_cached(DrawTextCachedParams {
                        canvas,
                        text: label,
                        x: CONTENT_PADDING + 4.0,
                        y: y + 22.0,
                        size: 13.0,
                        bold: true,
                        paint: &paint,
                    });
                }
            }
            SettingsItem::GroupStart => {
                groups.in_group = true;
                groups.current_row = 0;
                let total_h = group_height_from(items, i + 1);
                groups.row_count = items[i + 1..]
                    .iter()
                    .take_while(|item| !matches!(item, SettingsItem::GroupEnd))
                    .filter(|item| item.is_row())
                    .count();
                if y + total_h >= visible_min_y && y <= visible_max_y {
                    let mut shadow = Paint::default();
                    shadow.set_anti_alias(true);
                    shadow.set_color(theme.shadow);
                    canvas.draw_round_rect(
                        Rect::from_xywh(CONTENT_PADDING, y + 2.0, content_w, total_h),
                        GROUP_RADIUS,
                        GROUP_RADIUS,
                        &shadow,
                    );
                    let mut bg = Paint::default();
                    bg.set_anti_alias(true);
                    bg.set_color(theme.group_bg);
                    canvas.draw_round_rect(
                        Rect::from_xywh(CONTENT_PADDING, y, content_w, total_h),
                        GROUP_RADIUS,
                        GROUP_RADIUS,
                        &bg,
                    );
                    bg.set_style(skia_safe::paint::Style::Stroke);
                    bg.set_stroke_width(0.75);
                    bg.set_color(theme.group_border);
                    canvas.draw_round_rect(
                        Rect::from_xywh(
                            CONTENT_PADDING + 0.375,
                            y + 0.375,
                            content_w - 0.75,
                            total_h - 0.75,
                        ),
                        GROUP_RADIUS,
                        GROUP_RADIUS,
                        &bg,
                    );
                }
            }
            SettingsItem::GroupEnd => {
                groups.in_group = false;
            }
            SettingsItem::RowStepper {
                label,
                value,
                enabled,
            } => {
                draw_row_stepper(
                    &ctx,
                    y,
                    label,
                    value,
                    *enabled,
                    &active_stepper_value,
                    &mut groups,
                );
            }
            SettingsItem::RowSwitch {
                label,
                on: _,
                enabled,
            } => {
                draw_row_switch(&ctx, y, label, *enabled, anims.get(switch_idx), &mut groups);
                switch_idx += 1;
            }
            SettingsItem::RowFontPicker {
                label,
                btn_label,
                reset_label,
            } => {
                draw_row_font_picker(&ctx, y, label, btn_label, reset_label, &mut groups);
            }
            SettingsItem::RowFolderPicker {
                label,
                btn_label,
                clear_label,
                current_path,
                enabled,
            } => {
                draw_row_folder_picker(
                    &ctx,
                    y,
                    label,
                    btn_label,
                    clear_label,
                    current_path,
                    *enabled,
                    &mut groups,
                );
            }
            SettingsItem::RowSourceSelect {
                label,
                options,
                enabled,
            } => {
                draw_row_source_select(
                    &ctx,
                    y,
                    label,
                    options,
                    *enabled,
                    active_source_button,
                    &mut groups,
                );
            }
            SettingsItem::RowButton {
                label,
                btn_label,
                enabled,
            } => {
                draw_row_button(&ctx, y, label, btn_label, *enabled, &mut groups);
            }
            SettingsItem::RowAppItem {
                label,
                active,
                enabled,
            } => {
                draw_row_app_item(&ctx, y, label, *active, *enabled, &mut groups);
            }
            SettingsItem::RowLabel { label } => {
                draw_row_label(&ctx, y, label, &mut groups);
            }
            SettingsItem::CenterLink { label, color } => {
                draw_center_link(&ctx, y, item.height(), label, *color);
            }
            SettingsItem::CenterText { text, size, color } => {
                draw_center_text(&ctx, y, item.height(), text, *size, *color);
            }
            SettingsItem::Spacer { .. } => {}
            SettingsItem::Custom { .. } => {}
            SettingsItem::WidgetPreview { .. } => {
                draw_widget_preview(WidgetPreviewParams {
                    canvas,
                    item_y: y,
                    width,
                    content_width: content_w,
                    visible_min_y,
                    visible_max_y,
                    island_style,
                    expanded_width,
                    expanded_height,
                    base_width,
                    base_height,
                    widget_editor_mode,
                    widget_layout,
                    plugin_widget_layout,
                    plugin_widgets,
                    widget_dragging,
                    widget_drag_hover_slot,
                    widget_preview_hover_slot,
                    compact_widget_layout,
                    compact_widget_dragging,
                    compact_widget_drag_hover_slot,
                    compact_widget_preview_hover_slot,
                    theme,
                });
            }
        }
        y += item.height();
        i += 1;
    }
}

fn group_height_from(items: &[SettingsItem], start: usize) -> f32 {
    let mut h = 0.0;
    for item in &items[start..] {
        if matches!(item, SettingsItem::GroupEnd) {
            break;
        }
        h += item.height();
    }
    h
}

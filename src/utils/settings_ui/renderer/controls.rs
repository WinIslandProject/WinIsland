use skia_safe::{Canvas, Color, FontStyle, Paint, Rect};

use crate::utils::color::SettingsTheme;
use crate::utils::font::{DrawTextInRectParams, FontManager};

use super::super::items::{
    CONTENT_PADDING, GROUP_INNER_PAD, POPUP_BTN_R, STEPPER_BTN_SIZE, TOGGLE_H, TOGGLE_INSET,
    TOGGLE_KNOB, TOGGLE_R, TOGGLE_W,
};

pub(super) struct PillBtnParams<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) label: &'a str,
    pub(super) text_color: Color,
    pub(super) bg_color: Color,
    pub(super) hover_bg_color: Color,
    pub(super) border_color: Color,
    pub(super) hovered: bool,
}

pub(super) fn draw_row_separator(
    canvas: &Canvas,
    theme: &SettingsTheme,
    content_w: f32,
    sep_y: f32,
) {
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    let mut sep = Paint::default();
    sep.set_anti_alias(true);
    sep.set_color(theme.separator);
    sep.set_stroke_width(0.5);
    sep.set_style(skia_safe::paint::Style::Stroke);
    canvas.draw_line(
        (row_x, sep_y),
        (CONTENT_PADDING + content_w - GROUP_INNER_PAD, sep_y),
        &sep,
    );
}

pub(super) fn draw_switch(
    canvas: &Canvas,
    x: f32,
    y: f32,
    pos: f32,
    enabled: bool,
    theme: &SettingsTheme,
) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let (off_color, on_color) = if enabled {
        (theme.toggle_off, theme.toggle_on)
    } else {
        (theme.toggle_off, theme.toggle_off)
    };
    let r = off_color.r() as f32 + (on_color.r() as f32 - off_color.r() as f32) * pos;
    let g = off_color.g() as f32 + (on_color.g() as f32 - off_color.g() as f32) * pos;
    let b = off_color.b() as f32 + (on_color.b() as f32 - off_color.b() as f32) * pos;
    paint.set_color(Color::from_rgb(r as u8, g as u8, b as u8));
    canvas.draw_round_rect(
        Rect::from_xywh(x, y, TOGGLE_W, TOGGLE_H),
        TOGGLE_R,
        TOGGLE_R,
        &paint,
    );

    let mut border = Paint::default();
    border.set_anti_alias(true);
    border.set_style(skia_safe::paint::Style::Stroke);
    border.set_stroke_width(0.75);
    border.set_color(theme.control_border);
    canvas.draw_round_rect(
        Rect::from_xywh(x + 0.375, y + 0.375, TOGGLE_W - 0.75, TOGGLE_H - 0.75),
        TOGGLE_R,
        TOGGLE_R,
        &border,
    );

    let knob_x = x + TOGGLE_INSET + (pos * (TOGGLE_W - TOGGLE_KNOB - TOGGLE_INSET * 2.0));
    let knob_y = y + TOGGLE_INSET;

    let mut shadow = Paint::default();
    shadow.set_anti_alias(true);
    shadow.set_color(Color::from_argb(40, 0, 0, 0));
    canvas.draw_round_rect(
        Rect::from_xywh(knob_x, knob_y + 1.0, TOGGLE_KNOB, TOGGLE_KNOB),
        TOGGLE_KNOB / 2.0,
        TOGGLE_KNOB / 2.0,
        &shadow,
    );

    paint.set_color(Color::WHITE);
    canvas.draw_round_rect(
        Rect::from_xywh(knob_x, knob_y, TOGGLE_KNOB, TOGGLE_KNOB),
        TOGGLE_KNOB / 2.0,
        TOGGLE_KNOB / 2.0,
        &paint,
    );
}

pub(super) fn draw_stepper_btn(
    canvas: &Canvas,
    x: f32,
    y: f32,
    label: &str,
    enabled: bool,
    theme: &SettingsTheme,
    hovered: bool,
) {
    let fm = FontManager::global();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    if hovered && enabled {
        paint.set_color(theme.control_hover);
        canvas.draw_round_rect(
            Rect::from_xywh(x, y, STEPPER_BTN_SIZE, STEPPER_BTN_SIZE),
            POPUP_BTN_R,
            POPUP_BTN_R,
            &paint,
        );
    }
    paint.set_color(if enabled {
        theme.text_pri
    } else {
        theme.text_sec
    });
    let font = fm.get_font(16.0, false);
    let (_, bounds) = font.measure_str(label, None);
    let text_x = x + (STEPPER_BTN_SIZE - bounds.width()) / 2.0 - bounds.left();
    let text_y = y + (STEPPER_BTN_SIZE - bounds.height()) / 2.0 - bounds.top();
    canvas.draw_str(label, (text_x, text_y), &font, &paint);
}

pub(super) fn draw_pill_btn(params: PillBtnParams<'_>) {
    let fm = FontManager::global();
    let canvas = params.canvas;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(if params.hovered {
        params.hover_bg_color
    } else {
        params.bg_color
    });
    canvas.draw_round_rect(
        Rect::from_xywh(params.x, params.y, params.w, params.h),
        POPUP_BTN_R,
        POPUP_BTN_R,
        &paint,
    );
    paint.set_color(params.border_color);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(0.75);
    canvas.draw_round_rect(
        Rect::from_xywh(
            params.x + 0.375,
            params.y + 0.375,
            params.w - 0.75,
            params.h - 0.75,
        ),
        POPUP_BTN_R,
        POPUP_BTN_R,
        &paint,
    );
    paint.set_style(skia_safe::paint::Style::Fill);
    paint.set_color(params.text_color);
    fm.draw_text_in_rect(DrawTextInRectParams {
        canvas,
        text: params.label,
        x: params.x,
        y: params.y + 17.0,
        w: params.w,
        size: 12.0,
        bold: false,
        paint: &paint,
    });
}

pub(super) fn truncate_text(fm: &FontManager, text: &str, size: f32, max_w: f32) -> String {
    let w = fm.measure_text_cached(text, size, FontStyle::normal());
    if w <= max_w {
        return text.to_string();
    }
    let ellipsis = "...";
    let ew = fm.measure_text_cached(ellipsis, size, FontStyle::normal());
    let mut result = String::new();
    let mut current_w = 0.0;
    for c in text.chars() {
        let cw = fm.measure_text_cached(&c.to_string(), size, FontStyle::normal());
        if current_w + cw + ew > max_w {
            result.push_str(ellipsis);
            return result;
        }
        current_w += cw;
        result.push(c);
    }
    result
}

use crate::utils::color::SettingsTheme;
use crate::utils::color::settings_color;
use winisland_render::FontStyle;
use winisland_render::Painter;
use winisland_render::Point;
use winisland_render::Radius;
use winisland_render::Rect;
use winisland_render::Rgba;
use winisland_render::StrokeCap;
use winisland_render::text::{DrawTextCachedParams, DrawTextInRectParams, FontManager};

use super::super::items::{
    CONTENT_PADDING, GROUP_INNER_PAD, POPUP_BTN_R, STEPPER_BTN_SIZE, TOGGLE_H, TOGGLE_INSET,
    TOGGLE_KNOB, TOGGLE_R, TOGGLE_W,
};

pub(super) struct PillBtnParams<'a> {
    pub(super) painter: Painter<'a>,
    pub(super) rect: Rect,
    pub(super) label: &'a str,
    pub(super) text_color: Rgba,
    pub(super) bg_color: Rgba,
    pub(super) hover_bg_color: Rgba,
    pub(super) border_color: Rgba,
    pub(super) hovered: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct SettingsPainter<'a> {
    painter: Painter<'a>,
}

impl<'a> SettingsPainter<'a> {
    pub(crate) fn new(painter: Painter<'a>) -> Self {
        Self { painter }
    }

    pub(crate) fn text(
        self,
        text: &str,
        position: (f32, f32),
        size: f32,
        bold: bool,
        color: impl Into<Rgba>,
    ) {
        FontManager::global().draw_text_cached(DrawTextCachedParams {
            painter: self.painter,
            text,
            x: position.0,
            y: position.1,
            size,
            bold,
            color: color.into(),
            blur: None,
        });
    }

    pub(crate) fn centered_text(
        self,
        text: &str,
        position: (f32, f32),
        size: f32,
        bold: bool,
        color: impl Into<Rgba>,
    ) {
        let style = if bold {
            FontStyle::bold()
        } else {
            FontStyle::normal()
        };
        let width = FontManager::global().measure_text_cached(text, size, style);
        self.text(
            text,
            (position.0 - width / 2.0, position.1),
            size,
            bold,
            color,
        );
    }
}

pub(super) fn draw_row_separator(
    painter: Painter<'_>,
    theme: &SettingsTheme,
    content_w: f32,
    sep_y: f32,
) {
    let row_x = CONTENT_PADDING + GROUP_INNER_PAD;
    painter.stroke_line(
        Point::new(row_x, sep_y),
        Point::new(CONTENT_PADDING + content_w - GROUP_INNER_PAD, sep_y),
        0.5,
        settings_color(theme.separator),
        StrokeCap::Butt,
    );
}

pub(super) fn draw_switch(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    pos: f32,
    enabled: bool,
    theme: &SettingsTheme,
) {
    let (off_color, on_color) = if enabled {
        (theme.toggle_off, theme.toggle_on)
    } else {
        (theme.toggle_off, theme.toggle_off)
    };
    let r = off_color.r() as f32 + (on_color.r() as f32 - off_color.r() as f32) * pos;
    let g = off_color.g() as f32 + (on_color.g() as f32 - off_color.g() as f32) * pos;
    let b = off_color.b() as f32 + (on_color.b() as f32 - off_color.b() as f32) * pos;
    painter.fill_round_rect(
        Rect::from_xywh(x, y, TOGGLE_W, TOGGLE_H),
        Radius::uniform(TOGGLE_R),
        Rgba::from_rgb(r as u8, g as u8, b as u8),
    );

    painter.stroke_round_rect(
        Rect::from_xywh(x + 0.375, y + 0.375, TOGGLE_W - 0.75, TOGGLE_H - 0.75),
        Radius::uniform(TOGGLE_R),
        0.75,
        settings_color(theme.control_border),
    );

    let knob_x = x + TOGGLE_INSET + (pos * (TOGGLE_W - TOGGLE_KNOB - TOGGLE_INSET * 2.0));
    let knob_y = y + TOGGLE_INSET;

    painter.fill_round_rect(
        Rect::from_xywh(knob_x, knob_y + 1.0, TOGGLE_KNOB, TOGGLE_KNOB),
        Radius::uniform(TOGGLE_KNOB / 2.0),
        Rgba::from_argb(40, 0, 0, 0),
    );

    painter.fill_round_rect(
        Rect::from_xywh(knob_x, knob_y, TOGGLE_KNOB, TOGGLE_KNOB),
        Radius::uniform(TOGGLE_KNOB / 2.0),
        Rgba::WHITE,
    );
}

pub(super) fn draw_stepper_btn(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    label: &str,
    enabled: bool,
    theme: &SettingsTheme,
    hovered: bool,
) {
    let fm = FontManager::global();
    if hovered && enabled {
        painter.fill_round_rect(
            Rect::from_xywh(x, y, STEPPER_BTN_SIZE, STEPPER_BTN_SIZE),
            Radius::uniform(POPUP_BTN_R),
            settings_color(theme.control_hover),
        );
    }
    let color = settings_color(if enabled {
        theme.text_pri
    } else {
        theme.text_sec
    });
    let bounds = fm.measure_str(label, 16.0, false);
    let text_x = x + (STEPPER_BTN_SIZE - bounds.width()) / 2.0 - bounds.left;
    let text_y = y + (STEPPER_BTN_SIZE - bounds.height()) / 2.0 - bounds.top;
    fm.draw_str(
        painter,
        label,
        Point::new(text_x, text_y),
        16.0,
        false,
        color,
    );
}

pub(super) fn draw_pill_btn(params: PillBtnParams<'_>) {
    let fm = FontManager::global();
    let painter = params.painter;
    painter.fill_round_rect(
        params.rect,
        Radius::uniform(POPUP_BTN_R),
        if params.hovered {
            params.hover_bg_color
        } else {
            params.bg_color
        },
    );
    painter.stroke_round_rect(
        Rect::from_xywh(
            params.rect.left + 0.375,
            params.rect.top + 0.375,
            params.rect.width() - 0.75,
            params.rect.height() - 0.75,
        ),
        Radius::uniform(POPUP_BTN_R),
        0.75,
        params.border_color,
    );
    fm.draw_text_in_rect(DrawTextInRectParams {
        painter,
        text: params.label,
        x: params.rect.left,
        y: params.rect.top + 17.0,
        w: params.rect.width(),
        size: 12.0,
        bold: false,
        color: params.text_color,
        blur: None,
    });
}

pub(crate) fn ellipsize_text(
    fm: &FontManager,
    text: &str,
    size: f32,
    style: FontStyle,
    max_width: f32,
) -> String {
    if fm.measure_text_cached(text, size, style) <= max_width {
        return text.to_string();
    }
    let ellipsis = "…";
    let ellipsis_width = fm.measure_text_cached(ellipsis, size, style);
    let mut fitted = String::new();
    for character in text.chars() {
        fitted.push(character);
        if fm.measure_text_cached(&fitted, size, style) + ellipsis_width > max_width {
            fitted.pop();
            break;
        }
    }
    fitted.push_str(ellipsis);
    fitted
}

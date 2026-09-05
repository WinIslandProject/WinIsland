use skia_safe::Color;

pub const COLOR_CARD_HIGHLIGHT: Color = Color::from_rgb(72, 72, 74);
pub const COLOR_ACCENT: Color = Color::from_rgb(10, 132, 255);
pub const COLOR_TEXT_PRI: Color = Color::from_rgb(245, 245, 247);
pub const COLOR_TEXT_SEC: Color = Color::from_rgb(174, 174, 178);
pub const COLOR_DANGER: Color = Color::from_rgb(230, 55, 45);
pub const COLOR_DISABLED: Color = Color::from_rgb(99, 99, 102);

pub const COLOR_WIN_BG: Color = Color::from_rgb(28, 28, 30);
pub const COLOR_SIDEBAR_BG: Color = Color::from_rgb(36, 36, 38);
pub const COLOR_GROUP_BG: Color = Color::from_rgb(44, 44, 46);
pub const COLOR_TOGGLE_ON: Color = Color::from_rgb(48, 209, 88);
pub const COLOR_TOGGLE_OFF: Color = Color::from_rgb(99, 99, 102);

pub fn color_sidebar_hover() -> Color {
    Color::from_argb(20, 255, 255, 255)
}

pub fn color_separator() -> Color {
    Color::from_argb(26, 255, 255, 255)
}

pub struct SettingsTheme {
    pub win_bg: Color,
    pub sidebar_bg: Color,
    pub group_bg: Color,
    pub card_highlight: Color,
    pub text_pri: Color,
    pub text_sec: Color,
    pub disabled: Color,
    pub accent: Color,
    pub danger: Color,
    pub toggle_on: Color,
    pub toggle_off: Color,
    pub selection_bg: Color,
    pub selection_text: Color,

    pub sidebar_hover: Color,
    pub separator: Color,
    pub popup_bg: Color,
    pub popup_border: Color,
    pub popup_shadow: Color,
    pub popup_separator: Color,
    pub control_bg: Color,
    pub control_hover: Color,
    pub control_disabled: Color,
    pub control_border: Color,
    pub group_border: Color,
    pub shadow: Color,
    pub scrollbar: Color,
}

pub fn dark_settings_theme() -> SettingsTheme {
    SettingsTheme {
        win_bg: COLOR_WIN_BG,
        sidebar_bg: COLOR_SIDEBAR_BG,
        group_bg: COLOR_GROUP_BG,
        card_highlight: COLOR_CARD_HIGHLIGHT,
        text_pri: COLOR_TEXT_PRI,
        text_sec: COLOR_TEXT_SEC,
        disabled: COLOR_DISABLED,
        accent: COLOR_ACCENT,
        danger: COLOR_DANGER,
        toggle_on: COLOR_TOGGLE_ON,
        toggle_off: COLOR_TOGGLE_OFF,
        selection_bg: COLOR_ACCENT,
        selection_text: COLOR_TEXT_PRI,

        sidebar_hover: color_sidebar_hover(),
        separator: color_separator(),
        popup_bg: Color::from_rgb(50, 50, 52),
        popup_border: Color::from_argb(40, 255, 255, 255),
        popup_shadow: Color::from_argb(60, 0, 0, 0),
        popup_separator: Color::from_argb(30, 255, 255, 255),
        control_bg: Color::from_rgb(58, 58, 60),
        control_hover: Color::from_rgb(72, 72, 74),
        control_disabled: Color::from_rgb(48, 48, 50),
        control_border: Color::from_argb(36, 255, 255, 255),
        group_border: Color::from_argb(24, 255, 255, 255),
        shadow: Color::from_argb(45, 0, 0, 0),
        scrollbar: Color::from_argb(60, 255, 255, 255),
    }
}

pub fn light_settings_theme() -> SettingsTheme {
    SettingsTheme {
        win_bg: Color::from_rgb(244, 245, 247),
        sidebar_bg: Color::from_rgb(250, 250, 252),
        group_bg: Color::from_rgb(255, 255, 255),
        card_highlight: Color::from_rgb(239, 242, 247),
        text_pri: Color::from_rgb(31, 34, 40),
        text_sec: Color::from_rgb(91, 98, 110),
        disabled: Color::from_rgb(146, 151, 161),
        accent: Color::from_rgb(0, 103, 192),
        danger: Color::from_rgb(196, 43, 28),
        toggle_on: Color::from_rgb(0, 120, 212),
        toggle_off: Color::from_rgb(176, 181, 190),
        selection_bg: Color::from_rgb(224, 238, 250),
        selection_text: Color::from_rgb(0, 85, 153),

        sidebar_hover: Color::from_argb(14, 20, 32, 48),
        separator: Color::from_argb(18, 24, 32, 44),
        popup_bg: Color::from_rgb(255, 255, 255),
        popup_border: Color::from_argb(24, 20, 28, 40),
        popup_shadow: Color::from_argb(20, 20, 28, 40),
        popup_separator: Color::from_argb(12, 20, 28, 40),
        control_bg: Color::from_rgb(247, 248, 250),
        control_hover: Color::from_rgb(235, 239, 244),
        control_disabled: Color::from_rgb(241, 242, 245),
        control_border: Color::from_argb(24, 20, 28, 40),
        group_border: Color::from_argb(14, 20, 28, 40),
        shadow: Color::from_argb(20, 20, 28, 40),
        scrollbar: Color::from_argb(52, 52, 58, 68),
    }
}

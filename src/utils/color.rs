use winisland_render::Rgba;

pub(crate) fn settings_color(color: Rgba) -> Rgba {
    color
}

pub const COLOR_CARD_HIGHLIGHT: Rgba = Rgba::from_rgb(72, 72, 74);
pub const COLOR_ACCENT: Rgba = Rgba::from_rgb(10, 132, 255);
pub const COLOR_TEXT_PRI: Rgba = Rgba::from_rgb(245, 245, 247);
pub const COLOR_TEXT_SEC: Rgba = Rgba::from_rgb(174, 174, 178);
pub const COLOR_DANGER: Rgba = Rgba::from_rgb(230, 55, 45);
pub const COLOR_DISABLED: Rgba = Rgba::from_rgb(99, 99, 102);

pub const COLOR_WIN_BG: Rgba = Rgba::from_rgb(28, 28, 30);
pub const COLOR_SIDEBAR_BG: Rgba = Rgba::from_rgb(36, 36, 38);
pub const COLOR_GROUP_BG: Rgba = Rgba::from_rgb(44, 44, 46);
pub const COLOR_TOGGLE_ON: Rgba = Rgba::from_rgb(48, 209, 88);
pub const COLOR_TOGGLE_OFF: Rgba = Rgba::from_rgb(99, 99, 102);

pub fn color_sidebar_hover() -> Rgba {
    Rgba::from_argb(20, 255, 255, 255)
}

pub fn color_separator() -> Rgba {
    Rgba::from_argb(26, 255, 255, 255)
}

pub struct SettingsTheme {
    pub win_bg: Rgba,
    pub sidebar_bg: Rgba,
    pub group_bg: Rgba,
    pub card_highlight: Rgba,
    pub text_pri: Rgba,
    pub text_sec: Rgba,
    pub disabled: Rgba,
    pub accent: Rgba,
    pub danger: Rgba,
    pub toggle_on: Rgba,
    pub toggle_off: Rgba,
    pub selection_bg: Rgba,
    pub selection_text: Rgba,

    pub sidebar_hover: Rgba,
    pub separator: Rgba,
    pub popup_bg: Rgba,
    pub popup_border: Rgba,
    pub popup_shadow: Rgba,
    pub popup_separator: Rgba,
    pub control_bg: Rgba,
    pub control_hover: Rgba,
    pub control_disabled: Rgba,
    pub control_border: Rgba,
    pub group_border: Rgba,
    pub shadow: Rgba,
    pub scrollbar: Rgba,
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
        popup_bg: Rgba::from_rgb(50, 50, 52),
        popup_border: Rgba::from_argb(40, 255, 255, 255),
        popup_shadow: Rgba::from_argb(60, 0, 0, 0),
        popup_separator: Rgba::from_argb(30, 255, 255, 255),
        control_bg: Rgba::from_rgb(58, 58, 60),
        control_hover: Rgba::from_rgb(72, 72, 74),
        control_disabled: Rgba::from_rgb(48, 48, 50),
        control_border: Rgba::from_argb(36, 255, 255, 255),
        group_border: Rgba::from_argb(24, 255, 255, 255),
        shadow: Rgba::from_argb(45, 0, 0, 0),
        scrollbar: Rgba::from_argb(60, 255, 255, 255),
    }
}

pub fn light_settings_theme() -> SettingsTheme {
    SettingsTheme {
        win_bg: Rgba::from_rgb(244, 245, 247),
        sidebar_bg: Rgba::from_rgb(250, 250, 252),
        group_bg: Rgba::from_rgb(255, 255, 255),
        card_highlight: Rgba::from_rgb(239, 242, 247),
        text_pri: Rgba::from_rgb(31, 34, 40),
        text_sec: Rgba::from_rgb(91, 98, 110),
        disabled: Rgba::from_rgb(146, 151, 161),
        accent: Rgba::from_rgb(0, 103, 192),
        danger: Rgba::from_rgb(196, 43, 28),
        toggle_on: Rgba::from_rgb(0, 120, 212),
        toggle_off: Rgba::from_rgb(176, 181, 190),
        selection_bg: Rgba::from_rgb(224, 238, 250),
        selection_text: Rgba::from_rgb(0, 85, 153),

        sidebar_hover: Rgba::from_argb(14, 20, 32, 48),
        separator: Rgba::from_argb(18, 24, 32, 44),
        popup_bg: Rgba::from_rgb(255, 255, 255),
        popup_border: Rgba::from_argb(24, 20, 28, 40),
        popup_shadow: Rgba::from_argb(20, 20, 28, 40),
        popup_separator: Rgba::from_argb(12, 20, 28, 40),
        control_bg: Rgba::from_rgb(247, 248, 250),
        control_hover: Rgba::from_rgb(235, 239, 244),
        control_disabled: Rgba::from_rgb(241, 242, 245),
        control_border: Rgba::from_argb(24, 20, 28, 40),
        group_border: Rgba::from_argb(14, 20, 28, 40),
        shadow: Rgba::from_argb(20, 20, 28, 40),
        scrollbar: Rgba::from_argb(52, 52, 58, 68),
    }
}

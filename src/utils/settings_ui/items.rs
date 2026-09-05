use skia_safe::{Color, Rect};

pub const CONTENT_PADDING: f32 = 24.0;
pub const ROW_HEIGHT: f32 = 48.0;
pub const GROUP_RADIUS: f32 = 12.0;
pub const GROUP_INNER_PAD: f32 = 16.0;
pub const SIDEBAR_PAD: f32 = 8.0;
pub const SIDEBAR_SEL_RADIUS: f32 = 7.0;

pub const TOGGLE_W: f32 = 36.0;
pub const TOGGLE_H: f32 = 20.0;
pub const TOGGLE_R: f32 = 10.0;
pub const TOGGLE_KNOB: f32 = 16.0;
pub const TOGGLE_INSET: f32 = 2.0;

pub const STEPPER_BTN_SIZE: f32 = 28.0;
pub const STEPPER_VALUE_W: f32 = 52.0;
pub const STEPPER_GAP: f32 = 0.0;

pub const POPUP_BTN_W: f32 = 112.0;
pub const POPUP_BTN_H: f32 = 28.0;
pub const POPUP_BTN_R: f32 = 7.0;
pub const POPUP_ITEM_H: f32 = 32.0;
pub const POPUP_MENU_R: f32 = 10.0;
pub const POPUP_MENU_PAD: f32 = 5.0;
pub const PICKER_BTN_W: f32 = 72.0;
pub const PICKER_BTN_GAP: f32 = 6.0;

pub fn trailing_control_rect(
    y: f32,
    row_height: f32,
    content_width: f32,
    control_width: f32,
    control_height: f32,
) -> Rect {
    Rect::from_xywh(
        CONTENT_PADDING + content_width - GROUP_INNER_PAD - control_width,
        y + (row_height - control_height) / 2.0,
        control_width,
        control_height,
    )
}

pub fn picker_button_rects(y: f32, row_height: f32, content_width: f32) -> (Rect, Rect) {
    let primary = trailing_control_rect(y, row_height, content_width, PICKER_BTN_W, POPUP_BTN_H);
    let secondary = Rect::from_xywh(
        primary.left - PICKER_BTN_GAP - PICKER_BTN_W,
        primary.top,
        PICKER_BTN_W,
        POPUP_BTN_H,
    );
    (primary, secondary)
}

#[derive(Clone)]
pub enum SettingsItem {
    SectionHeader {
        label: String,
    },
    GroupStart,
    GroupEnd,
    RowStepper {
        label: String,
        value: String,
        enabled: bool,
    },
    RowSwitch {
        label: String,
        on: bool,
        enabled: bool,
    },
    RowFontPicker {
        label: String,
        btn_label: String,
        reset_label: Option<String>,
    },
    RowFolderPicker {
        label: String,
        btn_label: String,
        clear_label: Option<String>,
        current_path: Option<String>,
        enabled: bool,
    },
    RowSourceSelect {
        label: String,
        options: Vec<(String, bool)>,
        enabled: bool,
    },
    RowButton {
        label: String,
        btn_label: String,
        enabled: bool,
    },
    RowAppItem {
        label: String,
        active: bool,
        enabled: bool,
    },
    RowLabel {
        label: String,
    },
    CenterLink {
        label: String,
        color: Color,
    },
    CenterText {
        text: String,
        size: f32,
        color: Color,
    },
    Spacer {
        height: f32,
    },
    Custom {
        height: f32,
    },
    WidgetPreview {
        height: f32,
    },
}

impl SettingsItem {
    pub fn height(&self) -> f32 {
        match self {
            SettingsItem::SectionHeader { .. } => 36.0,
            SettingsItem::GroupStart => 0.0,
            SettingsItem::GroupEnd => 16.0,
            SettingsItem::CenterLink { .. } => 40.0,
            SettingsItem::CenterText { .. } => 35.0,
            SettingsItem::Spacer { height } => *height,
            SettingsItem::Custom { height } => *height,
            SettingsItem::WidgetPreview { height } => *height,
            SettingsItem::RowFolderPicker { current_path, .. } => {
                if current_path.as_ref().is_some_and(|p| !p.is_empty()) {
                    64.0
                } else {
                    ROW_HEIGHT
                }
            }
            _ => ROW_HEIGHT,
        }
    }

    pub fn is_row(&self) -> bool {
        matches!(
            self,
            SettingsItem::RowStepper { .. }
                | SettingsItem::RowSwitch { .. }
                | SettingsItem::RowFontPicker { .. }
                | SettingsItem::RowFolderPicker { .. }
                | SettingsItem::RowSourceSelect { .. }
                | SettingsItem::RowButton { .. }
                | SettingsItem::RowAppItem { .. }
                | SettingsItem::RowLabel { .. }
        )
    }
}

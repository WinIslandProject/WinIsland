use skia_safe::Color;
use skia_safe::Rect;

use crate::utils::settings_ui::items::{
    CONTENT_PADDING, GROUP_INNER_PAD, POPUP_BTN_H, POPUP_BTN_W, ROW_HEIGHT, STEPPER_BTN_SIZE,
    STEPPER_GAP, STEPPER_VALUE_W, SettingsItem,
};
use crate::utils::settings_ui::{ClickResult, hit_test};

use super::{SIDEBAR_W, SettingsApp};

pub mod about;
pub mod general;
pub mod music;
pub mod plugins;
pub mod widgets;

pub(crate) struct SettingsPage<A> {
    items: Vec<SettingsItem>,
    actions: Vec<Option<A>>,
}

impl<A> SettingsPage<A> {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, item: SettingsItem) {
        self.items.push(item);
        self.actions.push(None);
    }

    pub(crate) fn push_action(&mut self, item: SettingsItem, action: A) {
        self.items.push(item);
        self.actions.push(Some(action));
    }

    pub(crate) fn section(&mut self, label: String) {
        self.push(SettingsItem::SectionHeader { label });
    }

    pub(crate) fn group_start(&mut self) {
        self.push(SettingsItem::GroupStart);
    }

    pub(crate) fn group_end(&mut self) {
        self.push(SettingsItem::GroupEnd);
    }

    pub(crate) fn row_switch(&mut self, label: String, on: bool, enabled: bool, action: A) {
        self.push_action(SettingsItem::RowSwitch { label, on, enabled }, action);
    }

    pub(crate) fn row_stepper(&mut self, label: String, value: String, enabled: bool, action: A) {
        self.push_action(
            SettingsItem::RowStepper {
                label,
                value,
                enabled,
            },
            action,
        );
    }

    pub(crate) fn row_source(
        &mut self,
        label: String,
        options: Vec<(String, bool)>,
        enabled: bool,
        action: A,
    ) {
        self.push_action(
            SettingsItem::RowSourceSelect {
                label,
                options,
                enabled,
            },
            action,
        );
    }

    pub(crate) fn row_button(
        &mut self,
        label: String,
        btn_label: String,
        enabled: bool,
        action: A,
    ) {
        self.push_action(
            SettingsItem::RowButton {
                label,
                btn_label,
                enabled,
            },
            action,
        );
    }

    pub(crate) fn center_link(&mut self, label: String, color: Color, action: A) {
        self.push_action(SettingsItem::CenterLink { label, color }, action);
    }

    pub(crate) fn row_label(&mut self, label: String) {
        self.push(SettingsItem::RowLabel { label });
    }

    pub(crate) fn row_app(&mut self, label: String, active: bool, enabled: bool, action: A) {
        self.push_action(
            SettingsItem::RowAppItem {
                label,
                active,
                enabled,
            },
            action,
        );
    }

    pub(crate) fn row_folder(
        &mut self,
        label: String,
        btn_label: String,
        clear_label: Option<String>,
        current_path: Option<String>,
        enabled: bool,
        action: A,
    ) {
        self.push_action(
            SettingsItem::RowFolderPicker {
                label,
                btn_label,
                clear_label,
                current_path,
                enabled,
            },
            action,
        );
    }

    pub(crate) fn items(&self) -> &[SettingsItem] {
        &self.items
    }

    pub(crate) fn action(&self, result: &ClickResult) -> Option<&A> {
        result
            .item_index()
            .and_then(|index| self.actions.get(index))
            .and_then(Option::as_ref)
    }

    pub(crate) fn into_items(self) -> Vec<SettingsItem> {
        self.items
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PageInput {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) start_y: f32,
}

impl PageInput {
    pub(crate) fn hit_test<A>(&self, page: &SettingsPage<A>) -> ClickResult {
        hit_test(page.items(), self.x, self.y, self.start_y, self.width)
    }

    pub(crate) fn popup_button_rect<A>(
        &self,
        page: &SettingsPage<A>,
        item_index: usize,
        scroll_y: f32,
    ) -> Rect {
        let item_y = self.start_y
            + page
                .items()
                .iter()
                .take(item_index)
                .map(SettingsItem::height)
                .sum::<f32>();
        let button_x = SIDEBAR_W + CONTENT_PADDING + self.width - GROUP_INNER_PAD - POPUP_BTN_W;
        let button_y = item_y + (ROW_HEIGHT - POPUP_BTN_H) / 2.0 - scroll_y;
        Rect::from_xywh(button_x, button_y, POPUP_BTN_W, POPUP_BTN_H)
    }

    pub(crate) fn stepper_value_rect<A>(
        &self,
        page: &SettingsPage<A>,
        item_index: usize,
        scroll_y: f32,
    ) -> Rect {
        let item_y = self.start_y
            + page
                .items()
                .iter()
                .take(item_index)
                .map(SettingsItem::height)
                .sum::<f32>();
        let content_w = self.width - CONTENT_PADDING * 2.0;
        let button_x = CONTENT_PADDING + content_w - GROUP_INNER_PAD - STEPPER_BTN_SIZE;
        let value_x = button_x - STEPPER_GAP - STEPPER_VALUE_W;
        let value_y = item_y + (ROW_HEIGHT - STEPPER_BTN_SIZE) / 2.0 - scroll_y;
        Rect::from_xywh(
            SIDEBAR_W + value_x,
            value_y,
            STEPPER_VALUE_W,
            STEPPER_BTN_SIZE,
        )
    }
}

impl SettingsApp {
    pub(crate) fn show_popup(&mut self, popup: super::PopupState) {
        self.popup = Some(popup);
        self.anim
            .set_with_speed(super::POPUP_OPACITY_KEY, 1.0, 0.25);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(crate) fn persist_settings_change(&mut self) {
        self.mark_items_dirty();
        crate::core::persistence::save_config(&self.config);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

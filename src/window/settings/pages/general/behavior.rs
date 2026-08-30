use crate::core::config::{AppConfig, MAX_HIDDEN_WIDTH, MIN_HIDDEN_WIDTH};
use crate::core::i18n::{available_langs, current_lang, init_i18n, set_lang, tr};
use crate::utils::autostart::set_autostart;
use crate::utils::font::FontManager;
use crate::utils::settings_ui::items::SettingsItem;
use crate::utils::settings_ui::{ClickResult, StepDirection};
use crate::window::settings::{NumberInputHandler, PopupState};

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum BehaviorAction {
    AutoStart,
    AutoHide,
    HiddenWidth,
    RightClickDrag,
    NotificationDisplay,
    ReplaceNativeVolumeFlyout,
    HideDelay,
    Language,
    CheckForUpdates,
    UpdateChannel,
    UpdateInterval,
    CheckUpdatesNow,
    ResetDefaults,
}

impl SettingsApp {
    pub(super) fn build_behavior_page(&self) -> SettingsPage<BehaviorAction> {
        let mut page = SettingsPage::new();
        page.section(tr("section_behavior"));
        page.group_start();
        page.row_switch(
            tr("start_boot"),
            self.config.auto_start,
            true,
            BehaviorAction::AutoStart,
        );
        page.row_switch(
            tr("auto_hide"),
            self.config.auto_hide,
            true,
            BehaviorAction::AutoHide,
        );
        if self.config.auto_hide {
            page.row_stepper(
                tr("hide_delay"),
                format!("{:.0}", self.config.auto_hide_delay),
                true,
                BehaviorAction::HideDelay,
            );
        }
        page.row_stepper(
            tr("hidden_width"),
            if self.config.hidden_width >= MAX_HIDDEN_WIDTH {
                tr("hidden_width_off")
            } else {
                format!("{:.0}", self.config.hidden_width)
            },
            true,
            BehaviorAction::HiddenWidth,
        );
        page.row_switch(
            tr("right_click_drag"),
            self.config.right_click_drag,
            true,
            BehaviorAction::RightClickDrag,
        );
        page.row_switch(
            tr("notification_display"),
            self.config.notification_display,
            true,
            BehaviorAction::NotificationDisplay,
        );
        page.row_switch(
            tr("replace_native_volume_flyout"),
            self.config.replace_native_volume_flyout,
            true,
            BehaviorAction::ReplaceNativeVolumeFlyout,
        );

        let language = current_lang();
        page.row_source(
            tr("language"),
            available_langs()
                .iter()
                .map(|entry| (entry.name.clone(), entry.code == language))
                .collect(),
            true,
            BehaviorAction::Language,
        );
        page.group_end();
        page.section(tr("section_updates"));
        page.group_start();
        page.row_switch(
            tr("check_updates"),
            self.config.check_for_updates,
            true,
            BehaviorAction::CheckForUpdates,
        );
        if self.config.check_for_updates {
            page.row_source(
                tr("update_channel"),
                vec![
                    (tr("channel_stable"), self.config.update_channel == "stable"),
                    (tr("channel_beta"), self.config.update_channel == "beta"),
                ],
                true,
                BehaviorAction::UpdateChannel,
            );
            page.row_stepper(
                tr("update_interval"),
                format!("{:.0}", self.config.update_check_interval),
                true,
                BehaviorAction::UpdateInterval,
            );
        }
        page.row_button(
            tr("check_updates_manual"),
            tr("update_check_btn"),
            true,
            BehaviorAction::CheckUpdatesNow,
        );
        page.group_end();
        page.push(SettingsItem::Spacer { height: 10.0 });
        page.center_link(
            tr("reset_defaults"),
            self.theme().danger,
            BehaviorAction::ResetDefaults,
        );
        page
    }

    pub(super) fn handle_behavior_click(&mut self, input: PageInput) {
        let page = self.build_behavior_page();
        let result = input.hit_test(&page);
        let Some(action) = page.action(&result).copied() else {
            return;
        };

        if let ClickResult::StepperValue(item_index) = &result {
            let (value, on_commit): (String, NumberInputHandler) = match action {
                BehaviorAction::HideDelay => (
                    format!("{:.0}", self.config.auto_hide_delay),
                    set_hide_delay,
                ),
                BehaviorAction::HiddenWidth => {
                    (format!("{:.0}", self.config.hidden_width), set_hidden_width)
                }
                BehaviorAction::UpdateInterval => (
                    format!("{:.0}", self.config.update_check_interval),
                    set_update_interval,
                ),
                _ => return,
            };
            self.begin_number_input(
                input.stepper_value_rect(&page, *item_index, self.scroll_y),
                value,
                on_commit,
            );
            return;
        }

        let changed = match (action, &result) {
            (BehaviorAction::AutoStart, ClickResult::Switch(_)) => {
                self.config.auto_start = !self.config.auto_start;
                let _ = set_autostart(self.config.auto_start);
                true
            }
            (BehaviorAction::AutoHide, ClickResult::Switch(_)) => {
                self.config.auto_hide = !self.config.auto_hide;
                true
            }
            (BehaviorAction::RightClickDrag, ClickResult::Switch(_)) => {
                self.config.right_click_drag = !self.config.right_click_drag;
                true
            }
            (BehaviorAction::NotificationDisplay, ClickResult::Switch(_)) => {
                self.config.notification_display = !self.config.notification_display;
                true
            }
            (BehaviorAction::ReplaceNativeVolumeFlyout, ClickResult::Switch(_)) => {
                self.config.replace_native_volume_flyout =
                    !self.config.replace_native_volume_flyout;
                true
            }
            (BehaviorAction::CheckForUpdates, ClickResult::Switch(_)) => {
                self.config.check_for_updates = !self.config.check_for_updates;
                true
            }
            (BehaviorAction::HideDelay, _) => {
                let Some(direction) = result.step_direction() else {
                    return;
                };
                self.config.auto_hide_delay =
                    step(self.config.auto_hide_delay, direction, 1.0, 1.0, 60.0);
                true
            }
            (BehaviorAction::HiddenWidth, _) => {
                let Some(direction) = result.step_direction() else {
                    return;
                };
                self.config.hidden_width = step(
                    self.config.hidden_width,
                    direction,
                    1.0,
                    MIN_HIDDEN_WIDTH,
                    MAX_HIDDEN_WIDTH,
                );
                true
            }
            (BehaviorAction::UpdateInterval, _) => {
                let Some(direction) = result.step_direction() else {
                    return;
                };
                self.config.update_check_interval =
                    step(self.config.update_check_interval, direction, 1.0, 1.0, 24.0);
                true
            }
            (BehaviorAction::ResetDefaults, ClickResult::CenterLink(_)) => {
                self.config = AppConfig::default();
                init_i18n(&self.config.language);
                crate::ui::widget::expanded::calendar::clear_calendar_text_cache();
                FontManager::global().set_custom_font_path(self.config.custom_font_path.as_deref());
                true
            }
            (BehaviorAction::CheckUpdatesNow, ClickResult::RowButton(_)) => {
                crate::utils::updater::check_updates_manually();
                false
            }
            _ => false,
        };
        if changed {
            self.persist_settings_change();
            return;
        }

        let ClickResult::SourceButton(item_index) = result else {
            return;
        };
        let button_rect = input.popup_button_rect(&page, item_index, self.scroll_y);
        let scale = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let popup = match action {
            BehaviorAction::Language => {
                let languages = available_langs();
                let current = current_lang();
                let selected = languages
                    .iter()
                    .position(|entry| entry.code == current)
                    .unwrap_or(0);
                PopupState::new(
                    select_language,
                    button_rect,
                    languages.iter().map(|entry| entry.name.clone()).collect(),
                    languages.iter().map(|entry| entry.code.clone()).collect(),
                    selected,
                    self.win_w / scale,
                    self.win_h / scale,
                )
            }
            BehaviorAction::UpdateChannel => PopupState::new(
                select_update_channel,
                button_rect,
                vec![tr("channel_stable"), tr("channel_beta")],
                vec!["stable".to_string(), "beta".to_string()],
                usize::from(self.config.update_channel == "beta"),
                self.win_w / scale,
                self.win_h / scale,
            ),
            _ => return,
        };
        self.show_popup(popup);
    }
}

fn step(value: f32, direction: StepDirection, amount: f32, min: f32, max: f32) -> f32 {
    match direction {
        StepDirection::Decrement => value - amount,
        StepDirection::Increment => value + amount,
    }
    .clamp(min, max)
}

fn select_language(app: &mut SettingsApp, value: &str) {
    app.config.language = value.to_string();
    set_lang(value);
    crate::ui::widget::expanded::calendar::clear_calendar_text_cache();
}

fn select_update_channel(app: &mut SettingsApp, value: &str) {
    app.config.update_channel = value.to_string();
}

fn set_hide_delay(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.auto_hide_delay = value.clamp(1.0, 60.0);
    }
}

fn set_hidden_width(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.hidden_width = value.clamp(MIN_HIDDEN_WIDTH, MAX_HIDDEN_WIDTH);
    }
}

fn set_update_interval(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.update_check_interval = value.clamp(1.0, 24.0);
    }
}

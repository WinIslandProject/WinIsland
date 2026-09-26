use crate::utils::settings_ui::ClickResult;
use crate::utils::settings_ui::items::SettingsItem;
use crate::window::settings::PopupState;
use winisland_core::config::{AppConfig, AppConfigField, MAX_HIDDEN_WIDTH};
use winisland_core::i18n::{available_langs, current_lang, init_i18n, set_lang, tr};
use winisland_render::text::FontManager;

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum BehaviorAction {
    AutoStart,
    Language,
    CheckUpdatesNow,
    ResetDefaults,
    HideIsland,
    Exit,
}

impl SettingsApp {
    pub(super) fn build_behavior_page(&self) -> SettingsPage<BehaviorAction> {
        let caps = crate::platform::capabilities();
        let mut page = SettingsPage::new();
        page.section(tr("section_behavior"));
        page.group_start();
        page.row_switch(
            tr("start_boot"),
            self.config.auto_start,
            caps.autostart,
            BehaviorAction::AutoStart,
        );
        page.setting(&self.config, AppConfigField::AutoHide, true);
        page.setting(&self.config, AppConfigField::FullscreenAutoHide, true);
        if self.config.auto_hide {
            page.setting(&self.config, AppConfigField::AutoHideDelay, true);
        }
        page.push_setting(
            SettingsItem::RowStepper {
                label: tr("hidden_width"),
                value: if self.config.hidden_width >= MAX_HIDDEN_WIDTH {
                    tr("hidden_width_off")
                } else {
                    format!("{:.0}", self.config.hidden_width)
                },
                enabled: true,
            },
            AppConfigField::HiddenWidth,
        );
        page.setting(&self.config, AppConfigField::RightClickDrag, true);
        page.setting(
            &self.config,
            AppConfigField::NotificationDisplay,
            caps.toast_events,
        );
        page.setting(
            &self.config,
            AppConfigField::ReplaceNativeVolumeFlyout,
            caps.input_hooks && caps.volume_control,
        );
        page.setting(
            &self.config,
            AppConfigField::BrightnessOverlayEnabled,
            caps.brightness_control,
        );
        if !caps.autostart || !caps.toast_events || !caps.input_hooks || !caps.brightness_control {
            page.row_label(tr("platform_unavailable"));
        }
        if !caps.tray {
            page.row_button(
                tr("tray_hide"),
                tr("tray_hide"),
                true,
                BehaviorAction::HideIsland,
            );
            page.row_button(tr("tray_exit"), tr("tray_exit"), true, BehaviorAction::Exit);
        }

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
        page.setting(&self.config, AppConfigField::CheckForUpdates, true);
        if self.config.check_for_updates {
            page.setting(&self.config, AppConfigField::UpdateChannel, true);
            page.setting(&self.config, AppConfigField::UpdateCheckInterval, true);
        }
        page.row_button(
            tr("check_updates_manual"),
            tr("update_check_btn"),
            true,
            BehaviorAction::CheckUpdatesNow,
        );
        page.group_end();
        page.spacer(10.0);
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
        if self.handle_setting_click(&page, &result, input) {
            return;
        }
        let Some(action) = page.action(&result).copied() else {
            return;
        };

        let changed = match (action, &result) {
            (BehaviorAction::AutoStart, ClickResult::Switch(_)) => {
                let enabled = !self.config.auto_start;
                match crate::platform::shell().set_autostart(enabled) {
                    Ok(()) => {
                        self.config.auto_start = enabled;
                        true
                    }
                    Err(error) => {
                        crate::platform::update_capabilities(|caps| caps.autostart = false);
                        log::warn!("Autostart is unavailable: {error}");
                        false
                    }
                }
            }
            (BehaviorAction::HideIsland, ClickResult::RowButton(_)) => {
                self.plugin_request =
                    Some(crate::window::settings::PluginSettingsRequest::HideIsland);
                false
            }
            (BehaviorAction::Exit, ClickResult::RowButton(_)) => {
                self.plugin_request = Some(crate::window::settings::PluginSettingsRequest::Exit);
                false
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
        let (win_w, win_h) = self.logical_window_size();
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
                    win_w,
                    win_h,
                )
            }
            _ => return,
        };
        self.show_popup(popup);
    }
}

fn select_language(app: &mut SettingsApp, value: &str) {
    app.config.language = value.to_string();
    set_lang(value);
    crate::ui::widget::expanded::calendar::clear_calendar_text_cache();
}

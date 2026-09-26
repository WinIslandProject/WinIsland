use crate::utils::settings_ui::ClickResult;
use crate::window::settings::PopupState;
use winisland_core::config::AppConfigField;
use winisland_core::i18n::tr;
use winisland_render::text::FontManager;

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum EffectsAction {
    IslandStyle,
    CustomFont,
}

impl SettingsApp {
    pub(super) fn build_effects_page(&self) -> SettingsPage<EffectsAction> {
        let mut page = SettingsPage::new();
        page.section(tr("section_effects"));
        page.group_start();
        page.setting(&self.config, AppConfigField::SettingsTheme, true);
        page.setting(&self.config, AppConfigField::MotionBlur, true);
        page.setting(&self.config, AppConfigField::AnimationFps, true);
        page.setting(&self.config, AppConfigField::ExpandedIdleFps, true);
        page.group_end();
        page.group_start();
        let host_backdrop = crate::platform::capabilities().host_backdrop;
        let style = if !host_backdrop && self.config.island_style == "glass" {
            "dynamic"
        } else {
            self.config.island_style.as_str()
        };
        let mut styles = vec![(tr("style_default"), style == "default")];
        if host_backdrop {
            styles.push((tr("style_glass"), style == "glass"));
        }
        styles.push((tr("style_dynamic"), style == "dynamic"));
        page.row_source(tr("island_style"), styles, true, EffectsAction::IslandStyle);
        if !host_backdrop {
            page.row_label(tr("platform_unavailable"));
        }
        page.row_font(
            tr("custom_font"),
            tr("font_select"),
            self.config
                .custom_font_path
                .as_ref()
                .map(|_| tr("font_reset")),
            EffectsAction::CustomFont,
        );
        page.group_end();
        page
    }

    pub(super) fn handle_effects_click(&mut self, input: PageInput) {
        let page = self.build_effects_page();
        let result = input.hit_test(&page);
        if self.handle_setting_click(&page, &result, input) {
            return;
        }
        let Some(action) = page.action(&result).copied() else {
            return;
        };

        let changed = match (action, &result) {
            (EffectsAction::CustomFont, ClickResult::FontSelect(_)) => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Fonts", &["ttf", "otf"])
                    .pick_file()
                else {
                    return;
                };
                self.config.custom_font_path = Some(path.to_string_lossy().into_owned());
                FontManager::global().set_custom_font_path(self.config.custom_font_path.as_deref());
                true
            }
            (EffectsAction::CustomFont, ClickResult::FontReset(_)) => {
                self.config.custom_font_path = None;
                FontManager::global().set_custom_font_path(None);
                true
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
            EffectsAction::IslandStyle => {
                let host_backdrop = crate::platform::capabilities().host_backdrop;
                let mut labels = vec![tr("style_default")];
                let mut values = vec!["default".to_string()];
                if host_backdrop {
                    labels.push(tr("style_glass"));
                    values.push("glass".to_string());
                }
                labels.push(tr("style_dynamic"));
                values.push("dynamic".to_string());
                let selected = values
                    .iter()
                    .position(|value| value == &self.config.island_style)
                    .unwrap_or(values.len() - 1);
                PopupState::new(
                    select_island_style,
                    button_rect,
                    labels,
                    values,
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

fn select_island_style(app: &mut SettingsApp, value: &str) {
    app.config.island_style = if value == "glass" && !crate::platform::capabilities().host_backdrop
    {
        "dynamic".to_string()
    } else {
        value.to_string()
    };
}

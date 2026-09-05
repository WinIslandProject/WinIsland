use crate::core::i18n::tr;
use crate::utils::font::FontManager;
use crate::utils::settings_ui::ClickResult;
use crate::window::settings::PopupState;

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum EffectsAction {
    SettingsTheme,
    MotionBlur,
    IslandStyle,
    CustomFont,
}

impl SettingsApp {
    pub(super) fn build_effects_page(&self) -> SettingsPage<EffectsAction> {
        let mut page = SettingsPage::new();
        page.section(tr("section_effects"));
        page.group_start();
        page.row_source(
            tr("settings_theme"),
            vec![
                (tr("theme_system"), self.config.settings_theme == "system"),
                (tr("theme_light"), self.config.settings_theme == "light"),
                (tr("theme_dark"), self.config.settings_theme == "dark"),
            ],
            true,
            EffectsAction::SettingsTheme,
        );
        page.row_switch(
            tr("motion_blur"),
            self.config.motion_blur,
            true,
            EffectsAction::MotionBlur,
        );
        page.group_end();
        page.group_start();
        page.row_source(
            tr("island_style"),
            vec![
                (tr("style_default"), self.config.island_style == "default"),
                (tr("style_glass"), self.config.island_style == "glass"),
                (tr("style_mica"), self.config.island_style == "mica"),
                (tr("style_dynamic"), self.config.island_style == "dynamic"),
            ],
            true,
            EffectsAction::IslandStyle,
        );
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
        let Some(action) = page.action(&result).copied() else {
            return;
        };

        let changed = match (action, &result) {
            (EffectsAction::MotionBlur, ClickResult::Switch(_)) => {
                self.config.motion_blur = !self.config.motion_blur;
                true
            }
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
            EffectsAction::SettingsTheme => PopupState::new(
                select_theme,
                button_rect,
                vec![tr("theme_system"), tr("theme_light"), tr("theme_dark")],
                vec![
                    "system".to_string(),
                    "light".to_string(),
                    "dark".to_string(),
                ],
                match self.config.settings_theme.as_str() {
                    "light" => 1,
                    "dark" => 2,
                    _ => 0,
                },
                win_w,
                win_h,
            ),
            EffectsAction::IslandStyle => PopupState::new(
                select_island_style,
                button_rect,
                vec![
                    tr("style_default"),
                    tr("style_glass"),
                    tr("style_mica"),
                    tr("style_dynamic"),
                ],
                vec![
                    "default".to_string(),
                    "glass".to_string(),
                    "mica".to_string(),
                    "dynamic".to_string(),
                ],
                match self.config.island_style.as_str() {
                    "glass" => 1,
                    "mica" => 2,
                    "dynamic" => 3,
                    _ => 0,
                },
                win_w,
                win_h,
            ),
            _ => return,
        };
        self.show_popup(popup);
    }
}

fn select_theme(app: &mut SettingsApp, value: &str) {
    app.config.settings_theme = value.to_string();
    app.update_theme();
}

fn select_island_style(app: &mut SettingsApp, value: &str) {
    app.config.island_style = value.to_string();
}

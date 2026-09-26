use crate::utils::settings_ui::ClickResult;
use crate::window::settings::PopupState;
use winisland_core::config::AppConfigField;
use winisland_core::i18n::tr;

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum AppearanceAction {
    Monitor,
}

impl SettingsApp {
    pub(super) fn build_appearance_page(&self) -> SettingsPage<AppearanceAction> {
        let mut page = SettingsPage::new();
        page.section(tr("section_appearance"));
        page.group_start();
        page.setting(&self.config, AppConfigField::CompactScale, true);
        page.setting(&self.config, AppConfigField::ExpandedScale, true);
        page.setting(&self.config, AppConfigField::BaseWidth, true);
        page.setting(&self.config, AppConfigField::BaseHeight, true);
        page.setting(&self.config, AppConfigField::ExpandedWidth, true);
        page.setting(&self.config, AppConfigField::ExpandedHeight, true);
        page.setting(&self.config, AppConfigField::PositionXOffset, true);
        page.setting(&self.config, AppConfigField::PositionYOffset, true);
        page.setting(&self.config, AppConfigField::FontSize, true);

        let monitors = Self::get_monitor_list();
        let selected_monitor =
            (self.config.monitor_index as usize).min(monitors.len().saturating_sub(1));
        page.row_source(
            tr("monitor"),
            monitors
                .into_iter()
                .enumerate()
                .map(|(index, name)| (name, index == selected_monitor))
                .collect(),
            true,
            AppearanceAction::Monitor,
        );
        page.group_end();
        page
    }

    pub(super) fn handle_appearance_click(&mut self, input: PageInput) {
        let page = self.build_appearance_page();
        let result = input.hit_test(&page);
        if self.handle_setting_click(&page, &result, input) {
            return;
        }
        let (Some(AppearanceAction::Monitor), ClickResult::SourceButton(item_index)) =
            (page.action(&result), result)
        else {
            return;
        };
        let button_rect = input.popup_button_rect(&page, item_index, self.scroll_y);
        let (win_w, win_h) = self.logical_window_size();

        let monitors = Self::get_monitor_list();
        let selected = (self.config.monitor_index as usize).min(monitors.len().saturating_sub(1));
        let values = (0..monitors.len()).map(|index| index.to_string()).collect();
        self.show_popup(PopupState::new(
            select_monitor,
            button_rect,
            monitors,
            values,
            selected,
            win_w,
            win_h,
        ));
    }
}

fn select_monitor(app: &mut SettingsApp, value: &str) {
    app.config.monitor_index = value.parse().unwrap_or(0);
}

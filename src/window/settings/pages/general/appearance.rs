use crate::core::i18n::tr;
use crate::utils::settings_ui::{ClickResult, StepDirection};
use crate::window::settings::{NumberInputHandler, PopupState};

use super::super::{PageInput, SettingsPage};
use super::SettingsApp;

#[derive(Clone, Copy)]
pub(super) enum AppearanceAction {
    GlobalScale,
    BaseWidth,
    BaseHeight,
    ExpandedWidth,
    ExpandedHeight,
    PositionX,
    PositionY,
    FontSize,
    Monitor,
}

impl SettingsApp {
    pub(super) fn build_appearance_page(&self) -> SettingsPage<AppearanceAction> {
        let mut page = SettingsPage::new();
        page.section(tr("section_appearance"));
        page.group_start();
        page.row_stepper(
            tr("global_scale"),
            format!("{:.2}", self.config.global_scale),
            true,
            AppearanceAction::GlobalScale,
        );
        page.row_stepper(
            tr("base_width"),
            self.config.base_width.to_string(),
            true,
            AppearanceAction::BaseWidth,
        );
        page.row_stepper(
            tr("base_height"),
            self.config.base_height.to_string(),
            true,
            AppearanceAction::BaseHeight,
        );
        page.row_stepper(
            tr("expanded_width"),
            self.config.expanded_width.to_string(),
            true,
            AppearanceAction::ExpandedWidth,
        );
        page.row_stepper(
            tr("expanded_height"),
            self.config.expanded_height.to_string(),
            true,
            AppearanceAction::ExpandedHeight,
        );
        page.row_stepper(
            tr("position_x_offset"),
            self.config.position_x_offset.to_string(),
            true,
            AppearanceAction::PositionX,
        );
        page.row_stepper(
            tr("position_y_offset"),
            self.config.position_y_offset.to_string(),
            true,
            AppearanceAction::PositionY,
        );
        page.row_stepper(
            tr("font_size"),
            format!("{:.0}", self.config.font_size),
            true,
            AppearanceAction::FontSize,
        );

        let monitors = self.get_monitor_list();
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
        let Some(action) = page.action(&result).copied() else {
            return;
        };

        if let ClickResult::StepperValue(item_index) = &result {
            let (value, on_commit): (String, NumberInputHandler) = match action {
                AppearanceAction::GlobalScale => {
                    (format!("{:.2}", self.config.global_scale), set_global_scale)
                }
                AppearanceAction::BaseWidth => (self.config.base_width.to_string(), set_base_width),
                AppearanceAction::BaseHeight => {
                    (self.config.base_height.to_string(), set_base_height)
                }
                AppearanceAction::ExpandedWidth => {
                    (self.config.expanded_width.to_string(), set_expanded_width)
                }
                AppearanceAction::ExpandedHeight => {
                    (self.config.expanded_height.to_string(), set_expanded_height)
                }
                AppearanceAction::PositionX => {
                    (self.config.position_x_offset.to_string(), set_position_x)
                }
                AppearanceAction::PositionY => {
                    (self.config.position_y_offset.to_string(), set_position_y)
                }
                AppearanceAction::FontSize => {
                    (format!("{:.0}", self.config.font_size), set_font_size)
                }
                AppearanceAction::Monitor => return,
            };
            self.begin_number_input(
                input.stepper_value_rect(&page, *item_index, self.scroll_y),
                value,
                on_commit,
            );
            return;
        }

        if let Some(direction) = result.step_direction() {
            match action {
                AppearanceAction::GlobalScale => {
                    self.config.global_scale =
                        (step(self.config.global_scale, direction, 0.05, 0.5, 5.0) * 100.0).round()
                            / 100.0;
                }
                AppearanceAction::BaseWidth => {
                    self.config.base_width =
                        step(self.config.base_width, direction, 5.0, 40.0, 400.0);
                }
                AppearanceAction::BaseHeight => {
                    self.config.base_height =
                        step(self.config.base_height, direction, 2.0, 15.0, 200.0);
                }
                AppearanceAction::ExpandedWidth => {
                    self.config.expanded_width =
                        step(self.config.expanded_width, direction, 10.0, 200.0, 2000.0);
                }
                AppearanceAction::ExpandedHeight => {
                    self.config.expanded_height =
                        step(self.config.expanded_height, direction, 10.0, 100.0, 1000.0);
                }
                AppearanceAction::PositionX => {
                    self.config.position_x_offset += signed_step(direction, 5);
                }
                AppearanceAction::PositionY => {
                    self.config.position_y_offset += signed_step(direction, 5);
                }
                AppearanceAction::FontSize => {
                    self.config.font_size = step(self.config.font_size, direction, 1.0, 0.0, 30.0);
                }
                AppearanceAction::Monitor => return,
            }
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
            AppearanceAction::Monitor => {
                let monitors = self.get_monitor_list();
                let selected =
                    (self.config.monitor_index as usize).min(monitors.len().saturating_sub(1));
                let values = (0..monitors.len()).map(|index| index.to_string()).collect();
                PopupState::new(
                    select_monitor,
                    button_rect,
                    monitors,
                    values,
                    selected,
                    self.win_w / scale,
                    self.win_h / scale,
                )
            }
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

fn signed_step(direction: StepDirection, amount: i32) -> i32 {
    match direction {
        StepDirection::Decrement => -amount,
        StepDirection::Increment => amount,
    }
}

fn select_monitor(app: &mut SettingsApp, value: &str) {
    app.config.monitor_index = value.parse().unwrap_or(0);
}

fn set_global_scale(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.global_scale = value.clamp(0.5, 5.0);
    }
}

fn set_base_width(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.base_width = value.clamp(40.0, 400.0);
    }
}

fn set_base_height(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.base_height = value.clamp(15.0, 200.0);
    }
}

fn set_expanded_width(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.expanded_width = value.clamp(200.0, 2000.0);
    }
}

fn set_expanded_height(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.expanded_height = value.clamp(100.0, 1000.0);
    }
}

fn set_position_x(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<i32>() {
        app.config.position_x_offset = value;
    }
}

fn set_position_y(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<i32>() {
        app.config.position_y_offset = value;
    }
}

fn set_font_size(app: &mut SettingsApp, value: &str) {
    if let Ok(value) = value.parse::<f32>() {
        app.config.font_size = value.clamp(0.0, 30.0);
    }
}

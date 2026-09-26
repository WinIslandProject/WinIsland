use settings_schema::{Kind, Settings, Value};
use winisland_core::config::{AppConfig, AppConfigField};
use winisland_core::i18n::tr;

use crate::utils::settings_ui::ClickResult;
use crate::utils::settings_ui::items::SettingsItem;
use crate::window::settings::{PopupState, SettingsApp};

use super::{PageInput, SettingsPage};

impl<A> SettingsPage<A> {
    pub(crate) fn setting(&mut self, config: &AppConfig, field: AppConfigField, enabled: bool) {
        let schema = AppConfig::schema(field);
        let label = tr(schema.label);
        let item = match (&schema.kind, config.get(field)) {
            (Kind::Toggle, Value::Bool(on)) => SettingsItem::RowSwitch { label, on, enabled },
            (Kind::Number(number), Value::Number(value)) => SettingsItem::RowStepper {
                label,
                value: number.format(value),
                enabled,
            },
            (Kind::Choice(choices), Value::Text(value)) => SettingsItem::RowSourceSelect {
                label,
                options: choices
                    .iter()
                    .map(|choice| (tr(choice.label), choice.value == value))
                    .collect(),
                enabled,
            },
            _ => return,
        };
        self.push_setting(item, field);
    }
}

impl SettingsApp {
    pub(crate) fn handle_setting_click<A>(
        &mut self,
        page: &SettingsPage<A>,
        result: &ClickResult,
        input: PageInput,
    ) -> bool {
        let Some(field) = page.setting_field(result) else {
            return false;
        };
        match (
            &AppConfig::schema(field).kind,
            self.config.get(field),
            result,
        ) {
            (Kind::Toggle, Value::Bool(on), ClickResult::Switch(_)) => {
                self.update_setting(field, Value::Bool(!on));
            }
            (Kind::Number(number), Value::Number(value), ClickResult::StepperDec(_)) => {
                self.update_setting(field, Value::Number(number.stepped(value, -1)));
            }
            (Kind::Number(number), Value::Number(value), ClickResult::StepperInc(_)) => {
                self.update_setting(field, Value::Number(number.stepped(value, 1)));
            }
            (Kind::Number(number), Value::Number(value), ClickResult::StepperValue(index)) => {
                self.pending_setting = Some(field);
                let rect = input.stepper_value_rect(page, *index, self.scroll_y);
                self.begin_number_input(rect, number.format(value), commit_setting_number);
            }
            (Kind::Choice(choices), Value::Text(value), ClickResult::SourceButton(index)) => {
                self.pending_setting = Some(field);
                let rect = input.popup_button_rect(page, *index, self.scroll_y);
                let (width, height) = self.logical_window_size();
                self.show_popup(PopupState::new(
                    select_setting_choice,
                    rect,
                    choices.iter().map(|choice| tr(choice.label)).collect(),
                    choices
                        .iter()
                        .map(|choice| choice.value.to_string())
                        .collect(),
                    choices
                        .iter()
                        .position(|choice| choice.value == value)
                        .unwrap_or_default(),
                    width,
                    height,
                ));
            }
            _ => {}
        }
        true
    }

    fn update_setting(&mut self, field: AppConfigField, value: Value) {
        if self.apply_setting(field, value) {
            self.persist_settings_change();
        }
    }

    fn apply_setting(&mut self, field: AppConfigField, value: Value) -> bool {
        if !self.config.set(field, value) {
            return false;
        }
        match field {
            AppConfigField::SettingsTheme => self.update_theme(),
            AppConfigField::LyricsMode if self.config.lyrics_mode == "lrc" => {
                self.config.show_lyrics = true;
            }
            _ => {}
        }
        true
    }
}

fn select_setting_choice(app: &mut SettingsApp, value: &str) {
    if let Some(field) = app.pending_setting.take() {
        app.apply_setting(field, Value::Text(value.to_string()));
    }
}

fn commit_setting_number(app: &mut SettingsApp, value: &str) {
    if let Some(field) = app.pending_setting.take()
        && let Ok(value) = value.parse()
    {
        app.apply_setting(field, Value::Number(value));
    }
}

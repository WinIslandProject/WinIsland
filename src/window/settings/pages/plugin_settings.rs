use crate::core::plugin_settings::PluginSettingsItem;
use crate::utils::settings_ui::{ClickResult, StepDirection};
use crate::window::settings::{PendingPluginSetting, PopupState, SettingsApp};

use super::{PageInput, SettingsPage};

#[derive(Clone)]
enum PluginSettingsActionKind {
    Switch(bool),
    Select {
        value: String,
        options: Vec<(String, String)>,
    },
    Stepper {
        value: f64,
        minimum: f64,
        maximum: f64,
        step: f64,
    },
    Button,
}

#[derive(Clone)]
struct PluginSettingsAction {
    resource_id: u64,
    key: String,
    kind: PluginSettingsActionKind,
}

impl SettingsApp {
    fn build_plugin_settings_page(&self) -> SettingsPage<PluginSettingsAction> {
        let mut result = SettingsPage::new();
        let Some(page) = self.active_plugin_settings_page() else {
            return result;
        };
        for item in &page.items {
            match item {
                PluginSettingsItem::Section(label) => result.section(label.clone()),
                PluginSettingsItem::GroupStart => result.group_start(),
                PluginSettingsItem::GroupEnd => result.group_end(),
                PluginSettingsItem::Label(label) => result.row_label(label.clone()),
                PluginSettingsItem::Switch {
                    key,
                    label,
                    value,
                    enabled,
                } => result.row_switch(
                    label.clone(),
                    *value,
                    *enabled,
                    PluginSettingsAction {
                        resource_id: page.resource_id,
                        key: key.clone(),
                        kind: PluginSettingsActionKind::Switch(*value),
                    },
                ),
                PluginSettingsItem::Select {
                    key,
                    label,
                    value,
                    options,
                    enabled,
                } => result.row_source(
                    label.clone(),
                    options
                        .iter()
                        .map(|option| (option.label.clone(), option.value == *value))
                        .collect(),
                    *enabled,
                    PluginSettingsAction {
                        resource_id: page.resource_id,
                        key: key.clone(),
                        kind: PluginSettingsActionKind::Select {
                            value: value.clone(),
                            options: options
                                .iter()
                                .map(|option| (option.label.clone(), option.value.clone()))
                                .collect(),
                        },
                    },
                ),
                PluginSettingsItem::Stepper {
                    key,
                    label,
                    value,
                    minimum,
                    maximum,
                    step,
                    enabled,
                } => result.row_stepper(
                    label.clone(),
                    format_number(*value),
                    *enabled,
                    PluginSettingsAction {
                        resource_id: page.resource_id,
                        key: key.clone(),
                        kind: PluginSettingsActionKind::Stepper {
                            value: *value,
                            minimum: *minimum,
                            maximum: *maximum,
                            step: *step,
                        },
                    },
                ),
                PluginSettingsItem::Button {
                    key,
                    label,
                    button_label,
                    enabled,
                } => result.row_button(
                    label.clone(),
                    button_label.clone(),
                    *enabled,
                    PluginSettingsAction {
                        resource_id: page.resource_id,
                        key: key.clone(),
                        kind: PluginSettingsActionKind::Button,
                    },
                ),
            }
        }
        if self
            .plugin_settings_error
            .as_ref()
            .is_some_and(|(resource_id, _)| *resource_id == page.resource_id)
            && let Some((_, error)) = &self.plugin_settings_error
        {
            result.spacer(8.0);
            result.center_text(error.clone(), 12.0, self.theme().danger);
        }
        result
    }

    pub(crate) fn build_plugin_settings_items(
        &self,
    ) -> Vec<crate::utils::settings_ui::items::SettingsItem> {
        self.build_plugin_settings_page().into_items()
    }

    pub(crate) fn handle_plugin_settings_click(&mut self, input: PageInput) {
        let page = self.build_plugin_settings_page();
        let result = input.hit_test(&page);
        let Some(action) = page.action(&result).cloned() else {
            return;
        };
        match (&action.kind, &result) {
            (PluginSettingsActionKind::Switch(value), ClickResult::Switch(_)) => {
                self.dispatch_plugin_setting(
                    action.resource_id,
                    &action.key,
                    &(!value).to_string(),
                );
            }
            (
                PluginSettingsActionKind::Stepper {
                    value,
                    minimum,
                    maximum,
                    step,
                },
                result,
            ) if result.step_direction().is_some() => {
                let delta = match result.step_direction().unwrap_or(StepDirection::Increment) {
                    StepDirection::Decrement => -*step,
                    StepDirection::Increment => *step,
                };
                let value = (*value + delta).clamp(*minimum, *maximum);
                self.dispatch_plugin_setting(
                    action.resource_id,
                    &action.key,
                    &format_number(value),
                );
            }
            (
                PluginSettingsActionKind::Stepper {
                    value,
                    minimum,
                    maximum,
                    ..
                },
                ClickResult::StepperValue(item_index),
            ) => {
                self.pending_plugin_setting = Some(PendingPluginSetting {
                    resource_id: action.resource_id,
                    key: action.key,
                    minimum: Some(*minimum),
                    maximum: Some(*maximum),
                });
                let rect = input.stepper_value_rect(&page, *item_index, self.scroll_y);
                self.begin_number_input(rect, format_number(*value), apply_plugin_number);
            }
            (
                PluginSettingsActionKind::Select { value, options },
                ClickResult::SourceButton(item_index),
            ) => {
                self.pending_plugin_setting = Some(PendingPluginSetting {
                    resource_id: action.resource_id,
                    key: action.key,
                    minimum: None,
                    maximum: None,
                });
                let selected = options
                    .iter()
                    .position(|(_, option_value)| option_value == value)
                    .unwrap_or(0);
                let rect = input.popup_button_rect(&page, *item_index, self.scroll_y);
                let (width, height) = self.logical_window_size();
                self.show_popup(PopupState::new(
                    apply_plugin_selection,
                    rect,
                    options.iter().map(|(label, _)| label.clone()).collect(),
                    options.iter().map(|(_, value)| value.clone()).collect(),
                    selected,
                    width,
                    height,
                ));
            }
            (PluginSettingsActionKind::Button, ClickResult::RowButton(_)) => {
                self.dispatch_plugin_setting(action.resource_id, &action.key, "");
            }
            _ => {}
        }
    }

    fn dispatch_plugin_setting(&mut self, resource_id: u64, key: &str, value: &str) {
        match crate::plugin::manager::dispatch_settings_change(resource_id, key, value) {
            Ok(()) => {
                self.plugin_settings_error = None;
                self.apply_plugin_setting_value(resource_id, key, value);
            }
            Err(error) => {
                log::warn!("Plugin settings callback failed: {error}");
                self.plugin_settings_error = Some((resource_id, error));
            }
        }
        self.mark_items_dirty();
        self.request_redraw();
    }

    fn apply_plugin_setting_value(&mut self, resource_id: u64, key: &str, value: &str) {
        let Some(page) = self
            .plugin_settings_pages
            .iter_mut()
            .find(|page| page.resource_id == resource_id)
        else {
            return;
        };
        for item in &mut page.items {
            match item {
                PluginSettingsItem::Switch {
                    key: item_key,
                    value: item_value,
                    ..
                } if item_key == key => {
                    if let Ok(value) = value.parse() {
                        *item_value = value;
                    }
                    return;
                }
                PluginSettingsItem::Select {
                    key: item_key,
                    value: item_value,
                    ..
                } if item_key == key => {
                    value.clone_into(item_value);
                    return;
                }
                PluginSettingsItem::Stepper {
                    key: item_key,
                    value: item_value,
                    ..
                } if item_key == key => {
                    if let Ok(value) = value.parse() {
                        *item_value = value;
                    }
                    return;
                }
                _ => {}
            }
        }
    }
}

fn apply_plugin_selection(app: &mut SettingsApp, value: &str) {
    let Some(pending) = app.pending_plugin_setting.take() else {
        return;
    };
    app.dispatch_plugin_setting(pending.resource_id, &pending.key, value);
}

fn apply_plugin_number(app: &mut SettingsApp, value: &str) {
    let Some(pending) = app.pending_plugin_setting.take() else {
        return;
    };
    let Some(minimum) = pending.minimum else {
        return;
    };
    let Some(maximum) = pending.maximum else {
        return;
    };
    let Ok(value) = value.parse::<f64>() else {
        return;
    };
    app.dispatch_plugin_setting(
        pending.resource_id,
        &pending.key,
        &format_number(value.clamp(minimum, maximum)),
    );
}

fn format_number(value: f64) -> String {
    value.to_string()
}

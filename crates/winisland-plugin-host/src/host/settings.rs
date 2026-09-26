use winisland_core::plugin_settings::{
    PluginSettingsItem, PluginSettingsOption, PluginSettingsPage,
};
use winisland_plugin_api::abi::PluginStatus;
use winisland_plugin_api::types::v2::settings::{
    SETTINGS_ITEM_BUTTON, SETTINGS_ITEM_FLAG_DISABLED, SETTINGS_ITEM_GROUP_END,
    SETTINGS_ITEM_GROUP_START, SETTINGS_ITEM_LABEL, SETTINGS_ITEM_SECTION, SETTINGS_ITEM_SELECT,
    SETTINGS_ITEM_STEPPER, SETTINGS_ITEM_SWITCH,
};

use super::PluginHost;
use crate::PluginHostError;
use crate::resources::ResourceKind;

impl PluginHost {
    pub fn settings_pages_snapshot(&self) -> Option<(u64, Vec<PluginSettingsPage>)> {
        let state = self.runtime.state.lock().ok()?;
        let mut pages = state
            .settings
            .iter()
            .map(|(id, page)| {
                let items = page
                    .items
                    .iter()
                    .filter_map(|item| {
                        let raw = &item.raw;
                        let label = fixed(&raw.label);
                        let key = fixed(&raw.key);
                        let value = fixed(&raw.value);
                        let enabled = raw.flags & SETTINGS_ITEM_FLAG_DISABLED == 0;
                        match raw.kind {
                            SETTINGS_ITEM_SECTION => Some(PluginSettingsItem::Section(label)),
                            SETTINGS_ITEM_GROUP_START => Some(PluginSettingsItem::GroupStart),
                            SETTINGS_ITEM_GROUP_END => Some(PluginSettingsItem::GroupEnd),
                            SETTINGS_ITEM_LABEL => Some(PluginSettingsItem::Label(label)),
                            SETTINGS_ITEM_SWITCH => Some(PluginSettingsItem::Switch {
                                key,
                                label,
                                value: value == "true",
                                enabled,
                            }),
                            SETTINGS_ITEM_SELECT => Some(PluginSettingsItem::Select {
                                key,
                                label,
                                value,
                                options: item
                                    .options
                                    .iter()
                                    .map(|option| PluginSettingsOption {
                                        value: fixed(&option.value),
                                        label: fixed(&option.label),
                                    })
                                    .collect(),
                                enabled,
                            }),
                            SETTINGS_ITEM_STEPPER => Some(PluginSettingsItem::Stepper {
                                key,
                                label,
                                value: value.parse().unwrap_or(0.0),
                                minimum: raw.minimum,
                                maximum: raw.maximum,
                                step: raw.step,
                                enabled,
                            }),
                            SETTINGS_ITEM_BUTTON => Some(PluginSettingsItem::Button {
                                key,
                                label,
                                button_label: value,
                                enabled,
                            }),
                            _ => None,
                        }
                    })
                    .collect();
                PluginSettingsPage {
                    resource_id: *id,
                    key: page.key.clone(),
                    title: page.title.clone(),
                    icon: page.icon.clone(),
                    items,
                    sequence: page.sequence,
                }
            })
            .collect::<Vec<_>>();
        pages.sort_by_key(|page| page.sequence);
        Some((state.settings_revision, pages))
    }

    pub fn dispatch_settings_change(
        &self,
        page_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), PluginHostError> {
        let token = self
            .runtime
            .resources
            .owner(ResourceKind::Settings, page_id)
            .map_err(|_| PluginHostError::Invalid("settings page is stale".into()))?;
        let entries = self.entries.borrow();
        let instance = entries
            .iter()
            .find(|entry| entry.token() == token)
            .ok_or_else(|| PluginHostError::Invalid("settings owner is unavailable".into()))?;
        let status = instance.call_settings_change(page_id, key, value)?;
        if status == PluginStatus::Ok {
            Ok(())
        } else {
            Err(PluginHostError::Execution(format!(
                "settings callback returned {status:?}"
            )))
        }
    }
}

fn fixed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.split(|byte| *byte == 0).next().unwrap_or(bytes)).into_owned()
}

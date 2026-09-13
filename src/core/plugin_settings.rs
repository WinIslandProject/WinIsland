use crate::plugin::types::ResourceId;

#[derive(Clone)]
pub struct PluginSettingsOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone)]
pub enum PluginSettingsItem {
    Section(String),
    GroupStart,
    GroupEnd,
    Label(String),
    Switch {
        key: String,
        label: String,
        value: bool,
        enabled: bool,
    },
    Select {
        key: String,
        label: String,
        value: String,
        options: Vec<PluginSettingsOption>,
        enabled: bool,
    },
    Stepper {
        key: String,
        label: String,
        value: f64,
        minimum: f64,
        maximum: f64,
        step: f64,
        enabled: bool,
    },
    Button {
        key: String,
        label: String,
        button_label: String,
        enabled: bool,
    },
}

impl PluginSettingsItem {
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Switch { key, .. }
            | Self::Select { key, .. }
            | Self::Stepper { key, .. }
            | Self::Button { key, .. } => Some(key),
            Self::Section(_) | Self::GroupStart | Self::GroupEnd | Self::Label(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct PluginSettingsPage {
    pub resource_id: ResourceId,
    pub key: String,
    pub title: String,
    pub icon: Vec<u8>,
    pub items: Vec<PluginSettingsItem>,
    pub sequence: u64,
}

use crate::{ByteSliceV1, PluginResultC, ResourceId};

pub const SETTINGS_ITEM_SECTION: u32 = 1;
pub const SETTINGS_ITEM_GROUP_START: u32 = 2;
pub const SETTINGS_ITEM_GROUP_END: u32 = 3;
pub const SETTINGS_ITEM_LABEL: u32 = 4;
pub const SETTINGS_ITEM_SWITCH: u32 = 5;
pub const SETTINGS_ITEM_SELECT: u32 = 6;
pub const SETTINGS_ITEM_STEPPER: u32 = 7;
pub const SETTINGS_ITEM_BUTTON: u32 = 8;

pub const SETTINGS_ITEM_FLAG_DISABLED: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SettingsOptionV1 {
    pub struct_size: u32,
    /// Stable value returned to the plugin. Max 127 bytes plus NUL.
    pub value: [u8; 128],
    /// User-facing option label. Max 255 bytes plus NUL.
    pub label: [u8; 256],
}

impl Default for SettingsOptionV1 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            value: [0; 128],
            label: [0; 256],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SettingsItemV1 {
    pub struct_size: u32,
    /// One of the `SETTINGS_ITEM_*` constants.
    pub kind: u32,
    /// Combination of `SETTINGS_ITEM_FLAG_*` values.
    pub flags: u32,
    /// Stable key for interactive items. Max 63 ASCII bytes plus NUL.
    pub key: [u8; 64],
    /// User-facing section, row, or control label. Max 255 bytes plus NUL.
    pub label: [u8; 256],
    /// Current switch/select/stepper value, or the button label.
    pub value: [u8; 256],
    /// Borrowed options used only by `SETTINGS_ITEM_SELECT`.
    pub options: *const SettingsOptionV1,
    pub option_count: u32,
    /// Numeric bounds used only by `SETTINGS_ITEM_STEPPER`.
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
}

impl Default for SettingsItemV1 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            kind: SETTINGS_ITEM_LABEL,
            flags: 0,
            key: [0; 64],
            label: [0; 256],
            value: [0; 256],
            options: std::ptr::null(),
            option_count: 0,
            minimum: 0.0,
            maximum: 0.0,
            step: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SettingsChangeV1 {
    pub struct_size: u32,
    /// Key of the changed item.
    pub key: [u8; 64],
    /// New value, or an empty string for a button press.
    pub value: [u8; 256],
}

/// Handle a user setting change. Return an error to reject the new value.
pub type SettingsChangedFnV1 = unsafe extern "C" fn(
    callback_data: *mut std::ffi::c_void,
    page_id: ResourceId,
    change: *const SettingsChangeV1,
) -> PluginResultC;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SettingsPageDataV1 {
    pub struct_size: u32,
    /// Stable page key within this plugin. Max 63 ASCII bytes plus NUL.
    pub key: [u8; 64],
    /// Sidebar and page title. Max 127 bytes plus NUL.
    pub title: [u8; 128],
    /// Optional encoded PNG, JPEG, or WebP icon copied by the host.
    pub icon: ByteSliceV1,
    /// Borrowed declarative items copied by the host during create or update.
    pub items: *const SettingsItemV1,
    pub item_count: u32,
    /// Called on the settings UI thread after a user action.
    pub on_change: Option<SettingsChangedFnV1>,
    pub callback_data: *mut std::ffi::c_void,
}

impl Default for SettingsPageDataV1 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            key: [0; 64],
            title: [0; 128],
            icon: ByteSliceV1::empty(),
            items: std::ptr::null(),
            item_count: 0,
            on_change: None,
            callback_data: std::ptr::null_mut(),
        }
    }
}

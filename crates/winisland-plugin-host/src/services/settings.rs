use std::collections::HashSet;
use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_SETTINGS, PluginStatus};
use winisland_plugin_api::types::v2::settings::{
    SETTINGS_ITEM_BUTTON, SETTINGS_ITEM_FLAG_DISABLED, SETTINGS_ITEM_GROUP_END,
    SETTINGS_ITEM_GROUP_START, SETTINGS_ITEM_LABEL, SETTINGS_ITEM_SECTION, SETTINGS_ITEM_SELECT,
    SETTINGS_ITEM_STEPPER, SETTINGS_ITEM_SWITCH, SettingsItemV2, SettingsOptionV2,
    SettingsPageDataV2,
};
use winisland_plugin_api::types::v2::{PluginToken, ResourceId};

use super::{read_bytes, read_struct, runtime};
use crate::resources::ResourceKind;
use crate::runtime::{SettingsItemRecord, SettingsRecord};

fn fixed(value: &[u8]) -> String {
    String::from_utf8_lossy(value.split(|byte| *byte == 0).next().unwrap_or(value)).into_owned()
}

fn key(value: &[u8; 64]) -> Result<String, PluginStatus> {
    let Some(end) = value.iter().position(|byte| *byte == 0) else {
        return Err(PluginStatus::InvalidArgument);
    };
    if end == 0
        || !value[..end]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
    {
        return Err(PluginStatus::InvalidArgument);
    }
    Ok(String::from_utf8_lossy(&value[..end]).into_owned())
}

unsafe fn copy_page(data: SettingsPageDataV2) -> Result<(SettingsRecord, usize), PluginStatus> {
    let page_key = key(&data.key)?;
    let title = fixed(&data.title);
    if title.trim().is_empty()
        || data.items.is_null()
        || data.item_count == 0
        || data.item_count > 64
    {
        return Err(PluginStatus::InvalidArgument);
    }
    // SAFETY: The caller keeps the bounded icon slice readable until return.
    let icon = unsafe { read_bytes(data.icon, 1024 * 1024) }?;
    // SAFETY: The caller keeps the bounded item array readable until return.
    let input_items = unsafe { std::slice::from_raw_parts(data.items, data.item_count as usize) };
    let mut items = Vec::with_capacity(input_items.len());
    let mut keys = HashSet::new();
    let mut group_open = false;
    let mut has_actions = false;
    let mut bytes = page_key.len() + title.len() + icon.len();
    for input in input_items {
        if input.struct_size < std::mem::size_of::<SettingsItemV2>() as u32
            || input.flags & !SETTINGS_ITEM_FLAG_DISABLED != 0
        {
            return Err(PluginStatus::InvalidArgument);
        }
        let label = fixed(&input.label);
        let interactive = match input.kind {
            SETTINGS_ITEM_SECTION => {
                if group_open || label.trim().is_empty() {
                    return Err(PluginStatus::InvalidArgument);
                }
                false
            }
            SETTINGS_ITEM_GROUP_START => {
                if group_open {
                    return Err(PluginStatus::InvalidArgument);
                }
                group_open = true;
                false
            }
            SETTINGS_ITEM_GROUP_END => {
                if !group_open {
                    return Err(PluginStatus::InvalidArgument);
                }
                group_open = false;
                false
            }
            SETTINGS_ITEM_LABEL => {
                if !group_open || label.trim().is_empty() {
                    return Err(PluginStatus::InvalidArgument);
                }
                false
            }
            SETTINGS_ITEM_SWITCH
            | SETTINGS_ITEM_SELECT
            | SETTINGS_ITEM_STEPPER
            | SETTINGS_ITEM_BUTTON => {
                if !group_open || label.trim().is_empty() {
                    return Err(PluginStatus::InvalidArgument);
                }
                let item_key = key(&input.key)?;
                if !keys.insert(item_key) {
                    return Err(PluginStatus::InvalidArgument);
                }
                true
            }
            _ => return Err(PluginStatus::InvalidArgument),
        };
        has_actions |= interactive;
        if input.kind == SETTINGS_ITEM_SWITCH
            && !matches!(fixed(&input.value).as_str(), "true" | "false")
        {
            return Err(PluginStatus::InvalidArgument);
        }
        if input.kind == SETTINGS_ITEM_STEPPER {
            let Ok(value) = fixed(&input.value).parse::<f64>() else {
                return Err(PluginStatus::InvalidArgument);
            };
            if !value.is_finite()
                || !input.minimum.is_finite()
                || !input.maximum.is_finite()
                || !input.step.is_finite()
                || input.minimum > input.maximum
                || input.step <= 0.0
                || !(input.minimum..=input.maximum).contains(&value)
            {
                return Err(PluginStatus::InvalidArgument);
            }
        }
        let options = if input.kind == SETTINGS_ITEM_SELECT {
            if input.options.is_null() || input.option_count == 0 || input.option_count > 64 {
                return Err(PluginStatus::InvalidArgument);
            }
            // SAFETY: The caller keeps the bounded option array readable until return.
            let options =
                unsafe { std::slice::from_raw_parts(input.options, input.option_count as usize) };
            let mut values = HashSet::new();
            let mut copied = Vec::with_capacity(options.len());
            for option in options {
                if option.struct_size < std::mem::size_of::<SettingsOptionV2>() as u32
                    || fixed(&option.label).trim().is_empty()
                {
                    return Err(PluginStatus::InvalidArgument);
                }
                let value = fixed(&option.value);
                if value.is_empty() || !values.insert(value) {
                    return Err(PluginStatus::InvalidArgument);
                }
                bytes = bytes
                    .checked_add(option.value.len() + option.label.len())
                    .ok_or(PluginStatus::LimitExceeded)?;
                copied.push(*option);
            }
            if !copied
                .iter()
                .any(|option| fixed(&option.value) == fixed(&input.value))
            {
                return Err(PluginStatus::InvalidArgument);
            }
            copied
        } else {
            Vec::new()
        };
        bytes = bytes
            .checked_add(input.key.len() + label.len() + input.value.len())
            .ok_or(PluginStatus::LimitExceeded)?;
        if bytes > 2 * 1024 * 1024 {
            return Err(PluginStatus::LimitExceeded);
        }
        let mut raw = *input;
        raw.options = std::ptr::null();
        items.push(SettingsItemRecord { raw, options });
    }
    if group_open || (has_actions && data.on_change.is_none()) {
        return Err(PluginStatus::InvalidArgument);
    }
    Ok((
        SettingsRecord {
            sequence: 0,
            key: page_key,
            title,
            icon,
            items,
            on_change: data.on_change,
            callback_data: data.callback_data as usize,
            in_flight: 0,
        },
        bytes,
    ))
}

pub unsafe extern "C" fn create(
    context: *mut c_void,
    token: PluginToken,
    data: *const SettingsPageDataV2,
    out: *mut ResourceId,
) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: The ABI caller provides a live instance context and sized input.
    let (host, data) = match unsafe {
        runtime(context).and_then(|host| read_struct(data).map(|data| (host, data)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_SETTINGS) {
        return status;
    }
    // SAFETY: Borrowed icon, items, and options are copied before return.
    let (record, bytes) = match unsafe { copy_page(data) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let id = match host
        .resources
        .allocate(token, ResourceKind::Settings, bytes)
    {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Settings, id);
        return PluginStatus::Internal;
    };
    state.settings_revision = state.settings_revision.wrapping_add(1);
    let mut record = record;
    record.sequence = state.settings_revision;
    state.settings.insert(id, record);
    // SAFETY: The ABI caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn update(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
    data: *const SettingsPageDataV2,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context and sized input.
    let (host, data) = match unsafe {
        runtime(context).and_then(|host| read_struct(data).map(|data| (host, data)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_SETTINGS) {
        return status;
    }
    // SAFETY: Borrowed icon, items, and options are copied before return.
    let (record, bytes) = match unsafe { copy_page(data) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(previous) = state.settings.get(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    if previous.in_flight > 0 {
        return PluginStatus::LimitExceeded;
    }
    if previous.key != record.key {
        return PluginStatus::InvalidArgument;
    }
    let sequence = previous.sequence;
    if let Err(status) = host
        .resources
        .resize(token, ResourceKind::Settings, id.get(), bytes)
    {
        return status;
    }
    state.settings_revision = state.settings_revision.wrapping_add(1);
    let mut record = record;
    record.sequence = sequence;
    state.settings.insert(id.get(), record);
    PluginStatus::Ok
}

pub unsafe extern "C" fn release(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_SETTINGS) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if state
        .settings
        .get(&id.get())
        .is_some_and(|record| record.in_flight > 0)
    {
        return PluginStatus::LimitExceeded;
    }
    if let Err(status) = host
        .resources
        .release(token, ResourceKind::Settings, id.get())
    {
        return status;
    }
    state.settings.remove(&id.get());
    state.settings_revision = state.settings_revision.wrapping_add(1);
    PluginStatus::Ok
}

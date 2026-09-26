use std::collections::HashMap;
use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_I18N, PluginStatus};
use winisland_plugin_api::types::v2::i18n::TranslationPairV2;
use winisland_plugin_api::types::v2::{PluginToken, ResourceId, Utf8Slice};

use super::{read_utf8, runtime};
use crate::resources::ResourceKind;
use crate::runtime::TranslationBundle;

pub unsafe extern "C" fn register_bundle(
    context: *mut c_void,
    token: PluginToken,
    locale: Utf8Slice,
    pairs: *const TranslationPairV2,
    count: u32,
    out: *mut ResourceId,
) -> PluginStatus {
    if out.is_null() || pairs.is_null() || count == 0 || count > 4096 {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_I18N) {
        return status;
    }
    // SAFETY: The borrowed locale is bounded and copied before return.
    let locale = match unsafe { read_utf8(locale, 64) } {
        Ok(value) if !value.is_empty() => value,
        _ => return PluginStatus::InvalidArgument,
    };
    // SAFETY: The ABI caller keeps the bounded pair array readable during this call.
    let pairs = unsafe { std::slice::from_raw_parts(pairs, count as usize) };
    let mut entries = HashMap::with_capacity(pairs.len());
    let mut bytes = 0_usize;
    for pair in pairs {
        // SAFETY: Both borrowed strings are copied during this call.
        let key = match unsafe { read_utf8(pair.key, 64 * 1024) } {
            Ok(value) if !value.is_empty() => value,
            _ => return PluginStatus::InvalidArgument,
        };
        // SAFETY: Both borrowed strings are copied during this call.
        let value = match unsafe { read_utf8(pair.value, 64 * 1024) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        bytes = match bytes
            .checked_add(key.len())
            .and_then(|n| n.checked_add(value.len()))
        {
            Some(bytes) if bytes <= 1024 * 1024 => bytes,
            _ => return PluginStatus::LimitExceeded,
        };
        if entries.insert(key, value).is_some() {
            return PluginStatus::InvalidArgument;
        }
    }
    let id = match host.resources.allocate(token, ResourceKind::I18n, bytes) {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::I18n, id);
        return PluginStatus::Internal;
    };
    state.translations.insert(
        id,
        TranslationBundle {
            locale: locale.clone(),
            entries: entries.clone(),
        },
    );
    drop(state);
    if winisland_core::i18n::register_plugin_translation_bundle(
        id,
        locale,
        entries.into_iter().collect(),
    )
    .is_err()
    {
        if let Ok(mut state) = host.state.lock() {
            state.translations.remove(&id);
        }
        let _ = host.resources.release(token, ResourceKind::I18n, id);
        return PluginStatus::Internal;
    }
    // SAFETY: The ABI caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn release_bundle(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_I18N) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if let Err(status) = host.resources.require(token, ResourceKind::I18n, id.get()) {
        return status;
    }
    if winisland_core::i18n::release_plugin_translation_bundle(id.get()).is_err() {
        return PluginStatus::Internal;
    }
    if let Err(status) = host.resources.release(token, ResourceKind::I18n, id.get()) {
        return status;
    }
    state.translations.remove(&id.get());
    PluginStatus::Ok
}

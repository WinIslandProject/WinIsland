use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_LYRICS, PluginStatus};
use winisland_plugin_api::types::v2::lyrics::LyricsTransformerDataV2;
use winisland_plugin_api::types::v2::{PluginToken, ResourceId};

use super::{read_struct, runtime};
use crate::resources::ResourceKind;
use crate::runtime::LyricsRecord;

pub unsafe extern "C" fn register(
    context: *mut c_void,
    token: PluginToken,
    data: *const LyricsTransformerDataV2,
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
    if let Err(status) = host.registry.require(token, CAP_LYRICS) {
        return status;
    }
    if data.flags != 0 {
        return PluginStatus::InvalidArgument;
    }
    let Some(on_transform) = data.on_transform else {
        return PluginStatus::InvalidArgument;
    };
    let id = match host.resources.allocate(token, ResourceKind::Lyrics, 0) {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Lyrics, id);
        return PluginStatus::Internal;
    };
    state.lyrics.insert(
        id,
        LyricsRecord {
            on_transform,
            callback_data: data.callback_data as usize,
            in_flight: 0,
        },
    );
    // SAFETY: The ABI caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
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
    if let Err(status) = host.registry.require(token, CAP_LYRICS) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if state
        .lyrics
        .get(&id.get())
        .is_some_and(|record| record.in_flight > 0)
    {
        return PluginStatus::LimitExceeded;
    }
    if let Err(status) = host
        .resources
        .release(token, ResourceKind::Lyrics, id.get())
    {
        return status;
    }
    state.lyrics.remove(&id.get());
    PluginStatus::Ok
}

use std::ffi::c_void;
use std::time::Instant;

use winisland_plugin_api::abi::{CAP_CONTEXT, PluginStatus};
use winisland_plugin_api::types::v2::context::{CONTEXT_FLAG_SHOW_COMPACT, ContextDataV2};
use winisland_plugin_api::types::v2::{PluginToken, ResourceId};

use super::{read_struct, runtime};
use crate::resources::ResourceKind;
use crate::runtime::ContextRecord;

fn valid(data: &ContextDataV2) -> bool {
    data.priority <= 2
        && data.flags & !CONTEXT_FLAG_SHOW_COMPACT == 0
        && data.title.first().is_some_and(|byte| *byte != 0)
}

pub unsafe extern "C" fn create(
    context: *mut c_void,
    token: PluginToken,
    data: *const ContextDataV2,
    out: *mut ResourceId,
) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: ABI callers provide a live instance context and a readable sized input.
    let (host, data) = match unsafe {
        runtime(context).and_then(|host| read_struct(data).map(|data| (host, data)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_CONTEXT) {
        return status;
    }
    if !valid(&data) {
        return PluginStatus::InvalidArgument;
    }
    let id = match host.resources.allocate(token, ResourceKind::Context, 0) {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Context, id);
        return PluginStatus::Internal;
    };
    state.contexts.insert(
        id,
        ContextRecord {
            data,
            updated_at: Instant::now(),
        },
    );
    state.context_revision = state.context_revision.wrapping_add(1);
    // SAFETY: The caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn update(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
    data: *const ContextDataV2,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context and a readable sized input.
    let (host, data) = match unsafe {
        runtime(context).and_then(|host| read_struct(data).map(|data| (host, data)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_CONTEXT) {
        return status;
    }
    if let Err(status) = host
        .resources
        .require(token, ResourceKind::Context, id.get())
    {
        return status;
    }
    if !valid(&data) {
        return PluginStatus::InvalidArgument;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(record) = state.contexts.get_mut(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    record.data = data;
    record.updated_at = Instant::now();
    state.context_revision = state.context_revision.wrapping_add(1);
    PluginStatus::Ok
}

pub unsafe extern "C" fn release(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_CONTEXT) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if let Err(status) = host
        .resources
        .release(token, ResourceKind::Context, id.get())
    {
        return status;
    }
    state.contexts.remove(&id.get());
    state.context_revision = state.context_revision.wrapping_add(1);
    PluginStatus::Ok
}

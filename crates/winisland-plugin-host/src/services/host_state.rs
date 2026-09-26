use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_HOST_STATE, PluginStatus};
use winisland_plugin_api::types::v2::context::HostStateV2;
use winisland_plugin_api::types::v2::{HostStateChangedFnV2, PluginToken, ResourceId};

use super::runtime;
use crate::resources::ResourceKind;
use crate::runtime::SubscriptionRecord;

pub unsafe extern "C" fn get(
    context: *mut c_void,
    token: PluginToken,
    out: *mut HostStateV2,
) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_HOST_STATE) {
        return status;
    }
    let Ok(state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    // SAFETY: The caller supplied a writable HostStateV2 output pointer.
    unsafe { *out = state.host_state };
    PluginStatus::Ok
}

pub unsafe extern "C" fn subscribe(
    context: *mut c_void,
    token: PluginToken,
    callback: HostStateChangedFnV2,
    callback_data: *mut c_void,
    out: *mut ResourceId,
) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_HOST_STATE) {
        return status;
    }
    let id = match host
        .resources
        .allocate(token, ResourceKind::HostStateSubscription, 0)
    {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host
            .resources
            .release(token, ResourceKind::HostStateSubscription, id);
        return PluginStatus::Internal;
    };
    state.subscriptions.insert(
        id,
        SubscriptionRecord {
            token,
            callback,
            callback_data: callback_data as usize,
            in_flight: 0,
        },
    );
    // SAFETY: The caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn release_subscription(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_HOST_STATE) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if state
        .subscriptions
        .get(&id.get())
        .is_some_and(|record| record.in_flight > 0)
    {
        return PluginStatus::LimitExceeded;
    }
    if let Err(status) =
        host.resources
            .release(token, ResourceKind::HostStateSubscription, id.get())
    {
        return status;
    }
    state.subscriptions.remove(&id.get());
    PluginStatus::Ok
}

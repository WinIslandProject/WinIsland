use std::ffi::c_void;
use std::sync::Arc;

use winisland_plugin_api::abi::{CAP_WIDGET, PluginStatus};
use winisland_plugin_api::draw::v2::MAX_LIST_BYTES;
use winisland_plugin_api::types::v2::widget::WidgetSpecV2;
use winisland_plugin_api::types::v2::{PluginToken, WidgetId};

use super::{read_struct, runtime};
use crate::resources::ResourceKind;
use crate::runtime::WidgetRecord;

fn valid_spec(spec: &WidgetSpecV2) -> bool {
    (1..=4).contains(&spec.span_cols)
        && (1..=4).contains(&spec.span_rows)
        && spec.min_width.is_finite()
        && spec.min_height.is_finite()
        && spec.min_width >= 0.0
        && spec.min_height >= 0.0
}

pub unsafe extern "C" fn create(
    context: *mut c_void,
    token: PluginToken,
    spec: *const WidgetSpecV2,
    out: *mut WidgetId,
) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: ABI callers provide a live instance context and a readable sized spec.
    let (host, spec) = match unsafe {
        runtime(context).and_then(|host| read_struct(spec).map(|spec| (host, spec)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    if !valid_spec(&spec) {
        return PluginStatus::InvalidArgument;
    }
    let id = match host.resources.allocate(token, ResourceKind::Widget, 0) {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Widget, id);
        return PluginStatus::Internal;
    };
    state.widgets.insert(
        id,
        WidgetRecord {
            spec,
            draw_list: Arc::from([]),
            logical_width: spec.min_width.max(spec.span_cols as f32 * 60.0),
            logical_height: spec.min_height.max(spec.span_rows as f32 * 48.0),
            redraw: true,
            failures: 0,
            disabled: false,
        },
    );
    // SAFETY: The caller supplied a writable WidgetId output pointer.
    unsafe { *out = WidgetId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn update(
    context: *mut c_void,
    token: PluginToken,
    id: WidgetId,
    spec: *const WidgetSpecV2,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context and a readable sized spec.
    let (host, spec) = match unsafe {
        runtime(context).and_then(|host| read_struct(spec).map(|spec| (host, spec)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    if !valid_spec(&spec) {
        return PluginStatus::InvalidArgument;
    }
    if let Err(status) = host
        .resources
        .require(token, ResourceKind::Widget, id.get())
    {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(record) = state.widgets.get_mut(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    if record.disabled {
        return PluginStatus::StaleHandle;
    }
    record.spec = spec;
    record.redraw = true;
    PluginStatus::Ok
}

pub unsafe extern "C" fn release(
    context: *mut c_void,
    token: PluginToken,
    id: WidgetId,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if let Err(status) = host
        .resources
        .release(token, ResourceKind::Widget, id.get())
    {
        return status;
    }
    state.widgets.remove(&id.get());
    PluginStatus::Ok
}

pub unsafe extern "C" fn submit_draw_list(
    context: *mut c_void,
    token: PluginToken,
    id: WidgetId,
    data: *const u8,
    len: u32,
) -> PluginStatus {
    if data.is_null() {
        return PluginStatus::InvalidArgument;
    }
    if len as usize > MAX_LIST_BYTES {
        return PluginStatus::LimitExceeded;
    }
    // SAFETY: ABI callers provide a live instance context and `len` readable data bytes.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    if let Err(status) = host
        .resources
        .require(token, ResourceKind::Widget, id.get())
    {
        return status;
    }
    // SAFETY: The caller keeps the borrowed list readable until this call returns.
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(record) = state.widgets.get_mut(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    if record.disabled {
        return PluginStatus::StaleHandle;
    }
    record.draw_list = Arc::from(bytes);
    record.redraw = true;
    PluginStatus::Ok
}

pub unsafe extern "C" fn request_redraw(
    context: *mut c_void,
    token: PluginToken,
    id: WidgetId,
) -> PluginStatus {
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    if let Err(status) = host
        .resources
        .require(token, ResourceKind::Widget, id.get())
    {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(record) = state.widgets.get_mut(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    if record.disabled {
        return PluginStatus::StaleHandle;
    }
    record.redraw = true;
    PluginStatus::Ok
}

pub unsafe extern "C" fn logical_size(
    context: *mut c_void,
    token: PluginToken,
    id: WidgetId,
    out_width: *mut f32,
    out_height: *mut f32,
) -> PluginStatus {
    if out_width.is_null() || out_height.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: ABI callers provide a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_WIDGET) {
        return status;
    }
    if let Err(status) = host
        .resources
        .require(token, ResourceKind::Widget, id.get())
    {
        return status;
    }
    let Ok(state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let Some(record) = state.widgets.get(&id.get()) else {
        return PluginStatus::StaleHandle;
    };
    // SAFETY: The caller supplied two writable float pointers.
    unsafe {
        *out_width = record.logical_width;
        *out_height = record.logical_height;
    }
    PluginStatus::Ok
}

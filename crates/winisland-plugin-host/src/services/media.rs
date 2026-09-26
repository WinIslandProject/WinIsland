use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_MEDIA, PluginStatus};
use winisland_plugin_api::types::v2::context::{
    MEDIA_CONTROL_NEXT, MEDIA_CONTROL_PREVIOUS, MEDIA_CONTROL_SEEK, MEDIA_CONTROL_TOGGLE_PLAY,
    MEDIA_FLAG_PLAYING, MediaSourceDataV2,
};
use winisland_plugin_api::types::v2::{PluginToken, ResourceId};

use super::{read_bytes, read_struct, runtime, write_buffer};
use crate::resources::ResourceKind;
use crate::runtime::MediaRecord;

fn fixed_text(value: &[u8]) -> String {
    String::from_utf8_lossy(value.split(|byte| *byte == 0).next().unwrap_or(value)).into_owned()
}

unsafe fn copy_media(data: &MediaSourceDataV2) -> Result<MediaRecord, PluginStatus> {
    let title = fixed_text(&data.title);
    let controls = MEDIA_CONTROL_NEXT
        | MEDIA_CONTROL_PREVIOUS
        | MEDIA_CONTROL_SEEK
        | MEDIA_CONTROL_TOGGLE_PLAY;
    if title.is_empty()
        || data.flags & !MEDIA_FLAG_PLAYING != 0
        || data.available_controls & !controls != 0
        || (data.available_controls != 0 && data.on_command.is_none())
    {
        return Err(PluginStatus::InvalidArgument);
    }
    // SAFETY: The caller keeps the bounded cover slice readable until return.
    let cover = unsafe { read_bytes(data.cover, 16 * 1024 * 1024) }?;
    Ok(MediaRecord {
        sequence: 0,
        in_flight: 0,
        title,
        artist: fixed_text(&data.artist),
        album: fixed_text(&data.album),
        flags: data.flags,
        duration_ms: data.duration_ms,
        position_ms: data.position_ms,
        available_controls: data.available_controls,
        cover,
        on_command: data.on_command,
        callback_data: data.callback_data as usize,
    })
}

pub unsafe extern "C" fn create(
    context: *mut c_void,
    token: PluginToken,
    data: *const MediaSourceDataV2,
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
    if let Err(status) = host.registry.require(token, CAP_MEDIA) {
        return status;
    }
    // SAFETY: The borrowed cover is copied before returning.
    let record = match unsafe { copy_media(&data) } {
        Ok(record) => record,
        Err(status) => return status,
    };
    let id = match host
        .resources
        .allocate(token, ResourceKind::Media, record.cover.len())
    {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Media, id);
        return PluginStatus::Internal;
    };
    state.media_revision = state.media_revision.wrapping_add(1);
    let mut record = record;
    record.sequence = state.media_revision;
    state.media.insert(id, record);
    // SAFETY: The ABI caller supplied a writable ResourceId output pointer.
    unsafe { *out = ResourceId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn update(
    context: *mut c_void,
    token: PluginToken,
    id: ResourceId,
    data: *const MediaSourceDataV2,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context and sized input.
    let (host, data) = match unsafe {
        runtime(context).and_then(|host| read_struct(data).map(|data| (host, data)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_MEDIA) {
        return status;
    }
    // SAFETY: The borrowed cover is copied before returning.
    let record = match unsafe { copy_media(&data) } {
        Ok(record) => record,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if !state.media.contains_key(&id.get()) {
        return PluginStatus::StaleHandle;
    }
    if state
        .media
        .get(&id.get())
        .is_some_and(|media| media.in_flight > 0)
    {
        return PluginStatus::LimitExceeded;
    }
    if let Err(status) =
        host.resources
            .resize(token, ResourceKind::Media, id.get(), record.cover.len())
    {
        return status;
    }
    state.media_revision = state.media_revision.wrapping_add(1);
    let mut record = record;
    record.sequence = state.media_revision;
    state.media.insert(id.get(), record);
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
    if let Err(status) = host.registry.require(token, CAP_MEDIA) {
        return status;
    }
    let Ok(mut state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    if state
        .media
        .get(&id.get())
        .is_some_and(|media| media.in_flight > 0)
    {
        return PluginStatus::LimitExceeded;
    }
    if let Err(status) = host.resources.release(token, ResourceKind::Media, id.get()) {
        return status;
    }
    state.media.remove(&id.get());
    state.media_revision = state.media_revision.wrapping_add(1);
    PluginStatus::Ok
}

pub unsafe extern "C" fn current_title(
    context: *mut c_void,
    token: PluginToken,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_MEDIA) {
        return status;
    }
    let Ok(state) = host.state.lock() else {
        return PluginStatus::Internal;
    };
    let title = state
        .host_state
        .media_title
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or(&[]);
    // SAFETY: The helper checks output pointers and capacity before writing.
    unsafe { write_buffer(title, buffer, capacity, required) }
}

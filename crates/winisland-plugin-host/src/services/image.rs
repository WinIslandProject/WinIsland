use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_IMAGE, PluginStatus};
use winisland_plugin_api::types::v2::{ByteSlice, ImageId, PluginToken};
use winisland_render::Image;

use super::{read_bytes, runtime};
use crate::resources::ResourceKind;
use crate::runtime::HostRuntime;

fn insert(host: &HostRuntime, token: PluginToken, image: Image, out: *mut ImageId) -> PluginStatus {
    if out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    let (width, height) = image.dimensions();
    let Some(bytes) = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return PluginStatus::LimitExceeded;
    };
    let id = match host.resources.allocate(token, ResourceKind::Image, bytes) {
        Ok(id) => id,
        Err(status) => return status,
    };
    let Ok(mut state) = host.state.lock() else {
        let _ = host.resources.release(token, ResourceKind::Image, id);
        return PluginStatus::Internal;
    };
    state.images.insert(id, image);
    // SAFETY: The ABI caller supplies a writable ImageId output pointer.
    unsafe { *out = ImageId::from_raw(id) };
    PluginStatus::Ok
}

pub unsafe extern "C" fn decode(
    context: *mut c_void,
    token: PluginToken,
    encoded: ByteSlice,
    out: *mut ImageId,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_IMAGE) {
        return status;
    }
    let encoded = match unsafe { read_bytes(encoded, 16 * 1024 * 1024) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(image) = Image::decode(&encoded) else {
        return PluginStatus::InvalidArgument;
    };
    insert(host, token, image, out)
}

pub unsafe extern "C" fn upload_rgba(
    context: *mut c_void,
    token: PluginToken,
    width: u32,
    height: u32,
    rgba: ByteSlice,
    out: *mut ImageId,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_IMAGE) {
        return status;
    }
    let Some(expected) = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return PluginStatus::InvalidArgument;
    };
    if width == 0 || height == 0 || width > 4096 || height > 4096 || expected != rgba.len as usize {
        return PluginStatus::InvalidArgument;
    }
    let rgba = match unsafe { read_bytes(rgba, 64 * 1024 * 1024) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(image) = Image::from_rgba8(width as i32, height as i32, &rgba) else {
        return PluginStatus::InvalidArgument;
    };
    insert(host, token, image, out)
}

pub unsafe extern "C" fn album_art(
    context: *mut c_void,
    token: PluginToken,
    out: *mut ImageId,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_IMAGE) {
        return status;
    }
    let image = match host.state.lock() {
        Ok(state) => state.album_art.clone(),
        Err(_) => return PluginStatus::Internal,
    };
    let Some(image) = image else {
        return PluginStatus::IoError;
    };
    insert(host, token, image, out)
}

pub unsafe extern "C" fn release(
    context: *mut c_void,
    token: PluginToken,
    image: ImageId,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_IMAGE) {
        return status;
    }
    if let Err(status) = host
        .resources
        .release(token, ResourceKind::Image, image.get())
    {
        return status;
    }
    match host.state.lock() {
        Ok(mut state) => {
            state.images.remove(&image.get());
            PluginStatus::Ok
        }
        Err(_) => PluginStatus::Internal,
    }
}

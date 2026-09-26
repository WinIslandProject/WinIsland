use std::ffi::c_void;
use std::io::Read;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use winisland_plugin_api::abi::{CAP_STORE, PluginStatus};
use winisland_plugin_api::types::v2::{ByteSlice, PluginToken, Utf8Slice};

use super::{read_bytes, read_utf8, runtime, write_buffer};
use crate::runtime::HostRuntime;

const MAX_VALUE: usize = 1024 * 1024;

fn path(host: &HostRuntime, token: PluginToken, key: &str) -> Result<PathBuf, PluginStatus> {
    let plugin = host.registry.get(token)?;
    if plugin.stopping {
        return Err(PluginStatus::StaleHandle);
    }
    let digest = Sha256::digest(key.as_bytes());
    let mut name = String::with_capacity(68);
    for byte in digest {
        use std::fmt::Write;
        write!(name, "{byte:02x}").map_err(|_| PluginStatus::Internal)?;
    }
    name.push_str(".bin");
    Ok(host.store_root.join(plugin.id).join("store").join(name))
}

fn key(value: Utf8Slice) -> Result<String, PluginStatus> {
    let key = unsafe { read_utf8(value, 255) }?;
    if key.is_empty() {
        return Err(PluginStatus::InvalidArgument);
    }
    Ok(key)
}

pub unsafe extern "C" fn get(
    context: *mut c_void,
    token: PluginToken,
    raw_key: Utf8Slice,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
    found: *mut u8,
) -> PluginStatus {
    if required.is_null() || found.is_null() {
        return PluginStatus::InvalidArgument;
    }
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_STORE) {
        return status;
    }
    let key = match key(raw_key) {
        Ok(key) => key,
        Err(status) => return status,
    };
    let path = match path(host, token, &key) {
        Ok(path) => path,
        Err(status) => return status,
    };
    let Ok(_guard) = host.store_lock.lock() else {
        return PluginStatus::Internal;
    };
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            unsafe {
                *required = 0;
                *found = 0;
            }
            return PluginStatus::Ok;
        }
        Err(_) => return PluginStatus::IoError,
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_VALUE as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return PluginStatus::IoError;
    }
    if bytes.len() > MAX_VALUE {
        return PluginStatus::LimitExceeded;
    }
    unsafe { *found = 1 };
    unsafe { write_buffer(&bytes, buffer, capacity, required) }
}

pub unsafe extern "C" fn set(
    context: *mut c_void,
    token: PluginToken,
    raw_key: Utf8Slice,
    value: ByteSlice,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_STORE) {
        return status;
    }
    let key = match key(raw_key) {
        Ok(key) => key,
        Err(status) => return status,
    };
    let value = match unsafe { read_bytes(value, MAX_VALUE) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let path = match path(host, token, &key) {
        Ok(path) => path,
        Err(status) => return status,
    };
    let Ok(_guard) = host.store_lock.lock() else {
        return PluginStatus::Internal;
    };
    let Some(parent) = path.parent() else {
        return PluginStatus::Internal;
    };
    if std::fs::create_dir_all(parent).is_err() || std::fs::write(path, value).is_err() {
        return PluginStatus::IoError;
    }
    PluginStatus::Ok
}

pub unsafe extern "C" fn delete(
    context: *mut c_void,
    token: PluginToken,
    raw_key: Utf8Slice,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_STORE) {
        return status;
    }
    let key = match key(raw_key) {
        Ok(key) => key,
        Err(status) => return status,
    };
    let path = match path(host, token, &key) {
        Ok(path) => path,
        Err(status) => return status,
    };
    let Ok(_guard) = host.store_lock.lock() else {
        return PluginStatus::Internal;
    };
    match std::fs::remove_file(path) {
        Ok(()) => PluginStatus::Ok,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PluginStatus::Ok,
        Err(_) => PluginStatus::IoError,
    }
}

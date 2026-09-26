pub mod context;
pub mod host_state;
pub mod i18n;
pub mod image;
pub mod log;
pub mod lyrics;
pub mod media;
pub mod settings;
pub mod store;
pub mod text;
pub mod widget;

use std::ffi::c_void;

use winisland_plugin_api::abi::PluginStatus;
use winisland_plugin_api::types::v2::{ByteSlice, Utf8Slice};

use crate::runtime::HostRuntime;

pub(crate) unsafe fn runtime<'a>(context: *mut c_void) -> Result<&'a HostRuntime, PluginStatus> {
    if context.is_null() {
        return Err(PluginStatus::InvalidArgument);
    }
    // SAFETY: Every service table is bound to a live Box<HostRuntime> address.
    Ok(unsafe { &*context.cast::<HostRuntime>() })
}

pub(crate) unsafe fn read_struct<T: Copy>(value: *const T) -> Result<T, PluginStatus> {
    if value.is_null() {
        return Err(PluginStatus::InvalidArgument);
    }
    // SAFETY: The plugin promises a readable ABI struct header.
    let size = unsafe { std::ptr::read_unaligned(value.cast::<u32>()) };
    if size < std::mem::size_of::<T>() as u32 {
        return Err(PluginStatus::InvalidArgument);
    }
    // SAFETY: The validated struct header covers the current ABI layout.
    Ok(unsafe { std::ptr::read_unaligned(value) })
}

pub(crate) unsafe fn read_bytes(value: ByteSlice, max_len: usize) -> Result<Vec<u8>, PluginStatus> {
    if value.len as usize > max_len || (value.ptr.is_null() && value.len != 0) {
        return Err(PluginStatus::InvalidArgument);
    }
    if value.len == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: The plugin keeps this bounded borrowed slice live until return.
    Ok(unsafe { std::slice::from_raw_parts(value.ptr, value.len as usize) }.to_vec())
}

pub(crate) unsafe fn read_utf8(value: Utf8Slice, max_len: usize) -> Result<String, PluginStatus> {
    let bytes = unsafe {
        read_bytes(
            ByteSlice {
                ptr: value.ptr,
                len: value.len,
            },
            max_len,
        )
    }?;
    String::from_utf8(bytes).map_err(|_| PluginStatus::InvalidArgument)
}

pub(crate) unsafe fn write_buffer(
    value: &[u8],
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> PluginStatus {
    if required.is_null() {
        return PluginStatus::InvalidArgument;
    }
    let Ok(len) = u32::try_from(value.len()) else {
        return PluginStatus::LimitExceeded;
    };
    // SAFETY: The caller supplies a writable required-length pointer.
    unsafe { *required = len };
    if capacity < len {
        return PluginStatus::LimitExceeded;
    }
    if len != 0 {
        if buffer.is_null() {
            return PluginStatus::InvalidArgument;
        }
        // SAFETY: The caller supplies at least `capacity` writable bytes.
        unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), buffer, len as usize) };
    }
    PluginStatus::Ok
}

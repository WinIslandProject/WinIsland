use std::ffi::c_void;

use winisland_plugin_api::abi::{CAP_TEXT, PluginStatus};
use winisland_plugin_api::types::v2::{PluginToken, TextMetricsV2, TextStyleV2, Utf8Slice};
use winisland_render::text::FontManager;

use super::{read_utf8, runtime, write_buffer};

pub unsafe extern "C" fn measure(
    context: *mut c_void,
    token: PluginToken,
    text: Utf8Slice,
    style: *const TextStyleV2,
    out: *mut TextMetricsV2,
) -> PluginStatus {
    if style.is_null() || out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_TEXT) {
        return status;
    }
    // SAFETY: TextStyleV2 has a fixed layout and the plugin keeps it readable during this call.
    let style = unsafe { std::ptr::read_unaligned(style) };
    if !style.size.is_finite()
        || style.size <= 0.0
        || style.size > 10_000.0
        || !(100..=900).contains(&style.weight)
        || style.italic > 1
        || style.reserved != 0
    {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: Borrowed UTF-8 slices are bounded and copied before font measurement.
    let (text, family) = match unsafe {
        read_utf8(text, 64 * 1024)
            .and_then(|text| read_utf8(style.family, 255).map(|family| (text, family)))
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let Some(metrics) = FontManager::global().measure_plugin_text(
        &text,
        style.size,
        style.weight,
        style.italic != 0,
        &family,
    ) else {
        return PluginStatus::Internal;
    };
    // SAFETY: The ABI caller supplied a writable output metrics pointer.
    unsafe {
        *out = TextMetricsV2 {
            width: metrics.width,
            height: metrics.height,
            ascent: metrics.ascent,
            descent: metrics.descent,
        }
    };
    PluginStatus::Ok
}

pub unsafe extern "C" fn font_family(
    context: *mut c_void,
    token: PluginToken,
    index: u32,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> PluginStatus {
    // SAFETY: The ABI caller provides a live instance context.
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    if let Err(status) = host.registry.require(token, CAP_TEXT) {
        return status;
    }
    let Some(family) = FontManager::global().plugin_font_family(index) else {
        return PluginStatus::InvalidArgument;
    };
    // SAFETY: The helper checks output pointers and capacity before writing.
    unsafe { write_buffer(family.as_bytes(), buffer, capacity, required) }
}

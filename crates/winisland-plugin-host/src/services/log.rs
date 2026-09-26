use std::ffi::c_void;

use winisland_plugin_api::abi::PluginStatus;
use winisland_plugin_api::types::v2::{PluginToken, Utf8Slice};

use super::{read_utf8, runtime};

pub unsafe extern "C" fn write(
    context: *mut c_void,
    token: PluginToken,
    level: u32,
    message: Utf8Slice,
) -> PluginStatus {
    let host = match unsafe { runtime(context) } {
        Ok(host) => host,
        Err(status) => return status,
    };
    let plugin = match host.registry.get(token) {
        Ok(plugin) if !plugin.stopping => plugin,
        Ok(_) => return PluginStatus::StaleHandle,
        Err(status) => return status,
    };
    let message = match unsafe { read_utf8(message, 64 * 1024) } {
        Ok(message) => message,
        Err(status) => return status,
    };
    let level = match level {
        0 => log::Level::Error,
        1 => log::Level::Warn,
        2 => log::Level::Info,
        3 => log::Level::Debug,
        4 => log::Level::Trace,
        _ => return PluginStatus::InvalidArgument,
    };
    log::log!(target: "winisland::plugin", level, "[{} {}] {}", plugin.id, plugin.version, message);
    PluginStatus::Ok
}

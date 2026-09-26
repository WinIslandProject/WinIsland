use std::env;

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_registry::CURRENT_USER;
use winisland_platform::PlatformError;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "WinIsland";

pub(super) fn enabled() -> Result<bool, PlatformError> {
    let key = CURRENT_USER
        .open(RUN_KEY)
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
    Ok(key.get_type(VALUE_NAME).is_ok())
}

pub(super) fn set(enabled: bool) -> Result<(), PlatformError> {
    let key = CURRENT_USER
        .create(RUN_KEY)
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
    let result = if enabled {
        let exe = env::current_exe().map_err(|error| PlatformError::Backend(error.to_string()))?;
        key.set_string(VALUE_NAME, format!("\"{}\"", exe.display()))
    } else {
        key.remove_value(VALUE_NAME)
    };
    match result {
        Err(error) if !enabled && error.code() == ERROR_FILE_NOT_FOUND.to_hresult() => Ok(()),
        result => result.map_err(|error| PlatformError::Backend(error.to_string())),
    }
}

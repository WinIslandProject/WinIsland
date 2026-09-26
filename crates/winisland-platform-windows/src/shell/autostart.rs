use std::env;

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::w;
use winisland_platform::PlatformError;

pub(super) fn enabled() -> Result<bool, PlatformError> {
    let mut key = HKEY::default();
    // SAFETY: Static NUL-terminated registry names and a valid output handle are supplied.
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            KEY_READ,
            &mut key,
        )
        .ok()
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let result = RegQueryValueExW(key, w!("WinIsland"), None, None, None, None).is_ok();
        let _ = RegCloseKey(key);
        Ok(result)
    }
}

pub(super) fn set(enabled: bool) -> Result<(), PlatformError> {
    let mut key = HKEY::default();
    // SAFETY: Static NUL-terminated registry names and an initialized output handle are supplied.
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let result = if enabled {
            let exe =
                env::current_exe().map_err(|error| PlatformError::Backend(error.to_string()))?;
            let quoted = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = quoted.encode_utf16().chain(Some(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2);
            RegSetValueExW(key, w!("WinIsland"), Some(0), REG_SZ, Some(bytes))
        } else {
            RegDeleteValueW(key, w!("WinIsland"))
        };
        let _ = RegCloseKey(key);
        if !enabled && result == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        result
            .ok()
            .map_err(|error| PlatformError::Backend(error.to_string()))
    }
}

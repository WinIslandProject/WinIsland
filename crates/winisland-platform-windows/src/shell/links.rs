use std::path::Path;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;
use winisland_platform::PlatformError;

fn open(target: &str) -> Result<(), PlatformError> {
    let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The UTF-16 target is NUL-terminated and remains live for the synchronous call.
    let result = unsafe {
        ShellExecuteW(
            None,
            None,
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(PlatformError::Backend(format!(
            "ShellExecuteW returned {}",
            result.0 as isize
        )))
    }
}

pub(super) fn open_url(url: &str) -> Result<(), PlatformError> {
    open(url)
}

pub(super) fn reveal_path(path: &Path) -> Result<(), PlatformError> {
    open(&path.to_string_lossy())
}

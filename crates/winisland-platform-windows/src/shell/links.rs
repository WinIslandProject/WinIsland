use std::path::Path;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::HSTRING;
use winisland_platform::PlatformError;

fn open(target: &HSTRING) -> Result<(), PlatformError> {
    // SAFETY: The target is NUL-terminated and remains live for the synchronous call.
    let result = unsafe { ShellExecuteW(None, None, target, None, None, SW_SHOWNORMAL) };
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
    open(&url.into())
}

pub(super) fn reveal_path(path: &Path) -> Result<(), PlatformError> {
    open(&path.into())
}

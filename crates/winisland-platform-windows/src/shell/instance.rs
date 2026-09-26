use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;
use winisland_platform::{InstanceLock, PlatformError};

struct WindowsInstanceLock(HANDLE);

impl InstanceLock for WindowsInstanceLock {}

impl Drop for WindowsInstanceLock {
    fn drop(&mut self) {
        // SAFETY: This is the one owned handle returned by CreateMutexW.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn acquire(key: &str) -> Result<Option<Box<dyn InstanceLock>>, PlatformError> {
    let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The key is NUL-terminated and lives for the entire call.
    let handle = unsafe { CreateMutexW(None, true, PCWSTR(key.as_ptr())) }
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
    // SAFETY: GetLastError is read on the same thread immediately after CreateMutexW.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // SAFETY: This handle is owned but not retained when the mutex already exists.
        let _ = unsafe { CloseHandle(handle) };
        Ok(None)
    } else {
        Ok(Some(Box::new(WindowsInstanceLock(handle))))
    }
}

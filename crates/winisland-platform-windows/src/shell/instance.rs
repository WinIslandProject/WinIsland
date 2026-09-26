use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::{HSTRING, Owned};
use winisland_platform::{InstanceLock, PlatformError};

struct WindowsInstanceLock {
    _mutex: Owned<HANDLE>,
}

impl InstanceLock for WindowsInstanceLock {}

pub(super) fn acquire(key: &str) -> Result<Option<Box<dyn InstanceLock>>, PlatformError> {
    let key = HSTRING::from(key);
    // SAFETY: The key outlives the call, and the new mutex handle is owned by the guard.
    let mutex = unsafe { CreateMutexW(None, true, &key).map(|mutex| Owned::new(mutex)) }
        .map_err(|error| PlatformError::Backend(error.to_string()))?;
    // SAFETY: GetLastError is read on the same thread immediately after CreateMutexW.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        Ok(None)
    } else {
        Ok(Some(Box::new(WindowsInstanceLock { _mutex: mutex })))
    }
}

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HANDLE};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::core::{Owned, PWSTR};

pub(crate) fn open(process_id: u32) -> Option<Owned<HANDLE>> {
    // SAFETY: The requested access only queries identity, and the new handle is owned by the guard.
    unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .ok()
            .map(|process| Owned::new(process))
    }
}

pub(crate) fn app_user_model_id(process: HANDLE) -> Option<String> {
    let mut length = 0;
    // SAFETY: The first call only queries the required buffer length.
    if unsafe { GetApplicationUserModelId(process, &mut length, None) } != ERROR_INSUFFICIENT_BUFFER
        || length == 0
    {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: The buffer has the length reported by the first call.
    if unsafe { GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) }
        != ERROR_SUCCESS
    {
        return None;
    }
    buffer.truncate(length.saturating_sub(1) as usize);
    String::from_utf16(&buffer).ok()
}

use std::path::Path;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HANDLE, HWND, LPARAM,
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{
    ACTIVATEOPTIONS, ApplicationActivationManager, IApplicationActivationManager,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use windows::core::{BOOL, PCWSTR, PWSTR};
use winisland_platform::PlatformError;

use crate::com::ComGuard;

pub(super) fn application(app_user_model_id: &str) -> Result<bool, PlatformError> {
    if app_user_model_id.is_empty() {
        return Ok(false);
    }
    let app_id = app_user_model_id.to_owned();
    std::thread::spawn(move || activate_on_sta(&app_id))
        .join()
        .map_err(|_| PlatformError::Backend("application activation thread panicked".into()))?
}

fn activate_on_sta(app_user_model_id: &str) -> Result<bool, PlatformError> {
    let _com = ComGuard::sta()?;
    let app_id: Vec<u16> = app_user_model_id.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The COM apartment is initialized here; both UTF-16 buffers live through the call.
    let result = unsafe {
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                .map_err(|error| PlatformError::Backend(error.to_string()))?;
        manager.ActivateApplication(
            PCWSTR(app_id.as_ptr()),
            PCWSTR::null(),
            ACTIVATEOPTIONS::default(),
        )
    };
    match result {
        Ok(process_id) => {
            log::info!("Application activated: {app_user_model_id} (process {process_id})");
            Ok(true)
        }
        Err(error) => {
            log::debug!("Application could not be activated: {error:?}");
            Ok(false)
        }
    }
}

struct MediaWindowSearch {
    source_app_id: String,
    executable_name: String,
    window: Option<HWND>,
}

pub(super) fn media_application(source_app_id: &str) -> Result<bool, PlatformError> {
    if source_app_id.is_empty() {
        return Ok(false);
    }
    let executable_name = Path::new(source_app_id)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(source_app_id)
        .to_string();
    let mut search = MediaWindowSearch {
        source_app_id: source_app_id.to_string(),
        executable_name,
        window: None,
    };
    // SAFETY: EnumWindows invokes the callback synchronously while search remains live.
    let _ = unsafe {
        EnumWindows(
            Some(find_media_window),
            LPARAM((&mut search as *mut MediaWindowSearch) as isize),
        )
    };
    if let Some(hwnd) = search.window {
        // SAFETY: hwnd was obtained from EnumWindows; both operations retain no caller data.
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            if SetForegroundWindow(hwnd).as_bool() {
                return Ok(true);
            }
        }
    }
    application(source_app_id)
}

unsafe extern "system" fn find_media_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the MediaWindowSearch pointer supplied to synchronous EnumWindows.
    let search = unsafe { &mut *(lparam.0 as *mut MediaWindowSearch) };
    // SAFETY: hwnd is supplied by EnumWindows and is valid for these synchronous queries.
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok() {
        return true.into();
    }
    let mut process_id = 0;
    // SAFETY: hwnd is supplied by EnumWindows and process_id is writable.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id == 0 || process_id == std::process::id() {
        return true.into();
    }
    // SAFETY: OpenProcess only queries a process handle by the ID returned above.
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return true.into();
    };
    let matches = process_application_user_model_id(process)
        .is_some_and(|id| id.eq_ignore_ascii_case(&search.source_app_id))
        || process_executable_name(process)
            .is_some_and(|name| name.eq_ignore_ascii_case(&search.executable_name));
    // SAFETY: process is the one owned handle returned by OpenProcess.
    let _ = unsafe { CloseHandle(process) };
    if matches {
        search.window = Some(hwnd);
        false.into()
    } else {
        true.into()
    }
}

fn process_application_user_model_id(process: HANDLE) -> Option<String> {
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

fn process_executable_name(process: HANDLE) -> Option<String> {
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    // SAFETY: The buffer is writable for length UTF-16 code units.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

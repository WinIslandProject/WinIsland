use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HWND, LPARAM,
};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_HOSTBACKDROPBRUSH, DwmSetWindowAttribute};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, SetProcessWorkingSetSize,
};
use windows::Win32::UI::Shell::{
    ACTIVATEOPTIONS, ApplicationActivationManager, IApplicationActivationManager,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetWindow, GetWindowLongPtrW,
    GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible, SW_RESTORE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX,
    WS_THICKFRAME,
};
use windows::core::{BOOL, PCWSTR, PWSTR, s, w};

type SetWindowCompositionAttribute =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> BOOL;

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}

fn set_window_composition_attribute() -> Option<SetWindowCompositionAttribute> {
    static FUNCTION: OnceLock<Option<SetWindowCompositionAttribute>> = OnceLock::new();
    *FUNCTION.get_or_init(|| {
        // SAFETY: user32.dll is loaded for every GUI process. The export address remains valid for
        // the process lifetime and is cast to its native SetWindowCompositionAttribute signature.
        unsafe {
            let module = GetModuleHandleW(w!("user32.dll")).ok()?;
            let function = GetProcAddress(module, s!("SetWindowCompositionAttribute"))?;
            Some(std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                SetWindowCompositionAttribute,
            >(function))
        }
    })
}

pub fn enable_host_backdrop(hwnd: HWND) -> bool {
    let enabled: i32 = 1;
    // SAFETY: hwnd belongs to the live WinIsland window and enabled points to an initialized BOOL-
    // compatible value for the duration of the synchronous DWM call.
    if unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_HOSTBACKDROPBRUSH,
            &enabled as *const _ as *const _,
            size_of::<i32>() as u32,
        )
    }
    .is_ok()
    {
        return true;
    }

    let Some(set_attribute) = set_window_composition_attribute() else {
        return false;
    };
    let mut policy = AccentPolicy {
        state: 5,
        flags: 0,
        gradient_color: 0,
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: 19,
        data: (&mut policy as *mut AccentPolicy).cast(),
        size: size_of::<AccentPolicy>(),
    };
    // SAFETY: the dynamically resolved function uses the native ABI verified above. hwnd is live,
    // and data points to an initialized accent policy for the duration of the synchronous call.
    unsafe { set_attribute(hwnd, &mut data).as_bool() }
}

// SAFETY: FindWindowW is called with a null-terminated wide string derived
// from the title parameter. The function returns an HWND that may be invalid
// or null, which we check via is_invalid() before returning.
pub fn find_window(title: &str) -> Option<HWND> {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd = FindWindowW(None, PCWSTR::from_raw(wide.as_ptr()));
        if let Ok(hwnd) = hwnd
            && !hwnd.is_invalid()
        {
            Some(hwnd)
        } else {
            None
        }
    }
}

// SAFETY: ShowWindow and SetForegroundWindow are called on a validated HWND.
// These are UI operations that may fail silently if the window is in a
// different input state (e.g., UIPI blocked), which we accept by discarding
// the result.
pub fn bring_window_to_front(title: &str) {
    if let Some(hwnd) = find_window(title) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

// SAFETY: GetWindowLongPtrW reads and SetWindowLongPtrW writes the extended
// window style of a validated HWND. SetWindowPos refreshes the non-client
// frame after the update without changing size, position, z-order, or focus.
pub fn modify_window_ex_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (current | add_flags) & !remove_flags;
        if new_style == current {
            return;
        }
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

// SAFETY: GetWindowLongPtrW reads and SetWindowLongPtrW writes the window
// style of a validated HWND. Bitwise operations on the style flags are safe
// and the updated style takes effect immediately.
pub fn modify_window_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let new_style = (current | add_flags) & !remove_flags;
        if new_style != current {
            let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
        }
    }
}

pub fn enforce_overlay_window_styles(hwnd: HWND) {
    modify_window_ex_style(
        hwnd,
        WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize,
        WS_EX_APPWINDOW.0 as isize,
    );
    modify_window_style(
        hwnd,
        0,
        WS_MAXIMIZEBOX.0 as isize | WS_THICKFRAME.0 as isize,
    );
    set_window_topmost(hwnd);
}

// SAFETY: SetWindowPos is called on a validated HWND with flags that preserve
// its size and position. The HWND_TOPMOST flag updates only the window's z-order
// without stealing focus.
pub fn set_window_topmost(hwnd: HWND) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

pub fn activate_application(app_user_model_id: &str) -> bool {
    if app_user_model_id.is_empty() {
        return false;
    }

    // SAFETY: The current thread uses COM only while activating the application. A successful
    // initialization is balanced before returning; if another apartment already initialized the
    // thread, its existing apartment remains in use.
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let app_id = app_user_model_id.to_string();
    let app_user_model_id: Vec<u16> = app_id.encode_utf16().chain(std::iter::once(0)).collect();
    let result = (|| unsafe {
        // SAFETY: The activation manager is a system COM local server. Both UTF-16 buffers are
        // null-terminated and remain valid for the duration of ActivateApplication.
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)?;
        manager.ActivateApplication(
            PCWSTR(app_user_model_id.as_ptr()),
            PCWSTR::null(),
            ACTIVATEOPTIONS::default(),
        )
    })();
    if com_initialized {
        // SAFETY: This balances the successful CoInitializeEx call above.
        unsafe { CoUninitialize() };
    }
    match result {
        Ok(process_id) => {
            log::info!("Application activated: {app_id} (process {process_id})");
            true
        }
        Err(error) => {
            log::debug!("Application could not be activated: {error:?}");
            false
        }
    }
}

struct MediaWindowSearch {
    source_app_id: String,
    executable_name: String,
    window: Option<HWND>,
}

pub fn activate_media_application(source_app_id: &str) -> bool {
    if source_app_id.is_empty() {
        return false;
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
    // SAFETY: EnumWindows invokes the callback synchronously. lparam points to search for the
    // entire enumeration, and the callback only stores HWND values owned by other processes.
    let _ = unsafe {
        EnumWindows(
            Some(find_media_window),
            LPARAM((&mut search as *mut MediaWindowSearch) as isize),
        )
    };
    if let Some(hwnd) = search.window {
        // SAFETY: hwnd was returned by EnumWindows and remained valid through the synchronous
        // search. Restoring and foregrounding it do not retain any caller-owned pointers.
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            if SetForegroundWindow(hwnd).as_bool() {
                return true;
            }
        }
    }
    activate_application(source_app_id)
}

unsafe extern "system" fn find_media_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the MediaWindowSearch pointer supplied to synchronous EnumWindows above.
    let search = unsafe { &mut *(lparam.0 as *mut MediaWindowSearch) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok() {
        return true.into();
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id == 0 || process_id == std::process::id() {
        return true.into();
    }
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return true.into();
    };
    let matches = process_application_user_model_id(process)
        .is_some_and(|id| id.eq_ignore_ascii_case(&search.source_app_id))
        || process_executable_name(process)
            .is_some_and(|name| name.eq_ignore_ascii_case(&search.executable_name));
    // SAFETY: process is the owned handle returned by OpenProcess and is closed exactly once.
    let _ = unsafe { CloseHandle(process) };
    if matches {
        search.window = Some(hwnd);
        false.into()
    } else {
        true.into()
    }
}

fn process_application_user_model_id(
    process: windows::Win32::Foundation::HANDLE,
) -> Option<String> {
    let mut length = 0;
    // SAFETY: the first call queries the required UTF-16 buffer length and writes only length.
    if unsafe { GetApplicationUserModelId(process, &mut length, None) } != ERROR_INSUFFICIENT_BUFFER
        || length == 0
    {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: buffer has the length requested by the first call and remains valid synchronously.
    if unsafe { GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) }
        != ERROR_SUCCESS
    {
        return None;
    }
    buffer.truncate(length.saturating_sub(1) as usize);
    String::from_utf16(&buffer).ok()
}

fn process_executable_name(process: windows::Win32::Foundation::HANDLE) -> Option<String> {
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    // SAFETY: buffer is writable for length UTF-16 code units and length is updated synchronously.
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

pub fn trim_process_working_set() {
    // SAFETY: GetCurrentProcess returns a pseudo-handle valid in the current process.
    // Passing usize::MAX for both limits requests the documented working-set trim operation.
    unsafe {
        let process = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
    }
}

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
use windows::Win32::UI::Shell::{
    ACTIVATEOPTIONS, ApplicationActivationManager, IApplicationActivationManager,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_RESTORE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_THICKFRAME,
};
use windows::core::PCWSTR;

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
            log::info!("Notification application activated: {app_id} (process {process_id})");
            true
        }
        Err(error) => {
            log::debug!("Notification application could not be activated: {error:?}");
            false
        }
    }
}

pub fn trim_process_working_set() {
    // SAFETY: GetCurrentProcess returns a pseudo-handle valid in the current process.
    // Passing usize::MAX for both limits requests the documented working-set trim operation.
    unsafe {
        let process = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
    }
}

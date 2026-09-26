use std::ffi::c_void;
use std::sync::OnceLock;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_HOSTBACKDROPBRUSH, DWMWINDOWATTRIBUTE, DwmSetWindowAttribute,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_THICKFRAME,
};
use windows::core::{BOOL, s, w};

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

pub(crate) fn enable_host_backdrop(hwnd: HWND) -> bool {
    let enabled: i32 = 1;
    // SAFETY: hwnd belongs to the live backdrop window and enabled points to an initialized BOOL-
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

// SAFETY: GetWindowLongPtrW reads and SetWindowLongPtrW writes the extended
// window style of a validated HWND. SetWindowPos refreshes the non-client
// frame after the update without changing size, position, z-order, or focus.
fn modify_window_ex_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
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
fn modify_window_style(hwnd: HWND, add_flags: isize, remove_flags: isize) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let new_style = (current | add_flags) & !remove_flags;
        if new_style != current {
            let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
        }
    }
}

pub(crate) fn enforce_overlay_window_styles(hwnd: HWND, topmost: bool) {
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
    set_window_topmost(hwnd, topmost);
}

// SAFETY: SetWindowPos is called on a validated HWND with flags that preserve
// its size and position. The HWND_TOPMOST flag updates only the window's z-order
// without stealing focus.
pub(crate) fn set_window_topmost(hwnd: HWND, topmost: bool) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(if topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            }),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

pub(crate) fn set_titlebar_theme(hwnd: HWND, light: bool) {
    let use_dark: i32 = if light { 0 } else { 1 };
    // SAFETY: hwnd belongs to a live settings window and use_dark is an initialized BOOL-
    // compatible value for the duration of the synchronous DWM call.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20),
            &use_dark as *const _ as *const _,
            size_of::<i32>() as u32,
        );
    }
}

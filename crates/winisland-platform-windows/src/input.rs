use std::cell::RefCell;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetDoubleClickTime, VK_LBUTTON, VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use winisland_platform::{InputHooks, PlatformError, VirtualKey, VolumeKey, VolumeKeyEvent};

type VolumeCallback = Box<dyn Fn(VolumeKeyEvent) -> bool + Send + Sync>;

static VOLUME_CALLBACK: Mutex<Option<VolumeCallback>> = Mutex::new(None);

thread_local! {
    static VOLUME_HOOK: RefCell<Option<HookGuard>> = const { RefCell::new(None) };
}

struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        // SAFETY: The hook was installed by this process and is removed on its message thread.
        if unsafe { UnhookWindowsHookEx(self.0) }.is_err() {
            log::warn!("Volume keyboard hook could not be removed");
        }
    }
}

pub struct WindowsInput;

impl InputHooks for WindowsInput {
    fn install_volume_keys(
        &self,
        callback: Box<dyn Fn(VolumeKeyEvent) -> bool + Send + Sync>,
    ) -> Result<(), PlatformError> {
        VOLUME_HOOK.with(|slot| {
            if slot.borrow().is_none() {
                // SAFETY: The current module contains the static callback; the caller pumps messages.
                let hook = unsafe {
                    let module = GetModuleHandleW(None)
                        .map_err(|error| PlatformError::Backend(error.to_string()))?;
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(volume_keyboard_hook),
                        Some(HINSTANCE(module.0)),
                        0,
                    )
                    .map_err(|error| PlatformError::Backend(error.to_string()))?
                };
                *slot.borrow_mut() = Some(HookGuard(hook));
                log::info!("Volume keys are handled by WinIsland");
            }
            *VOLUME_CALLBACK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(callback);
            Ok(())
        })
    }

    fn uninstall_volume_keys(&self) -> Result<(), PlatformError> {
        *VOLUME_CALLBACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        VOLUME_HOOK.with(|slot| {
            if slot.borrow_mut().take().is_some() {
                log::info!("Native volume flyout replacement disabled");
            }
        });
        Ok(())
    }

    fn key_down(&self, key: VirtualKey) -> Result<bool, PlatformError> {
        let code = match key {
            VirtualKey::LeftMouseButton => VK_LBUTTON.0 as i32,
        };
        // SAFETY: GetAsyncKeyState queries virtual key state without pointer arguments.
        Ok(unsafe { (GetAsyncKeyState(code) as u16 & 0x8000) != 0 })
    }

    fn double_click_interval(&self) -> Duration {
        // SAFETY: GetDoubleClickTime has no input pointers and reads a system setting.
        Duration::from_millis(u64::from(unsafe { GetDoubleClickTime() }))
    }
}

unsafe extern "system" fn volume_keyboard_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
    {
        // SAFETY: For HC_ACTION, lparam points to KBDLLHOOKSTRUCT until this callback returns.
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let key = match key.vkCode {
            value if value == VK_VOLUME_UP.0 as u32 => Some(VolumeKey::Up),
            value if value == VK_VOLUME_DOWN.0 as u32 => Some(VolumeKey::Down),
            value if value == VK_VOLUME_MUTE.0 as u32 => Some(VolumeKey::Mute),
            _ => None,
        };
        if let Some(key) = key {
            let event = VolumeKeyEvent {
                key,
                is_down: matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN),
            };
            let handled = VOLUME_CALLBACK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|callback| callback(event));
            if handled {
                return LRESULT(1);
            }
        }
    }
    // SAFETY: Unhandled input is forwarded with the original hook parameters.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

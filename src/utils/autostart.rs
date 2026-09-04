use std::env;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
};
use windows::core::w;

pub fn set_autostart(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let app_name = w!("WinIsland");
    let sub_key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

    if enabled {
        let exe_path = env::current_exe()?;
        let exe_path_str = exe_path.to_str().ok_or("Invalid exe path")?;
        let quoted_path = format!("\"{exe_path_str}\"");
        let exe_path_wide: Vec<u16> = quoted_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: RegCreateKeyExW and RegSetValueExW write to HKEY_CURRENT_USER\Run.
        // The key path is a static string literal. The value data pointer points to
        // a valid null-terminated UTF-16 buffer. RegCloseKey properly closes the handle.
        unsafe {
            let mut hkey = HKEY::default();
            let res = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub_key,
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            );

            if res.is_ok() {
                let _ = RegSetValueExW(
                    hkey,
                    app_name,
                    Some(0),
                    REG_SZ,
                    Some(std::slice::from_raw_parts(
                        exe_path_wide.as_ptr() as *const u8,
                        exe_path_wide.len() * 2,
                    )),
                );
                let _ = RegCloseKey(hkey);
                log::info!("Autostart: enabled ({exe_path_str})");
            } else {
                log::error!("Autostart: RegCreateKeyExW failed: {res:?}");
            }
        }
    } else {
        // SAFETY: RegCreateKeyExW opens HKEY_CURRENT_USER\Run for deletion.
        // The key path is a static string literal. RegDeleteValueW removes the
        // WinIsland value. RegCloseKey properly closes the handle.
        unsafe {
            let mut hkey = HKEY::default();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub_key,
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
            .is_ok()
            {
                let _ = RegDeleteValueW(hkey, app_name);
                let _ = RegCloseKey(hkey);
                log::info!("Autostart: disabled");
            }
        }
    }
    Ok(())
}

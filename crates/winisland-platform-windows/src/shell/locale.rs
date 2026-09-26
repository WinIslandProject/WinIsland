use windows::Win32::Globalization::GetUserDefaultLocaleName;

pub(super) fn system_locale() -> String {
    let mut buffer = [0u16; 128];
    // SAFETY: The buffer is writable for all 128 UTF-16 code units.
    let len = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if len > 0 {
        String::from_utf16_lossy(&buffer[..len as usize - 1])
    } else {
        String::new()
    }
}

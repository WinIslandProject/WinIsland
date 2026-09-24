use windows::Win32::Globalization::GetUserDefaultLocaleName;

pub fn system_locale() -> String {
    let mut buffer = [0u16; 128];
    // SAFETY: GetUserDefaultLocaleName reads the system locale into the provided
    // buffer. The buffer is stack-allocated with 128 elements, sufficient for any
    // valid locale name. from_utf16_lossy handles potentially malformed input.
    unsafe {
        let len = GetUserDefaultLocaleName(&mut buffer);
        if len > 0 {
            return String::from_utf16_lossy(&buffer[..len as usize - 1]);
        }
    }
    String::new()
}

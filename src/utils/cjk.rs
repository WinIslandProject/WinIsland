use windows::Win32::Globalization::{LCMAP_SIMPLIFIED_CHINESE, LCMapStringEx};
use windows::core::w;

pub fn to_simplified(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let source: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: Both calls use valid UTF-16 slices; the first obtains the exact output length.
    unsafe {
        let len = LCMapStringEx(
            w!("zh-CN"),
            LCMAP_SIMPLIFIED_CHINESE,
            &source,
            None,
            None,
            None,
            windows::Win32::Foundation::LPARAM(0),
        );
        if len <= 0 {
            return text.to_string();
        }
        let mut output = vec![0u16; len as usize];
        let written = LCMapStringEx(
            w!("zh-CN"),
            LCMAP_SIMPLIFIED_CHINESE,
            &source,
            Some(&mut output),
            None,
            None,
            windows::Win32::Foundation::LPARAM(0),
        );
        if written <= 0 {
            text.to_string()
        } else {
            String::from_utf16_lossy(&output[..written as usize])
        }
    }
}

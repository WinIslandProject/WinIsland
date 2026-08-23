pub mod context;
pub mod i18n;
pub mod lyrics;
pub mod metadata;
pub mod widget;

/// Opaque plugin-owned instance handle.
pub type PluginHandle = *mut std::ffi::c_void;

/// Host-issued identity used to validate every plugin-to-host call.
pub type PluginToken = u64;

/// Host-issued identifier for a plugin-owned resource.
pub type ResourceId = u64;

/// Invalid token or resource identifier.
pub const INVALID_ID: u64 = 0;

/// Borrowed bytes that are valid only for the duration of an FFI call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ByteSliceV1 {
    pub ptr: *const u8,
    pub len: u32,
}

impl ByteSliceV1 {
    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    pub fn from_slice(value: &[u8]) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len().min(u32::MAX as usize) as u32,
        }
    }
}

/// Borrowed UTF-8 bytes that are valid only for the duration of an FFI call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Utf8SliceV1 {
    pub ptr: *const u8,
    pub len: u32,
}

impl Utf8SliceV1 {
    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    pub fn borrowed(value: &str) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len().min(u32::MAX as usize) as u32,
        }
    }
}

/// The return type for fallible plugin host calls.
///
/// This is a C-compatible equivalent of `Result<(), String>`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PluginResultC {
    /// Zero for success, non-zero for failure.
    pub status: i32,
    /// Null-terminated UTF-8 error message (max 255 bytes + NUL).
    pub error: [u8; 256],
}

impl PluginResultC {
    /// Construct a success result.
    pub fn ok() -> Self {
        Self {
            status: 0,
            error: [0u8; 256],
        }
    }

    /// Construct an error result with the given message.
    ///
    /// The message is truncated to 255 bytes if it exceeds the buffer.
    pub fn err(msg: &str) -> Self {
        let mut error = [0u8; 256];
        let bytes = msg.as_bytes();
        let mut len = bytes.len().min(255);
        while !msg.is_char_boundary(len) {
            len -= 1;
        }
        error[..len].copy_from_slice(&bytes[..len]);
        Self { status: 1, error }
    }

    /// Convert back into a Rust `Result`.
    pub fn into_result(self) -> Result<(), String> {
        if self.status == 0 {
            Ok(())
        } else {
            let end = self.error.iter().position(|&b| b == 0).unwrap_or(256);
            Err(String::from_utf8_lossy(&self.error[..end]).into_owned())
        }
    }
}

/// Fill a fixed-size byte buffer with a string, zeroing the rest.
///
/// Useful for initialising `#[repr(C)]` struct fields with a
/// null-terminated string. The string is truncated if it doesn't fit.
///
/// ```rust
/// use winisland_plugin_api::str_to_fixed;
/// let buf: [u8; 64] = str_to_fixed("hello");
/// assert_eq!(&buf[..6], b"hello\0");
/// assert_eq!(buf[6..].iter().all(|&b| b == 0), true);
/// ```
pub const fn str_to_fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let max_len = N.saturating_sub(1);
    let mut len = if bytes.len() < max_len {
        bytes.len()
    } else {
        max_len
    };
    while len < bytes.len() && len > 0 && bytes[len] & 0b1100_0000 == 0b1000_0000 {
        len -= 1;
    }
    let mut index = 0;
    while index < len {
        buf[index] = bytes[index];
        index += 1;
    }
    buf
}

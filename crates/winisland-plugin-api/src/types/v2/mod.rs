pub mod context;
pub mod i18n;
pub mod lyrics;
pub mod settings;
pub mod widget;

use std::ffi::c_void;

use crate::abi::PluginStatus;

macro_rules! handle {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name(pub(crate) u64);

        impl $name {
            pub const INVALID: Self = Self(0);

            pub const fn get(self) -> u64 {
                self.0
            }

            /// # Safety
            /// The host must validate the raw value and its owner before use.
            pub const unsafe fn from_raw(raw: u64) -> Self {
                Self(raw)
            }
        }
    };
}

handle!(PluginToken);
handle!(ResourceId);
handle!(WidgetId);
handle!(ImageId);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ByteSlice {
    pub ptr: *const u8,
    pub len: u32,
}

impl ByteSlice {
    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    pub fn borrowed(value: &[u8]) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len().min(u32::MAX as usize) as u32,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Utf8Slice {
    pub ptr: *const u8,
    pub len: u32,
}

impl Utf8Slice {
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextStyleV2 {
    pub size: f32,
    pub weight: u16,
    pub italic: u8,
    pub reserved: u8,
    pub family: Utf8Slice,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextMetricsV2 {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
}

pub type HostStateChangedFnV2 = unsafe extern "C" fn(
    callback_data: *mut c_void,
    state: *const context::HostStateV2,
) -> PluginStatus;

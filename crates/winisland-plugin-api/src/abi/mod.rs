mod layout;
mod tables;

use std::ffi::c_void;

use crate::types::metadata::PluginMetadataC;
use crate::types::v2::{PluginToken, WidgetId};

pub use tables::*;

pub const ABI_VERSION_2: u32 = 2;
pub const PLUGIN_ENTRY_SYMBOL_V2: &[u8] = b"winisland_plugin_entry_v2";

pub const IFACE_CONTEXT: u32 = 0x01;
pub const IFACE_MEDIA: u32 = 0x02;
pub const IFACE_I18N: u32 = 0x03;
pub const IFACE_HOST_STATE: u32 = 0x04;
pub const IFACE_WIDGET: u32 = 0x05;
pub const IFACE_LYRICS_TRANSFORM: u32 = 0x06;
pub const IFACE_SETTINGS: u32 = 0x07;
pub const IFACE_TEXT: u32 = 0x08;
pub const IFACE_IMAGE: u32 = 0x09;
pub const IFACE_STORE: u32 = 0x0a;
pub const IFACE_LOG: u32 = 0x0b;
pub const IFACE_VERSION_1: u32 = 1;

pub const CAP_CONTEXT: u64 = 1 << 0;
pub const CAP_MEDIA: u64 = 1 << 1;
pub const CAP_I18N: u64 = 1 << 2;
pub const CAP_HOST_STATE: u64 = 1 << 3;
pub const CAP_WIDGET: u64 = 1 << 4;
pub const CAP_LYRICS: u64 = 1 << 5;
pub const CAP_SETTINGS: u64 = 1 << 6;
pub const CAP_TEXT: u64 = 1 << 7;
pub const CAP_IMAGE: u64 = 1 << 8;
pub const CAP_STORE: u64 = 1 << 9;
pub const KNOWN_CAPABILITIES_V2: u64 = CAP_CONTEXT
    | CAP_MEDIA
    | CAP_I18N
    | CAP_HOST_STATE
    | CAP_WIDGET
    | CAP_LYRICS
    | CAP_SETTINGS
    | CAP_TEXT
    | CAP_IMAGE
    | CAP_STORE;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginStatus(i32);

#[allow(non_upper_case_globals)]
impl PluginStatus {
    pub const Ok: Self = Self(0);
    pub const InvalidArgument: Self = Self(1);
    pub const StaleHandle: Self = Self(2);
    pub const CapabilityMissing: Self = Self(3);
    pub const LimitExceeded: Self = Self(4);
    pub const UnsupportedVersion: Self = Self(5);
    pub const IoError: Self = Self(6);
    pub const Internal: Self = Self(7);

    pub const fn code(self) -> i32 {
        self.0
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TablePrefix {
    pub struct_size: u32,
    pub version: u32,
    pub context: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginHostV2 {
    pub prefix: TablePrefix,
    pub host_build: u32,
    pub query: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> *const c_void>,
}

pub type PluginHandleV2 = *mut c_void;
pub type PluginCreateFnV2 =
    unsafe extern "C" fn(*const PluginCreateInfoV2, *mut PluginHandleV2) -> PluginStatus;
pub type PluginShutdownFnV2 = unsafe extern "C" fn(PluginHandleV2) -> PluginStatus;
pub type PluginDestroyFnV2 = unsafe extern "C" fn(PluginHandleV2);
pub type PluginTickFnV2 = unsafe extern "C" fn(PluginHandleV2, WidgetId, f64) -> PluginStatus;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginCreateInfoV2 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub plugin_token: PluginToken,
    pub host_api: *const PluginHostV2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginDescriptorV2 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub capabilities: u64,
    pub metadata: PluginMetadataC,
    pub create: Option<PluginCreateFnV2>,
    pub shutdown: Option<PluginShutdownFnV2>,
    pub destroy: Option<PluginDestroyFnV2>,
    pub on_tick: Option<PluginTickFnV2>,
}

pub type PluginEntryFnV2 = unsafe extern "C" fn() -> *const PluginDescriptorV2;

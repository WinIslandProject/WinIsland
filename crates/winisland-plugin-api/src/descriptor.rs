use crate::{HostApiV1, PluginHandle, PluginMetadataC, PluginResultC, PluginToken};

pub const ABI_VERSION_1: u32 = 1;
pub const PLUGIN_ENTRY_SYMBOL_V1: &[u8] = b"winisland_plugin_entry_v1";

pub const CAPABILITY_CONTEXT: u64 = 1 << 0;
pub const CAPABILITY_MEDIA: u64 = 1 << 1;
pub const CAPABILITY_I18N: u64 = 1 << 2;
pub const CAPABILITY_HOST_STATE: u64 = 1 << 3;
pub const CAPABILITY_WIDGET: u64 = 1 << 4;
pub const CAPABILITY_LYRICS_TRANSFORM: u64 = 1 << 5;
pub const KNOWN_CAPABILITIES: u64 = CAPABILITY_CONTEXT
    | CAPABILITY_MEDIA
    | CAPABILITY_I18N
    | CAPABILITY_HOST_STATE
    | CAPABILITY_WIDGET
    | CAPABILITY_LYRICS_TRANSFORM;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginCreateInfoV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub plugin_token: PluginToken,
    pub host_api: *const HostApiV1,
}

pub type PluginCreateFnV1 = unsafe extern "C" fn(
    create_info: *const PluginCreateInfoV1,
    out_handle: *mut PluginHandle,
) -> PluginResultC;
/// Stop all plugin work and join every thread that may execute plugin code.
///
/// The host destroys the plugin handle and unloads the DLL only after this
/// returns success. A failed `create` may return a cleanup handle; the host
/// then follows the same `shutdown` and `destroy` sequence.
pub type PluginShutdownFnV1 = unsafe extern "C" fn(PluginHandle) -> PluginResultC;
pub type PluginDestroyFnV1 = unsafe extern "C" fn(PluginHandle);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginDescriptorV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    /// Each `CAPABILITY_*` bit authorizes the corresponding host interface.
    pub capabilities: u64,
    pub metadata: PluginMetadataC,
    pub create: Option<PluginCreateFnV1>,
    pub shutdown: Option<PluginShutdownFnV1>,
    pub destroy: Option<PluginDestroyFnV1>,
}

pub type PluginEntryFnV1 = unsafe extern "C" fn() -> *const PluginDescriptorV1;

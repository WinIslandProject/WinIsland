use std::ffi::c_void;

use crate::{
    ContextDataV1, HostStateV1, LyricsTransformerDataV1, MediaSourceDataV1, PluginResultC,
    PluginToken, ResourceId, TranslationPairV1, Utf8SliceV1, WidgetDataV1,
};

pub const INTERFACE_VERSION_1: u32 = 1;
pub const INTERFACE_CONTEXT: u32 = 1;
pub const INTERFACE_MEDIA: u32 = 2;
pub const INTERFACE_I18N: u32 = 3;
pub const INTERFACE_HOST_STATE: u32 = 4;
pub const INTERFACE_WIDGET: u32 = 5;
pub const INTERFACE_LYRICS_TRANSFORM: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostApiV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub query_interface:
        Option<unsafe extern "C" fn(interface_id: u32, version: u32) -> *const c_void>,
}

impl HostApiV1 {
    /// Query the ABI v1 context service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn context_api(&self) -> Option<ContextApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_CONTEXT) }
    }

    /// Query the ABI v1 media service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn media_api(&self) -> Option<MediaApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_MEDIA) }
    }

    /// Query the ABI v1 translation service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn i18n_api(&self) -> Option<I18nApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_I18N) }
    }

    /// Query the ABI v1 host-state service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn host_state_api(&self) -> Option<HostStateApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_HOST_STATE) }
    }

    /// Query the ABI v1 widget service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn widget_api(&self) -> Option<WidgetApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_WIDGET) }
    }

    /// Query the ABI v1 lyric transformation service table.
    ///
    /// # Safety
    /// `self` and its function pointers must originate from WinIsland and remain
    /// valid for the duration of this call.
    pub unsafe fn lyrics_transform_api(&self) -> Option<LyricsTransformApiV1> {
        // SAFETY: The caller guarantees this host table came from WinIsland.
        unsafe { self.query(INTERFACE_LYRICS_TRANSFORM) }
    }

    unsafe fn query<T: Copy>(&self, interface_id: u32) -> Option<T> {
        if self.abi_version != crate::ABI_VERSION_1
            || self.struct_size < std::mem::size_of::<Self>() as u32
        {
            return None;
        }
        let query = self.query_interface?;
        // SAFETY: The caller guarantees the host function pointer is valid.
        let pointer = unsafe { query(interface_id, INTERFACE_VERSION_1) };
        if pointer.is_null() {
            return None;
        }
        let header = pointer.cast::<u32>();
        // SAFETY: Every service table begins with two readable u32 fields.
        let struct_size = unsafe { std::ptr::read_unaligned(header) };
        // SAFETY: The second header field follows the first u32.
        let version = unsafe { std::ptr::read_unaligned(header.add(1)) };
        if struct_size < std::mem::size_of::<T>() as u32 || version != INTERFACE_VERSION_1 {
            return None;
        }
        // SAFETY: The validated prefix contains a complete copyable v1 table.
        Some(unsafe { std::ptr::read_unaligned(pointer.cast::<T>()) })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ContextApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub create: Option<
        unsafe extern "C" fn(PluginToken, *const ContextDataV1, *mut ResourceId) -> PluginResultC,
    >,
    pub update: Option<
        unsafe extern "C" fn(PluginToken, ResourceId, *const ContextDataV1) -> PluginResultC,
    >,
    pub release: Option<unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MediaApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub create: Option<
        unsafe extern "C" fn(
            PluginToken,
            *const MediaSourceDataV1,
            *mut ResourceId,
        ) -> PluginResultC,
    >,
    pub update: Option<
        unsafe extern "C" fn(PluginToken, ResourceId, *const MediaSourceDataV1) -> PluginResultC,
    >,
    pub release: Option<unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct I18nApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub register_bundle: Option<
        unsafe extern "C" fn(
            PluginToken,
            Utf8SliceV1,
            *const TranslationPairV1,
            u32,
            *mut ResourceId,
        ) -> PluginResultC,
    >,
    pub release_bundle: Option<unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostStateApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub get: Option<unsafe extern "C" fn(PluginToken, *mut HostStateV1) -> PluginResultC>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WidgetApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub create: Option<
        unsafe extern "C" fn(PluginToken, *const WidgetDataV1, *mut ResourceId) -> PluginResultC,
    >,
    pub update:
        Option<unsafe extern "C" fn(PluginToken, ResourceId, *const WidgetDataV1) -> PluginResultC>,
    pub release: Option<unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyricsTransformApiV1 {
    pub struct_size: u32,
    pub version: u32,
    pub register: Option<
        unsafe extern "C" fn(
            PluginToken,
            *const LyricsTransformerDataV1,
            *mut ResourceId,
        ) -> PluginResultC,
    >,
    pub release: Option<unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC>,
}

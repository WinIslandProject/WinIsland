use std::ffi::c_void;

use crate::abi::{PluginStatus, TablePrefix};
use crate::types::v2::context::{ContextDataV2, HostStateV2, MediaSourceDataV2};
use crate::types::v2::i18n::TranslationPairV2;
use crate::types::v2::lyrics::LyricsTransformerDataV2;
use crate::types::v2::settings::SettingsPageDataV2;
use crate::types::v2::widget::WidgetSpecV2;
use crate::types::v2::{
    ByteSlice, HostStateChangedFnV2, ImageId, PluginToken, ResourceId, TextMetricsV2, TextStyleV2,
    Utf8Slice, WidgetId,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ContextApiV2 {
    pub prefix: TablePrefix,
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            *const ContextDataV2,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub update: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            ResourceId,
            *const ContextDataV2,
        ) -> PluginStatus,
    >,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MediaApiV2 {
    pub prefix: TablePrefix,
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            *const MediaSourceDataV2,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub update: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            ResourceId,
            *const MediaSourceDataV2,
        ) -> PluginStatus,
    >,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
    pub current_title: Option<
        unsafe extern "C" fn(*mut c_void, PluginToken, *mut u8, u32, *mut u32) -> PluginStatus,
    >,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct I18nApiV2 {
    pub prefix: TablePrefix,
    pub register_bundle: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            Utf8Slice,
            *const TranslationPairV2,
            u32,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub release_bundle:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostStateApiV2 {
    pub prefix: TablePrefix,
    pub get:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, *mut HostStateV2) -> PluginStatus>,
    pub subscribe: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            HostStateChangedFnV2,
            *mut c_void,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub release_subscription:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WidgetApiV2 {
    pub prefix: TablePrefix,
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            *const WidgetSpecV2,
            *mut WidgetId,
        ) -> PluginStatus,
    >,
    pub update: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            WidgetId,
            *const WidgetSpecV2,
        ) -> PluginStatus,
    >,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, WidgetId) -> PluginStatus>,
    pub submit_draw_list: Option<
        unsafe extern "C" fn(*mut c_void, PluginToken, WidgetId, *const u8, u32) -> PluginStatus,
    >,
    pub request_redraw:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, WidgetId) -> PluginStatus>,
    pub logical_size: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            WidgetId,
            *mut f32,
            *mut f32,
        ) -> PluginStatus,
    >,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyricsTransformApiV2 {
    pub prefix: TablePrefix,
    pub register: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            *const LyricsTransformerDataV2,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SettingsApiV2 {
    pub prefix: TablePrefix,
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            *const SettingsPageDataV2,
            *mut ResourceId,
        ) -> PluginStatus,
    >,
    pub update: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            ResourceId,
            *const SettingsPageDataV2,
        ) -> PluginStatus,
    >,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, ResourceId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextApiV2 {
    pub prefix: TablePrefix,
    pub measure: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            Utf8Slice,
            *const TextStyleV2,
            *mut TextMetricsV2,
        ) -> PluginStatus,
    >,
    pub font_family: Option<
        unsafe extern "C" fn(*mut c_void, PluginToken, u32, *mut u8, u32, *mut u32) -> PluginStatus,
    >,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ImageApiV2 {
    pub prefix: TablePrefix,
    pub decode: Option<
        unsafe extern "C" fn(*mut c_void, PluginToken, ByteSlice, *mut ImageId) -> PluginStatus,
    >,
    pub upload_rgba: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            u32,
            u32,
            ByteSlice,
            *mut ImageId,
        ) -> PluginStatus,
    >,
    pub album_art:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, *mut ImageId) -> PluginStatus>,
    pub release: Option<unsafe extern "C" fn(*mut c_void, PluginToken, ImageId) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StoreApiV2 {
    pub prefix: TablePrefix,
    pub get: Option<
        unsafe extern "C" fn(
            *mut c_void,
            PluginToken,
            Utf8Slice,
            *mut u8,
            u32,
            *mut u32,
            *mut u8,
        ) -> PluginStatus,
    >,
    pub set: Option<
        unsafe extern "C" fn(*mut c_void, PluginToken, Utf8Slice, ByteSlice) -> PluginStatus,
    >,
    pub delete: Option<unsafe extern "C" fn(*mut c_void, PluginToken, Utf8Slice) -> PluginStatus>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LogApiV2 {
    pub prefix: TablePrefix,
    pub write:
        Option<unsafe extern "C" fn(*mut c_void, PluginToken, u32, Utf8Slice) -> PluginStatus>,
}

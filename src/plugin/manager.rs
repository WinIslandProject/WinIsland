#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use super::loader::NativePlugin;
use super::types::{
    ABI_VERSION_1, ByteSliceV1, CAPABILITY_CONTEXT, CAPABILITY_HOST_STATE, CAPABILITY_I18N,
    CAPABILITY_LYRICS_TRANSFORM, CAPABILITY_MEDIA, CAPABILITY_WIDGET, ContextApiV1, ContextDataV1,
    DrawApiV1, HostApiV1, HostState, HostStateApiV1, HostStateV1, I18nApiV1, INTERFACE_CONTEXT,
    INTERFACE_HOST_STATE, INTERFACE_I18N, INTERFACE_LYRICS_TRANSFORM, INTERFACE_MEDIA,
    INTERFACE_VERSION_1, INTERFACE_WIDGET, INVALID_ID, LYRICS_TEXT_FLAG_WORD_SYNCED, LyricsTextV1,
    LyricsTransformApiV1, LyricsTransformFnV1, LyricsTransformerDataV1, MediaApiV1, MediaCommandV1,
    MediaSourceDataV1, PluginError, PluginResultC, PluginToken, ResourceId, TranslationPairV1,
    Utf8SliceV1, WidgetApiV1, WidgetDataV1, WidgetDrawContextV1, context_from_ffi, read_c_str,
    widget_from_ffi,
};
use super::zip_loader::{self, PluginManifest};
use skia_safe::{Canvas, Color, ColorType, ISize, ImageInfo, Paint, Rect};

const MAX_COVER_BYTES: u32 = 16 * 1024 * 1024;
const MAX_CONTEXTS_PER_PLUGIN: usize = 64;
const MAX_MEDIA_SOURCES_PER_PLUGIN: usize = 4;
const MAX_MEDIA_BYTES_PER_PLUGIN: usize = 32 * 1024 * 1024;
const MAX_I18N_BUNDLES_PER_PLUGIN: usize = 16;
const MAX_I18N_BYTES_PER_PLUGIN: usize = 4 * 1024 * 1024;
const MAX_WIDGETS_PER_PLUGIN: usize = 8;
const MAX_LYRICS_TRANSFORMERS_PER_PLUGIN: usize = 4;
const MAX_TRANSFORMED_LYRIC_BYTES: u32 = 256 * 1024;
const MAX_TRANSLATION_PAIRS: u32 = 4096;
const MAX_TRANSLATION_STRING_BYTES: u32 = 64 * 1024;
const MAX_TRANSLATION_BUNDLE_BYTES: usize = 1024 * 1024;
const DISABLED_PLUGINS_FILE: &str = ".disabled-plugins";

#[derive(Clone)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub github_link: String,
    pub enabled: bool,
    pub icon: Option<Vec<u8>>,
    pub readme: Option<String>,
}

#[derive(Clone)]
pub struct PendingMediaSource {
    pub resource_id: ResourceId,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub is_playing: bool,
    pub available_controls: u32,
    pub cover_data: Vec<u8>,
}

pub enum MediaSourceEvent {
    Set(PendingMediaSource),
    Clear,
}

enum ContextEvent {
    Upsert(crate::core::context::PluginContext),
    Remove(ResourceId),
}

enum WidgetEvent {
    Upsert(crate::core::plugin_widget::PluginWidget),
    Remove(ResourceId),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Context,
    Media,
    I18n,
    Widget,
    LyricsTransform,
}

struct ResourceOwner {
    plugin: PluginToken,
    kind: ResourceKind,
    size_bytes: usize,
}

struct PluginRegistration {
    id: String,
    capabilities: u64,
    stopping: bool,
}

struct MediaResource {
    data: PendingMediaSource,
    sequence: u64,
    on_command: Option<super::types::MediaCommandFnV1>,
    callback_data: usize,
    in_flight: u32,
}

struct LyricsTransformerResource {
    sequence: u64,
    on_transform: LyricsTransformFnV1,
    callback_data: usize,
    in_flight: u32,
}

#[derive(Default)]
struct RuntimeState {
    plugins: HashMap<PluginToken, PluginRegistration>,
    resources: HashMap<ResourceId, ResourceOwner>,
    context_events: HashMap<ResourceId, ContextEvent>,
    visible_contexts: HashSet<ResourceId>,
    media: HashMap<ResourceId, MediaResource>,
    media_sequence: u64,
    media_dirty: bool,
    widget_events: HashMap<ResourceId, WidgetEvent>,
    widget_keys: HashMap<(PluginToken, String), ResourceId>,
    lyrics_transformers: HashMap<ResourceId, LyricsTransformerResource>,
    lyrics_transformer_sequence: u64,
    host_state: HostState,
}

static RUNTIME: OnceLock<Mutex<RuntimeState>> = OnceLock::new();
static NEXT_PLUGIN_TOKEN: AtomicU64 = AtomicU64::new(1);
static NEXT_RESOURCE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_BACKUP_ID: AtomicU64 = AtomicU64::new(1);

static HOST_API: HostApiV1 = HostApiV1 {
    struct_size: std::mem::size_of::<HostApiV1>() as u32,
    abi_version: ABI_VERSION_1,
    query_interface: Some(query_interface),
};
static CONTEXT_API: ContextApiV1 = ContextApiV1 {
    struct_size: std::mem::size_of::<ContextApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    create: Some(context_create),
    update: Some(context_update),
    release: Some(context_release),
};
static MEDIA_API: MediaApiV1 = MediaApiV1 {
    struct_size: std::mem::size_of::<MediaApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    create: Some(media_create),
    update: Some(media_update),
    release: Some(media_release),
};
static I18N_API: I18nApiV1 = I18nApiV1 {
    struct_size: std::mem::size_of::<I18nApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    register_bundle: Some(i18n_register_bundle),
    release_bundle: Some(i18n_release_bundle),
};
static HOST_STATE_API: HostStateApiV1 = HostStateApiV1 {
    struct_size: std::mem::size_of::<HostStateApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    get: Some(host_state_get),
};
static WIDGET_API: WidgetApiV1 = WidgetApiV1 {
    struct_size: std::mem::size_of::<WidgetApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    create: Some(widget_create),
    update: Some(widget_update),
    release: Some(widget_release),
};
static LYRICS_TRANSFORM_API: LyricsTransformApiV1 = LyricsTransformApiV1 {
    struct_size: std::mem::size_of::<LyricsTransformApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    register: Some(lyrics_transform_register),
    release: Some(lyrics_transform_release),
};
static DRAW_API: DrawApiV1 = DrawApiV1 {
    struct_size: std::mem::size_of::<DrawApiV1>() as u32,
    version: INTERFACE_VERSION_1,
    draw_text: Some(ffi_draw_text),
    measure_text: Some(ffi_measure_text),
    draw_rect: Some(ffi_draw_rect),
    draw_round_rect: Some(ffi_draw_round_rect),
    draw_circle: Some(ffi_draw_circle),
    draw_line: Some(ffi_draw_line),
    draw_arc: Some(ffi_draw_arc),
    draw_image: Some(ffi_draw_image),
    save: Some(ffi_save),
    restore: Some(ffi_restore),
    translate: Some(ffi_translate),
};

fn runtime() -> &'static Mutex<RuntimeState> {
    RUNTIME.get_or_init(|| Mutex::new(RuntimeState::default()))
}

pub fn host_api() -> *const HostApiV1 {
    &HOST_API
}

pub fn draw_api() -> &'static DrawApiV1 {
    &DRAW_API
}

unsafe extern "C" fn query_interface(interface_id: u32, version: u32) -> *const c_void {
    if version != INTERFACE_VERSION_1 {
        return std::ptr::null();
    }
    match interface_id {
        INTERFACE_CONTEXT => std::ptr::from_ref(&CONTEXT_API).cast(),
        INTERFACE_MEDIA => std::ptr::from_ref(&MEDIA_API).cast(),
        INTERFACE_I18N => std::ptr::from_ref(&I18N_API).cast(),
        INTERFACE_HOST_STATE => std::ptr::from_ref(&HOST_STATE_API).cast(),
        INTERFACE_WIDGET => std::ptr::from_ref(&WIDGET_API).cast(),
        INTERFACE_LYRICS_TRANSFORM => std::ptr::from_ref(&LYRICS_TRANSFORM_API).cast(),
        _ => std::ptr::null(),
    }
}

fn next_id(counter: &AtomicU64) -> u64 {
    loop {
        let id = counter.fetch_add(1, Ordering::Relaxed);
        if id != INVALID_ID {
            return id;
        }
    }
}

fn require_capability(
    state: &RuntimeState,
    token: PluginToken,
    capability: u64,
) -> Result<(), &'static str> {
    match state.plugins.get(&token) {
        Some(plugin) if plugin.capabilities & capability != 0 => Ok(()),
        Some(_) => Err("capability was not declared"),
        None => Err("invalid plugin token"),
    }
}

fn require_resource(
    state: &RuntimeState,
    token: PluginToken,
    id: ResourceId,
    kind: ResourceKind,
) -> Result<(), &'static str> {
    match state.resources.get(&id) {
        Some(owner) if owner.plugin == token && owner.kind == kind => Ok(()),
        Some(_) => Err("resource is owned by another plugin"),
        None => Err("resource was not found"),
    }
}

fn resource_count(state: &RuntimeState, token: PluginToken, kind: ResourceKind) -> usize {
    state
        .resources
        .values()
        .filter(|owner| owner.plugin == token && owner.kind == kind)
        .count()
}

fn resource_bytes(
    state: &RuntimeState,
    token: PluginToken,
    kind: ResourceKind,
    except: Option<ResourceId>,
) -> usize {
    state
        .resources
        .iter()
        .filter(|(id, owner)| Some(**id) != except && owner.plugin == token && owner.kind == kind)
        .map(|(_, owner)| owner.size_bytes)
        .sum()
}

unsafe fn read_struct<T: Copy>(value: *const T) -> Result<T, &'static str> {
    if value.is_null() {
        return Err("input pointer is null");
    }
    // SAFETY: Plugin inputs are trusted ABI values and every v1 struct starts with struct_size.
    let struct_size = unsafe { std::ptr::read_unaligned(value.cast::<u32>()) };
    if struct_size < std::mem::size_of::<T>() as u32 {
        return Err("input struct is truncated");
    }
    // SAFETY: The size check proves the complete ABI v1 prefix is available.
    Ok(unsafe { std::ptr::read_unaligned(value) })
}

unsafe fn read_widget_data(value: *const WidgetDataV1) -> Result<WidgetDataV1, &'static str> {
    if value.is_null() {
        return Err("input pointer is null");
    }
    // SAFETY: Plugin widget inputs start with a readable struct_size field.
    let struct_size = unsafe { std::ptr::read_unaligned(value.cast::<u32>()) } as usize;
    let legacy_size = std::mem::offset_of!(WidgetDataV1, key);
    if struct_size < legacy_size {
        return Err("input struct is truncated");
    }
    let mut data = std::mem::MaybeUninit::<WidgetDataV1>::zeroed();
    // SAFETY: The trusted plugin reports at least the legacy prefix length. Copying only the
    // smaller of its version and the host version preserves compatibility with both layouts.
    unsafe {
        std::ptr::copy_nonoverlapping(
            value.cast::<u8>(),
            data.as_mut_ptr().cast::<u8>(),
            struct_size.min(std::mem::size_of::<WidgetDataV1>()),
        );
        Ok(data.assume_init())
    }
}

fn validate_widget_key(key: &[u8; 64]) -> Result<Option<String>, &'static str> {
    let end = key.iter().position(|byte| *byte == 0).unwrap_or(key.len());
    if end == 0 {
        return Ok(None);
    }
    if end == key.len()
        || !key[..end]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
    {
        return Err("widget key must match [a-zA-Z0-9_-]{1,63}");
    }
    Ok(Some(String::from_utf8_lossy(&key[..end]).into_owned()))
}

unsafe fn read_utf8(value: Utf8SliceV1, max_len: u32) -> Result<String, &'static str> {
    if value.len > max_len {
        return Err("UTF-8 value exceeds the size limit");
    }
    if value.len == 0 {
        return Ok(String::new());
    }
    if value.ptr.is_null() {
        return Err("UTF-8 pointer is null");
    }
    // SAFETY: The plugin guarantees this borrowed range is valid for the call.
    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len as usize) };
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "value is not valid UTF-8")
}

unsafe extern "C" fn context_create(
    token: PluginToken,
    data: *const ContextDataV1,
    out_id: *mut ResourceId,
) -> PluginResultC {
    if out_id.is_null() {
        return PluginResultC::err("resource output pointer is null");
    }
    // SAFETY: read_widget_data validates and copies the versioned widget structure.
    let data = match unsafe { read_struct(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    if read_c_str(&data.title).is_empty() {
        return PluginResultC::err("context title is empty");
    }
    if data.priority > super::types::PRIORITY_HIGH
        || data.flags & !super::types::CONTEXT_FLAG_SHOW_COMPACT != 0
    {
        return PluginResultC::err("context contains unknown priority or flags");
    }
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_CONTEXT) {
        return PluginResultC::err(error);
    }
    if resource_count(&state, token, ResourceKind::Context) >= MAX_CONTEXTS_PER_PLUGIN {
        return PluginResultC::err("context resource limit reached");
    }
    let id = next_id(&NEXT_RESOURCE_ID);
    let context = context_from_ffi(token, id, &data);
    let size_bytes = context.title.len() + context.body.len() + context.compact_text.len();
    state.resources.insert(
        id,
        ResourceOwner {
            plugin: token,
            kind: ResourceKind::Context,
            size_bytes,
        },
    );
    state
        .context_events
        .insert(id, ContextEvent::Upsert(context));
    // SAFETY: out_id was checked non-null and belongs to the caller.
    unsafe { out_id.write(id) };
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn context_update(
    token: PluginToken,
    id: ResourceId,
    data: *const ContextDataV1,
) -> PluginResultC {
    // SAFETY: read_widget_data validates and copies the versioned widget structure.
    let data = match unsafe { read_struct(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    if read_c_str(&data.title).is_empty() {
        return PluginResultC::err("context title is empty");
    }
    if data.priority > super::types::PRIORITY_HIGH
        || data.flags & !super::types::CONTEXT_FLAG_SHOW_COMPACT != 0
    {
        return PluginResultC::err("context contains unknown priority or flags");
    }
    let context = context_from_ffi(token, id, &data);
    let size_bytes = context.title.len() + context.body.len() + context.compact_text.len();
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Context) {
        return PluginResultC::err(error);
    }
    if let Some(owner) = state.resources.get_mut(&id) {
        owner.size_bytes = size_bytes;
    }
    state
        .context_events
        .insert(id, ContextEvent::Upsert(context));
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn context_release(token: PluginToken, id: ResourceId) -> PluginResultC {
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Context) {
        return PluginResultC::err(error);
    }
    state.resources.remove(&id);
    state.context_events.remove(&id);
    if state.visible_contexts.remove(&id) {
        state.context_events.insert(id, ContextEvent::Remove(id));
    }
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

fn copy_media(data: &MediaSourceDataV1, id: ResourceId) -> Result<MediaResource, &'static str> {
    let title = read_c_str(&data.title);
    if title.is_empty() {
        return Err("media title is empty");
    }
    if data.cover.len > MAX_COVER_BYTES {
        return Err("cover exceeds 16 MiB");
    }
    let known_controls = super::types::MEDIA_CONTROL_TOGGLE_PLAY
        | super::types::MEDIA_CONTROL_PREVIOUS
        | super::types::MEDIA_CONTROL_NEXT
        | super::types::MEDIA_CONTROL_SEEK;
    if data.flags & !super::types::MEDIA_FLAG_PLAYING != 0
        || data.available_controls & !known_controls != 0
    {
        return Err("media source contains unknown flags or controls");
    }
    if data.available_controls != 0 && data.on_command.is_none() {
        return Err("media controls require an on_command callback");
    }
    let cover_data = if data.cover.len == 0 {
        Vec::new()
    } else {
        if data.cover.ptr.is_null() {
            return Err("cover pointer is null");
        }
        // SAFETY: The plugin guarantees the cover range is valid for this call.
        unsafe { std::slice::from_raw_parts(data.cover.ptr, data.cover.len as usize) }.to_vec()
    };
    Ok(MediaResource {
        data: PendingMediaSource {
            resource_id: id,
            title,
            artist: read_c_str(&data.artist),
            album: read_c_str(&data.album),
            duration_ms: data.duration_ms,
            position_ms: data.position_ms,
            is_playing: data.flags & super::types::MEDIA_FLAG_PLAYING != 0,
            available_controls: data.available_controls,
            cover_data,
        },
        sequence: 0,
        on_command: data.on_command,
        callback_data: data.callback_data as usize,
        in_flight: 0,
    })
}

unsafe extern "C" fn media_create(
    token: PluginToken,
    data: *const MediaSourceDataV1,
    out_id: *mut ResourceId,
) -> PluginResultC {
    if out_id.is_null() {
        return PluginResultC::err("resource output pointer is null");
    }
    // SAFETY: Validation is performed by read_struct before the value is used.
    let data = match unsafe { read_struct(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    let id = next_id(&NEXT_RESOURCE_ID);
    let mut media = match copy_media(&data, id) {
        Ok(media) => media,
        Err(error) => return PluginResultC::err(error),
    };
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_MEDIA) {
        return PluginResultC::err(error);
    }
    if resource_count(&state, token, ResourceKind::Media) >= MAX_MEDIA_SOURCES_PER_PLUGIN {
        return PluginResultC::err("media resource limit reached");
    }
    if resource_bytes(&state, token, ResourceKind::Media, None)
        .saturating_add(media.data.cover_data.len())
        > MAX_MEDIA_BYTES_PER_PLUGIN
    {
        return PluginResultC::err("media resources exceed the 32 MiB limit");
    }
    state.media_sequence = state.media_sequence.wrapping_add(1);
    media.sequence = state.media_sequence;
    state.resources.insert(
        id,
        ResourceOwner {
            plugin: token,
            kind: ResourceKind::Media,
            size_bytes: media.data.cover_data.len(),
        },
    );
    state.media.insert(id, media);
    state.media_dirty = true;
    // SAFETY: out_id was checked non-null and belongs to the caller.
    unsafe { out_id.write(id) };
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn media_update(
    token: PluginToken,
    id: ResourceId,
    data: *const MediaSourceDataV1,
) -> PluginResultC {
    // SAFETY: Validation is performed by read_struct before the value is used.
    let data = match unsafe { read_struct(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    let mut media = match copy_media(&data, id) {
        Ok(media) => media,
        Err(error) => return PluginResultC::err(error),
    };
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Media) {
        return PluginResultC::err(error);
    }
    if state
        .media
        .get(&id)
        .is_some_and(|media| media.in_flight != 0)
    {
        return PluginResultC::err("media callback is in progress");
    }
    if resource_bytes(&state, token, ResourceKind::Media, Some(id))
        .saturating_add(media.data.cover_data.len())
        > MAX_MEDIA_BYTES_PER_PLUGIN
    {
        return PluginResultC::err("media resources exceed the 32 MiB limit");
    }
    state.media_sequence = state.media_sequence.wrapping_add(1);
    media.sequence = state.media_sequence;
    if let Some(owner) = state.resources.get_mut(&id) {
        owner.size_bytes = media.data.cover_data.len();
    }
    state.media.insert(id, media);
    state.media_dirty = true;
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn media_release(token: PluginToken, id: ResourceId) -> PluginResultC {
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Media) {
        return PluginResultC::err(error);
    }
    if state
        .media
        .get(&id)
        .is_some_and(|media| media.in_flight != 0)
    {
        return PluginResultC::err("media callback is in progress");
    }
    state.resources.remove(&id);
    state.media.remove(&id);
    state.media_dirty = true;
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn i18n_register_bundle(
    token: PluginToken,
    language: Utf8SliceV1,
    pairs: *const TranslationPairV1,
    count: u32,
    out_id: *mut ResourceId,
) -> PluginResultC {
    if out_id.is_null() {
        return PluginResultC::err("resource output pointer is null");
    }
    if count == 0 || count > MAX_TRANSLATION_PAIRS || pairs.is_null() {
        return PluginResultC::err("translation bundle is empty or too large");
    }
    // SAFETY: The plugin owns the borrowed language bytes for this call.
    let language = match unsafe { read_utf8(language, 64) } {
        Ok(language) if !language.is_empty() => language,
        Ok(_) => return PluginResultC::err("language is empty"),
        Err(error) => return PluginResultC::err(error),
    };
    // SAFETY: count is bounded and the plugin guarantees this borrowed array is valid.
    let pairs = unsafe { std::slice::from_raw_parts(pairs, count as usize) };
    let mut copied = Vec::with_capacity(pairs.len());
    let mut total_bytes = 0usize;
    for pair in pairs {
        // SAFETY: Translation strings are borrowed for this call and copied immediately.
        let key = match unsafe { read_utf8(pair.key, MAX_TRANSLATION_STRING_BYTES) } {
            Ok(key) if !key.is_empty() => key,
            Ok(_) => return PluginResultC::err("translation key is empty"),
            Err(error) => return PluginResultC::err(error),
        };
        // SAFETY: Translation strings are borrowed for this call and copied immediately.
        let value = match unsafe { read_utf8(pair.value, MAX_TRANSLATION_STRING_BYTES) } {
            Ok(value) => value,
            Err(error) => return PluginResultC::err(error),
        };
        total_bytes = match total_bytes.checked_add(key.len() + value.len()) {
            Some(total) if total <= MAX_TRANSLATION_BUNDLE_BYTES => total,
            _ => return PluginResultC::err("translation bundle exceeds 1 MiB"),
        };
        copied.push((key, value));
    }

    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_I18N) {
        return PluginResultC::err(error);
    }
    if resource_count(&state, token, ResourceKind::I18n) >= MAX_I18N_BUNDLES_PER_PLUGIN {
        return PluginResultC::err("translation bundle limit reached");
    }
    if resource_bytes(&state, token, ResourceKind::I18n, None).saturating_add(total_bytes)
        > MAX_I18N_BYTES_PER_PLUGIN
    {
        return PluginResultC::err("translation bundles exceed the 4 MiB limit");
    }
    let id = next_id(&NEXT_RESOURCE_ID);
    if let Err(error) = crate::core::i18n::register_plugin_translation_bundle(id, language, copied)
    {
        return PluginResultC::err(error);
    }
    state.resources.insert(
        id,
        ResourceOwner {
            plugin: token,
            kind: ResourceKind::I18n,
            size_bytes: total_bytes,
        },
    );
    // SAFETY: out_id was checked non-null and belongs to the caller.
    unsafe { out_id.write(id) };
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn i18n_release_bundle(token: PluginToken, id: ResourceId) -> PluginResultC {
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::I18n) {
        return PluginResultC::err(error);
    }
    if let Err(error) = crate::core::i18n::release_plugin_translation_bundle(id) {
        return PluginResultC::err(error);
    }
    state.resources.remove(&id);
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn host_state_get(
    token: PluginToken,
    out_state: *mut HostStateV1,
) -> PluginResultC {
    if out_state.is_null() {
        return PluginResultC::err("host state output pointer is null");
    }
    // SAFETY: HostStateV1 begins with struct_size, which is readable by contract.
    let struct_size = unsafe { std::ptr::read_unaligned(out_state.cast::<u32>()) };
    if struct_size < std::mem::size_of::<HostStateV1>() as u32 {
        return PluginResultC::err("host state output struct is truncated");
    }
    let state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_HOST_STATE) {
        return PluginResultC::err(error);
    }
    let snapshot = HostStateV1::from(&state.host_state);
    // SAFETY: The size check proves the caller provided a complete v1 output struct.
    unsafe { out_state.write(snapshot) };
    PluginResultC::ok()
}

unsafe extern "C" fn lyrics_transform_register(
    token: PluginToken,
    data: *const LyricsTransformerDataV1,
    out_id: *mut ResourceId,
) -> PluginResultC {
    if out_id.is_null() {
        return PluginResultC::err("resource output pointer is null");
    }
    // SAFETY: Validation is performed by read_struct before the value is used.
    let data = match unsafe { read_struct(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    if data.flags != 0 {
        return PluginResultC::err("lyrics transformer contains unknown flags");
    }
    let Some(on_transform) = data.on_transform else {
        return PluginResultC::err("lyrics transform callback is required");
    };
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_LYRICS_TRANSFORM) {
        return PluginResultC::err(error);
    }
    if resource_count(&state, token, ResourceKind::LyricsTransform)
        >= MAX_LYRICS_TRANSFORMERS_PER_PLUGIN
    {
        return PluginResultC::err("lyrics transformer resource limit reached");
    }
    let id = next_id(&NEXT_RESOURCE_ID);
    state.lyrics_transformer_sequence = state.lyrics_transformer_sequence.wrapping_add(1);
    let sequence = state.lyrics_transformer_sequence;
    state.resources.insert(
        id,
        ResourceOwner {
            plugin: token,
            kind: ResourceKind::LyricsTransform,
            size_bytes: 0,
        },
    );
    state.lyrics_transformers.insert(
        id,
        LyricsTransformerResource {
            sequence,
            on_transform,
            callback_data: data.callback_data as usize,
            in_flight: 0,
        },
    );
    // SAFETY: out_id was checked non-null and belongs to the caller.
    unsafe { out_id.write(id) };
    PluginResultC::ok()
}

unsafe extern "C" fn lyrics_transform_release(token: PluginToken, id: ResourceId) -> PluginResultC {
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::LyricsTransform) {
        return PluginResultC::err(error);
    }
    if state
        .lyrics_transformers
        .get(&id)
        .is_some_and(|transformer| transformer.in_flight != 0)
    {
        return PluginResultC::err("lyrics transform callback is in progress");
    }
    state.resources.remove(&id);
    state.lyrics_transformers.remove(&id);
    PluginResultC::ok()
}

fn ctx_ref<'a>(ctx: *const WidgetDrawContextV1) -> Option<&'a WidgetDrawContextV1> {
    // SAFETY: The context is host-provided and valid for the whole on_draw call.
    let ctx = unsafe { ctx.as_ref() }?;
    (ctx.struct_size >= std::mem::size_of::<WidgetDrawContextV1>() as u32
        && ctx.version == INTERFACE_VERSION_1)
        .then_some(ctx)
}

fn ctx_canvas(ctx: &WidgetDrawContextV1) -> Option<&Canvas> {
    if ctx.canvas_handle.is_null() {
        return None;
    }
    // SAFETY: The host sets canvas_handle before invoking on_draw and the
    // canvas outlives the whole synchronous callback. Skia canvas operations
    // take &self, so a shared reference is sufficient.
    Some(unsafe { &*(ctx.canvas_handle.cast::<Canvas>()) })
}

const MAX_DRAW_TEXT_BYTES: u32 = 64 * 1024;
const MAX_DRAW_IMAGE_BYTES: usize = 16 * 1024 * 1024;

thread_local! {
    static DRAW_TRANSFORMS: RefCell<Vec<(f32, f32)>> = const { RefCell::new(Vec::new()) };
}

pub fn reset_draw_transform() {
    DRAW_TRANSFORMS.with(|transforms| {
        let mut transforms = transforms.borrow_mut();
        transforms.clear();
        transforms.push((0.0, 0.0));
    });
}

fn draw_transform() -> (f32, f32) {
    DRAW_TRANSFORMS.with(|transforms| transforms.borrow().last().copied().unwrap_or((0.0, 0.0)))
}

fn ctx_text<'a>(text: Utf8SliceV1) -> Option<&'a str> {
    if text.len == 0 {
        return Some("");
    }
    if text.ptr.is_null() || text.len > MAX_DRAW_TEXT_BYTES {
        return None;
    }
    // SAFETY: The plugin guarantees this borrowed range is valid for the call.
    let bytes = unsafe { std::slice::from_raw_parts(text.ptr, text.len as usize) };
    std::str::from_utf8(bytes).ok()
}

fn argb_paint(color: u32, alpha: u8) -> Paint {
    let a = ((color >> 24) & 0xFF) as u8;
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(
        (a as u32 * alpha as u32 / 255) as u8,
        r,
        g,
        b,
    ));
    paint
}

fn stroke_paint(color: u32, alpha: u8, width: f32) -> Paint {
    let mut paint = argb_paint(color, alpha);
    paint.set_style(skia_safe::paint::Style::Stroke);
    paint.set_stroke_width(width);
    paint.set_stroke_cap(skia_safe::paint::Cap::Round);
    paint
}

unsafe extern "C" fn ffi_draw_text(
    ctx: *const WidgetDrawContextV1,
    x: f32,
    y: f32,
    text: Utf8SliceV1,
    size: f32,
    bold: u8,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let Some(text) = ctx_text(text) else {
        return;
    };
    let (tx, ty) = draw_transform();
    let size = size.clamp(1.0, 512.0);
    let font = crate::utils::font::FontManager::global().get_font(size * ctx.scale, bold != 0);
    let (_, metrics) = font.metrics();
    let baseline = (y + ty) * ctx.scale - metrics.ascent;
    let paint = argb_paint(color, ctx.alpha);
    canvas.draw_str(text, ((x + tx) * ctx.scale, baseline), &font, &paint);
}

unsafe extern "C" fn ffi_measure_text(
    ctx: *const WidgetDrawContextV1,
    text: Utf8SliceV1,
    size: f32,
    bold: u8,
) -> f32 {
    let Some(ctx) = ctx_ref(ctx) else {
        return 0.0;
    };
    let Some(text) = ctx_text(text) else {
        return 0.0;
    };
    let size = size.clamp(1.0, 512.0);
    let font = crate::utils::font::FontManager::global().get_font(size * ctx.scale, bold != 0);
    let (advance, _) = font.measure_str(text, None);
    if ctx.scale > 0.0 {
        advance / ctx.scale
    } else {
        0.0
    }
}

unsafe extern "C" fn ffi_draw_rect(
    ctx: *const WidgetDrawContextV1,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let (tx, ty) = draw_transform();
    canvas.draw_rect(
        Rect::from_xywh(
            (x + tx) * ctx.scale,
            (y + ty) * ctx.scale,
            w * ctx.scale,
            h * ctx.scale,
        ),
        &argb_paint(color, ctx.alpha),
    );
}

unsafe extern "C" fn ffi_draw_round_rect(
    ctx: *const WidgetDrawContextV1,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let (tx, ty) = draw_transform();
    let radius = radius * ctx.scale;
    canvas.draw_round_rect(
        Rect::from_xywh(
            (x + tx) * ctx.scale,
            (y + ty) * ctx.scale,
            w * ctx.scale,
            h * ctx.scale,
        ),
        radius,
        radius,
        &argb_paint(color, ctx.alpha),
    );
}

unsafe extern "C" fn ffi_draw_circle(
    ctx: *const WidgetDrawContextV1,
    cx: f32,
    cy: f32,
    r: f32,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let (tx, ty) = draw_transform();
    canvas.draw_circle(
        ((cx + tx) * ctx.scale, (cy + ty) * ctx.scale),
        r * ctx.scale,
        &argb_paint(color, ctx.alpha),
    );
}

unsafe extern "C" fn ffi_draw_line(
    ctx: *const WidgetDrawContextV1,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    stroke_width: f32,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let (tx, ty) = draw_transform();
    canvas.draw_line(
        ((x1 + tx) * ctx.scale, (y1 + ty) * ctx.scale),
        ((x2 + tx) * ctx.scale, (y2 + ty) * ctx.scale),
        &stroke_paint(color, ctx.alpha, stroke_width * ctx.scale),
    );
}

unsafe extern "C" fn ffi_draw_arc(
    ctx: *const WidgetDrawContextV1,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    start_angle: f32,
    sweep_angle: f32,
    stroke_width: f32,
    color: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    let (tx, ty) = draw_transform();
    canvas.draw_arc(
        Rect::from_xywh(
            (x + tx) * ctx.scale,
            (y + ty) * ctx.scale,
            w * ctx.scale,
            h * ctx.scale,
        ),
        start_angle,
        sweep_angle,
        false,
        &stroke_paint(color, ctx.alpha, stroke_width * ctx.scale),
    );
}

unsafe extern "C" fn ffi_draw_image(
    ctx: *const WidgetDrawContextV1,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bitmap: ByteSliceV1,
    bitmap_width: u32,
    bitmap_height: u32,
) {
    let Some(ctx) = ctx_ref(ctx) else {
        return;
    };
    let Some(canvas) = ctx_canvas(ctx) else {
        return;
    };
    if bitmap.ptr.is_null() || bitmap.len == 0 || bitmap_width == 0 || bitmap_height == 0 {
        return;
    }
    let pixel_bytes = (bitmap_width as usize)
        .checked_mul(bitmap_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(usize::MAX);
    if pixel_bytes > bitmap.len as usize || pixel_bytes > MAX_DRAW_IMAGE_BYTES {
        return;
    }
    let info = ImageInfo::new(
        ISize::new(bitmap_width as i32, bitmap_height as i32),
        ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    // SAFETY: The plugin guarantees this borrowed range is valid for the call.
    let pixels = unsafe { std::slice::from_raw_parts(bitmap.ptr, pixel_bytes) };
    let image = skia_safe::images::raster_from_data(
        &info,
        skia_safe::Data::new_copy(pixels),
        bitmap_width as usize * 4,
    );
    let Some(image) = image else {
        return;
    };
    let (tx, ty) = draw_transform();
    canvas.draw_image_rect(
        &image,
        None,
        Rect::from_xywh(
            (x + tx) * ctx.scale,
            (y + ty) * ctx.scale,
            w * ctx.scale,
            h * ctx.scale,
        ),
        &argb_paint(0xFFFF_FFFF, ctx.alpha),
    );
}

unsafe extern "C" fn ffi_save(ctx: *const WidgetDrawContextV1) {
    let Some(_ctx) = ctx_ref(ctx) else {
        return;
    };
    DRAW_TRANSFORMS.with(|transforms| {
        let mut transforms = transforms.borrow_mut();
        if transforms.len() >= 64 {
            return;
        }
        let top = transforms.last().copied().unwrap_or((0.0, 0.0));
        transforms.push(top);
    });
}

unsafe extern "C" fn ffi_restore(ctx: *const WidgetDrawContextV1) {
    let Some(_ctx) = ctx_ref(ctx) else {
        return;
    };
    DRAW_TRANSFORMS.with(|transforms| {
        let mut transforms = transforms.borrow_mut();
        if transforms.len() > 1 {
            transforms.pop();
        }
    });
}

unsafe extern "C" fn ffi_translate(ctx: *const WidgetDrawContextV1, dx: f32, dy: f32) {
    let Some(_ctx) = ctx_ref(ctx) else {
        return;
    };
    DRAW_TRANSFORMS.with(|transforms| {
        let mut transforms = transforms.borrow_mut();
        if let Some(top) = transforms.last_mut() {
            *top = (top.0 + dx, top.1 + dy);
        } else {
            transforms.push((dx, dy));
        }
    });
}

unsafe extern "C" fn widget_create(
    token: PluginToken,
    data: *const WidgetDataV1,
    out_id: *mut ResourceId,
) -> PluginResultC {
    if out_id.is_null() {
        return PluginResultC::err("widget output pointer is null");
    }
    // SAFETY: Validation is performed by read_struct before the value is used.
    let data = match unsafe { read_widget_data(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    if data.span_cols == 0
        || data.span_cols > crate::core::config::WIDGET_GRID_COLS as u32
        || data.span_rows == 0
        || data.span_rows > crate::core::config::WIDGET_GRID_ROWS as u32
    {
        return PluginResultC::err("widget span is out of range");
    }
    if data.flags & !super::types::WIDGET_FLAG_SHOW_COMPACT != 0 {
        return PluginResultC::err("widget contains unknown flags");
    }
    if data.on_draw.is_none() {
        return PluginResultC::err("widget render callback is required");
    }
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_capability(&state, token, CAPABILITY_WIDGET) {
        return PluginResultC::err(error);
    }
    if resource_count(&state, token, ResourceKind::Widget) >= MAX_WIDGETS_PER_PLUGIN {
        return PluginResultC::err("widget resource limit reached");
    }
    let plugin_id = match state.plugins.get(&token) {
        Some(plugin) => plugin.id.clone(),
        None => return PluginResultC::err("invalid plugin token"),
    };
    let key = match validate_widget_key(&data.key) {
        Ok(key) => key,
        Err(error) => return PluginResultC::err(error),
    };
    if key
        .as_ref()
        .is_some_and(|key| state.widget_keys.contains_key(&(token, key.to_string())))
    {
        return PluginResultC::err("widget key is already registered by this plugin");
    }
    let id = next_id(&NEXT_RESOURCE_ID);
    let widget = widget_from_ffi(&plugin_id, id, &data);
    let size_bytes = widget.title.len() + widget.body.len();
    state.resources.insert(
        id,
        ResourceOwner {
            plugin: token,
            kind: ResourceKind::Widget,
            size_bytes,
        },
    );
    if let Some(key) = key {
        state.widget_keys.insert((token, key), id);
    }
    state.widget_events.insert(id, WidgetEvent::Upsert(widget));
    // SAFETY: out_id was checked non-null and belongs to the caller.
    unsafe { out_id.write(id) };
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn widget_update(
    token: PluginToken,
    id: ResourceId,
    data: *const WidgetDataV1,
) -> PluginResultC {
    // SAFETY: Validation is performed by read_struct before the value is used.
    let data = match unsafe { read_widget_data(data) } {
        Ok(data) => data,
        Err(error) => return PluginResultC::err(error),
    };
    if data.span_cols == 0
        || data.span_cols > crate::core::config::WIDGET_GRID_COLS as u32
        || data.span_rows == 0
        || data.span_rows > crate::core::config::WIDGET_GRID_ROWS as u32
    {
        return PluginResultC::err("widget span is out of range");
    }
    if data.flags & !super::types::WIDGET_FLAG_SHOW_COMPACT != 0 {
        return PluginResultC::err("widget contains unknown flags");
    }
    if data.on_draw.is_none() {
        return PluginResultC::err("widget render callback is required");
    }
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Widget) {
        return PluginResultC::err(error);
    }
    let key = match validate_widget_key(&data.key) {
        Ok(key) => key,
        Err(error) => return PluginResultC::err(error),
    };
    let existing_key = state
        .widget_keys
        .iter()
        .find_map(|((owner, key), resource)| {
            (*owner == token && *resource == id).then_some(key.as_str())
        });
    if existing_key != key.as_deref() {
        return PluginResultC::err("widget key cannot change after creation");
    }
    let plugin_id = match state.plugins.get(&token) {
        Some(plugin) => plugin.id.clone(),
        None => return PluginResultC::err("invalid plugin token"),
    };
    let widget = widget_from_ffi(&plugin_id, id, &data);
    let size_bytes = widget.title.len() + widget.body.len();
    if let Some(owner) = state.resources.get_mut(&id) {
        owner.size_bytes = size_bytes;
    }
    state.widget_events.insert(id, WidgetEvent::Upsert(widget));
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

unsafe extern "C" fn widget_release(token: PluginToken, id: ResourceId) -> PluginResultC {
    let mut state = match runtime().lock() {
        Ok(state) => state,
        Err(_) => return PluginResultC::err("plugin runtime lock is poisoned"),
    };
    if let Err(error) = require_resource(&state, token, id, ResourceKind::Widget) {
        return PluginResultC::err(error);
    }
    state.resources.remove(&id);
    state
        .widget_keys
        .retain(|_, resource_id| *resource_id != id);
    state.widget_events.remove(&id);
    state.widget_events.insert(id, WidgetEvent::Remove(id));
    drop(state);
    crate::utils::event_loop::wake();
    PluginResultC::ok()
}

pub fn update_host_state(state: HostState) {
    if let Ok(mut runtime) = runtime().lock() {
        runtime.host_state = state;
    }
}

struct ActiveLyricsTransformer {
    resource_id: ResourceId,
    plugin_id: String,
    on_transform: LyricsTransformFnV1,
    callback_data: usize,
}

struct LyricsTransformLease {
    transformers: Vec<ActiveLyricsTransformer>,
}

impl LyricsTransformLease {
    fn acquire() -> Self {
        let Ok(mut state) = runtime().lock() else {
            return Self {
                transformers: Vec::new(),
            };
        };
        let mut available = state
            .lyrics_transformers
            .iter()
            .filter_map(|(&resource_id, transformer)| {
                let owner = state.resources.get(&resource_id)?;
                let plugin = state.plugins.get(&owner.plugin)?;
                (!plugin.stopping)
                    .then(|| (resource_id, transformer.sequence, plugin.id.to_string()))
            })
            .collect::<Vec<_>>();
        available.sort_by_key(|(_, sequence, _)| *sequence);
        let transformers = available
            .into_iter()
            .filter_map(|(resource_id, _, plugin_id)| {
                let transformer = state.lyrics_transformers.get_mut(&resource_id)?;
                transformer.in_flight = transformer.in_flight.saturating_add(1);
                Some(ActiveLyricsTransformer {
                    resource_id,
                    plugin_id,
                    on_transform: transformer.on_transform,
                    callback_data: transformer.callback_data,
                })
            })
            .collect();
        Self { transformers }
    }
}

impl Drop for LyricsTransformLease {
    fn drop(&mut self) {
        if let Ok(mut state) = runtime().lock() {
            for transformer in &self.transformers {
                if let Some(resource) = state.lyrics_transformers.get_mut(&transformer.resource_id)
                {
                    resource.in_flight = resource.in_flight.saturating_sub(1);
                }
            }
        }
    }
}

fn transform_lyric_text(
    transformer: &ActiveLyricsTransformer,
    input: &LyricsTextV1,
) -> Result<String, String> {
    let mut required = 0u32;
    // SAFETY: The callback belongs to a leased, loaded plugin. Input and out_len
    // remain valid for this synchronous size-query call.
    unsafe {
        (transformer.on_transform)(
            transformer.callback_data as *mut c_void,
            transformer.resource_id,
            input,
            std::ptr::null_mut(),
            0,
            &mut required,
        )
    }
    .into_result()?;
    if required > MAX_TRANSFORMED_LYRIC_BYTES {
        return Err("transformed lyric line exceeds 256 KiB".to_string());
    }
    if required == 0 {
        return Ok(String::new());
    }

    let mut output = vec![0u8; required as usize];
    let mut written = required;
    // SAFETY: The callback belongs to a leased, loaded plugin. The output buffer
    // and out_len remain writable for this synchronous transform call.
    unsafe {
        (transformer.on_transform)(
            transformer.callback_data as *mut c_void,
            transformer.resource_id,
            input,
            output.as_mut_ptr(),
            required,
            &mut written,
        )
    }
    .into_result()?;
    if written > required {
        return Err("lyrics transform wrote beyond the advertised length".to_string());
    }
    output.truncate(written as usize);
    String::from_utf8(output).map_err(|_| "lyrics transform returned invalid UTF-8".to_string())
}

pub fn apply_lyrics_transforms(
    mut lyrics: Arc<Vec<crate::core::lyrics::LyricLine>>,
) -> Arc<Vec<crate::core::lyrics::LyricLine>> {
    let lease = LyricsTransformLease::acquire();
    if lease.transformers.is_empty() {
        return lyrics;
    }

    let mut reported_errors = HashSet::new();
    for line in Arc::make_mut(&mut lyrics) {
        for transformer in &lease.transformers {
            let input = LyricsTextV1 {
                flags: if line.is_word_synced() {
                    LYRICS_TEXT_FLAG_WORD_SYNCED
                } else {
                    0
                },
                line_time_ms: line.time_ms,
                text: Utf8SliceV1::borrowed(&line.text),
                ..Default::default()
            };
            let transformed = match transform_lyric_text(transformer, &input) {
                Ok(transformed) => transformed,
                Err(error) => {
                    if reported_errors.insert(transformer.resource_id) {
                        log::warn!(
                            "Lyrics transformer '{}' failed: {error}",
                            transformer.plugin_id
                        );
                    }
                    continue;
                }
            };
            if !line.replace_text_preserving_timings(transformed)
                && reported_errors.insert(transformer.resource_id)
            {
                log::warn!(
                    "Lyrics transformer '{}' changed the character count of a word-synced line",
                    transformer.plugin_id
                );
            }
        }
    }
    lyrics
}

pub fn update_host_media(title: &str, artist: &str, is_playing: bool) {
    if let Ok(mut runtime) = runtime().lock() {
        if runtime.host_state.media_title != title {
            title.clone_into(&mut runtime.host_state.media_title);
        }
        if runtime.host_state.media_artist != artist {
            artist.clone_into(&mut runtime.host_state.media_artist);
        }
        runtime.host_state.is_playing = is_playing;
    }
}

pub fn update_host_theme(is_light: bool) {
    if let Ok(mut runtime) = runtime().lock() {
        runtime.host_state.theme = if is_light {
            "light".to_string()
        } else {
            "dark".to_string()
        };
    }
}

pub fn drain_pending_contexts(manager: &mut crate::core::context::ContextManager) {
    let events = match runtime().lock() {
        Ok(mut runtime) => {
            let events = runtime
                .context_events
                .drain()
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            for event in &events {
                match event {
                    ContextEvent::Upsert(context) => {
                        runtime.visible_contexts.insert(context.id);
                    }
                    ContextEvent::Remove(id) => {
                        runtime.visible_contexts.remove(id);
                    }
                }
            }
            events
        }
        Err(_) => return,
    };
    for event in events {
        match event {
            ContextEvent::Upsert(context) => manager.upsert_context(context),
            ContextEvent::Remove(id) => {
                manager.remove_context(id);
            }
        }
    }
}

pub fn drain_widget_events(manager: &mut crate::core::plugin_widget::WidgetManager) -> bool {
    let events = match runtime().try_lock() {
        Ok(mut runtime) => runtime
            .widget_events
            .drain()
            .map(|(_, event)| event)
            .collect::<Vec<_>>(),
        Err(_) => return false,
    };
    let changed = !events.is_empty();
    for event in events {
        match event {
            WidgetEvent::Upsert(widget) => manager.upsert_widget(widget),
            WidgetEvent::Remove(id) => {
                manager.remove_widget(id);
            }
        }
    }
    changed
}

pub fn drain_media_source_event() -> Option<MediaSourceEvent> {
    let mut runtime = runtime().lock().ok()?;
    if !runtime.media_dirty {
        return None;
    }
    runtime.media_dirty = false;
    Some(
        runtime
            .media
            .values()
            .max_by_key(|media| media.sequence)
            .map_or(MediaSourceEvent::Clear, |media| {
                MediaSourceEvent::Set(media.data.clone())
            }),
    )
}

pub fn dispatch_media_command(
    resource_id: ResourceId,
    command: u32,
    position_ms: u64,
) -> Result<(), String> {
    let required_control = match command {
        super::types::MEDIA_COMMAND_TOGGLE_PLAY => super::types::MEDIA_CONTROL_TOGGLE_PLAY,
        super::types::MEDIA_COMMAND_PREVIOUS => super::types::MEDIA_CONTROL_PREVIOUS,
        super::types::MEDIA_COMMAND_NEXT => super::types::MEDIA_CONTROL_NEXT,
        super::types::MEDIA_COMMAND_SEEK => super::types::MEDIA_CONTROL_SEEK,
        _ => return Err("unknown media command".to_string()),
    };
    let (callback, callback_data) = {
        let mut runtime = runtime()
            .lock()
            .map_err(|_| "plugin runtime lock is poisoned".to_string())?;
        let plugin_token = runtime
            .resources
            .get(&resource_id)
            .filter(|owner| owner.kind == ResourceKind::Media)
            .map(|owner| owner.plugin)
            .ok_or_else(|| "media resource was not found".to_string())?;
        if runtime
            .plugins
            .get(&plugin_token)
            .is_none_or(|plugin| plugin.stopping)
        {
            return Err("plugin is shutting down".to_string());
        }
        let media = runtime
            .media
            .get_mut(&resource_id)
            .ok_or_else(|| "media resource was not found".to_string())?;
        if media.data.available_controls & required_control == 0 {
            return Err("media control is not supported".to_string());
        }
        let callback = media
            .on_command
            .ok_or_else(|| "media command callback is missing".to_string())?;
        media.in_flight = media.in_flight.saturating_add(1);
        (callback, media.callback_data)
    };
    let command = MediaCommandV1 {
        struct_size: std::mem::size_of::<MediaCommandV1>() as u32,
        command,
        position_ms,
    };
    // SAFETY: Media callbacks are invoked on the winit thread while the plugin is loaded.
    unsafe { callback(callback_data as *mut c_void, resource_id, &command) };
    if let Ok(mut runtime) = runtime().lock()
        && let Some(media) = runtime.media.get_mut(&resource_id)
    {
        media.in_flight = media.in_flight.saturating_sub(1);
    }
    Ok(())
}

fn register_plugin(
    token: PluginToken,
    plugin_id: &str,
    capabilities: u64,
) -> Result<(), PluginError> {
    let mut runtime = runtime()
        .lock()
        .map_err(|_| PluginError::ExecutionError("plugin runtime lock is poisoned".to_string()))?;
    runtime.plugins.insert(
        token,
        PluginRegistration {
            id: plugin_id.to_string(),
            capabilities,
            stopping: false,
        },
    );
    Ok(())
}

fn begin_plugin_shutdown(token: PluginToken) -> Result<bool, PluginError> {
    let mut runtime = runtime()
        .lock()
        .map_err(|_| PluginError::ExecutionError("plugin runtime lock is poisoned".to_string()))?;
    let was_stopping = runtime
        .plugins
        .get(&token)
        .ok_or_else(|| PluginError::ExecutionError("plugin token is not registered".to_string()))?
        .stopping;
    let callback_in_progress = runtime.resources.iter().any(|(&id, owner)| {
        if owner.plugin != token {
            return false;
        }
        match owner.kind {
            ResourceKind::Media => runtime
                .media
                .get(&id)
                .is_some_and(|media| media.in_flight != 0),
            ResourceKind::LyricsTransform => runtime
                .lyrics_transformers
                .get(&id)
                .is_some_and(|transformer| transformer.in_flight != 0),
            _ => false,
        }
    });
    if callback_in_progress {
        return Err(PluginError::ExecutionError(
            "plugin callback is in progress".to_string(),
        ));
    }
    if let Some(plugin) = runtime.plugins.get_mut(&token) {
        plugin.stopping = true;
    }
    Ok(was_stopping)
}

fn restore_plugin_shutdown_state(token: PluginToken, stopping: bool) {
    if let Ok(mut runtime) = runtime().lock()
        && let Some(plugin) = runtime.plugins.get_mut(&token)
    {
        plugin.stopping = stopping;
    }
}

fn revoke_plugin(token: PluginToken) {
    let i18n_resources = {
        let Ok(mut runtime) = runtime().lock() else {
            return;
        };
        runtime.plugins.remove(&token);
        runtime.widget_keys.retain(|(owner, _), _| *owner != token);
        let resources = runtime
            .resources
            .iter()
            .filter_map(|(&id, owner)| (owner.plugin == token).then_some((id, owner.kind)))
            .collect::<Vec<_>>();
        let mut i18n_resources = Vec::new();
        let mut media_removed = false;
        for (id, kind) in resources {
            runtime.resources.remove(&id);
            match kind {
                ResourceKind::Context => {
                    runtime.context_events.remove(&id);
                    if runtime.visible_contexts.remove(&id) {
                        runtime.context_events.insert(id, ContextEvent::Remove(id));
                    }
                }
                ResourceKind::Media => {
                    runtime.media.remove(&id);
                    media_removed = true;
                }
                ResourceKind::I18n => i18n_resources.push(id),
                ResourceKind::Widget => {
                    runtime.widget_events.remove(&id);
                    runtime.widget_events.insert(id, WidgetEvent::Remove(id));
                }
                ResourceKind::LyricsTransform => {
                    runtime.lyrics_transformers.remove(&id);
                }
            }
        }
        runtime.media_dirty |= media_removed;
        i18n_resources
    };
    for id in i18n_resources {
        if let Err(error) = crate::core::i18n::release_plugin_translation_bundle(id) {
            log::error!("Failed to release plugin translation bundle {id}: {error}");
        }
    }
    crate::utils::event_loop::wake();
}

pub struct PluginManager {
    entries: RefCell<Vec<NativePlugin>>,
    plugin_dir: PathBuf,
}

impl PluginManager {
    pub fn new<P: AsRef<Path>>(plugin_dir: P) -> Self {
        let plugin_dir = plugin_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&plugin_dir);
        Self {
            entries: RefCell::new(Vec::new()),
            plugin_dir,
        }
    }

    pub fn load_all(&self) {
        let dlls = discover_plugins(&self.plugin_dir);
        let disabled = disabled_plugin_ids(&self.plugin_dir);
        log::info!(
            "Discovering ABI v1 plugins in {}: {} DLL(s) found",
            self.plugin_dir.display(),
            dlls.len()
        );
        for (path, manifest) in dlls {
            if let Some(manifest) = manifest.as_ref()
                && disabled.contains(&manifest.id)
            {
                log::info!("Plugin '{}' is disabled", manifest.id);
                continue;
            }
            if let Err(error) = self.load_dll_checked(&path, manifest.as_ref()) {
                log::warn!("Failed to load plugin '{}': {error}", path.display());
            }
        }
    }

    pub(crate) fn load_dll(&self, path: &Path) {
        if let Err(error) = self.load_dll_checked(path, None) {
            log::warn!("Failed to load plugin '{}': {error}", path.display());
        }
    }

    fn load_dll_checked(
        &self,
        path: &Path,
        manifest: Option<&PluginManifest>,
    ) -> Result<(), PluginError> {
        let mut plugin = NativePlugin::load(path)?;
        let plugin_id = plugin.metadata().id.clone();
        if let Some(manifest) = manifest {
            validate_manifest_metadata(manifest, plugin.metadata())?;
        }
        if disabled_plugin_ids(&self.plugin_dir).contains(&plugin_id) {
            log::info!("Plugin '{}' is disabled", plugin_id);
            return Ok(());
        }
        let mut entries = self.entries.try_borrow_mut().map_err(|_| {
            PluginError::ExecutionError("plugin list is already borrowed".to_string())
        })?;
        if entries.iter().any(|entry| entry.metadata().id == plugin_id) {
            return Err(PluginError::InvalidPlugin(format!(
                "plugin '{}' is already loaded",
                plugin_id
            )));
        }

        let token = next_id(&NEXT_PLUGIN_TOKEN);
        register_plugin(token, &plugin_id, plugin.capabilities())?;
        if let Err(error) = plugin.initialize(token, host_api()) {
            if let Err(shutdown_error) = begin_plugin_shutdown(token) {
                entries.push(plugin);
                return Err(PluginError::ExecutionError(format!(
                    "{error}; cleanup could not start: {shutdown_error}"
                )));
            }
            match plugin.shutdown() {
                Ok(()) => {
                    revoke_plugin(token);
                    return Err(error);
                }
                Err(shutdown_error) => {
                    entries.push(plugin);
                    return Err(PluginError::ExecutionError(format!(
                        "{error}; cleanup also failed: {shutdown_error}"
                    )));
                }
            }
        }
        log::info!(
            "Loaded ABI v1 plugin: {} v{} by {} ({})",
            plugin.metadata().name,
            plugin.metadata().version,
            plugin.metadata().author,
            plugin_id
        );
        log::debug!("Plugin description: {}", plugin.metadata().description);
        entries.push(plugin);
        Ok(())
    }

    pub fn unload(&self, plugin_id: &str) -> Result<(), PluginError> {
        if self.unload_if_loaded(plugin_id)? {
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    pub fn unload_if_loaded(&self, plugin_id: &str) -> Result<bool, PluginError> {
        let mut entries = self.entries.try_borrow_mut().map_err(|_| {
            PluginError::ExecutionError("plugin list is already borrowed".to_string())
        })?;
        let Some(index) = entries
            .iter()
            .position(|plugin| plugin.metadata().id == plugin_id)
        else {
            return Ok(false);
        };
        let token = entries[index].token();
        let was_stopping = begin_plugin_shutdown(token)?;
        if let Err(error) = entries[index].shutdown() {
            restore_plugin_shutdown_state(token, was_stopping);
            return Err(error);
        }
        let plugin = entries.remove(index);
        revoke_plugin(token);
        drop(plugin);
        Ok(true)
    }

    pub fn len(&self) -> usize {
        self.entries.try_borrow().map_or(0, |entries| entries.len())
    }

    pub fn installed_plugins(&self) -> Vec<InstalledPlugin> {
        let disabled = disabled_plugin_ids(&self.plugin_dir);
        collect_installed_plugins(&self.plugin_dir, &disabled, self.loaded_plugin_snapshot())
    }

    pub fn installed_plugins_async(&self) -> mpsc::Receiver<Vec<InstalledPlugin>> {
        let plugin_dir = self.plugin_dir.clone();
        let loaded_plugins = self.loaded_plugin_snapshot();
        let (tx, rx) = mpsc::channel();
        let spawn_result = std::thread::Builder::new()
            .name("winisland-plugin-scan".to_string())
            .spawn(move || {
                let disabled = disabled_plugin_ids(&plugin_dir);
                let plugins = collect_installed_plugins(&plugin_dir, &disabled, loaded_plugins);
                let _ = tx.send(plugins);
                crate::utils::event_loop::wake();
            });
        if let Err(error) = spawn_result {
            log::warn!("Failed to start plugin scan: {error}");
        }
        rx
    }

    fn loaded_plugin_snapshot(&self) -> Vec<InstalledPlugin> {
        let Ok(entries) = self.entries.try_borrow() else {
            return Vec::new();
        };
        entries
            .iter()
            .map(|plugin| {
                let metadata = plugin.metadata();
                InstalledPlugin {
                    id: metadata.id.clone(),
                    name: metadata.name.clone(),
                    author: metadata.author.clone(),
                    version: metadata.version.clone(),
                    description: metadata.description.clone(),
                    github_link: String::new(),
                    enabled: true,
                    icon: None,
                    readme: None,
                }
            })
            .collect()
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<(), String> {
        validate_plugin_id(plugin_id)?;
        if !self
            .installed_plugins()
            .iter()
            .any(|plugin| plugin.id == plugin_id)
        {
            return Err(format!("Plugin '{plugin_id}' is not installed"));
        }
        let mut disabled = disabled_plugin_ids(&self.plugin_dir);
        if enabled {
            disabled.remove(plugin_id);
        } else {
            disabled.insert(plugin_id.to_string());
        }
        write_disabled_plugin_ids(&self.plugin_dir, &disabled)
    }

    pub fn uninstall_plugin(&self, plugin_id: &str) -> Result<(), String> {
        validate_plugin_id(plugin_id)?;
        let targets = uninstall_targets(&self.plugin_dir, plugin_id);
        if targets.is_empty() {
            return Err(format!("Plugin '{plugin_id}' is not installed"));
        }

        let loaded_source = self.entries.try_borrow().ok().and_then(|entries| {
            entries
                .iter()
                .find(|plugin| plugin.metadata().id == plugin_id)
                .map(|plugin| plugin.path().to_path_buf())
        });
        let was_loaded = self
            .unload_if_loaded(plugin_id)
            .map_err(|error| error.to_string())?;
        let uninstall_id = NEXT_BACKUP_ID.fetch_add(1, Ordering::Relaxed);
        let mut moved = Vec::with_capacity(targets.len());
        for (index, source) in targets.into_iter().enumerate() {
            let backup = self.plugin_dir.join(format!(
                ".{plugin_id}.uninstall-{}-{uninstall_id}-{index}",
                std::process::id()
            ));
            if let Err(error) = std::fs::rename(&source, &backup) {
                let rollback = restore_uninstall_targets(&moved);
                let reload = if was_loaded {
                    reload_plugin_source(self, loaded_source.as_deref())
                } else {
                    Ok(())
                };
                return Err(rollback_message(
                    format!("Cannot remove plugin files: {error}"),
                    rollback.and(reload),
                ));
            }
            moved.push((source, backup));
        }

        let mut disabled = disabled_plugin_ids(&self.plugin_dir);
        disabled.remove(plugin_id);
        if let Err(error) = write_disabled_plugin_ids(&self.plugin_dir, &disabled) {
            let rollback = restore_uninstall_targets(&moved);
            let reload = if was_loaded {
                reload_plugin_source(self, loaded_source.as_deref())
            } else {
                Ok(())
            };
            return Err(rollback_message(error, rollback.and(reload)));
        }

        for (_, backup) in moved {
            let result = if backup.is_dir() {
                std::fs::remove_dir_all(&backup)
            } else {
                std::fs::remove_file(&backup)
            };
            if let Err(error) = result {
                log::warn!(
                    "Plugin '{}' was uninstalled, but temporary files '{}' could not be removed: {error}",
                    plugin_id,
                    backup.display()
                );
            }
        }
        Ok(())
    }

    pub fn install_from_zip(&self, path: &Path) -> Result<PluginManifest, String> {
        let (manifest, staging) = zip_loader::extract_plugin(path, &self.plugin_dir)?;
        if let Err(error) = self.activate_staged_plugin(&manifest, &staging) {
            let _ = std::fs::remove_dir_all(staging);
            return Err(error);
        }
        Ok(manifest)
    }

    pub fn activate_staged_plugin(
        &self,
        manifest: &PluginManifest,
        staging: &Path,
    ) -> Result<(), String> {
        let staged_entry = staging.join(&manifest.entry);
        let validation = NativePlugin::load(&staged_entry).map_err(|error| error.to_string())?;
        validate_manifest_metadata(manifest, validation.metadata())
            .map_err(|error| error.to_string())?;
        drop(validation);

        let destination = self.plugin_dir.join(manifest.safe_dir_name());
        let old_source = self.entries.try_borrow().ok().and_then(|entries| {
            entries
                .iter()
                .find(|plugin| plugin.metadata().id == manifest.id)
                .map(|plugin| plugin.path().to_path_buf())
        });
        let old_relative = old_source
            .as_ref()
            .and_then(|path| path.strip_prefix(&destination).ok().map(Path::to_path_buf));
        if old_source.is_some() && old_relative.is_none() {
            return Err(
                "Cannot replace a manually installed root DLL with a packaged plugin".to_string(),
            );
        }
        self.unload_if_loaded(&manifest.id)
            .map_err(|error| error.to_string())?;

        let backup = self.plugin_dir.join(format!(
            ".{}.backup-{}-{}",
            manifest.safe_dir_name(),
            std::process::id(),
            NEXT_BACKUP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let had_previous = destination.exists();
        if had_previous && let Err(error) = std::fs::rename(&destination, &backup) {
            let reload = reload_previous(
                self,
                old_source.as_deref(),
                old_relative.as_deref(),
                &destination,
            );
            return Err(rollback_message(
                format!("Cannot back up the existing plugin: {error}"),
                reload,
            ));
        }
        if let Err(error) = std::fs::rename(staging, &destination) {
            let mut message = format!("Cannot activate the staged plugin: {error}");
            if had_previous && let Err(restore_error) = std::fs::rename(&backup, &destination) {
                message.push_str(&format!(
                    "; cannot restore the previous plugin directory: {restore_error}"
                ));
            }
            let reload = reload_previous(
                self,
                old_source.as_deref(),
                old_relative.as_deref(),
                &destination,
            );
            return Err(rollback_message(message, reload));
        }

        let new_entry = destination.join(&manifest.entry);
        if let Err(error) = self.load_dll_checked(&new_entry, Some(manifest)) {
            if self.is_loaded(&manifest.id) {
                return Err(format!(
                    "New plugin failed after creating a non-stoppable instance: {error}"
                ));
            }
            let mut message = format!("Cannot initialize the new plugin: {error}");
            if let Err(move_error) = std::fs::rename(&destination, staging)
                && let Err(remove_error) = std::fs::remove_dir_all(&destination)
            {
                message.push_str(&format!(
                    "; cannot remove the failed plugin directory ({move_error}; {remove_error})"
                ));
            }
            if had_previous && let Err(restore_error) = std::fs::rename(&backup, &destination) {
                message.push_str(&format!(
                    "; cannot restore the previous plugin directory: {restore_error}"
                ));
            }
            let reload = reload_previous(
                self,
                old_source.as_deref(),
                old_relative.as_deref(),
                &destination,
            );
            return Err(rollback_message(message, reload));
        }
        if had_previous && let Err(error) = std::fs::remove_dir_all(&backup) {
            log::warn!(
                "Cannot remove plugin backup '{}': {error}",
                backup.display()
            );
        }
        Ok(())
    }

    fn is_loaded(&self, plugin_id: &str) -> bool {
        self.entries.try_borrow().is_ok_and(|entries| {
            entries
                .iter()
                .any(|plugin| plugin.metadata().id == plugin_id)
        })
    }

    pub fn read_manifest_from_zip(&self, path: &Path) -> Result<PluginManifest, String> {
        zip_loader::read_manifest_from_zip(path)
    }

    pub fn validate_zip(&self, path: &Path) -> Result<(), String> {
        zip_loader::validate_zip(path)
    }

    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        for mut plugin in self.entries.get_mut().drain(..) {
            let token = plugin.token();
            if let Err(error) = begin_plugin_shutdown(token) {
                log::error!("{error}; keeping the plugin DLL loaded");
                std::mem::forget(plugin);
                continue;
            }
            match plugin.shutdown() {
                Ok(()) => revoke_plugin(token),
                Err(error) => {
                    log::error!("{error}; keeping the plugin DLL loaded");
                    std::mem::forget(plugin);
                }
            }
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        let directory = dirs::config_dir()
            .unwrap_or_default()
            .join("WinIsland")
            .join("plugins");
        Self::new(directory)
    }
}

fn discover_plugins(directory: &Path) -> Vec<(PathBuf, Option<PluginManifest>)> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }
            let manifest_path = path.join("plugin.yml");
            match zip_loader::read_manifest_file(&manifest_path) {
                Ok(manifest) => {
                    let entry = path.join(&manifest.entry);
                    if entry.is_file() {
                        plugins.push((entry, Some(manifest)));
                    } else {
                        log::warn!("Plugin entry '{}' is missing", entry.display());
                    }
                }
                Err(error) if manifest_path.exists() => {
                    log::warn!("Skipping '{}': {error}", path.display());
                }
                Err(_) => (),
            }
        } else if path.extension().is_some_and(|ext| ext == "dll") {
            plugins.push((path, None));
        }
    }
    plugins
}

fn discover_packaged_plugins(directory: &Path, disabled: &HashSet<String>) -> Vec<InstalledPlugin> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let directory = entry.path();
            if !directory.is_dir()
                || directory
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'))
            {
                return None;
            }
            let manifest = zip_loader::read_manifest_file(&directory.join("plugin.yml")).ok()?;
            let icon = plugin_asset_path(
                &directory,
                manifest.icon.as_deref(),
                &["icon.png", "icon.jpg", "icon.jpeg", "icon.webp"],
            )
            .and_then(|path| read_plugin_icon(&path));
            let readme = plugin_asset_path(
                &directory,
                manifest.readme.as_deref(),
                &["README.md", "README.markdown", "README.txt"],
            )
            .and_then(|path| {
                zip_loader::read_bounded_file(&path, zip_loader::MAX_PLUGIN_README_BYTES).ok()
            })
            .and_then(|bytes| String::from_utf8(bytes).ok());
            Some(InstalledPlugin {
                enabled: !disabled.contains(&manifest.id),
                id: manifest.id,
                name: manifest.name,
                author: manifest.author,
                version: manifest.version,
                description: manifest.description,
                github_link: manifest.github_link,
                icon,
                readme,
            })
        })
        .collect()
}

fn collect_installed_plugins(
    directory: &Path,
    disabled: &HashSet<String>,
    loaded_plugins: Vec<InstalledPlugin>,
) -> Vec<InstalledPlugin> {
    let mut plugins = discover_packaged_plugins(directory, disabled);
    for mut plugin in loaded_plugins {
        if plugins.iter().any(|entry| entry.id == plugin.id) {
            continue;
        }
        plugin.enabled = !disabled.contains(&plugin.id);
        plugins.push(plugin);
    }
    for path in discover_manual_plugin_dlls(directory) {
        let Ok(plugin) = NativePlugin::load(&path) else {
            continue;
        };
        let metadata = plugin.metadata();
        if plugins.iter().any(|entry| entry.id == metadata.id) {
            continue;
        }
        plugins.push(InstalledPlugin {
            id: metadata.id.clone(),
            name: metadata.name.clone(),
            author: metadata.author.clone(),
            version: metadata.version.clone(),
            description: metadata.description.clone(),
            github_link: String::new(),
            enabled: !disabled.contains(&metadata.id),
            icon: None,
            readme: None,
        });
    }
    plugins.sort_by_key(|plugin| plugin.name.to_lowercase());
    plugins
}

fn discover_manual_plugin_dlls(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
        })
        .collect()
}

fn uninstall_targets(directory: &Path, plugin_id: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                return None;
            }
            if path.is_dir() {
                return zip_loader::read_manifest_file(&path.join("plugin.yml"))
                    .ok()
                    .filter(|manifest| manifest.id == plugin_id)
                    .map(|_| path);
            }
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            {
                return None;
            }
            NativePlugin::load(&path)
                .ok()
                .filter(|plugin| plugin.metadata().id == plugin_id)
                .map(|_| path)
        })
        .collect()
}

fn restore_uninstall_targets(moved: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    let mut errors = Vec::new();
    for (source, backup) in moved.iter().rev() {
        if let Err(error) = std::fs::rename(backup, source) {
            errors.push(format!("cannot restore '{}': {error}", source.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn reload_plugin_source(manager: &PluginManager, source: Option<&Path>) -> Result<(), String> {
    let Some(source) = source else {
        return Ok(());
    };
    let manifest = source
        .parent()
        .and_then(|directory| zip_loader::read_manifest_file(&directory.join("plugin.yml")).ok());
    manager
        .load_dll_checked(source, manifest.as_ref())
        .map_err(|error| format!("cannot reload previous plugin: {error}"))
}

fn read_plugin_icon(path: &Path) -> Option<Vec<u8>> {
    let bytes = zip_loader::read_bounded_file(path, zip_loader::MAX_PLUGIN_ICON_BYTES).ok()?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    (width > 0 && height > 0 && width <= 2048 && height <= 2048).then_some(bytes)
}

fn plugin_asset_path(
    directory: &Path,
    declared: Option<&str>,
    fallbacks: &[&str],
) -> Option<PathBuf> {
    declared
        .map(|path| directory.join(path))
        .filter(|path| path.is_file())
        .or_else(|| {
            fallbacks
                .iter()
                .map(|path| directory.join(path))
                .find(|path| path.is_file())
        })
}

fn disabled_plugin_ids(directory: &Path) -> HashSet<String> {
    std::fs::read_to_string(directory.join(DISABLED_PLUGINS_FILE))
        .ok()
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .filter(|line| {
                    !line.is_empty()
                        && line.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
                        })
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn validate_plugin_id(plugin_id: &str) -> Result<(), String> {
    if plugin_id.is_empty()
        || !plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("Invalid plugin ID".to_string());
    }
    Ok(())
}

fn write_disabled_plugin_ids(directory: &Path, ids: &HashSet<String>) -> Result<(), String> {
    let path = directory.join(DISABLED_PLUGINS_FILE);
    let mut ids = ids.iter().collect::<Vec<_>>();
    ids.sort_unstable();
    if ids.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("Cannot remove '{}': {error}", path.display()))?;
        }
        return Ok(());
    }
    let contents = ids
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&path, contents)
        .map_err(|error| format!("Cannot write '{}': {error}", path.display()))
}

fn validate_manifest_metadata(
    manifest: &PluginManifest,
    metadata: &super::types::PluginMetadata,
) -> Result<(), PluginError> {
    let mismatched = manifest.id != metadata.id
        || manifest.name != metadata.name
        || manifest.version != metadata.version
        || manifest.author != metadata.author
        || manifest.description != metadata.description;
    if mismatched {
        return Err(PluginError::InvalidPlugin(
            "plugin.yml metadata does not match the DLL descriptor".to_string(),
        ));
    }
    Ok(())
}

fn reload_previous(
    manager: &PluginManager,
    old_source: Option<&Path>,
    old_relative: Option<&Path>,
    destination: &Path,
) -> Result<(), String> {
    let Some(path) = old_relative
        .map(|relative| destination.join(relative))
        .or_else(|| old_source.map(Path::to_path_buf))
    else {
        return Ok(());
    };
    if !path.is_file() {
        return Err(format!(
            "previous plugin entry '{}' is missing",
            path.display()
        ));
    }
    let manifest = zip_loader::read_manifest_file(&destination.join("plugin.yml")).ok();
    manager
        .load_dll_checked(&path, manifest.as_ref())
        .map_err(|error| {
            format!(
                "cannot reload previous plugin '{}': {error}",
                path.display()
            )
        })
}

fn rollback_message(mut message: String, rollback: Result<(), String>) -> String {
    if let Err(error) = rollback {
        message.push_str(&format!("; rollback also failed: {error}"));
    }
    message
}

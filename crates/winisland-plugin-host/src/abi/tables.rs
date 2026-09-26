use std::ffi::c_void;

use winisland_plugin_api::abi::{
    self, ContextApiV2, HostStateApiV2, I18nApiV2, ImageApiV2, LogApiV2, LyricsTransformApiV2,
    MediaApiV2, PluginHostV2, SettingsApiV2, StoreApiV2, TablePrefix, TextApiV2, WidgetApiV2,
};

use crate::runtime::HostRuntime;
use crate::services::{
    context, host_state, i18n, image, log, lyrics, media, settings, store, text, widget,
};

pub(crate) struct AbiTables {
    pub host: PluginHostV2,
    context: ContextApiV2,
    media: MediaApiV2,
    i18n: I18nApiV2,
    host_state: HostStateApiV2,
    widget: WidgetApiV2,
    lyrics: LyricsTransformApiV2,
    settings: SettingsApiV2,
    text: TextApiV2,
    image: ImageApiV2,
    store: StoreApiV2,
    log: LogApiV2,
}

fn prefix<T>(version: u32) -> TablePrefix {
    TablePrefix {
        struct_size: std::mem::size_of::<T>() as u32,
        version,
        context: std::ptr::null_mut(),
    }
}

impl AbiTables {
    pub fn new(host_build: u32) -> Self {
        Self {
            host: PluginHostV2 {
                prefix: prefix::<PluginHostV2>(abi::ABI_VERSION_2),
                host_build,
                query: Some(query),
            },
            context: ContextApiV2 {
                prefix: prefix::<ContextApiV2>(abi::IFACE_VERSION_1),
                create: Some(context::create),
                update: Some(context::update),
                release: Some(context::release),
            },
            media: MediaApiV2 {
                prefix: prefix::<MediaApiV2>(abi::IFACE_VERSION_1),
                create: Some(media::create),
                update: Some(media::update),
                release: Some(media::release),
                current_title: Some(media::current_title),
            },
            i18n: I18nApiV2 {
                prefix: prefix::<I18nApiV2>(abi::IFACE_VERSION_1),
                register_bundle: Some(i18n::register_bundle),
                release_bundle: Some(i18n::release_bundle),
            },
            host_state: HostStateApiV2 {
                prefix: prefix::<HostStateApiV2>(abi::IFACE_VERSION_1),
                get: Some(host_state::get),
                subscribe: Some(host_state::subscribe),
                release_subscription: Some(host_state::release_subscription),
            },
            widget: WidgetApiV2 {
                prefix: prefix::<WidgetApiV2>(abi::IFACE_VERSION_1),
                create: Some(widget::create),
                update: Some(widget::update),
                release: Some(widget::release),
                submit_draw_list: Some(widget::submit_draw_list),
                request_redraw: Some(widget::request_redraw),
                logical_size: Some(widget::logical_size),
            },
            lyrics: LyricsTransformApiV2 {
                prefix: prefix::<LyricsTransformApiV2>(abi::IFACE_VERSION_1),
                register: Some(lyrics::register),
                release: Some(lyrics::release),
            },
            settings: SettingsApiV2 {
                prefix: prefix::<SettingsApiV2>(abi::IFACE_VERSION_1),
                create: Some(settings::create),
                update: Some(settings::update),
                release: Some(settings::release),
            },
            text: TextApiV2 {
                prefix: prefix::<TextApiV2>(abi::IFACE_VERSION_1),
                measure: Some(text::measure),
                font_family: Some(text::font_family),
            },
            image: ImageApiV2 {
                prefix: prefix::<ImageApiV2>(abi::IFACE_VERSION_1),
                decode: Some(image::decode),
                upload_rgba: Some(image::upload_rgba),
                album_art: Some(image::album_art),
                release: Some(image::release),
            },
            store: StoreApiV2 {
                prefix: prefix::<StoreApiV2>(abi::IFACE_VERSION_1),
                get: Some(store::get),
                set: Some(store::set),
                delete: Some(store::delete),
            },
            log: LogApiV2 {
                prefix: prefix::<LogApiV2>(abi::IFACE_VERSION_1),
                write: Some(log::write),
            },
        }
    }

    pub fn bind(&mut self, context: *mut c_void) {
        self.host.prefix.context = context;
        self.context.prefix.context = context;
        self.media.prefix.context = context;
        self.i18n.prefix.context = context;
        self.host_state.prefix.context = context;
        self.widget.prefix.context = context;
        self.lyrics.prefix.context = context;
        self.settings.prefix.context = context;
        self.text.prefix.context = context;
        self.image.prefix.context = context;
        self.store.prefix.context = context;
        self.log.prefix.context = context;
    }
}

unsafe extern "C" fn query(
    context: *mut c_void,
    interface: u32,
    min_version: u32,
) -> *const c_void {
    if context.is_null() || min_version > abi::IFACE_VERSION_1 {
        return std::ptr::null();
    }
    // SAFETY: The context is the stable Box<HostRuntime> address while the host table is live.
    let runtime = unsafe { &*context.cast::<HostRuntime>() };
    let tables = &runtime.tables;
    match interface {
        abi::IFACE_CONTEXT => (&tables.context as *const ContextApiV2).cast(),
        abi::IFACE_MEDIA => (&tables.media as *const MediaApiV2).cast(),
        abi::IFACE_I18N => (&tables.i18n as *const I18nApiV2).cast(),
        abi::IFACE_HOST_STATE => (&tables.host_state as *const HostStateApiV2).cast(),
        abi::IFACE_WIDGET => (&tables.widget as *const WidgetApiV2).cast(),
        abi::IFACE_LYRICS_TRANSFORM => (&tables.lyrics as *const LyricsTransformApiV2).cast(),
        abi::IFACE_SETTINGS => (&tables.settings as *const SettingsApiV2).cast(),
        abi::IFACE_TEXT => (&tables.text as *const TextApiV2).cast(),
        abi::IFACE_IMAGE => (&tables.image as *const ImageApiV2).cast(),
        abi::IFACE_STORE => (&tables.store as *const StoreApiV2).cast(),
        abi::IFACE_LOG => (&tables.log as *const LogApiV2).cast(),
        _ => std::ptr::null(),
    }
}

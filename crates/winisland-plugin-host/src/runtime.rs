use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomPinned;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use winisland_plugin_api::abi::{PluginHostV2, PluginStatus};
use winisland_plugin_api::types::v2::context::{ContextDataV2, HostStateV2, MediaCommandFnV2};
use winisland_plugin_api::types::v2::lyrics::LyricsTransformFnV2;
use winisland_plugin_api::types::v2::settings::{
    SettingsChangedFnV2, SettingsItemV2, SettingsOptionV2,
};
use winisland_plugin_api::types::v2::widget::WidgetSpecV2;
use winisland_plugin_api::types::v2::{HostStateChangedFnV2, PluginToken, WidgetId};
use winisland_render::Image;

use crate::abi::AbiTables;
use crate::draw::replay::{PreparedFrame, prepare};
use crate::draw::validate::validate;
use crate::registry::Registry;
use crate::resources::ResourceTable;

pub struct HostRuntime {
    pub registry: Registry,
    pub resources: ResourceTable,
    pub(crate) tables: AbiTables,
    pub(crate) state: Mutex<ServiceState>,
    pub(crate) store_root: PathBuf,
    pub(crate) store_lock: Mutex<()>,
    _pin: PhantomPinned,
}

pub(crate) struct ServiceState {
    pub contexts: HashMap<u64, ContextRecord>,
    pub context_revision: u64,
    pub media: HashMap<u64, MediaRecord>,
    pub media_revision: u64,
    pub translations: HashMap<u64, TranslationBundle>,
    pub lyrics: HashMap<u64, LyricsRecord>,
    pub settings: HashMap<u64, SettingsRecord>,
    pub settings_revision: u64,
    pub widgets: HashMap<u64, WidgetRecord>,
    pub images: HashMap<u64, Image>,
    pub album_art: Option<Image>,
    pub host_state: HostStateV2,
    pub subscriptions: HashMap<u64, SubscriptionRecord>,
    pub disabled_widgets: Vec<WidgetId>,
}

pub(crate) struct ContextRecord {
    pub data: ContextDataV2,
    pub updated_at: Instant,
}

pub(crate) struct WidgetRecord {
    pub spec: WidgetSpecV2,
    pub draw_list: Arc<[u8]>,
    pub logical_width: f32,
    pub logical_height: f32,
    pub redraw: bool,
    pub failures: u32,
    pub disabled: bool,
}

#[derive(Debug)]
pub struct WidgetFrameError {
    pub reason: &'static str,
    pub consecutive_failures: u32,
    pub disabled: bool,
}

pub struct MediaRecord {
    pub sequence: u64,
    pub in_flight: u32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub flags: u32,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub available_controls: u32,
    pub cover: Vec<u8>,
    pub on_command: Option<MediaCommandFnV2>,
    pub callback_data: usize,
}

pub(crate) struct TranslationBundle {
    pub locale: String,
    pub entries: HashMap<String, String>,
}

pub struct LyricsRecord {
    pub on_transform: LyricsTransformFnV2,
    pub callback_data: usize,
    pub in_flight: u32,
}

pub struct SettingsItemRecord {
    pub raw: SettingsItemV2,
    pub options: Vec<SettingsOptionV2>,
}

pub struct SettingsRecord {
    pub sequence: u64,
    pub key: String,
    pub title: String,
    pub icon: Vec<u8>,
    pub items: Vec<SettingsItemRecord>,
    pub on_change: Option<SettingsChangedFnV2>,
    pub callback_data: usize,
    pub in_flight: u32,
}

pub struct SubscriptionRecord {
    pub token: PluginToken,
    pub callback: HostStateChangedFnV2,
    pub callback_data: usize,
    pub in_flight: u32,
}

// SAFETY: Service tables are immutable after construction; all mutable state uses synchronized
// registries and mutexes, and the context pointer remains stable inside Box<HostRuntime>.
unsafe impl Send for HostRuntime {}
// SAFETY: Concurrent FFI calls access mutable state only through synchronization.
unsafe impl Sync for HostRuntime {}

impl HostRuntime {
    pub fn new(host_build: u32, store_root: PathBuf) -> Pin<Box<Self>> {
        let mut runtime = Box::pin(Self {
            registry: Registry::new(),
            resources: ResourceTable::new(),
            tables: AbiTables::new(host_build),
            state: Mutex::new(ServiceState {
                contexts: HashMap::new(),
                context_revision: 0,
                media: HashMap::new(),
                media_revision: 0,
                translations: HashMap::new(),
                lyrics: HashMap::new(),
                settings: HashMap::new(),
                settings_revision: 0,
                widgets: HashMap::new(),
                images: HashMap::new(),
                album_art: None,
                host_state: HostStateV2::default(),
                subscriptions: HashMap::new(),
                disabled_widgets: Vec::new(),
            }),
            store_root,
            store_lock: Mutex::new(()),
            _pin: PhantomPinned,
        });
        // SAFETY: The Box allocation is pinned before binding its address into the tables.
        let runtime_mut = unsafe { Pin::as_mut(&mut runtime).get_unchecked_mut() };
        let context = (runtime_mut as *mut Self).cast::<c_void>();
        runtime_mut.tables.bind(context);
        runtime
    }

    pub fn host_api(&self) -> *const PluginHostV2 {
        &self.tables.host
    }

    pub fn set_host_state(
        &self,
        snapshot: HostStateV2,
    ) -> Result<Vec<(PluginToken, u64)>, PluginStatus> {
        let mut state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        let old = &state.host_state;
        if old.flags == snapshot.flags
            && old.is_playing == snapshot.is_playing
            && old.media_title == snapshot.media_title
            && old.media_artist == snapshot.media_artist
            && old.theme == snapshot.theme
        {
            return Ok(Vec::new());
        }
        state.host_state = snapshot;
        Ok(state
            .subscriptions
            .iter()
            .map(|(id, record)| (record.token, *id))
            .collect())
    }

    pub fn set_album_art(&self, image: Option<Image>) -> Result<(), PluginStatus> {
        self.state
            .lock()
            .map_err(|_| PluginStatus::Internal)?
            .album_art = image;
        Ok(())
    }

    pub fn image(&self, token: PluginToken, id: u64) -> Result<Image, PluginStatus> {
        use crate::resources::ResourceKind;
        self.resources.require(token, ResourceKind::Image, id)?;
        self.state
            .lock()
            .map_err(|_| PluginStatus::Internal)?
            .images
            .get(&id)
            .cloned()
            .ok_or(PluginStatus::StaleHandle)
    }

    pub fn widget_draw_list(
        &self,
        token: PluginToken,
        widget: WidgetId,
    ) -> Result<Arc<[u8]>, PluginStatus> {
        use crate::resources::ResourceKind;
        self.resources
            .require(token, ResourceKind::Widget, widget.get())?;
        let state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        let record = state
            .widgets
            .get(&widget.get())
            .ok_or(PluginStatus::StaleHandle)?;
        Ok(Arc::clone(&record.draw_list))
    }

    pub fn widget_ids(&self, token: PluginToken) -> Result<Vec<WidgetId>, PluginStatus> {
        use crate::resources::ResourceKind;
        self.registry
            .require(token, winisland_plugin_api::abi::CAP_WIDGET)?;
        self.resources.list(token, ResourceKind::Widget).map(|ids| {
            ids.into_iter()
                .map(|id| unsafe { WidgetId::from_raw(id) })
                .collect()
        })
    }

    pub fn prepare_widget_frame(
        &self,
        token: PluginToken,
        widget: WidgetId,
    ) -> Result<Option<PreparedFrame>, WidgetFrameError> {
        use crate::resources::ResourceKind;
        let bytes = self
            .widget_draw_list(token, widget)
            .map_err(|_| WidgetFrameError {
                reason: "widget handle is stale",
                consecutive_failures: 0,
                disabled: false,
            })?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let result = validate(&bytes, |id| {
            self.resources
                .require(token, ResourceKind::Image, id)
                .is_ok()
        })
        .map_err(|error| error.reason)
        .and_then(|validated| prepare(&validated, |id| self.image(token, id).ok()));
        let mut state = self.state.lock().map_err(|_| WidgetFrameError {
            reason: "plugin state lock is poisoned",
            consecutive_failures: 0,
            disabled: false,
        })?;
        let record = state
            .widgets
            .get_mut(&widget.get())
            .ok_or(WidgetFrameError {
                reason: "widget was released during frame preparation",
                consecutive_failures: 0,
                disabled: false,
            })?;
        if record.disabled {
            return Err(WidgetFrameError {
                reason: "widget was disabled after repeated failures",
                consecutive_failures: record.failures,
                disabled: true,
            });
        }
        if !Arc::ptr_eq(&record.draw_list, &bytes) {
            return match result {
                Ok(frame) => Ok(Some(frame)),
                Err(_) => Ok(None),
            };
        }
        match result {
            Ok(frame) => {
                record.failures = 0;
                Ok(Some(frame))
            }
            Err(reason) => {
                record.failures = record.failures.saturating_add(1);
                let failures = record.failures;
                let disabled = failures >= 30;
                if disabled {
                    record.disabled = true;
                    state.disabled_widgets.push(widget);
                }
                Err(WidgetFrameError {
                    reason,
                    consecutive_failures: failures,
                    disabled,
                })
            }
        }
    }

    pub fn drain_disabled_widgets(&self) -> Result<Vec<WidgetId>, PluginStatus> {
        let mut state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        Ok(std::mem::take(&mut state.disabled_widgets))
    }

    pub fn set_widget_logical_size(
        &self,
        widget: WidgetId,
        width: f32,
        height: f32,
    ) -> Result<(), PluginStatus> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(PluginStatus::InvalidArgument);
        }
        let mut state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        let record = state
            .widgets
            .get_mut(&widget.get())
            .ok_or(PluginStatus::StaleHandle)?;
        record.logical_width = width;
        record.logical_height = height;
        Ok(())
    }

    pub fn translation(&self, locale: &str, key: &str) -> Result<Option<String>, PluginStatus> {
        let state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        Ok(state
            .translations
            .values()
            .filter(|bundle| bundle.locale == locale)
            .find_map(|bundle| bundle.entries.get(key).cloned()))
    }

    pub fn revoke_plugin(&self, token: PluginToken) -> Result<usize, PluginStatus> {
        use crate::resources::ResourceKind;
        self.registry.begin_shutdown(token)?;
        let mut state = self.state.lock().map_err(|_| PluginStatus::Internal)?;
        let revoked = self.resources.revoke_plugin(token)?;
        let mut translations = Vec::new();
        for (id, kind) in &revoked {
            match kind {
                ResourceKind::Context => {
                    if state.contexts.remove(id).is_some() {
                        state.context_revision = state.context_revision.wrapping_add(1);
                    }
                }
                ResourceKind::Media => {
                    if state.media.remove(id).is_some() {
                        state.media_revision = state.media_revision.wrapping_add(1);
                    }
                }
                ResourceKind::I18n => {
                    state.translations.remove(id);
                    translations.push(*id);
                }
                ResourceKind::Widget => {
                    state.widgets.remove(id);
                }
                ResourceKind::Lyrics => {
                    state.lyrics.remove(id);
                }
                ResourceKind::Settings => {
                    if state.settings.remove(id).is_some() {
                        state.settings_revision = state.settings_revision.wrapping_add(1);
                    }
                }
                ResourceKind::HostStateSubscription => {
                    state.subscriptions.remove(id);
                }
                ResourceKind::Image => {
                    state.images.remove(id);
                }
            }
        }
        self.registry.revoke(token)?;
        drop(state);
        for id in translations {
            let _ = winisland_core::i18n::release_plugin_translation_bundle(id);
        }
        Ok(revoked.len())
    }
}

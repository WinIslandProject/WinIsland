use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use winisland_core::widgets::PluginWidget;
use winisland_plugin_api::abi::{ABI_VERSION_2, PluginStatus};
use winisland_plugin_api::types::v2::WidgetId;
use winisland_plugin_package::activate::read_manifest_file;
use winisland_plugin_package::manifest::PluginManifest;

use crate::PluginHostError;
use crate::draw::replay::PreparedFrame;
use crate::fault::{disabled_plugin_ids, set_plugin_disabled};
use crate::lifecycle::{LyricsBridge, PluginInstance};
use crate::loader::PluginLibrary;
use crate::resources::ResourceKind;
use crate::runtime::{HostRuntime, WidgetFrameError};

mod context;
mod media;
mod settings;
mod state;
pub use media::MediaSnapshot;

pub struct PluginHost {
    entries: RefCell<Vec<PluginInstance>>,
    runtime: Pin<Box<HostRuntime>>,
    plugin_dir: PathBuf,
    lyrics_bridge: LyricsBridge,
}

impl PluginHost {
    pub fn new(plugin_dir: PathBuf, host_build: u32) -> Result<Self, PluginHostError> {
        std::fs::create_dir_all(&plugin_dir)
            .map_err(|error| PluginHostError::Io(format!("{}: {error}", plugin_dir.display())))?;
        Ok(Self {
            entries: RefCell::new(Vec::new()),
            runtime: HostRuntime::new(host_build, plugin_dir.clone()),
            plugin_dir,
            lyrics_bridge: LyricsBridge::default(),
        })
    }

    pub fn runtime(&self) -> &HostRuntime {
        &self.runtime
    }

    pub fn lyrics_bridge(&self) -> LyricsBridge {
        self.lyrics_bridge.clone()
    }

    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    pub fn widgets_snapshot(&self) -> Vec<PluginWidget> {
        let entries = self.entries.borrow();
        let Ok(state) = self.runtime.state.lock() else {
            return Vec::new();
        };
        let mut widgets = Vec::new();
        for entry in entries.iter() {
            let token = entry.token();
            let plugin_id = &entry.library().metadata().id;
            let Ok(ids) = self.runtime.resources.list(token, ResourceKind::Widget) else {
                continue;
            };
            for id in ids {
                let Some(record) = state.widgets.get(&id) else {
                    continue;
                };
                if record.disabled {
                    continue;
                }
                let key = fixed_text(&record.spec.key);
                widgets.push(PluginWidget {
                    id,
                    plugin_id: plugin_id.clone(),
                    key: (!key.is_empty()).then_some(key),
                    span_cols: record.spec.span_cols,
                    span_rows: record.spec.span_rows,
                    title: fixed_text(&record.spec.title),
                    body: fixed_text(&record.spec.body),
                });
            }
        }
        widgets.sort_by_key(|widget| widget.id);
        widgets
    }

    pub fn prepare_widget_frame(
        &self,
        widget_id: u64,
    ) -> Result<Option<PreparedFrame>, WidgetFrameError> {
        let token = self
            .runtime
            .resources
            .owner(ResourceKind::Widget, widget_id)
            .map_err(|_| WidgetFrameError {
                reason: "widget handle is stale",
                consecutive_failures: 0,
                disabled: false,
            })?;
        let widget = unsafe { winisland_plugin_api::types::v2::WidgetId::from_raw(widget_id) };
        self.runtime.prepare_widget_frame(token, widget)
    }

    pub fn set_widget_logical_size(
        &self,
        widget_id: u64,
        width: f32,
        height: f32,
    ) -> Result<(), PluginStatus> {
        self.runtime
            .resources
            .owner(ResourceKind::Widget, widget_id)?;
        let widget = unsafe { WidgetId::from_raw(widget_id) };
        self.runtime.set_widget_logical_size(widget, width, height)
    }

    pub fn load_all(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let disabled = disabled_plugin_ids(&self.plugin_dir);
        let Ok(entries) = std::fs::read_dir(&self.plugin_dir) else {
            errors.push(format!("Cannot scan {}", self.plugin_dir.display()));
            return errors;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
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
                if !manifest_path.is_file() {
                    continue;
                }
                let manifest = match read_manifest_file(&manifest_path, ABI_VERSION_2) {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        let reason = if error.contains("ABI version 1") {
                            "该插件使用已废弃的 ABI v1，请升级或联系作者；建议先禁用该插件。"
                                .to_string()
                        } else {
                            error
                        };
                        errors.push(format!("{}: {reason}", path.display()));
                        continue;
                    }
                };
                if disabled.contains(&manifest.id) {
                    continue;
                }
                let dll = path.join(&manifest.entry);
                if let Err(error) = self.load_library(&dll, Some(&manifest)) {
                    errors.push(error.to_string());
                }
            } else if path.extension().is_some_and(|extension| extension == "dll")
                && let Err(error) = self.load_library(&path, None)
            {
                errors.push(error.to_string());
            }
        }
        errors
    }

    pub fn load_library(
        &self,
        path: &Path,
        manifest: Option<&PluginManifest>,
    ) -> Result<(), PluginHostError> {
        let library = PluginLibrary::open(path)?;
        let metadata = library.metadata().clone();
        if let Some(manifest) = manifest {
            for (field, packaged, declared) in [
                ("id", manifest.id.as_str(), metadata.id.as_str()),
                ("name", manifest.name.as_str(), metadata.name.as_str()),
                (
                    "version",
                    manifest.version.as_str(),
                    metadata.version.as_str(),
                ),
                ("author", manifest.author.as_str(), metadata.author.as_str()),
            ] {
                if packaged != declared {
                    return Err(PluginHostError::Invalid(format!(
                        "{}: manifest {field} differs from DLL descriptor",
                        path.display()
                    )));
                }
            }
        }
        if disabled_plugin_ids(&self.plugin_dir).contains(&metadata.id) {
            return Err(PluginHostError::Invalid(format!(
                "plugin '{}' is disabled",
                metadata.id
            )));
        }
        if self
            .entries
            .borrow()
            .iter()
            .any(|entry| entry.library().metadata().id == metadata.id)
        {
            return Err(PluginHostError::Invalid(format!(
                "plugin '{}' is already loaded",
                metadata.id
            )));
        }
        let token = self.runtime.registry.register(
            metadata.id.clone(),
            metadata.version.clone(),
            library.capabilities(),
        )?;
        let marker_dir = self
            .plugin_dir
            .parent()
            .unwrap_or(&self.plugin_dir)
            .to_path_buf();
        let instance =
            unsafe { PluginInstance::create(library, token, self.runtime.host_api(), marker_dir) };
        let mut instance = match instance {
            Ok(instance) => instance,
            Err(error) => {
                let _ = self.runtime.revoke_plugin(token);
                return Err(error);
            }
        };
        if let Ok(widgets) = self.runtime.widget_ids(token) {
            let _ = instance.set_tick_widgets(widgets);
        }
        if let Err(error) = instance.start_tick_worker(Duration::from_millis(16), &self.runtime) {
            if instance.shutdown().is_ok() {
                let _ = self.runtime.revoke_plugin(token);
                drop(instance);
            } else {
                self.entries.borrow_mut().push(instance);
            }
            return Err(error);
        }
        log::info!(
            "Loaded ABI v2 plugin {} v{} (capabilities=0x{:x})",
            metadata.id,
            metadata.version,
            instance.library().capabilities()
        );
        self.lyrics_bridge.attach(&instance);
        self.entries.borrow_mut().push(instance);
        Ok(())
    }

    pub fn activate_staged_plugin(
        &self,
        manifest: &PluginManifest,
        staging: &Path,
    ) -> Result<(), PluginHostError> {
        manifest
            .validate(ABI_VERSION_2)
            .map_err(PluginHostError::Invalid)?;
        let staged_entry = staging.join(&manifest.entry);
        let staged = PluginLibrary::open(&staged_entry)?;
        let metadata = staged.metadata();
        if metadata.id != manifest.id
            || metadata.name != manifest.name
            || metadata.version != manifest.version
            || metadata.author != manifest.author
        {
            return Err(PluginHostError::Invalid(
                "staged plugin metadata differs from plugin.yml".into(),
            ));
        }
        drop(staged);
        let destination = self.plugin_dir.join(manifest.safe_dir_name());
        let previous = if destination.exists() {
            read_manifest_file(&destination.join("plugin.yml"), ABI_VERSION_2).ok()
        } else {
            None
        };
        let loaded_path = self
            .entries
            .borrow()
            .iter()
            .find(|entry| entry.library().metadata().id == manifest.id)
            .map(|entry| entry.library().path().to_path_buf());
        let previous_entry = loaded_path
            .clone()
            .or_else(|| previous.as_ref().map(|old| destination.join(&old.entry)));
        if loaded_path
            .as_ref()
            .is_some_and(|path| !path.starts_with(&destination))
        {
            return Err(PluginHostError::Invalid(
                "cannot replace a loose DLL with a packaged plugin".into(),
            ));
        }
        let was_loaded = self.unload_if_loaded(&manifest.id)?;
        let backup = self.plugin_dir.join(format!(
            ".{}.backup-{}-{}",
            manifest.safe_dir_name(),
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        let had_previous = destination.exists();
        if had_previous && let Err(error) = std::fs::rename(&destination, &backup) {
            if was_loaded && let Some(path) = &previous_entry {
                let _ = self.load_library(path, previous.as_ref());
            }
            return Err(PluginHostError::Io(format!(
                "cannot back up plugin: {error}"
            )));
        }
        if let Err(error) = std::fs::rename(staging, &destination) {
            if had_previous {
                let _ = std::fs::rename(&backup, &destination);
            }
            if was_loaded && let Some(path) = &previous_entry {
                let _ = self.load_library(path, previous.as_ref());
            }
            return Err(PluginHostError::Io(format!(
                "cannot activate plugin: {error}"
            )));
        }
        if let Err(error) = self.load_library(&destination.join(&manifest.entry), Some(manifest)) {
            if self
                .entries
                .borrow()
                .iter()
                .any(|entry| entry.library().metadata().id == manifest.id)
            {
                return Err(PluginHostError::Execution(format!(
                    "new plugin could not be stopped after activation: {error}"
                )));
            }
            let rollback_new = std::fs::rename(&destination, staging);
            let rollback_old = if had_previous {
                std::fs::rename(&backup, &destination)
            } else {
                Ok(())
            };
            let reload = if was_loaded {
                previous_entry
                    .as_ref()
                    .map_or(Ok(()), |path| self.load_library(path, previous.as_ref()))
            } else {
                Ok(())
            };
            return Err(PluginHostError::Execution(format!(
                "plugin activation failed: {error}; rollback new={rollback_new:?}, old={rollback_old:?}, reload={reload:?}"
            )));
        }
        if had_previous && let Err(error) = std::fs::remove_dir_all(&backup) {
            log::warn!(
                "Plugin backup '{}' could not be removed: {error}",
                backup.display()
            );
        }
        Ok(())
    }

    pub fn unload_if_loaded(&self, plugin_id: &str) -> Result<bool, PluginHostError> {
        let mut entries = self.entries.borrow_mut();
        let Some(index) = entries
            .iter()
            .position(|entry| entry.library().metadata().id == plugin_id)
        else {
            return Ok(false);
        };
        let mut instance = entries.remove(index);
        if let Err(error) = instance.shutdown() {
            entries.insert(index, instance);
            return Err(error);
        }
        let token = instance.token();
        self.lyrics_bridge.detach(plugin_id);
        let revoke = self.runtime.revoke_plugin(token);
        drop(instance);
        revoke
            .map_err(|status| PluginHostError::Execution(format!("resource revoke: {status:?}")))?;
        Ok(true)
    }

    pub fn refresh_tick_widgets(&self) -> Vec<String> {
        let entries = self.entries.borrow();
        let mut errors = Vec::new();
        for entry in entries.iter() {
            if let Ok(widgets) = self.runtime.widget_ids(entry.token())
                && let Err(error) = entry.set_tick_widgets(widgets)
            {
                errors.push(format!("{}: {error}", entry.library().metadata().id));
            }
        }
        errors
    }

    pub fn drain_failed_plugins(&self) -> Vec<String> {
        let failed = self
            .entries
            .borrow()
            .iter()
            .filter_map(|entry| {
                entry
                    .tick_failure()
                    .map(|status| (entry.library().metadata().id.clone(), status))
            })
            .collect::<Vec<_>>();
        let mut messages = Vec::new();
        for (id, status) in failed {
            if let Err(error) = set_plugin_disabled(&self.plugin_dir, &id, true) {
                messages.push(format!("{id}: could not persist disabled state: {error}"));
            }
            match self.unload_if_loaded(&id) {
                Ok(_) => messages.push(format!(
                    "plugin {id} disabled after tick failure {status:?}"
                )),
                Err(error) => {
                    messages.push(format!("plugin {id} tick failed ({status:?}): {error}"))
                }
            }
        }
        messages
    }

    pub fn shutdown_all(&self) -> Vec<String> {
        let ids = self
            .entries
            .borrow()
            .iter()
            .map(|entry| entry.library().metadata().id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| {
                self.unload_if_loaded(&id)
                    .err()
                    .map(|error| error.to_string())
            })
            .collect()
    }
}

fn fixed_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.split(|byte| *byte == 0).next().unwrap_or(bytes)).into_owned()
}

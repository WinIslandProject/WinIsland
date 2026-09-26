use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CRASH_PLUGIN_FILE: &str = ".crash_plugin";
const DISABLED_PLUGINS_FILE: &str = ".disabled-plugins";

thread_local! {
    static ACTIVE_PLUGIN: RefCell<Option<Arc<PluginIdentity>>> = const { RefCell::new(None) };
}

#[derive(Debug)]
pub struct PluginIdentity {
    pub id: String,
    pub version: String,
    pub marker_dir: PathBuf,
}

pub struct PluginCallGuard {
    previous: Option<Arc<PluginIdentity>>,
    marker_path: PathBuf,
}

fn marker_path(identity: &PluginIdentity) -> PathBuf {
    identity
        .marker_dir
        .join(format!(".active-plugin-{:?}", std::thread::current().id()))
}

fn write_marker(identity: &PluginIdentity) -> std::io::Result<()> {
    std::fs::create_dir_all(&identity.marker_dir)?;
    std::fs::write(marker_path(identity), identity.id.as_bytes())
}

impl PluginCallGuard {
    pub fn enter(identity: &Arc<PluginIdentity>) -> Self {
        let active_path = marker_path(identity);
        if let Err(error) = write_marker(identity) {
            log::error!("Could not mark plugin callback {}: {error}", identity.id);
        }
        let previous = ACTIVE_PLUGIN.with(|active| active.replace(Some(Arc::clone(identity))));
        if let Some(previous) = &previous {
            let previous_path = marker_path(previous);
            if previous_path != active_path {
                let _ = std::fs::remove_file(previous_path);
            }
        }
        Self {
            previous,
            marker_path: active_path,
        }
    }
}

impl Drop for PluginCallGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.marker_path);
        if let Some(previous) = &self.previous
            && let Err(error) = write_marker(previous)
        {
            log::error!("Could not restore plugin callback marker: {error}");
        }
        ACTIVE_PLUGIN.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

pub fn current_plugin() -> Option<Arc<PluginIdentity>> {
    ACTIVE_PLUGIN.with(|active| active.try_borrow().ok().and_then(|active| active.clone()))
}

pub fn record_crash(config_dir: &Path) -> std::io::Result<Option<Arc<PluginIdentity>>> {
    let identity = current_plugin();
    if let Some(identity) = &identity {
        std::fs::create_dir_all(config_dir)?;
        std::fs::write(config_dir.join(CRASH_PLUGIN_FILE), identity.id.as_bytes())?;
    }
    Ok(identity)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 63
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn disabled_path(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(DISABLED_PLUGINS_FILE)
}

pub fn disabled_plugin_ids(plugin_dir: &Path) -> HashSet<String> {
    std::fs::read_to_string(disabled_path(plugin_dir))
        .ok()
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .filter(|id| valid_id(id))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_plugin_disabled(plugin_dir: &Path, id: &str, disabled: bool) -> std::io::Result<()> {
    if !valid_id(id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid plugin id",
        ));
    }
    let mut ids = disabled_plugin_ids(plugin_dir);
    if disabled {
        ids.insert(id.to_string());
    } else {
        ids.remove(id);
    }
    let path = disabled_path(plugin_dir);
    if ids.is_empty() {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    } else {
        std::fs::create_dir_all(plugin_dir)?;
        let mut ids = ids.into_iter().collect::<Vec<_>>();
        ids.sort_unstable();
        std::fs::write(path, ids.join("\n") + "\n")
    }
}

pub fn has_active_plugin_markers(plugin_dir: &Path) -> bool {
    let Some(marker_dir) = plugin_dir.parent() else {
        return false;
    };
    std::fs::read_dir(marker_dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".active-plugin-"))
        })
    })
}

pub fn recover_crashed_plugin(
    config_dir: &Path,
    plugin_dir: &Path,
) -> std::io::Result<Vec<String>> {
    let path = config_dir.join(CRASH_PLUGIN_FILE);
    let mut ids = HashSet::new();
    if let Ok(raw) = std::fs::read(&path)
        && let Some(id) = std::str::from_utf8(&raw).ok().filter(|id| valid_id(id))
    {
        ids.insert(id.to_string());
    }
    let mut marker_paths = Vec::new();
    if let Some(marker_dir) = plugin_dir.parent()
        && let Ok(entries) = std::fs::read_dir(marker_dir)
    {
        for entry in entries.flatten() {
            let marker = entry.path();
            if !entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".active-plugin-"))
            {
                continue;
            }
            if let Ok(raw) = std::fs::read(&marker)
                && let Some(id) = std::str::from_utf8(&raw).ok().filter(|id| valid_id(id))
            {
                ids.insert(id.to_string());
            }
            marker_paths.push(marker);
        }
    }
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable();
    for id in &ids {
        set_plugin_disabled(plugin_dir, id, true)?;
    }
    for marker in marker_paths {
        std::fs::remove_file(marker)?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(ids)
}

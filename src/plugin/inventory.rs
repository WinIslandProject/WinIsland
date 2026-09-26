use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use winisland_plugin_api::abi::ABI_VERSION_2;
use winisland_plugin_host::fault::{disabled_plugin_ids, set_plugin_disabled};
use winisland_plugin_host::loader::PluginLibrary;
use winisland_plugin_package::activate::{
    MAX_PLUGIN_ICON_BYTES, MAX_PLUGIN_README_BYTES, read_bounded_file, read_manifest_file,
};

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

pub struct PluginManager {
    pub(crate) plugin_dir: PathBuf,
}

impl PluginManager {
    pub fn new<P: AsRef<Path>>(plugin_dir: P) -> Self {
        let plugin_dir = plugin_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&plugin_dir);
        Self { plugin_dir }
    }

    pub fn installed_plugins(&self) -> Vec<InstalledPlugin> {
        scan(&self.plugin_dir)
    }

    pub fn installed_plugins_async(&self) -> mpsc::Receiver<Vec<InstalledPlugin>> {
        let directory = self.plugin_dir.clone();
        let (tx, rx) = mpsc::channel();
        let result = std::thread::Builder::new()
            .name("winisland-plugin-scan".to_string())
            .spawn(move || {
                let _ = tx.send(scan(&directory));
                crate::platform::wake();
            });
        if let Err(error) = result {
            log::warn!("Failed to start plugin scan: {error}");
        }
        rx
    }

    pub fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        validate_id(id)?;
        if !self
            .installed_plugins()
            .iter()
            .any(|plugin| plugin.id == id)
        {
            return Err(format!("Plugin '{id}' is not installed"));
        }
        set_plugin_disabled(&self.plugin_dir, id, !enabled).map_err(|error| error.to_string())
    }

    pub fn uninstall_plugin(&self, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let targets = uninstall_targets(&self.plugin_dir, id);
        if targets.is_empty() {
            return Err(format!("Plugin '{id}' is not installed"));
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let mut moved = Vec::with_capacity(targets.len());
        for (index, source) in targets.into_iter().enumerate() {
            let backup = self.plugin_dir.join(format!(
                ".{id}.uninstall-{}-{stamp}-{index}",
                std::process::id()
            ));
            if let Err(error) = std::fs::rename(&source, &backup) {
                return Err(with_rollback(
                    format!("Cannot remove plugin files: {error}"),
                    &moved,
                ));
            }
            moved.push((source, backup));
        }
        if let Err(error) = set_plugin_disabled(&self.plugin_dir, id, false) {
            return Err(with_rollback(error.to_string(), &moved));
        }
        for (_, backup) in moved {
            let result = if backup.is_dir() {
                std::fs::remove_dir_all(&backup)
            } else {
                std::fs::remove_file(&backup)
            };
            if let Err(error) = result {
                log::warn!(
                    "Could not remove plugin backup '{}': {error}",
                    backup.display()
                );
            }
        }
        Ok(())
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

fn scan(directory: &Path) -> Vec<InstalledPlugin> {
    let disabled = disabled_plugin_ids(directory);
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        let plugin = if path.is_dir() {
            packaged(&path, &disabled)
        } else if is_dll(&path) {
            manual(&path, &disabled)
        } else {
            None
        };
        if let Some(plugin) = plugin
            && !plugins
                .iter()
                .any(|entry: &InstalledPlugin| entry.id == plugin.id)
        {
            plugins.push(plugin);
        }
    }
    plugins.sort_by_key(|plugin| plugin.name.to_lowercase());
    plugins
}

fn packaged(directory: &Path, disabled: &HashSet<String>) -> Option<InstalledPlugin> {
    let manifest = read_manifest_file(&directory.join("plugin.yml"), ABI_VERSION_2).ok()?;
    if !directory.join(&manifest.entry).is_file() {
        return None;
    }
    let icon = asset_path(
        directory,
        manifest.icon.as_deref(),
        &["icon.png", "icon.jpg", "icon.jpeg", "icon.webp"],
    )
    .and_then(|path| read_icon(&path));
    let readme = asset_path(
        directory,
        manifest.readme.as_deref(),
        &["README.md", "README.markdown", "README.txt"],
    )
    .and_then(|path| read_bounded_file(&path, MAX_PLUGIN_README_BYTES).ok())
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
}

fn manual(path: &Path, disabled: &HashSet<String>) -> Option<InstalledPlugin> {
    let library = PluginLibrary::open(path).ok()?;
    let metadata = library.metadata();
    Some(InstalledPlugin {
        id: metadata.id.clone(),
        name: metadata.name.clone(),
        author: metadata.author.clone(),
        version: metadata.version.clone(),
        description: metadata.description.clone(),
        github_link: String::new(),
        enabled: !disabled.contains(&metadata.id),
        icon: None,
        readme: None,
    })
}

fn uninstall_targets(directory: &Path, id: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                read_manifest_file(&path.join("plugin.yml"), ABI_VERSION_2)
                    .ok()
                    .filter(|manifest| manifest.id == id)
                    .map(|_| path)
            } else if is_dll(&path) {
                PluginLibrary::open(&path)
                    .ok()
                    .filter(|library| library.metadata().id == id)
                    .map(|_| path)
            } else {
                None
            }
        })
        .collect()
}

fn is_dll(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("Invalid plugin ID".to_string());
    }
    Ok(())
}

fn with_rollback(message: String, moved: &[(PathBuf, PathBuf)]) -> String {
    let errors = moved
        .iter()
        .rev()
        .filter_map(|(source, backup)| {
            std::fs::rename(backup, source)
                .err()
                .map(|error| format!("cannot restore '{}': {error}", source.display()))
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        message
    } else {
        format!("{message}; rollback also failed: {}", errors.join("; "))
    }
}

fn read_icon(path: &Path) -> Option<Vec<u8>> {
    let bytes = read_bounded_file(path, MAX_PLUGIN_ICON_BYTES).ok()?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    (width > 0 && height > 0 && width <= 2048 && height <= 2048).then_some(bytes)
}

fn asset_path(directory: &Path, declared: Option<&str>, fallbacks: &[&str]) -> Option<PathBuf> {
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

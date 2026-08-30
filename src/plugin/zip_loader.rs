use serde::Deserialize;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FILENAME_COMPONENT: usize = 255;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_PLUGIN_ICON_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_PLUGIN_README_BYTES: u64 = 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 4096;
const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    #[serde(rename = "github-link")]
    pub github_link: String,
    #[serde(rename = "abi-version")]
    pub abi_version: u32,
    pub entry: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub readme: Option<String>,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 63
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("'id' must be at most 63 bytes and match [a-zA-Z0-9_-]+".into());
        }
        validate_text("name", &self.name, 127)?;
        validate_text("author", &self.author, 127)?;
        validate_text("version", &self.version, 31)?;
        validate_text("description", &self.description, 255)?;
        validate_text("github-link", &self.github_link, 2048)?;
        if self.abi_version != winisland_plugin_api::ABI_VERSION_1 {
            return Err(format!(
                "Unsupported plugin ABI version {}",
                self.abi_version
            ));
        }
        let entry = Path::new(&self.entry);
        if self.entry.is_empty()
            || self.entry.len() > MAX_FILENAME_COMPONENT
            || entry.components().count() != 1
            || !entry
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
        {
            return Err("'entry' must be a root-level .dll filename".into());
        }
        validate_asset_path(
            "icon",
            self.icon.as_deref(),
            &["png", "jpg", "jpeg", "webp"],
        )?;
        validate_asset_path("readme", self.readme.as_deref(), &["md", "markdown", "txt"])?;
        Ok(())
    }

    pub fn safe_dir_name(&self) -> &str {
        &self.id
    }
}

fn validate_asset_path(
    field: &str,
    value: Option<&str>,
    extensions: &[&str],
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let path = Path::new(value);
    if value.is_empty()
        || value.len() > 512
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extensions
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
            })
    {
        return Err(format!("'{field}' must be a safe relative asset path"));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("'{field}' is empty"));
    }
    if value.len() > max_bytes {
        return Err(format!("'{field}' exceeds {max_bytes} UTF-8 bytes"));
    }
    Ok(())
}

fn validate_entry(entry: &zip::read::ZipFile<'_, std::fs::File>) -> Result<String, String> {
    let name = entry.name();
    if entry.is_symlink() {
        return Err(format!("Zip entry '{name}' is a symlink"));
    }
    if name.starts_with('/') || name.starts_with('\\') || name.contains(':') || name.is_empty() {
        return Err(format!("Zip entry '{name}' has an unsafe path"));
    }
    let mut components = Vec::new();
    let path_components = name.split(['/', '\\']).collect::<Vec<_>>();
    for (index, component) in path_components.iter().enumerate() {
        if component.is_empty()
            && index + 1 == path_components.len()
            && (name.ends_with('/') || name.ends_with('\\'))
        {
            continue;
        }
        if component.is_empty()
            || *component == "."
            || *component == ".."
            || component.len() > MAX_FILENAME_COMPONENT
            || component.ends_with(['.', ' '])
            || is_windows_device_name(component)
        {
            return Err(format!("Zip entry '{name}' has an unsafe path component"));
        }
        components.push(*component);
    }
    if entry.size() > MAX_ENTRY_BYTES {
        return Err(format!("Zip entry '{name}' exceeds 256 MiB"));
    }
    Ok(components.join("/").to_lowercase())
}

fn is_windows_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn validate_archive(
    zip: &mut zip::ZipArchive<std::fs::File>,
    manifest: &PluginManifest,
) -> Result<(), String> {
    if zip.len() > MAX_ZIP_ENTRIES {
        return Err(format!("Plugin package exceeds {MAX_ZIP_ENTRIES} entries"));
    }
    let mut total = 0u64;
    let mut has_entry = false;
    let mut paths = HashSet::new();
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|error| format!("Zip read error: {error}"))?;
        let path = validate_entry(&entry)?;
        if !paths.insert(path) {
            return Err(format!(
                "Zip entry '{}' collides with another archive path",
                entry.name()
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "Plugin package size overflow".to_string())?;
        if total > MAX_TOTAL_BYTES {
            return Err("Plugin package exceeds 512 MiB uncompressed".into());
        }
        has_entry |= entry.name() == manifest.entry;
    }
    if !has_entry {
        return Err(format!("Entry DLL '{}' is missing", manifest.entry));
    }
    for (field, asset) in [("icon", &manifest.icon), ("readme", &manifest.readme)] {
        if let Some(asset) = asset
            && !paths.contains(&asset.replace('\\', "/").to_lowercase())
        {
            return Err(format!("{field} asset '{asset}' is missing"));
        }
    }
    Ok(())
}

fn read_zip_entry(
    zip: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let index = zip
        .index_for_name(name)
        .ok_or_else(|| format!("{name} not found in zip"))?;
    let mut entry = zip
        .by_index(index)
        .map_err(|error| format!("Cannot read {name}: {error}"))?;
    if entry.size() > max_bytes {
        return Err(format!("{name} exceeds the size limit"));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read {name}: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{name} exceeds the size limit"));
    }
    Ok(bytes)
}

pub fn read_manifest_from_zip(zip_path: &Path) -> Result<PluginManifest, String> {
    let file =
        std::fs::File::open(zip_path).map_err(|error| format!("Cannot open zip: {error}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| format!("Invalid zip: {error}"))?;
    read_manifest(&mut zip)
}

fn read_manifest(zip: &mut zip::ZipArchive<std::fs::File>) -> Result<PluginManifest, String> {
    let bytes = read_zip_entry(zip, "plugin.yml", MAX_MANIFEST_BYTES)?;
    let manifest: PluginManifest =
        serde_yaml::from_slice(&bytes).map_err(|error| format!("Invalid plugin.yml: {error}"))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn read_manifest_file(path: &Path) -> Result<PluginManifest, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("Cannot inspect '{}': {error}", path.display()))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!("'{}' exceeds 1 MiB", path.display()));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Cannot read '{}': {error}", path.display()))?;
    let manifest: PluginManifest = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("Invalid '{}': {error}", path.display()))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn read_bounded_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("Cannot inspect '{}': {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(format!("'{}' exceeds the size limit", path.display()));
    }
    std::fs::read(path).map_err(|error| format!("Cannot read '{}': {error}", path.display()))
}

pub fn extract_plugin(
    zip_path: &Path,
    plugin_dir: &Path,
) -> Result<(PluginManifest, PathBuf), String> {
    let file =
        std::fs::File::open(zip_path).map_err(|error| format!("Cannot open zip: {error}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| format!("Invalid zip: {error}"))?;
    let manifest = read_manifest(&mut zip)?;
    validate_archive(&mut zip, &manifest)?;

    std::fs::create_dir_all(plugin_dir)
        .map_err(|error| format!("Cannot create plugin directory: {error}"))?;
    let staging = plugin_dir.join(format!(
        ".{}.install-{}-{}",
        manifest.safe_dir_name(),
        std::process::id(),
        NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed)
    ));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|error| format!("Cannot reset staging directory: {error}"))?;
    }
    std::fs::create_dir(&staging)
        .map_err(|error| format!("Cannot create staging directory: {error}"))?;

    let result = (|| {
        let mut actual_total = 0u64;
        for index in 0..zip.len() {
            let mut entry = zip
                .by_index(index)
                .map_err(|error| format!("Zip read error: {error}"))?;
            validate_entry(&entry)?;
            let output = staging.join(entry.name());
            if entry.is_dir() {
                std::fs::create_dir_all(&output)
                    .map_err(|error| format!("Cannot create '{}': {error}", output.display()))?;
                continue;
            }
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("Cannot create '{}': {error}", parent.display()))?;
            }
            let mut file = std::fs::File::create(&output)
                .map_err(|error| format!("Cannot create '{}': {error}", output.display()))?;
            let written = std::io::copy(&mut entry.by_ref().take(MAX_ENTRY_BYTES + 1), &mut file)
                .map_err(|error| format!("Cannot extract '{}': {error}", entry.name()))?;
            if written > MAX_ENTRY_BYTES {
                return Err(format!("Zip entry '{}' exceeds 256 MiB", entry.name()));
            }
            actual_total = actual_total
                .checked_add(written)
                .ok_or_else(|| "Plugin package size overflow".to_string())?;
            if actual_total > MAX_TOTAL_BYTES {
                return Err("Plugin package exceeds 512 MiB uncompressed".into());
            }
        }
        let entry = staging.join(&manifest.entry);
        if !entry.is_file() {
            return Err(format!("Entry DLL '{}' is missing", manifest.entry));
        }
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok((manifest, staging))
}

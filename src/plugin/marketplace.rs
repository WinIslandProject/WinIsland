use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::header::{CACHE_CONTROL, PRAGMA};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::config::APP_VERSION;
use crate::plugin::zip_loader;

const CATALOG_URL: &str = "https://github.com/WinIslandProject/PluginMarketplace/releases/download/catalog-v1/catalog-v1.json";
const SIGNATURE_URL: &str = "https://github.com/WinIslandProject/PluginMarketplace/releases/download/catalog-v1/catalog-v1.sig";
const CATALOG_PUBLIC_KEY: [u8; 32] = [
    0x34, 0xad, 0x10, 0x72, 0x0d, 0x4e, 0x37, 0xb9, 0x96, 0x49, 0xcf, 0x54, 0xf6, 0x31, 0xee, 0x82,
    0xe4, 0x04, 0xd1, 0x6d, 0xca, 0x9a, 0x1a, 0xf8, 0xc4, 0xe4, 0x24, 0x28, 0x0b, 0x97, 0x95, 0x5f,
];
const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
const MAX_SIGNATURE_BYTES: usize = 1024;
const MAX_ICON_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOTAL_ICON_BYTES: usize = 32 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!(
            "WinIsland-PluginMarketplace/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .expect("marketplace HTTP client configuration is valid")
});

#[derive(Clone, Debug)]
pub struct MarketplaceCatalog {
    pub plugins: Vec<MarketplacePlugin>,
}

#[derive(Clone, Debug)]
pub struct MarketplacePlugin {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub repository: String,
    pub download_url: String,
    pub sha256: String,
    pub size: u64,
    pub min_winisland_version: String,
    pub categories: Vec<String>,
    pub readme: String,
    pub icon: Option<Vec<u8>>,
    pub revoked_reason: Option<String>,
}

impl MarketplacePlugin {
    pub fn is_compatible(&self) -> bool {
        compare_versions(APP_VERSION, &self.min_winisland_version)
            .is_some_and(|ordering| ordering != Ordering::Less)
    }

    pub fn has_update_for(&self, installed_version: &str) -> bool {
        compare_versions(installed_version, &self.version) == Some(Ordering::Less)
    }
}

#[derive(Deserialize)]
struct CatalogDocument {
    schema: u32,
    generated_at: String,
    plugins: Vec<CatalogPlugin>,
    revocations: Vec<CatalogRevocation>,
}

#[derive(Deserialize)]
struct CatalogPlugin {
    id: String,
    name: String,
    author: String,
    version: String,
    description: String,
    repository: String,
    source_commit: String,
    released_at: String,
    download_url: String,
    sha256: String,
    size: u64,
    abi_version: u32,
    min_winisland_version: String,
    categories: Vec<String>,
    readme: String,
    icon_url: Option<String>,
    icon_sha256: Option<String>,
}

#[derive(Deserialize)]
struct CatalogRevocation {
    id: String,
    version: String,
    reason: String,
}

pub async fn load_catalog() -> Result<MarketplaceCatalog, String> {
    let catalog_bytes = download_bytes(CATALOG_URL, MAX_CATALOG_BYTES).await?;
    let signature_bytes = download_bytes(SIGNATURE_URL, MAX_SIGNATURE_BYTES).await?;
    verify_catalog_signature(&catalog_bytes, &signature_bytes)?;
    let document: CatalogDocument = serde_json::from_slice(&catalog_bytes)
        .map_err(|error| format!("The marketplace catalog is invalid: {error}"))?;
    validate_catalog(document).await
}

pub async fn download_plugin(plugin: &MarketplacePlugin) -> Result<PathBuf, String> {
    validate_github_release_url(&plugin.download_url)?;
    validate_sha256(&plugin.sha256)?;
    if plugin.size == 0 || plugin.size > MAX_PACKAGE_BYTES {
        return Err("The plugin package size is invalid".into());
    }

    let directory = marketplace_cache_dir();
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create the marketplace cache: {error}"))?;
    let destination = directory.join(format!(
        "{}-{}-{}.winisland-plugin.zip",
        plugin.id,
        safe_version_component(&plugin.version),
        &plugin.sha256[..12]
    ));
    if destination.is_file() && hash_file(&destination)? == plugin.sha256 {
        validate_package_manifest(plugin, &destination)?;
        return Ok(destination);
    }

    let temporary = destination.with_extension(format!("download-{}", std::process::id()));
    let result = download_package(plugin, &temporary)
        .await
        .and_then(|()| validate_package_manifest(plugin, &temporary));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| format!("Could not replace the cached plugin: {error}"))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("Could not cache the plugin package: {error}"))?;
    Ok(destination)
}

async fn validate_catalog(document: CatalogDocument) -> Result<MarketplaceCatalog, String> {
    if document.schema != 1 {
        return Err(format!(
            "Unsupported marketplace catalog schema {}",
            document.schema
        ));
    }
    if document.generated_at.trim().is_empty() {
        return Err("The marketplace catalog has no generation timestamp".into());
    }
    if document.plugins.len() > 4096 || document.revocations.len() > 4096 {
        return Err("The marketplace catalog contains too many entries".into());
    }

    let mut ids = HashSet::new();
    let mut revocations = Vec::with_capacity(document.revocations.len());
    for revocation in document.revocations {
        validate_plugin_id(&revocation.id)?;
        validate_version(&revocation.version)?;
        validate_text("revocation reason", &revocation.reason, 1024)?;
        revocations.push(revocation);
    }

    let mut plugins = Vec::with_capacity(document.plugins.len());
    let mut total_icon_bytes = 0_usize;
    for entry in document.plugins {
        validate_plugin_entry(&entry)?;
        if !ids.insert(entry.id.to_ascii_lowercase()) {
            return Err(format!("Duplicate plugin ID '{}'", entry.id));
        }
        let revoked_reason = revocations
            .iter()
            .find(|revocation| {
                revocation.id.eq_ignore_ascii_case(&entry.id) && revocation.version == entry.version
            })
            .map(|revocation| revocation.reason.clone());
        let icon = match (&entry.icon_url, &entry.icon_sha256) {
            (Some(url), Some(expected)) => {
                validate_marketplace_asset_url(url)?;
                validate_sha256(expected)?;
                let bytes = download_bytes(url, MAX_ICON_BYTES).await?;
                if sha256_hex(&bytes) != *expected {
                    return Err(format!("The icon for '{}' failed verification", entry.name));
                }
                total_icon_bytes = total_icon_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| "The marketplace icons are too large".to_string())?;
                if total_icon_bytes > MAX_TOTAL_ICON_BYTES {
                    return Err("The marketplace icons are too large".into());
                }
                validate_icon(&entry.id, &bytes)?;
                Some(bytes)
            }
            (None, None) => None,
            _ => {
                return Err(format!(
                    "The icon metadata for '{}' is incomplete",
                    entry.name
                ));
            }
        };
        plugins.push(MarketplacePlugin {
            id: entry.id,
            name: entry.name,
            author: entry.author,
            version: entry.version,
            description: entry.description,
            repository: entry.repository,
            download_url: entry.download_url,
            sha256: entry.sha256,
            size: entry.size,
            min_winisland_version: entry.min_winisland_version,
            categories: entry.categories,
            readme: entry.readme,
            icon,
            revoked_reason,
        });
    }
    plugins.sort_by_key(|plugin| plugin.name.to_lowercase());
    Ok(MarketplaceCatalog { plugins })
}

fn validate_plugin_entry(entry: &CatalogPlugin) -> Result<(), String> {
    validate_plugin_id(&entry.id)?;
    validate_text("plugin name", &entry.name, 127)?;
    validate_text("plugin author", &entry.author, 127)?;
    validate_version(&entry.version)?;
    validate_text("plugin description", &entry.description, 1024)?;
    validate_github_repository_url(&entry.repository)?;
    if entry.source_commit.len() != 40
        || !entry
            .source_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "Plugin '{}' has an invalid source commit",
            entry.id
        ));
    }
    validate_text("release timestamp", &entry.released_at, 64)?;
    validate_github_release_url(&entry.download_url)?;
    validate_sha256(&entry.sha256)?;
    if entry.size == 0 || entry.size > MAX_PACKAGE_BYTES {
        return Err(format!("Plugin '{}' has an invalid package size", entry.id));
    }
    if entry.abi_version != winisland_plugin_api::ABI_VERSION_1 {
        return Err(format!(
            "Plugin '{}' requires unsupported ABI version {}",
            entry.id, entry.abi_version
        ));
    }
    validate_version(&entry.min_winisland_version)?;
    if entry.categories.len() > 16
        || entry.categories.iter().any(|category| {
            category.is_empty()
                || category.len() > 32
                || !category
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
    {
        return Err(format!("Plugin '{}' has invalid categories", entry.id));
    }
    if entry.readme.len() > 128 * 1024 {
        return Err(format!("Plugin '{}' has oversized documentation", entry.id));
    }
    Ok(())
}

async fn download_package(plugin: &MarketplacePlugin, destination: &Path) -> Result<(), String> {
    let response = request(&plugin.download_url).await?;
    if let Some(length) = response.content_length()
        && (length != plugin.size || length > MAX_PACKAGE_BYTES)
    {
        return Err("The plugin package size does not match the signed catalog".into());
    }
    let mut response = response;
    let mut file = std::fs::File::create(destination)
        .map_err(|error| format!("Could not create the plugin download: {error}"))?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Could not download the plugin: {error}"))?
    {
        total = total
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "The plugin package is too large".to_string())?;
        if total > plugin.size || total > MAX_PACKAGE_BYTES {
            return Err("The plugin package exceeds its signed size".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|error| format!("Could not save the plugin download: {error}"))?;
    }
    if total != plugin.size {
        return Err("The plugin package size does not match the signed catalog".into());
    }
    let digest = hex_digest(hasher.finalize().as_slice());
    if digest != plugin.sha256 {
        return Err("The plugin package failed SHA-256 verification".into());
    }
    file.sync_all()
        .map_err(|error| format!("Could not finish the plugin download: {error}"))
}

async fn download_bytes(url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let response = request(url).await?;
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("The marketplace response is too large".into());
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Could not download marketplace data: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("The marketplace response is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn request(url: &str) -> Result<reqwest::Response, String> {
    let response = HTTP_CLIENT
        .get(url)
        .header(CACHE_CONTROL, "no-cache, no-store")
        .header(PRAGMA, "no-cache")
        .send()
        .await
        .map_err(|error| format!("Could not connect to the plugin marketplace: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "The plugin marketplace returned HTTP {}",
            response.status()
        ));
    }
    Ok(response)
}

fn verify_catalog_signature(catalog: &[u8], encoded_signature: &[u8]) -> Result<(), String> {
    let encoded = std::str::from_utf8(encoded_signature)
        .map_err(|_| "The marketplace signature is not valid UTF-8")?
        .trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "The marketplace signature is not valid Base64")?;
    let signature = Signature::from_slice(&bytes)
        .map_err(|_| "The marketplace signature has an invalid length")?;
    let key = VerifyingKey::from_bytes(&CATALOG_PUBLIC_KEY)
        .map_err(|_| "The embedded marketplace public key is invalid")?;
    key.verify(catalog, &signature)
        .map_err(|_| "The plugin marketplace signature could not be verified".into())
}

fn validate_icon(plugin_id: &str, bytes: &[u8]) -> Result<(), String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Plugin '{plugin_id}' has an invalid icon: {error}"))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("Plugin '{plugin_id}' has an invalid icon: {error}"))?;
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return Err(format!(
            "Plugin '{plugin_id}' has unsupported icon dimensions"
        ));
    }
    Ok(())
}

fn validate_package_manifest(plugin: &MarketplacePlugin, path: &Path) -> Result<(), String> {
    let manifest = zip_loader::read_manifest_from_zip(path)?;
    let metadata_matches = manifest.id == plugin.id
        && manifest.name == plugin.name
        && manifest.author == plugin.author
        && manifest.version == plugin.version
        && manifest.description == plugin.description
        && manifest.github_link == plugin.repository;
    if !metadata_matches {
        return Err("The plugin package metadata does not match the signed catalog".into());
    }
    Ok(())
}

fn validate_plugin_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 63
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(format!("Invalid marketplace plugin ID '{value}'"));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, limit: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > limit {
        return Err(format!("Invalid marketplace {field}"));
    }
    Ok(())
}

fn validate_version(value: &str) -> Result<(), String> {
    compare_versions(value, value)
        .map(|_| ())
        .ok_or_else(|| format!("Invalid marketplace version '{value}'"))
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Invalid SHA-256 value in the marketplace catalog".into());
    }
    Ok(())
}

fn validate_github_repository_url(value: &str) -> Result<(), String> {
    let path = value
        .strip_prefix("https://github.com/")
        .ok_or_else(|| "Marketplace source repositories must use HTTPS GitHub URLs".to_string())?;
    if path.ends_with('/')
        || path.split('/').count() != 2
        || path
            .split('/')
            .any(|part| part.is_empty() || part.contains(['?', '#']))
    {
        return Err("Invalid GitHub repository URL in the marketplace catalog".into());
    }
    Ok(())
}

fn validate_github_release_url(value: &str) -> Result<(), String> {
    let path = value
        .strip_prefix("https://github.com/")
        .ok_or_else(|| "Marketplace downloads must use HTTPS GitHub URLs".to_string())?;
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() != 6
        || parts[0].is_empty()
        || parts[1].is_empty()
        || parts[2..4] != ["releases", "download"]
        || parts
            .iter()
            .any(|part| part.is_empty() || part.contains(['?', '#']))
    {
        return Err("Invalid GitHub Release URL in the marketplace catalog".into());
    }
    Ok(())
}

fn validate_marketplace_asset_url(value: &str) -> Result<(), String> {
    let prefix =
        "https://github.com/WinIslandProject/PluginMarketplace/releases/download/catalog-v1/icon-";
    let name = value.strip_prefix(prefix).ok_or_else(|| {
        "Marketplace icons must be served by the signed catalog release".to_string()
    })?;
    if name.is_empty() || name.contains(['/', '\\', '?', '#']) {
        return Err("Invalid marketplace icon URL".into());
    }
    Ok(())
}

fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = ParsedVersion::parse(left)?;
    let right = ParsedVersion::parse(right)?;
    Some(left.cmp(&right))
}

#[derive(Eq, PartialEq)]
struct ParsedVersion<'a> {
    core: [u64; 4],
    pre_release: Option<Vec<&'a str>>,
}

impl<'a> ParsedVersion<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let (value, build) = value
            .split_once('+')
            .map_or((value, None), |(value, build)| (value, Some(build)));
        if build.is_some_and(|build| !valid_version_identifiers(build, false)) {
            return None;
        }
        let (core, pre_release) = value
            .split_once('-')
            .map_or((value, None), |(core, pre)| (core, Some(pre)));
        let parts = core.split('.').collect::<Vec<_>>();
        if !(2..=4).contains(&parts.len()) || pre_release == Some("") {
            return None;
        }
        let mut numbers = [0; 4];
        for (index, part) in parts.into_iter().enumerate() {
            if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
                return None;
            }
            numbers[index] = part.parse().ok()?;
        }
        let pre_release = pre_release
            .filter(|value| valid_version_identifiers(value, true))
            .map(|value| value.split('.').collect::<Vec<_>>());
        if value.contains('-') && pre_release.is_none() {
            return None;
        }
        Some(Self {
            core: numbers,
            pre_release,
        })
    }
}

fn valid_version_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    value.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && (!reject_numeric_leading_zero
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.len() == 1
                || !part.starts_with('0'))
    })
}

impl Ord for ParsedVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => compare_pre_release(left, right),
            })
    }
}

impl PartialOrd for ParsedVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_pre_release(left: &[&str], right: &[&str]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn marketplace_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("WinIsland")
        .join("PluginMarketplace")
}

fn safe_version_component(version: &str) -> String {
    version
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("Could not open the cached plugin: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not verify the cached plugin: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize().as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes).as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

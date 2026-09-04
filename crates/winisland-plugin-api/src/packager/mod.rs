pub mod manifest;
pub mod packaging;
pub mod signing;

use ed25519_dalek::SigningKey;
use libloading::Library;
use manifest::PluginManifest;
use signing::{hash_file, load_signing_key, load_signing_key_from_env, sign_payload};
use std::path::{Path, PathBuf};

use crate::{
    ABI_VERSION_1, KNOWN_CAPABILITIES, PLUGIN_ENTRY_SYMBOL_V1, PluginDescriptorV1, PluginEntryFnV1,
};

/// A build-time tool that compiles, packages, and optionally signs
/// a WinIsland plugin DLL into a distributable ZIP archive.
///
/// # Usage
///
/// ```rust,no_run
/// use winisland_plugin_api::packager::PluginPackager;
///
/// PluginPackager::from_cargo()
///     .unwrap()
///     .signing_key_path("signing_key.pem")
///     .include_dir("assets")
///     .build()
///     .unwrap();
/// ```
pub struct PluginPackager {
    id: String,
    name: String,
    author: String,
    version: String,
    description: String,
    github_link: String,
    dll_name: String,
    dll_path: Option<PathBuf>,
    icon: Option<String>,
    readme: Option<String>,
    extra_dirs: Vec<String>,
    signing_key: Option<SigningKey>,
    output: Option<PathBuf>,
}

impl PluginPackager {
    /// Create a packager by reading `Cargo.toml` from the current working directory.
    ///
    /// Automatically fills in package metadata, repository URL, and the DLL
    /// filename from `[lib].name` when present.
    pub fn from_cargo() -> Result<Self, String> {
        let cargo_toml_path = Path::new("Cargo.toml");
        let contents = std::fs::read_to_string(cargo_toml_path).map_err(|e| {
            format!(
                "Cannot read Cargo.toml (run from the plugin project root): {}",
                e
            )
        })?;

        let value: toml::Value =
            toml::from_str(&contents).map_err(|e| format!("Cannot parse Cargo.toml: {}", e))?;

        let pkg = value
            .get("package")
            .ok_or_else(|| "Cargo.toml missing [package] section".to_string())?;

        let package_name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Cargo.toml missing package.name".to_string())?
            .to_string();

        let version = pkg
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0")
            .to_string();

        let author = pkg
            .get("authors")
            .and_then(|v| v.as_array())
            .map(|authors| {
                authors
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(":")
            })
            .unwrap_or_default();

        let description = pkg
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let github_link = pkg
            .get("repository")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();

        let plugin_metadata = pkg
            .get("metadata")
            .and_then(|metadata| metadata.get("winisland"));
        let id = plugin_metadata
            .and_then(|metadata| metadata.get("id"))
            .and_then(toml::Value::as_str)
            .unwrap_or(&package_name)
            .to_string();
        let name = plugin_metadata
            .and_then(|metadata| metadata.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or(&package_name)
            .to_string();

        let dll_name = value
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(|value| value.as_str())
            .map_or_else(|| package_name.replace('-', "_"), str::to_string);
        let icon = ["icon.png", "icon.jpg", "icon.jpeg", "icon.webp"]
            .into_iter()
            .find(|path| Path::new(path).is_file())
            .map(str::to_string);
        let readme = ["README.md", "README.markdown", "README.txt"]
            .into_iter()
            .find(|path| Path::new(path).is_file())
            .map(str::to_string);

        Ok(Self {
            id,
            name,
            author,
            version,
            description,
            github_link,
            dll_name,
            dll_path: None,
            icon,
            readme,
            extra_dirs: Vec::new(),
            signing_key: None,
            output: None,
        })
    }

    /// Create a packager with manually specified plugin name.
    pub fn new(name: &str) -> Self {
        Self {
            id: name.to_string(),
            name: name.to_string(),
            author: String::new(),
            version: "0.1.0".to_string(),
            description: String::new(),
            github_link: String::new(),
            dll_name: name.to_string().replace('-', "_"),
            dll_path: None,
            icon: None,
            readme: None,
            extra_dirs: Vec::new(),
            signing_key: None,
            output: None,
        }
    }

    /// Set the plugin ID returned by the ABI v1 descriptor.
    pub fn id(&mut self, value: &str) -> &mut Self {
        self.id = value.to_string();
        self
    }

    /// Set the human-readable plugin name returned by the ABI descriptor.
    pub fn name(&mut self, value: &str) -> &mut Self {
        self.name = value.to_string();
        self
    }

    /// Set the plugin author.
    pub fn author(&mut self, v: &str) -> &mut Self {
        self.author = v.to_string();
        self
    }

    /// Set the plugin version.
    pub fn version(&mut self, v: &str) -> &mut Self {
        self.version = v.to_string();
        self
    }

    /// Set the plugin description.
    pub fn description(&mut self, v: &str) -> &mut Self {
        self.description = v.to_string();
        self
    }

    /// Set the GitHub link for the plugin repository.
    pub fn github_link(&mut self, v: &str) -> &mut Self {
        self.github_link = v.to_string();
        self
    }

    /// Override the DLL filename (without `.dll` extension).
    ///
    /// By default the DLL name is derived from the crate name by
    /// replacing hyphens with underscores.
    pub fn dll_name(&mut self, name: &str) -> &mut Self {
        self.dll_name = name.to_string();
        self
    }

    /// Override the built DLL path.
    ///
    /// By default it looks for `target/release/<dll_name>.dll`.
    pub fn dll_path(&mut self, path: &str) -> &mut Self {
        self.dll_path = Some(PathBuf::from(path));
        self
    }

    /// Include an image displayed as the plugin icon in WinIsland settings.
    pub fn icon(&mut self, path: &str) -> &mut Self {
        self.icon = Some(path.to_string());
        self
    }

    /// Include a Markdown or text file displayed on the plugin details page.
    pub fn readme(&mut self, path: &str) -> &mut Self {
        self.readme = Some(path.to_string());
        self
    }

    /// Include an additional directory in the plugin ZIP.
    ///
    /// The directory is relative to the plugin project root.
    /// Can be called multiple times to include multiple directories.
    /// Typical uses: `assets`, `locales`, `fonts`.
    pub fn include_dir(&mut self, dir: &str) -> &mut Self {
        self.extra_dirs.push(dir.to_string());
        self
    }

    /// Sign the plugin with a key loaded from a PEM file.
    pub fn signing_key_path(&mut self, path: &str) -> &mut Self {
        match load_signing_key(Path::new(path)) {
            Ok(key) => {
                self.signing_key = Some(key);
            }
            Err(e) => {
                log::warn!("Signing key not loaded: {}", e);
            }
        }
        self
    }

    /// Sign the plugin with a key loaded from an environment variable.
    pub fn signing_key_env(&mut self, var: &str) -> &mut Self {
        match load_signing_key_from_env(var) {
            Ok(key) => {
                self.signing_key = Some(key);
            }
            Err(e) => {
                log::warn!("Signing key not loaded from env '{}': {}", var, e);
            }
        }
        self
    }

    /// Sign the plugin with a key provided directly as bytes.
    pub fn signing_key_bytes(&mut self, key_bytes: &[u8; 64]) -> &mut Self {
        match SigningKey::from_keypair_bytes(key_bytes) {
            Ok(key) => {
                self.signing_key = Some(key);
            }
            Err(e) => {
                log::warn!("Signing key not loaded from bytes: {}", e);
            }
        }
        self
    }

    /// Set the output ZIP path.
    ///
    /// Defaults to `target/<name>-<version>.zip`.
    pub fn output(&mut self, path: &str) -> &mut Self {
        self.output = Some(PathBuf::from(path));
        self
    }

    /// Execute the full build + package + sign pipeline.
    ///
    /// 1. Runs `cargo build --release`
    /// 2. Locates the built `.dll`
    /// 3. Creates a staging directory with the DLL and extra dirs
    /// 4. Generates `plugin.yml` with DLL hashes
    /// 5. Signs the manifest if a signing key was provided
    /// 6. Packs everything into a ZIP archive
    ///
    /// Returns the path to the generated ZIP file.
    pub fn build(&self) -> Result<PathBuf, String> {
        // 1. Build the DLL
        log::info!("Building plugin '{}' in release mode...", self.name);
        let status = std::process::Command::new("cargo")
            .args(["build", "--release", "--locked"])
            .status()
            .map_err(|e| format!("Failed to run cargo build: {}", e))?;

        if !status.success() {
            return Err("cargo build --release failed".to_string());
        }

        // 2. Locate the DLL
        let dll_path = self.locate_dll()?;
        let dll_dest_name = dll_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("plugin.dll");

        // 3. Create staging directory
        let staging = tempfile::tempdir().map_err(|e| format!("Cannot create temp dir: {}", e))?;
        let staging_path = staging.path();

        // 4. Copy DLL
        std::fs::copy(&dll_path, staging_path.join(dll_dest_name))
            .map_err(|e| format!("Cannot copy DLL: {}", e))?;

        // 5. Copy extra directories
        for dir in &self.extra_dirs {
            let src = package_relative_path(dir)?;
            if src.exists() {
                let dst = staging_path.join(src);
                copy_dir_all(src, &dst)?;
            } else {
                log::warn!("Extra directory '{}' not found, skipping", dir);
            }
        }
        for asset in [&self.icon, &self.readme].into_iter().flatten() {
            copy_package_file(asset, staging_path)?;
        }

        // 6. Compute DLL hashes
        let mut dll_hashes = Vec::new();
        let dll_hash = hash_file(&dll_path).map_err(|e| format!("Cannot hash DLL: {}", e))?;
        dll_hashes.push(dll_hash);
        // Also hash any extra DLLs in extra dirs
        for dir in &self.extra_dirs {
            let dll_dir = staging_path.join(package_relative_path(dir)?);
            if dll_dir.exists() {
                collect_dll_hashes(&dll_dir, &mut dll_hashes)?;
            }
        }

        // 7. Build manifest
        let mut manifest = PluginManifest {
            id: self.id.clone(),
            name: self.name.clone(),
            author: self.author.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            github_link: self.github_link.clone(),
            abi_version: crate::ABI_VERSION_1,
            entry: dll_dest_name.to_string(),
            icon: self.icon.clone(),
            readme: self.readme.clone(),
            signature: None,
            dll_hashes: Some(dll_hashes.clone()),
        };

        // 8. Sign the manifest
        if let Some(key) = &self.signing_key {
            let payload = manifest.signing_payload();
            let sig = sign_payload(key, payload.as_bytes());
            manifest.signature = Some(sig);
            log::info!("Plugin signed");
        } else {
            log::info!("Plugin not signed (no signing key provided)");
        }

        // 9. Validate and write plugin.yml
        manifest
            .validate()
            .map_err(|e| format!("Invalid manifest: {}", e))?;
        validate_dll_descriptor(&dll_path, &manifest)?;
        manifest
            .write_to_yaml(&staging_path.join("plugin.yml"))
            .map_err(|e| format!("Cannot write plugin.yml: {}", e))?;

        // 10. Create ZIP
        let output_path = self
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("target/{}-{}.zip", self.name, self.version)));

        packaging::create_zip(staging_path, &output_path)?;

        log::info!("Plugin packaged: {}", output_path.display());
        Ok(output_path)
    }

    fn locate_dll(&self) -> Result<PathBuf, String> {
        if let Some(path) = &self.dll_path {
            if path.exists() {
                return Ok(path.clone());
            }
            return Err(format!(
                "Specified DLL path does not exist: {}",
                path.display()
            ));
        }

        // Default: target/release/<dll_name>.dll
        let release_path = PathBuf::from(format!("target/release/{}.dll", self.dll_name));
        if release_path.exists() {
            return Ok(release_path);
        }

        // Fallback: target/release/<dll_name>.so (Linux/macOS)
        let release_so = PathBuf::from(format!("target/release/lib{}.so", self.dll_name));
        if release_so.exists() {
            return Ok(release_so);
        }

        Err(format!(
            "Cannot find built DLL. Expected at '{}' or '{}'. \
             Make sure 'cargo build --release' completed successfully.",
            release_path.display(),
            release_so.display(),
        ))
    }
}

fn validate_dll_descriptor(dll_path: &Path, manifest: &PluginManifest) -> Result<(), String> {
    // SAFETY: The packager loads the plugin DLL it just built and resolves the documented ABI
    // entry point. It does not create a plugin instance or invoke plugin-controlled callbacks.
    let library = unsafe { Library::new(dll_path) }
        .map_err(|error| format!("Cannot validate plugin DLL: {error}"))?;
    // SAFETY: The symbol type is the public WinIsland ABI v1 entry-point signature.
    let entry = unsafe { library.get::<PluginEntryFnV1>(PLUGIN_ENTRY_SYMBOL_V1) }
        .map_err(|error| format!("Plugin DLL has no ABI v1 entry point: {error}"))?;
    // SAFETY: The entry point is called synchronously while the DLL remains loaded.
    let pointer = unsafe { entry() };
    if pointer.is_null() {
        return Err("Plugin DLL returned a null descriptor".into());
    }
    // SAFETY: Every ABI descriptor starts with a readable struct_size field.
    let struct_size = unsafe { std::ptr::read_unaligned(pointer.cast::<u32>()) };
    if struct_size < std::mem::size_of::<PluginDescriptorV1>() as u32 {
        return Err("Plugin DLL returned a truncated ABI v1 descriptor".into());
    }
    // SAFETY: The size check proves the complete copyable ABI v1 prefix is available.
    let descriptor = unsafe { std::ptr::read_unaligned(pointer) };
    if descriptor.abi_version != ABI_VERSION_1
        || descriptor.capabilities & !KNOWN_CAPABILITIES != 0
        || descriptor.create.is_none()
        || descriptor.shutdown.is_none()
        || descriptor.destroy.is_none()
    {
        return Err("Plugin DLL contains an invalid ABI v1 descriptor".into());
    }

    let metadata = &descriptor.metadata;
    for (field, manifest_value, descriptor_value) in [
        ("id", manifest.id.as_str(), fixed_text(&metadata.id)?),
        ("name", manifest.name.as_str(), fixed_text(&metadata.name)?),
        (
            "version",
            manifest.version.as_str(),
            fixed_text(&metadata.version)?,
        ),
        (
            "author",
            manifest.author.as_str(),
            fixed_text(&metadata.author)?,
        ),
        (
            "description",
            manifest.description.as_str(),
            fixed_text(&metadata.description)?,
        ),
    ] {
        if manifest_value != descriptor_value {
            return Err(format!(
                "Plugin metadata mismatch for '{field}': Cargo/package value '{manifest_value}' does not match DLL descriptor '{descriptor_value}'"
            ));
        }
    }
    Ok(())
}

fn fixed_text(value: &[u8]) -> Result<&str, String> {
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    std::str::from_utf8(&value[..end])
        .map_err(|error| format!("Plugin DLL metadata is not valid UTF-8: {error}"))
}

fn package_relative_path(path: &str) -> Result<&Path, String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "Package directory '{path:?}' must be a relative path"
        ));
    }
    Ok(path)
}

fn copy_package_file(path: &str, staging: &Path) -> Result<(), String> {
    let source = package_relative_path(path)?;
    if !source.is_file() {
        return Err(format!(
            "Package file '{}' does not exist",
            source.display()
        ));
    }
    let destination = staging.join(source);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create '{}': {error}", parent.display()))?;
    }
    std::fs::copy(source, &destination)
        .map_err(|error| format!("Cannot copy '{}': {error}", source.display()))?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Cannot create dir '{}': {}", dst.display(), e))?;

    for entry in
        std::fs::read_dir(src).map_err(|e| format!("Cannot read dir '{}': {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("File type error: {}", e))?;
        let src_path = entry.path();
        let file_name = src_path
            .file_name()
            .ok_or_else(|| "Invalid filename".to_string())?;
        let dst_path = dst.join(file_name);

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Cannot copy '{}': {}", src_path.display(), e))?;
        }
    }
    Ok(())
}

fn collect_dll_hashes(dir: &Path, hashes: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "dll") {
            let hash = hash_file(&path)?;
            hashes.push(hash);
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            collect_dll_hashes(&path, hashes)?;
        }
    }
    Ok(())
}

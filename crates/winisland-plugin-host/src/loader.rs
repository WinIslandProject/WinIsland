use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};

use libloading::Library;
use libloading::os::windows::{
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, Library as WindowsLibrary,
};
use winisland_plugin_api::abi::{
    ABI_VERSION_2, KNOWN_CAPABILITIES_V2, PLUGIN_ENTRY_SYMBOL_V2, PluginDescriptorV2,
    PluginEntryFnV2,
};
use winisland_plugin_api::types::metadata::PluginMetadataC;

use crate::PluginHostError;

pub struct PluginLibrary {
    pub(crate) descriptor: PluginDescriptorV2,
    pub(crate) library: ManuallyDrop<Library>,
    path: PathBuf,
    metadata: PluginMetadata,
}

#[derive(Clone, Debug)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

fn read_fixed(value: &[u8]) -> String {
    String::from_utf8_lossy(value.split(|byte| *byte == 0).next().unwrap_or(value)).into_owned()
}

impl From<&PluginMetadataC> for PluginMetadata {
    fn from(value: &PluginMetadataC) -> Self {
        Self {
            id: read_fixed(&value.id),
            name: read_fixed(&value.name),
            version: read_fixed(&value.version),
            author: read_fixed(&value.author),
            description: read_fixed(&value.description),
        }
    }
}

impl PluginLibrary {
    pub fn open(path: &Path) -> Result<Self, PluginHostError> {
        let path = std::fs::canonicalize(path)
            .map_err(|error| PluginHostError::Io(format!("{}: {error}", path.display())))?;
        // SAFETY: The canonical path and restricted flags limit dependency resolution.
        let library: Library = unsafe {
            WindowsLibrary::load_with_flags(
                &path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }
        .map(Into::into)
        .map_err(|error| PluginHostError::Io(format!("{}: {error}", path.display())))?;
        // SAFETY: A symbol lookup does not call plugin code.
        let entry = unsafe { library.get::<PluginEntryFnV2>(PLUGIN_ENTRY_SYMBOL_V2) };
        let entry = entry.map_err(|error| {
            PluginHostError::Invalid(format!(
                "{}: No ABI v2 entry point: {error}",
                path.display()
            ))
        })?;
        // SAFETY: The exported entry point promises a stable descriptor allocation.
        let descriptor_ptr = unsafe { entry() };
        if descriptor_ptr.is_null() {
            return Err(PluginHostError::Invalid(format!(
                "{} returned a null descriptor",
                path.display()
            )));
        }
        // SAFETY: A valid ABI descriptor begins with a readable size field.
        let struct_size = unsafe { std::ptr::read_unaligned(descriptor_ptr.cast::<u32>()) };
        if struct_size < std::mem::size_of::<PluginDescriptorV2>() as u32 {
            return Err(PluginHostError::Invalid(format!(
                "{} returned a truncated ABI v2 descriptor",
                path.display()
            )));
        }
        // SAFETY: The size check covers the complete current descriptor layout.
        let descriptor = unsafe { std::ptr::read_unaligned(descriptor_ptr) };
        if descriptor.abi_version != ABI_VERSION_2 {
            return Err(PluginHostError::Invalid(format!(
                "{} uses unsupported ABI version {}",
                path.display(),
                descriptor.abi_version
            )));
        }
        if descriptor.capabilities & !KNOWN_CAPABILITIES_V2 != 0 {
            return Err(PluginHostError::Invalid(format!(
                "{} requires unsupported capabilities 0x{:x}",
                path.display(),
                descriptor.capabilities & !KNOWN_CAPABILITIES_V2
            )));
        }
        if descriptor.create.is_none()
            || descriptor.shutdown.is_none()
            || descriptor.destroy.is_none()
        {
            return Err(PluginHostError::Invalid(format!(
                "{} is missing lifecycle callbacks",
                path.display()
            )));
        }
        let metadata = PluginMetadata::from(&descriptor.metadata);
        if metadata.id.is_empty()
            || !metadata
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(PluginHostError::Invalid(format!(
                "invalid plugin id '{}'",
                metadata.id
            )));
        }
        Ok(Self {
            descriptor,
            library: ManuallyDrop::new(library),
            path,
            metadata,
        })
    }

    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn capabilities(&self) -> u64 {
        self.descriptor.capabilities
    }
}

impl Drop for PluginLibrary {
    fn drop(&mut self) {
        // SAFETY: PluginLibrary owns the DLL and has no active instance at this point.
        unsafe { ManuallyDrop::drop(&mut self.library) };
    }
}

use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};

use libloading::Library;
use libloading::os::windows::{
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, Library as WindowsLibrary,
};

use super::types::{
    ABI_VERSION_1, HostApiV1, KNOWN_CAPABILITIES, PLUGIN_ENTRY_SYMBOL_V1, PluginCreateInfoV1,
    PluginDescriptorV1, PluginEntryFnV1, PluginError, PluginHandle, PluginMetadata, PluginToken,
};

pub struct NativePlugin {
    metadata: PluginMetadata,
    descriptor: PluginDescriptorV1,
    handle: PluginHandle,
    token: PluginToken,
    created: bool,
    shutdown: bool,
    path: PathBuf,
    library: ManuallyDrop<Library>,
}

impl NativePlugin {
    pub fn load(path: &Path) -> Result<Self, PluginError> {
        let path = std::fs::canonicalize(path)
            .map_err(|error| PluginError::LoadFailed(format!("{}: {error}", path.display())))?;
        // SAFETY: Loading a native plugin executes trusted plugin code. The canonical path and
        // restricted flags ensure its dependencies come only from its directory or System32.
        let library: Library = unsafe {
            WindowsLibrary::load_with_flags(
                &path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }
        .map(Into::into)
        .map_err(|error| PluginError::LoadFailed(format!("{}: {error}", path.display())))?;
        // SAFETY: The symbol is validated against the documented ABI v1 signature.
        let entry =
            unsafe { library.get::<PluginEntryFnV1>(PLUGIN_ENTRY_SYMBOL_V1) }.map_err(|error| {
                PluginError::InvalidPlugin(format!(
                    "{} does not export winisland_plugin_entry_v1: {error}",
                    path.display()
                ))
            })?;
        // SAFETY: Calling the trusted plugin entry point does not transfer ownership.
        let descriptor_ptr = unsafe { entry() };
        if descriptor_ptr.is_null() {
            return Err(PluginError::InvalidPlugin(format!(
                "{} returned a null descriptor",
                path.display()
            )));
        }

        // SAFETY: A valid descriptor starts with a readable u32 struct_size field.
        let struct_size = unsafe { std::ptr::read_unaligned(descriptor_ptr.cast::<u32>()) };
        if struct_size < std::mem::size_of::<PluginDescriptorV1>() as u32 {
            return Err(PluginError::InvalidPlugin(format!(
                "{} returned a truncated ABI v1 descriptor",
                path.display()
            )));
        }
        // SAFETY: struct_size proves the complete ABI v1 prefix is available.
        let descriptor = unsafe { std::ptr::read_unaligned(descriptor_ptr) };
        if descriptor.abi_version != ABI_VERSION_1 {
            return Err(PluginError::InvalidPlugin(format!(
                "{} uses unsupported ABI version {}",
                path.display(),
                descriptor.abi_version
            )));
        }
        if descriptor.capabilities & !KNOWN_CAPABILITIES != 0 {
            return Err(PluginError::InvalidPlugin(format!(
                "{} requires unsupported capabilities 0x{:x}",
                path.display(),
                descriptor.capabilities & !KNOWN_CAPABILITIES
            )));
        }
        if descriptor.create.is_none()
            || descriptor.shutdown.is_none()
            || descriptor.destroy.is_none()
        {
            return Err(PluginError::InvalidPlugin(format!(
                "{} is missing required lifecycle functions",
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
            return Err(PluginError::InvalidPlugin(format!(
                "plugin id '{}' must match [a-zA-Z0-9_-]+",
                metadata.id
            )));
        }

        Ok(Self {
            metadata,
            descriptor,
            handle: std::ptr::null_mut(),
            token: 0,
            created: false,
            shutdown: false,
            path: path.to_path_buf(),
            library: ManuallyDrop::new(library),
        })
    }

    pub fn initialize(
        &mut self,
        token: PluginToken,
        host_api: *const HostApiV1,
    ) -> Result<(), PluginError> {
        let create_info = PluginCreateInfoV1 {
            struct_size: std::mem::size_of::<PluginCreateInfoV1>() as u32,
            abi_version: ABI_VERSION_1,
            plugin_token: token,
            host_api,
        };
        let create = self
            .descriptor
            .create
            .ok_or_else(|| PluginError::InvalidPlugin("missing create function".to_string()))?;
        let mut handle = std::ptr::null_mut();
        // SAFETY: create comes from the validated descriptor and receives ABI v1 data.
        let result = unsafe { create(&create_info, &mut handle) };
        if let Err(error) = result.into_result() {
            if !handle.is_null() {
                self.handle = handle;
                self.token = token;
                self.created = true;
            }
            return Err(PluginError::ExecutionError(format!(
                "plugin '{}' create failed: {error}",
                self.metadata.id
            )));
        }
        if handle.is_null() {
            return Err(PluginError::ExecutionError(format!(
                "plugin '{}' returned a null handle",
                self.metadata.id
            )));
        }
        self.handle = handle;
        self.token = token;
        self.created = true;
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), PluginError> {
        if !self.created || self.shutdown {
            return Ok(());
        }
        if let Some(shutdown) = self.descriptor.shutdown {
            // SAFETY: handle was returned by create and remains valid until destroy.
            let result = unsafe { shutdown(self.handle) };
            if let Err(error) = result.into_result() {
                return Err(PluginError::ExecutionError(format!(
                    "plugin '{}' shutdown failed: {error}",
                    self.metadata.id
                )));
            }
        }
        self.shutdown = true;
        Ok(())
    }

    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    pub fn capabilities(&self) -> u64 {
        self.descriptor.capabilities
    }

    pub fn token(&self) -> PluginToken {
        self.token
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for NativePlugin {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            log::error!("{error}; keeping the plugin DLL loaded");
            return;
        }
        if self.created
            && let Some(destroy) = self.descriptor.destroy
        {
            // SAFETY: shutdown has completed and destroy owns the plugin handle cleanup.
            unsafe { destroy(self.handle) };
        }
        // SAFETY: The DLL is unloaded only after all plugin function calls have completed.
        unsafe { ManuallyDrop::drop(&mut self.library) };
    }
}

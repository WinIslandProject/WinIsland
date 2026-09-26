use std::collections::HashMap;
use std::sync::Mutex;

use winisland_plugin_api::abi::{KNOWN_CAPABILITIES_V2, PluginStatus};
use winisland_plugin_api::types::v2::PluginToken;

use crate::PluginHostError;

pub struct Registry {
    inner: Mutex<RegistryState>,
}

struct RegistryState {
    next: u64,
    plugins: HashMap<PluginToken, Registration>,
}

#[derive(Clone)]
pub struct Registration {
    pub id: String,
    pub version: String,
    pub capabilities: u64,
    pub stopping: bool,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RegistryState {
                next: 1_u64 << 32,
                plugins: HashMap::new(),
            }),
        }
    }

    pub fn register(
        &self,
        id: String,
        version: String,
        capabilities: u64,
    ) -> Result<PluginToken, PluginHostError> {
        if capabilities & !KNOWN_CAPABILITIES_V2 != 0 {
            return Err(PluginHostError::Invalid(
                "unsupported plugin capability".into(),
            ));
        }
        let mut state = self
            .inner
            .lock()
            .map_err(|_| PluginHostError::Execution("plugin registry lock is poisoned".into()))?;
        if state.plugins.values().any(|plugin| plugin.id == id) {
            return Err(PluginHostError::Invalid(format!(
                "plugin '{id}' is already registered"
            )));
        }
        let raw = state.next;
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| PluginHostError::Execution("plugin token space exhausted".into()))?;
        // SAFETY: Only this registry constructs nonzero tokens and never reuses them.
        let token = unsafe { PluginToken::from_raw(raw) };
        state.plugins.insert(
            token,
            Registration {
                id,
                version,
                capabilities,
                stopping: false,
            },
        );
        Ok(token)
    }

    pub fn get(&self, token: PluginToken) -> Result<Registration, PluginStatus> {
        self.inner
            .lock()
            .map_err(|_| PluginStatus::Internal)?
            .plugins
            .get(&token)
            .cloned()
            .ok_or(PluginStatus::StaleHandle)
    }

    pub fn require(&self, token: PluginToken, capability: u64) -> Result<(), PluginStatus> {
        let plugin = self.get(token)?;
        if plugin.stopping {
            return Err(PluginStatus::StaleHandle);
        }
        if plugin.capabilities & capability == 0 {
            return Err(PluginStatus::CapabilityMissing);
        }
        Ok(())
    }

    pub fn begin_shutdown(&self, token: PluginToken) -> Result<(), PluginStatus> {
        let mut state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        let plugin = state
            .plugins
            .get_mut(&token)
            .ok_or(PluginStatus::StaleHandle)?;
        plugin.stopping = true;
        Ok(())
    }

    pub fn revoke(&self, token: PluginToken) -> Result<Registration, PluginStatus> {
        self.inner
            .lock()
            .map_err(|_| PluginStatus::Internal)?
            .plugins
            .remove(&token)
            .ok_or(PluginStatus::StaleHandle)
    }
}

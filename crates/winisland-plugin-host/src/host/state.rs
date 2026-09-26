use winisland_plugin_api::types::v2::context::HostStateV2;

use super::PluginHost;
use crate::PluginHostError;

impl PluginHost {
    pub fn set_host_state(&self, snapshot: HostStateV2) -> Result<(), PluginHostError> {
        let subscribers = self
            .runtime
            .set_host_state(snapshot)
            .map_err(|status| PluginHostError::Execution(format!("host state: {status:?}")))?;
        let entries = self.entries.borrow();
        for (token, id) in subscribers {
            if let Some(entry) = entries.iter().find(|entry| entry.token() == token) {
                entry.queue_host_state(id, snapshot)?;
            }
        }
        Ok(())
    }
}

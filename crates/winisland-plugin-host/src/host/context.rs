use std::time::Duration;

use winisland_core::context::{PluginContext, Priority};
use winisland_plugin_api::types::v2::context::CONTEXT_FLAG_SHOW_COMPACT;

use super::{PluginHost, fixed_text};

impl PluginHost {
    pub fn contexts_snapshot(&self) -> Option<(u64, Vec<PluginContext>)> {
        let state = self.runtime.state.lock().ok()?;
        let contexts = state
            .contexts
            .iter()
            .map(|(id, record)| {
                let data = &record.data;
                PluginContext {
                    id: *id,
                    priority: match data.priority {
                        0 => Priority::Low,
                        2 => Priority::High,
                        _ => Priority::Medium,
                    },
                    title: fixed_text(&data.title),
                    body: fixed_text(&data.body),
                    compact_text: fixed_text(&data.compact_text),
                    show_compact: data.flags & CONTEXT_FLAG_SHOW_COMPACT != 0,
                    expires_at: (data.timeout_ms != 0)
                        .then(|| record.updated_at + Duration::from_millis(data.timeout_ms as u64)),
                    updated_at: record.updated_at,
                }
            })
            .collect();
        Some((state.context_revision, contexts))
    }
}

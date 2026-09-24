mod types;

pub use types::*;

use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum MiniContent<'a> {
    Music,
    Plugin(&'a PluginContext),
}

pub struct ContextManager {
    plugin_contexts: Vec<PluginContext>,
    smtc_active: bool,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            plugin_contexts: Vec::new(),
            smtc_active: false,
        }
    }

    pub fn set_smtc_active(&mut self, active: bool) {
        self.smtc_active = active;
    }

    pub fn upsert_context(&mut self, context: PluginContext) {
        if let Some(existing) = self
            .plugin_contexts
            .iter_mut()
            .find(|existing| existing.id == context.id)
        {
            *existing = context;
        } else {
            self.plugin_contexts.push(context);
        }
    }

    pub fn remove_context(&mut self, id: u64) -> bool {
        let original_len = self.plugin_contexts.len();
        self.plugin_contexts.retain(|context| context.id != id);
        self.plugin_contexts.len() != original_len
    }

    pub fn current_mini(&self) -> Option<MiniContent<'_>> {
        if let Some(context) = self
            .plugin_contexts
            .iter()
            .filter(|context| context.show_compact)
            .max_by_key(|context| (context.priority, context.updated_at))
        {
            return Some(MiniContent::Plugin(context));
        }
        self.smtc_active.then_some(MiniContent::Music)
    }

    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let previous_len = self.plugin_contexts.len();
        self.plugin_contexts
            .retain(|context| context.expires_at.is_none_or(|expires_at| expires_at > now));
        self.plugin_contexts.len() != previous_len
    }
}

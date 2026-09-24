use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Medium = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub id: u64,
    pub priority: Priority,
    pub title: String,
    pub body: String,
    pub compact_text: String,
    pub show_compact: bool,
    pub expires_at: Option<Instant>,
    pub updated_at: Instant,
}

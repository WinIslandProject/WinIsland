use crate::core::config::PluginWidgetId;
#[derive(Clone)]
pub struct PluginWidget {
    pub id: u64,
    pub plugin_id: String,
    pub key: Option<String>,
    pub span_cols: u32,
    pub span_rows: u32,
    pub title: String,
    pub body: String,
}

impl PluginWidget {
    pub fn layout_id(&self) -> Option<PluginWidgetId> {
        Some(PluginWidgetId {
            plugin_id: self.plugin_id.clone(),
            widget_key: self.key.clone()?,
        })
    }

    pub fn span(&self) -> (usize, usize) {
        (self.span_cols as usize, self.span_rows as usize)
    }
}

pub struct WidgetManager {
    plugin_widgets: Vec<PluginWidget>,
}

impl WidgetManager {
    pub fn new() -> Self {
        Self {
            plugin_widgets: Vec::new(),
        }
    }

    pub fn upsert_widget(&mut self, widget: PluginWidget) {
        if let Some(existing) = self
            .plugin_widgets
            .iter_mut()
            .find(|existing| existing.id == widget.id)
        {
            *existing = widget;
        } else {
            self.plugin_widgets.push(widget);
        }
    }

    pub fn remove_widget(&mut self, id: u64) -> bool {
        let original_len = self.plugin_widgets.len();
        self.plugin_widgets.retain(|widget| widget.id != id);
        self.plugin_widgets.len() != original_len
    }

    pub fn widgets(&self) -> &[PluginWidget] {
        &self.plugin_widgets
    }

    pub fn configurable_widgets(&self) -> Vec<PluginWidget> {
        self.plugin_widgets
            .iter()
            .filter(|widget| widget.key.is_some())
            .cloned()
            .collect()
    }
}

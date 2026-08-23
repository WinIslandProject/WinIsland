use crate::core::config::{
    AppConfig, MAX_HIDDEN_WIDTH, MIN_HIDDEN_WIDTH, WIDGET_GRID_SLOTS, ensure_settings_widget,
    normalize_compact_widget_layout,
};
use std::fs;
use std::path::PathBuf;
pub fn get_config_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".winisland");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.push("config.toml");
    path
}
pub fn load_config() -> AppConfig {
    let path = get_config_path();
    let mut migrated = false;
    let mut config: AppConfig = if let Ok(content) = fs::read_to_string(&path)
        && let Ok(mut config) = toml::from_str::<AppConfig>(&content)
    {
        if let Ok(table) = toml::from_str::<toml::Table>(&content) {
            if let Some(fully_hide) = table.get("fully_hide").and_then(|value| value.as_bool()) {
                if !table.contains_key("hidden_width") && fully_hide {
                    config.hidden_width = MIN_HIDDEN_WIDTH;
                }
                migrated = true;
            }
            if !table.contains_key("lyrics_mode") {
                config.lyrics_mode = if config
                    .lyrics_local_dir
                    .as_deref()
                    .is_some_and(|dir| !dir.trim().is_empty())
                {
                    "lrc"
                } else {
                    "online"
                }
                .to_string();
                migrated = true;
            }
            if let Some(entries) = table
                .get("compact_widget_layout")
                .and_then(toml::Value::as_array)
            {
                for (entry, raw_entry) in config.compact_widget_layout.iter_mut().zip(entries) {
                    if raw_entry
                        .as_table()
                        .is_some_and(|table| !table.contains_key("alignment"))
                    {
                        entry.alignment =
                            crate::core::config::CompactWidgetAlignment::legacy_slot(entry.slot);
                        migrated = true;
                    }
                }
            }
        }
        config
    } else {
        log::info!("Config file not found, using defaults");
        let default = AppConfig::default();
        save_config(&default);
        return default;
    };
    config.global_scale = config.global_scale.clamp(0.5, 5.0);
    config.base_width = config.base_width.clamp(40.0, 400.0);
    config.base_height = config.base_height.clamp(15.0, 200.0);
    config.expanded_width = config.expanded_width.clamp(200.0, 2000.0);
    config.expanded_height = config.expanded_height.clamp(100.0, 1000.0);
    let hidden_width = config
        .hidden_width
        .clamp(MIN_HIDDEN_WIDTH, MAX_HIDDEN_WIDTH);
    if hidden_width != config.hidden_width {
        config.hidden_width = hidden_width;
        migrated = true;
    }
    if !matches!(config.lyrics_mode.as_str(), "online" | "lrc") {
        config.lyrics_mode = "online".to_string();
        migrated = true;
    }
    if config.lyrics_mode == "lrc" && !config.show_lyrics {
        config.show_lyrics = true;
        migrated = true;
    }
    if ensure_settings_widget(&mut config.widget_layout) {
        migrated = true;
    }
    if normalize_compact_widget_layout(&mut config.compact_widget_layout) {
        migrated = true;
    }
    let plugin_layout_len = config.plugin_widget_layout.len();
    config.plugin_widget_layout.retain(|entry| {
        entry.slot < WIDGET_GRID_SLOTS
            && entry.plugin_id.len() <= 255
            && entry.widget_key.len() <= 63
            && !entry.plugin_id.is_empty()
            && !entry.widget_key.is_empty()
            && entry
                .plugin_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            && entry
                .widget_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    });
    if config.plugin_widget_layout.len() != plugin_layout_len {
        migrated = true;
    }
    if migrated {
        save_config(&config);
    }
    config
}
pub fn save_config(config: &AppConfig) {
    let path = get_config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        let _ = fs::write(&path, content);
        log::info!("Config saved to: {}", path.display());
    }
}

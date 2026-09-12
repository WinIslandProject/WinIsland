use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::config::{
    AppConfig, MAX_HIDDEN_WIDTH, MIN_HIDDEN_WIDTH, WIDGET_GRID_SLOTS, ensure_settings_widget,
    normalize_compact_widget_layout,
};

static NEXT_CONFIG_WRITE_ID: AtomicU64 = AtomicU64::new(1);

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
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            log::info!("Config file not found, using defaults");
            let default = AppConfig::default();
            save_config(&default);
            return default;
        }
        Err(error) => {
            log::error!("Cannot read config '{}': {error}", path.display());
            return AppConfig::default();
        }
    };
    let mut config = match toml::from_str::<AppConfig>(&content) {
        Ok(config) => config,
        Err(error) => {
            log::error!("Cannot parse config '{}': {error}", path.display());
            match preserve_invalid_config(&path) {
                Ok(backup) => {
                    log::warn!(
                        "Invalid config preserved at '{}'; using defaults",
                        backup.display()
                    );
                    let default = AppConfig::default();
                    save_config(&default);
                    return default;
                }
                Err(backup_error) => {
                    log::error!(
                        "Cannot preserve invalid config '{}': {backup_error}",
                        path.display()
                    );
                    return AppConfig::default();
                }
            }
        }
    };
    if let Ok(table) = toml::from_str::<toml::Table>(&content) {
        if !table.contains_key("expanded_scale") {
            config.expanded_scale = config.compact_scale;
            migrated = true;
        }
        if let Some(fully_hide) = table.get("fully_hide").and_then(toml::Value::as_bool) {
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
    config.compact_scale = config.compact_scale.clamp(0.5, 5.0);
    config.expanded_scale = config.expanded_scale.clamp(0.5, 5.0);
    config.base_width = config.base_width.clamp(40.0, 400.0);
    config.base_height = config.base_height.clamp(15.0, 200.0);
    config.expanded_width = config.expanded_width.clamp(200.0, 2000.0);
    config.expanded_height = config.expanded_height.clamp(100.0, 1000.0);
    if config.island_style == "mica" {
        config.island_style = "default".to_string();
        migrated = true;
    }
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
    let content = match toml::to_string_pretty(config) {
        Ok(content) => content,
        Err(error) => {
            log::error!("Cannot serialize config: {error}");
            return;
        }
    };
    if let Err(error) = write_config_atomically(&path, content.as_bytes()) {
        log::error!("Cannot save config '{}': {error}", path.display());
    } else {
        log::info!("Config saved to: {}", path.display());
    }
}

fn preserve_invalid_config(path: &Path) -> std::io::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = NEXT_CONFIG_WRITE_ID.fetch_add(1, Ordering::Relaxed);
    let backup = path.with_file_name(format!(
        "config.invalid-{timestamp}-{}-{id}.toml",
        std::process::id()
    ));
    fs::copy(path, &backup)?;
    Ok(backup)
}

fn write_config_atomically(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let (temporary, mut file) = create_config_temp_file(path)?;
    let result = (|| {
        file.write_all(content)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_config_temp_file(path: &Path) -> std::io::Result<(PathBuf, fs::File)> {
    for _ in 0..16 {
        let id = NEXT_CONFIG_WRITE_ID.fetch_add(1, Ordering::Relaxed);
        let temporary =
            path.with_file_name(format!(".config.toml.tmp-{}-{id}", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not reserve a unique config temporary file",
    ))
}

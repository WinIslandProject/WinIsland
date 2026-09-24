use std::fs;
use std::path::PathBuf;

use crate::core::config::AppConfig;

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
    winisland_core::persistence::load_config_at(&get_config_path())
}

pub fn save_config(config: &AppConfig) {
    winisland_core::persistence::save_config_at(&get_config_path(), config)
}

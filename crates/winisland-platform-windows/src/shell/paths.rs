use std::path::PathBuf;

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub(super) fn config_dir() -> PathBuf {
    home_dir().join(".winisland")
}

pub(super) fn data_dir() -> PathBuf {
    config_dir()
}

pub(super) fn log_dir() -> PathBuf {
    config_dir().join("logs")
}

use super::AppConfig;

pub const CONFIG_VERSION: u32 = 1;

/// Stamps a config that was loaded as `from_version` with the current version.
///
/// Returns `true` when the config changed and therefore has to be written back.
pub fn migrate(config: &mut AppConfig, from_version: u32) -> bool {
    if from_version >= CONFIG_VERSION {
        return false;
    }
    config.config_version = CONFIG_VERSION;
    true
}

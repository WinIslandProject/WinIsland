//! Versioned C ABI and SDK for WinIsland native plugins.
//!
//! Plugins export `winisland_plugin_entry_v2` from a `cdylib` and receive
//! per-instance host service tables. See the crate README and SDK examples.

pub mod abi;
pub mod draw;
pub mod types;

#[cfg(feature = "sdk")]
pub mod sdk;

#[cfg(feature = "packager")]
pub mod packager;

pub use abi::*;
pub use types::metadata::PluginMetadataC;
pub use types::str_to_fixed;
pub use types::v2::*;

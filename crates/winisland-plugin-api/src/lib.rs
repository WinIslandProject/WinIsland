//! # WinIsland Plugin API
//!
//! C ABI types and tooling for developing [WinIsland](https://github.com/WinIslandProject/WinIsland) plugins.
//!
//! Plugins are trusted native DLLs that communicate with WinIsland through a
//! versioned C ABI and host service tables.
//!
//! ## Usage modes
//!
//! ### 1. Writing a plugin (core C ABI types, zero extra dependencies)
//!
//! ```toml
//! [dependencies]
//! winisland-plugin-api = "0.6"
//! ```
//!
//! Export the ABI v1 descriptor from a `cdylib`. See the crate README for a
//! complete lifecycle and Context example:
//!
//! ```rust,ignore
//! use winisland_plugin_api::*;
//!
//! #[unsafe(no_mangle)]
//! pub unsafe extern "C" fn winisland_plugin_entry_v1() -> *const PluginDescriptorV1 {
//!     &DESCRIPTOR
//! }
//! ```
//!
//! ### 2. Packaging a plugin (requires `packager` feature)
//!
//! ```toml
//! [dev-dependencies]
//! winisland-plugin-api = { version = "0.6", features = ["packager"] }
//! ```
//!
//! Add a `package.rs` example target that builds, signs and zips the plugin:
//!
//! ```rust,no_run
//! winisland_plugin_api::packager::PluginPackager::from_cargo()
//!     .unwrap()
//!     .build()
//!     .unwrap();
//! ```
//!
//! Then run `cargo run --example pack` to produce a `.zip` distributable.

pub mod descriptor;
pub mod draw;
pub mod host;
pub mod types;

#[cfg(feature = "packager")]
pub mod packager;

// ---------------------------------------------------------------------------
// Public re-exports — flat import for plugin authors
// ---------------------------------------------------------------------------

pub use descriptor::*;
pub use draw::{DrawApiV1, WidgetDrawContextV1, WidgetDrawFnV1};
pub use host::*;
pub use types::context::{
    CONTEXT_FLAG_SHOW_COMPACT, ContextDataV1, HostStateV1, MEDIA_COMMAND_NEXT,
    MEDIA_COMMAND_PREVIOUS, MEDIA_COMMAND_SEEK, MEDIA_COMMAND_TOGGLE_PLAY, MEDIA_CONTROL_NEXT,
    MEDIA_CONTROL_PREVIOUS, MEDIA_CONTROL_SEEK, MEDIA_CONTROL_TOGGLE_PLAY, MEDIA_FLAG_PLAYING,
    MediaCommandFnV1, MediaCommandV1, MediaSourceDataV1, PRIORITY_HIGH, PRIORITY_LOW,
    PRIORITY_MEDIUM,
};
pub use types::i18n::TranslationPairV1;
pub use types::lyrics::{
    LYRICS_TEXT_FLAG_WORD_SYNCED, LyricsTextV1, LyricsTransformFnV1, LyricsTransformerDataV1,
};
pub use types::metadata::PluginMetadataC;
pub use types::widget::{WIDGET_FLAG_SHOW_COMPACT, WidgetDataV1};
pub use types::{
    ByteSliceV1, INVALID_ID, PluginHandle, PluginResultC, PluginToken, ResourceId, Utf8SliceV1,
    str_to_fixed,
};

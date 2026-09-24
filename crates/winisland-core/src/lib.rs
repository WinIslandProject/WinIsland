//! # WinIsland Core
//!
//! Platform-independent domain layer of [WinIsland](https://github.com/WinIslandProject/WinIsland):
//! the configuration model, the pure algorithms, and the value types shared by the rest of the
//! application.
//!
//! ## What this crate deliberately does not do
//!
//! - No platform APIs, no window-system types, and no target-OS conditionals.
//! - No rendering: Skia stays behind the renderer.
//! - No UI or window code, and no dependency on any other crate of this workspace.
//! - No decisions about file locations: callers pass paths in.
//!
//! ## Dependencies
//!
//! The standard library plus light third-party crates. Anything platform or backend specific is
//! kept out, so that this crate stays compilable and reusable on its own.
//!
//! ## Dependents
//!
//! The `WinIsland` application crate, which owns every platform-specific detail and injects the
//! platform results this crate operates on.

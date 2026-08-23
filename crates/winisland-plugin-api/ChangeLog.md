# Changelog

This changelog lists published `winisland-plugin-api` releases only. Release notes are added when a version is published; there is no `Unreleased` section.

## 0.6.0 - Aug 24, 2026

Added:

- `CAPABILITY_LYRICS_TRANSFORM` and the `LyricsTransformApiV1` host service
- `LyricsTransformerDataV1` and a two-pass UTF-8 line transformation callback
- `LyricsTextV1` with line timestamps and `LYRICS_TEXT_FLAG_WORD_SYNCED`

Changed:

- Registered transformers process parsed lyrics once after fetching, before the host caches them
- Word-synchronised lines preserve their timing boundaries and reject transformed output with a
  different Unicode character count
- Plugin unload and transformer release are rejected while a lyric callback is in progress

## 0.5.0 - Aug 14, 2026

Added:

- Stable per-plugin widget keys through `WidgetDataV1::key`
- Persistent plugin widget placement and settings layout control using those stable keys

Changed:

- Widgets with a key can be managed in the layout editor and keep their placement across restarts;
  keyless 0.4 widgets remain supported
- Widget keys must be unique within a plugin, match `[a-zA-Z0-9_-]+`, and remain unchanged
  after creation

## 0.4.1 - Aug 14, 2026

Added:

- Optional `icon` and `readme` fields in packaged plugin manifests
- `PluginPackager::icon()` and `PluginPackager::readme()` for including plugin detail assets
- Automatic detection of supported root-level icon and README files during packaging

Fixed:

- Legacy package signature verification when the optional `icon` and `readme` fields are absent

## 0.4.0 - Aug 13, 2026

Added:

- `CAPABILITY_WIDGET` and the `WidgetApiV1` host service (create, update, release)
- `WidgetDataV1` — plugin-owned widget resources with grid span and a render callback
- `WidgetDrawContextV1` / `DrawApiV1` — host-provided drawing operations so plugins can render
  on the host Skia canvas without linking any graphics library
- Draw operations: text, text measurement, rect, rounded rect, circle, line, arc, image,
  and a plugin-local `save` / `restore` / `translate` transform stack
- `WIDGET_FLAG_SHOW_COMPACT` — reserved flag for future mini-island rendering
- `HostApiV1::widget_api()` for querying the widget service during `create`

Changed:

- Widgets are rendered synchronously on the render thread in the expanded widget page;
  coordinates are logical and relative to the widget slot, and the host applies `scale`/`alpha`
- `callback_data` uses the same opaque `*mut c_void` convention as the Media interface
- The `DrawApiV1` interface is versioned and validated through `WidgetDrawContextV1::draw_api()`

## 0.3.0 - Aug 9, 2026

Added:

- Native DLL ABI v1 with the `winisland_plugin_entry_v1` descriptor entry point
- `PluginToken` and `ResourceId` identities for host-validated ownership
- Capability declarations for Context, Media, i18n, and Host State services
- Versioned host service discovery through `HostApiV1::query_interface`
- Create, update, and release operations for Context and Media resources
- Optional Media control callbacks for play/pause, previous, next, and seek commands
- Releasable translation bundles and current media/theme Host State snapshots
- `id`, `abi-version`, and single `entry` DLL fields in `plugin.yml`
- Packager support for Cargo `repository` and `[lib].name` metadata

Changed:

- **Breaking**: removed the 0.2 `PluginVTable`, `PluginType`, `PluginInstanceC`,
  `HostApiC`, `plugin_get_instance`, and `plugin_set_host_api` interfaces
- **Breaking**: removed the unfinished Theme and Shortcut interfaces
- Plugin lifecycle is now strictly `create -> shutdown -> destroy`; WinIsland unloads a DLL
  only after `shutdown` succeeds
- Context IDs are host-issued numeric resources instead of plugin-provided strings
- Plugin Media remains active independently of the SMTC setting and exposes only declared controls
- Plugin worker-thread resource changes wake the WinIsland event loop
- Development examples and packaging documentation now target Rust 2024 and ABI v1

Fixed:

- Descriptor size, ABI version, capability, metadata, and lifecycle callback validation
- Per-plugin resource ownership checks, count limits, and memory limits
- Media callback reentrancy and unload synchronization
- UTF-8-safe fixed-buffer truncation and bounded borrowed-slice copying
- Context update/release event coalescing and media seek source binding
- Bounded ZIP extraction with staging, Windows path-collision checks, transactional activation,
  backup, and explicit rollback errors
- Translation bundle cleanup and host wake coalescing during plugin shutdown

## 0.2.0 - Jun 19, 2026

Added:

- `TranslationPairC` — FFI-safe translation key-value pair for plugin i18n
- `HostApiC::register_translations` — plugin registers translations during `on_load`;
  later registrations override earlier ones for the same key
- i18n overlay layer — `tr()` checks plugin-registered translations before `.lang` files

Changed:

- **Breaking**: `HostApiC` gains a new required field `register_translations`;
  all host implementations must provide this callback
- `lib.rs` split into modular files: `host.rs`, `vtable.rs`, `types/mod.rs`,
  `types/{metadata,content,context,theme,shortcut,i18n}.rs`
- All public types re-exported from crate root — import paths unchanged for plugin authors

## 0.1.3 - Jun 19, 2026

Added:

- `MediaSourceC` — plugin-injectable media source (title, artist, album, duration, position, cover art)
- `HostApiC::set_media_source` — replace SMTC with plugin-provided media data
- `HostApiC::clear_media_source` — restore SMTC as the active media source

Changed:

- `HostApiC` derives `Clone`, `Copy` for safe FFI usage
- `PluginResultC` derives `Debug`, `Clone`, `Copy`
- `ContextDataC`, `ContextIdC`, `HostStateC` — new push-based context types
- `PluginVTable::set_host_api` — optional slot for plugin to receive `HostApiC` pointer

## 0.1.2 - Jun 17, 2026

Added:

- README.md with crate-level documentation, usage examples and feature flags

## 0.1.1 - Jun 16, 2026

Added:

- `packager` feature: `PluginPackager` for building, signing and zipping plugins
- Cargo.toml metadata for crates.io publishing (repository, homepage, license, keywords, categories)
- `docs.rs` configuration with `packager` feature enabled

Changed:

- Use `str_to_fixed` helper for byte-buffer initialization, replacing manual padding loops
- Packager validates `manifest.yaml` during `build()`; checks for missing fields and oversized buffers
- `github_link` field in `Manifest` is now required (non-empty) to satisfy host validation

Fixed:

- `plugin_get_instance` doc example uses proper `#[no_mangle]` export, no extraneous `fn main`
- Broken doc links in packager module docs
- `BG_CACHE` size check in signing flow

## 0.1.0 - Jun 15, 2026

Added:

- Initial release — C ABI types extracted from the WinIsland host into a standalone crate
- Core types: `PluginInstanceC`, `PluginVTable`, `PluginMetadataC`, `IslandContentC`, `ThemeColorsC`, `AnimationConfigC`, `ShortcutC`, `PluginResultC`
- `PluginType` enum with `from_u32` conversion
- `PluginGetInstanceFn` — entry-point signature for plugin DLLs
- `str_to_fixed` / `read_c_str` / `read_opt_c_str` helpers for FFI byte-buffer handling
- Priority constants: `PRIORITY_LOW`, `PRIORITY_MEDIUM`, `PRIORITY_HIGH`
- Content tag constants: `ISLAND_CONTENT_TAG_MUSIC`, `ISLAND_CONTENT_TAG_NOTIFICATION`, `ISLAND_CONTENT_TAG_STATUS`

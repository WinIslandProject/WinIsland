# winisland-plugin-api

ABI v2 types and SDK for trusted native WinIsland plugins. Plugins are in-process Windows DLLs. The ABI is defined in `src/abi`, `src/types/v2`, and `src/draw/v2.rs`; the normative protocol is [03-plugin-abi-v2.md](../../../.planning/refactor/03-plugin-abi-v2.md).

## Start a plugin

Create a Rust `cdylib` with `winisland-plugin-api` as a dependency. Export `winisland_plugin_entry_v2` returning a static `PluginDescriptorV2`. The descriptor declares capabilities and `create`, `shutdown`, and `destroy` callbacks. `on_tick` is optional and belongs to the plugin descriptor.

The [runnable demo](../../../examples/plugin-v2-demo/src/lib.rs) creates a 2×1 widget, submits draw lists from its tick worker, and registers a lyric transformer. The [SDK examples](examples) show a minimal widget, media source, lyric transformer, and settings page.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
winisland-plugin-api = "0.7"
```

The host passes a plugin token and an instance-owned `PluginHostV2` table to `create`. Query service tables through the SDK `Host` wrapper or through `PluginHostV2.query_interface`. The eleven interfaces cover context, media, translations, host state, widgets, lyrics, settings, text, images, store, and logging. Each resource belongs to the plugin token that created it.

## Lifecycle and drawing

The host calls `create`, then optional `on_tick` on a plugin worker. The plugin builds a complete ABI v2 draw list and submits it through the widget table. WinIsland validates the entire list before replaying it with the render crate; render callbacks do not enter plugin code. `shutdown` must stop and join plugin-owned threads before returning success. The host then calls `destroy` and unloads the DLL. If shutdown fails, the DLL remains loaded.

Borrowed strings and byte slices are valid only for the synchronous call that receives them. Callback data must remain valid until its resource is released and no callback is active. Service methods return `PluginStatus`; stale or foreign resource handles are rejected.

## Package

A package is a ZIP containing `plugin.yml` and the DLL named by its `entry` field. Set `abi-version: 2`. The optional `packager` feature provides `PluginPackager` for building, signing, and zipping a plugin:

```toml
[dev-dependencies]
winisland-plugin-api = { version = "0.7", features = ["packager"] }
```

```rust,no_run
winisland_plugin_api::packager::PluginPackager::from_cargo()
    .unwrap()
    .build()
    .unwrap();
```

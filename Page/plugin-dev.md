# Plugin development

WinIsland plugin API `0.6` publishes native ABI v1. A plugin is a Windows DLL loaded directly into the WinIsland process. It can publish compact contexts and configurable widgets, replace the displayed media source, transform parsed lyrics, register translations, and inspect the current host state.

> Plugins are trusted native code. There is no sandbox, process boundary, permission prompt, or crash isolation. Install and distribute plugins with the same care as desktop executables.

The old `0.2` `PluginVTable`, `PluginType`, `plugin_get_instance`, and `plugin_set_host_api` interfaces are not supported by ABI v1.

## Documentation map

| Guide | Use it for |
|---|---|
| [Quickstart](/plugin-dev/quickstart) | Create, build, and load a complete Context plugin |
| [ABI and lifecycle](/plugin-dev/abi-lifecycle) | Descriptor validation, capabilities, threads, FFI rules, shutdown, and 0.2 migration |
| [Host services](/plugin-dev/services) | Context, Media, lyrics transformation, i18n, Host State, resource limits, and callback behavior |
| [Packaging and installation](/plugin-dev/packaging) | `PluginPackager`, `plugin.yml`, ZIP validation, updates, rollback, and troubleshooting |
| [API changelog](/api-changelog) | Published API versions and breaking changes |

Start with the quickstart even if you intend to build a Media or i18n plugin. It establishes the entry descriptor and lifecycle contract that every plugin must implement.

## Runtime architecture

```text
plugin DLL exports winisland_plugin_entry_v1()
    -> PluginDescriptorV1
    -> WinIsland validates ABI, capabilities, metadata, and callbacks
    -> WinIsland issues PluginToken and calls create(PluginCreateInfoV1)
    -> plugin queries versioned HostApiV1 service tables
    -> plugin creates host-owned resources identified by ResourceId
    -> WinIsland calls shutdown(handle)
    -> WinIsland revokes remaining resources
    -> WinIsland calls destroy(handle) and unloads the DLL
```

The lifecycle is strictly `create -> shutdown -> destroy`. `shutdown` must synchronously stop every worker and join every thread that can execute plugin code. WinIsland does not call `destroy` or unload the DLL when `shutdown` reports an error.

## Choose capabilities deliberately

`PluginDescriptorV1.capabilities` is both a declaration and an authorization boundary. Declare only services the plugin uses.

| Capability | Service | Typical use |
|---|---|---|
| `CAPABILITY_CONTEXT` | `ContextApiV1` | Build status, timers, ongoing activities, compact text |
| `CAPABILITY_MEDIA` | `MediaApiV1` | A custom now-playing source and optional playback controls |
| `CAPABILITY_I18N` | `I18nApiV1` | Plugin-owned translation keys for supported languages |
| `CAPABILITY_HOST_STATE` | `HostStateApiV1` | Read the displayed media and current light/dark theme |
| `CAPABILITY_WIDGET` | `WidgetApiV1` | Render widgets managed by the Settings layout editor |
| `CAPABILITY_LYRICS_TRANSFORM` | `LyricsTransformApiV1` | Transform parsed lyric text while preserving timing |

Querying a service does not grant access by itself. Every resource call also carries the host-issued `PluginToken`, and the host rejects calls made without the declared capability.

## Ownership model

- WinIsland issues one nonzero `PluginToken` per loaded instance.
- Service create/register calls issue nonzero `ResourceId` values.
- A token can update or release only its own resources of the correct service type.
- Plugins should release resources during `shutdown`; WinIsland revokes leftovers after successful shutdown.
- Worker threads may call host services. Resource changes wake the WinIsland event loop.
- A DLL and its function pointers must remain valid until shutdown completes and all callbacks have returned.

## Development workflow

1. Define a `cdylib` crate and depend on `winisland-plugin-api = "0.6"`.
2. Export one `winisland_plugin_entry_v1` function returning a static descriptor.
3. Validate `PluginCreateInfoV1`, query declared services, and return an opaque instance handle.
4. Keep all host-issued resource IDs in plugin-owned state.
5. Stop workers and release resources in `shutdown`, then free only plugin memory in `destroy`.
6. Run `cargo check`, strict Clippy, and `cargo build --release`.
7. Package one entry DLL plus optional dependencies/assets and install the ZIP by dropping it onto WinIsland.

## Compatibility contract

Crate version `0.6.x` exposes ABI version `1`. Runtime compatibility is selected by `ABI_VERSION_1`, each structure's `struct_size`, and service table versions, not by Rust crate metadata at DLL load time.

All public ABI structures use `#[repr(C)]`. Plugins should initialize versioned structures with `Default` where available and must not assume fields beyond the advertised `struct_size` exist. A new incompatible ABI requires a new entry symbol and ABI version rather than changing ABI v1 in place.

## Where to look next

Build the [minimal plugin](/plugin-dev/quickstart), then read [ABI and lifecycle](/plugin-dev/abi-lifecycle) before adding worker threads or callbacks. The [service reference](/plugin-dev/services) documents exact limits and ownership rules, while [packaging and installation](/plugin-dev/packaging) covers distribution and update failures.

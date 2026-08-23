# winisland-plugin-api

Versioned C ABI types and packaging tools for trusted native WinIsland plugins.

Plugins are loaded as in-process Windows DLLs. They are not sandboxed: install only plugins you trust. ABI v1 is published by crate version `0.6` and does not support the old `0.2` vtable ABI.

## Project setup

```toml
[package]
name = "hello-winisland-plugin"
version = "0.1.0"
edition = "2024"
authors = ["Example Author"]
description = "Minimal WinIsland ABI v1 plugin"
repository = "https://github.com/example/hello-winisland-plugin"

[lib]
name = "hello_winisland_plugin"
crate-type = ["cdylib"]

[dependencies]
winisland-plugin-api = "0.6"
```

## Minimal context plugin

```rust
use std::ffi::c_void;
use winisland_plugin_api::*;

struct Instance {
    token: PluginToken,
    context_api: ContextApiV1,
    context_id: ResourceId,
}

static DESCRIPTOR: PluginDescriptorV1 = PluginDescriptorV1 {
    struct_size: std::mem::size_of::<PluginDescriptorV1>() as u32,
    abi_version: ABI_VERSION_1,
    capabilities: CAPABILITY_CONTEXT,
    metadata: PluginMetadataC::new(
        "hello-winisland-plugin",
        "hello-winisland-plugin",
        "0.1.0",
        "Example Author",
        "Minimal WinIsland ABI v1 plugin",
    ),
    create: Some(create),
    shutdown: Some(shutdown),
    destroy: Some(destroy),
};

unsafe extern "C" fn create(
    create_info: *const PluginCreateInfoV1,
    out_handle: *mut PluginHandle,
) -> PluginResultC {
    if create_info.is_null() || out_handle.is_null() {
        return PluginResultC::err("null create argument");
    }
    // SAFETY: WinIsland supplies a complete ABI v1 create-info structure.
    let info = unsafe { &*create_info };
    if info.struct_size < std::mem::size_of::<PluginCreateInfoV1>() as u32
        || info.abi_version != ABI_VERSION_1
        || info.host_api.is_null()
    {
        return PluginResultC::err("unsupported create info");
    }
    // SAFETY: The host API pointer remains valid for the process lifetime.
    let host = unsafe { &*info.host_api };
    // SAFETY: `host` was supplied by WinIsland and validated above.
    let Some(context_api) = (unsafe { host.context_api() }) else {
        return PluginResultC::err("context API is unavailable");
    };
    let Some(create_context) = context_api.create else {
        return PluginResultC::err("context create is unavailable");
    };

    let context = ContextDataV1 {
        title: str_to_fixed("Hello WinIsland"),
        body: str_to_fixed("ABI v1 plugin is running"),
        compact_text: str_to_fixed("Hello"),
        ..Default::default()
    };
    let mut context_id = INVALID_ID;
    // SAFETY: The input and output pointers remain valid for this call.
    let result = unsafe { create_context(info.plugin_token, &context, &mut context_id) };
    if result.status != 0 {
        return result;
    }

    let instance = Box::new(Instance {
        token: info.plugin_token,
        context_api,
        context_id,
    });
    // SAFETY: WinIsland owns this opaque handle until `destroy`.
    unsafe { out_handle.write(Box::into_raw(instance).cast::<c_void>()) };
    PluginResultC::ok()
}

unsafe extern "C" fn shutdown(handle: PluginHandle) -> PluginResultC {
    if handle.is_null() {
        return PluginResultC::ok();
    }
    // SAFETY: `handle` was created from `Box<Instance>` in `create`.
    let instance = unsafe { &mut *handle.cast::<Instance>() };
    if instance.context_id != INVALID_ID {
        if let Some(release) = instance.context_api.release {
            // SAFETY: This resource belongs to the same plugin token.
            let result = unsafe { release(instance.token, instance.context_id) };
            if result.status != 0 {
                return result;
            }
        }
        instance.context_id = INVALID_ID;
    }
    PluginResultC::ok()
}

unsafe extern "C" fn destroy(handle: PluginHandle) {
    if !handle.is_null() {
        // SAFETY: `destroy` is called once after successful shutdown.
        unsafe { drop(Box::from_raw(handle.cast::<Instance>())) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// WinIsland calls this function using the documented ABI v1 signature.
pub unsafe extern "C" fn winisland_plugin_entry_v1() -> *const PluginDescriptorV1 {
    &DESCRIPTOR
}
```

Lifecycle is always `create -> shutdown -> destroy`. `shutdown` must stop and join every thread that can execute plugin code or call a host service. WinIsland destroys the handle and unloads the DLL only after `shutdown` succeeds.

## Host services

Declare each service in `PluginDescriptorV1.capabilities`, then query it from `HostApiV1` during `create`:

| Capability | Query method | Resource operations |
|---|---|---|
| `CAPABILITY_CONTEXT` | `context_api()` | create, update, release |
| `CAPABILITY_MEDIA` | `media_api()` | create, update, release, UI command callback |
| `CAPABILITY_I18N` | `i18n_api()` | register and release translation bundles |
| `CAPABILITY_HOST_STATE` | `host_state_api()` | read the current media/theme snapshot |
| `CAPABILITY_WIDGET` | `widget_api()` | create, update, release, per-frame render callback |
| `CAPABILITY_LYRICS_TRANSFORM` | `lyrics_transform_api()` | register and release parsed lyric text transformers |

All created resources belong to the host-issued `PluginToken`. A plugin cannot update or release another plugin's resources. WinIsland automatically revokes remaining resources after successful shutdown.

Borrowed slices (`ByteSliceV1`, `Utf8SliceV1`) only need to remain valid until the host call returns. Callback data in Media, Widget, and Lyrics Transform resources must remain valid until the resource is successfully released. Media callbacks run synchronously on the WinIsland event-loop thread; lyric callbacks run on a lyrics-fetch worker. Release or unload returns an error while the corresponding callback is executing.

Every pointer passed across the ABI must be non-null when required, correctly aligned for its declared type, and readable or writable for the complete call. All public ABI structs use `#[repr(C)]` and start with `struct_size` where versioned extension is supported.

## Widget rendering

Declare `CAPABILITY_WIDGET`, create a `WidgetDataV1` with a render callback, and the host places
the widget on the expanded island's widget page grid. The host calls `on_draw` synchronously on
every rendered frame; inside the callback you draw through the host-provided `DrawApiV1` drawing operations —
no graphics library is linked into the plugin.

```rust
// In `create`, after querying `widget_api`:
let create_widget = widget_api.create.ok_or(())?;   // Option<fn> field, same as Media

let mut widget = WidgetDataV1::default();           // span 2x1
widget.key = str_to_fixed("status");
widget.span_cols = 2;
widget.span_rows = 2;
widget.on_draw = Some(on_draw);

let mut widget_id = INVALID_ID;
let result = unsafe { create_widget(info.plugin_token, &widget, &mut widget_id) };
if result.status != 0 { return result; }

// The render callback is invoked on every frame:
unsafe extern "C" fn on_draw(callback_data: *mut c_void, ctx: *const WidgetDrawContextV1) {
    // SAFETY: The context is host-provided and valid for this call.
    let ctx = unsafe { &*ctx };
    // SAFETY: The drawing operations originate from the host and are ABI-versioned.
    let Some(draw) = (unsafe { ctx.draw_api() }) else { return; };

    // SAFETY: All draw calls are synchronous and the context stays valid.
    unsafe {
        draw.draw_round_rect.unwrap()(ctx, 0.0, 0.0, ctx.width, ctx.height, 12.0, 0x28FFFFFF);
        draw.draw_text.unwrap()(ctx, 16.0, 20.0, Utf8SliceV1::borrowed("hello"), 18.0, 1, 0xFFFFFFFF);
    }
    let _ = callback_data;
}
```

Contract notes:

- `key` is the stable identity of this widget within the plugin. Use 1-63 ASCII letters, digits,
  `_`, or `-`, keep it unique within the plugin, and never change it in `update` or across plugin
  versions. A keyed widget appears in **Settings > Widgets**, where its real render callback is
  used for the library preview and users can place, move, or remove it. The saved layout combines
  the plugin ID and widget key, so runtime `ResourceId` values are never persisted.
- An empty `key` is accepted only for compatibility with plugins built before 0.5. Such a widget
  keeps the legacy automatic free-slot placement and cannot be managed in Settings.
- Coordinates are **logical** and relative to the widget slot's top-left corner; the host applies
  the island `scale` and `alpha` automatically. `ctx.width` / `ctx.height` are the logical slot
  footprint dimensions (span columns/rows plus gaps).
- `save` / `restore` / `translate` maintain a plugin-local transform stack; they never touch the
  host canvas state, and unbalanced calls are contained per frame.
- Colors are `0xAARRGGBB`. `draw_text`'s `y` is the text top (ascent line). `draw_image` takes
  non-premultiplied RGBA8 pixels and the host applies the context alpha to the whole image.
- The callback runs on the render thread: keep it short, do not block, and do not retain `ctx`
  after returning. Release the widget resource in `shutdown` as shown for Context above.

## Lyrics transformation

Declare `CAPABILITY_LYRICS_TRANSFORM`, query `lyrics_transform_api()`, and register one callback:

```rust
let transformer = LyricsTransformerDataV1 {
    on_transform: Some(transform_lyrics),
    callback_data: state_ptr,
    ..Default::default()
};
let mut transformer_id = INVALID_ID;
let result = unsafe {
    lyrics_api.register.unwrap()(info.plugin_token, &transformer, &mut transformer_id)
};
```

The callback uses a two-pass output protocol. A real Simplified-to-Traditional plugin can replace
the example conversion with OpenCC or another converter:

```rust
unsafe extern "C" fn transform_lyrics(
    _callback_data: *mut std::ffi::c_void,
    _resource_id: ResourceId,
    input: *const LyricsTextV1,
    output: *mut u8,
    output_capacity: u32,
    out_len: *mut u32,
) -> PluginResultC {
    if input.is_null() || out_len.is_null() {
        return PluginResultC::err("null lyrics transform argument");
    }
    let input = unsafe { &*input };
    let bytes = unsafe { std::slice::from_raw_parts(input.text.ptr, input.text.len as usize) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return PluginResultC::err("invalid lyrics UTF-8");
    };
    let transformed = text.replace('汉', "漢");
    let transformed = transformed.as_bytes();
    unsafe { out_len.write(transformed.len() as u32) };
    if output.is_null() && output_capacity == 0 {
        return PluginResultC::ok();
    }
    if output.is_null() || output_capacity < transformed.len() as u32 {
        return PluginResultC::err("lyrics output buffer is too small");
    }
    unsafe { std::ptr::copy_nonoverlapping(transformed.as_ptr(), output, transformed.len()) };
    PluginResultC::ok()
}
```

WinIsland invokes every registered transformer in registration order once per parsed line, after
fetching and before caching. Each callback is called first with a null output to query its required
byte length, then with an allocated output buffer. Output must be valid UTF-8 and no larger than
256 KiB per line. For `LYRICS_TEXT_FLAG_WORD_SYNCED`, the transformed text must keep the same
Unicode character count; the host then rebuilds byte boundaries so per-word highlighting keeps its
original timing. Release the transformer during `shutdown`; release and unload are rejected while
its callback is active.

## Packaging

Enable the optional packager:

```toml
[dev-dependencies]
winisland-plugin-api = { version = "0.6", features = ["packager"] }

[[example]]
name = "pack"
path = "package.rs"
```

```rust
fn main() {
    winisland_plugin_api::packager::PluginPackager::from_cargo()
        .unwrap()
        .build()
        .unwrap();
}
```

Run `cargo run --example pack`.

To publish through the WinIsland plugin marketplace, keep the complete source in a public GitHub repository with a detected SPDX license. Add this tag workflow:

```yaml
name: Release WinIsland plugin

on:
  push:
    tags: ["v*"]

permissions:
  contents: write
  id-token: write
  attestations: write

jobs:
  release:
    uses: WinIslandProject/PluginMarketplace/.github/workflows/build-plugin.yml@main
```

After the first release, submit one registration file to [WinIslandProject/PluginMarketplace](https://github.com/WinIslandProject/PluginMarketplace). Future versions are discovered from new valid GitHub Releases without another registration pull request. The complete format is documented in the marketplace [contribution guide](https://github.com/WinIslandProject/PluginMarketplace/blob/main/CONTRIBUTING.md).

`from_cargo()` reads package metadata, `repository`, and `[lib].name`. The package metadata must exactly match `PluginMetadataC`; use builder methods such as `.name(...)` or `.id(...)` when overriding it.

The generated `plugin.yml` identifies one entry DLL:

```yaml
id: hello-winisland-plugin
name: hello-winisland-plugin
author: Example Author
version: 0.1.0
description: Minimal WinIsland ABI v1 plugin
github-link: https://github.com/example/hello-winisland-plugin
abi-version: 1
entry: hello_winisland_plugin.dll
icon: icon.png
readme: README.md
```

`icon` and `readme` are optional safe relative paths used by **Settings > Plugins**. `PluginPackager::from_cargo()` automatically includes conventional root files (`icon.png`/`.jpg`/`.jpeg`/`.webp` and `README.md`/`.markdown`/`.txt`). Use `.icon("path")` or `.readme("path")` to select another file. The README is rendered as CommonMark/GFM in the plugin details panel; enable and disable changes take effect after WinIsland restarts.

Additional DLLs and asset directories may be included as dependencies, but WinIsland only loads `entry` as the plugin. The packager can write hashes and an Ed25519 signature; host-side signature verification is not implemented yet.

## Features

| Feature | Description |
|---|---|
| default | Core ABI v1 types with no extra dependencies |
| `packager` | Build, ZIP, hash, and optional Ed25519 signing tools |

See the [WinIsland plugin development guide](https://tanikaze.icu/WinIsland/) for all services and installation details.

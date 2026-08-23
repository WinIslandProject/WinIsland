# Plugin quickstart

This guide builds a complete ABI v1 DLL that publishes one persistent Context. The example validates every required input, keeps the host-issued resource ID, releases it during shutdown, and frees the opaque instance only in `destroy`.

## Prerequisites

- Windows 10 version 2004 or later, or Windows 11
- Stable Rust with the `x86_64-pc-windows-msvc` toolchain
- Visual Studio Build Tools with the Desktop development with C++ workload
- A WinIsland build that supports plugin API `0.6` / ABI v1

Check the toolchain:

```powershell
rustup show
rustc --version
cargo --version
```

## Create the project

```powershell
cargo new --lib hello-winisland-plugin
cd hello-winisland-plugin
```

Replace `Cargo.toml` with the relevant package, library, and dependency settings:

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

`cdylib` is required: it produces a native DLL with the exported ABI entry point. The package metadata will also be reused by the packager, so keep it aligned with `PluginMetadataC` below.

## Implement the plugin

Use this as `src/lib.rs`:

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

    // SAFETY: WinIsland supplies a readable ABI create-info prefix.
    let info = unsafe { &*create_info };
    if info.struct_size < std::mem::size_of::<PluginCreateInfoV1>() as u32
        || info.abi_version != ABI_VERSION_1
        || info.host_api.is_null()
        || info.plugin_token == INVALID_ID
    {
        return PluginResultC::err("unsupported create info");
    }

    // SAFETY: The validated host API pointer remains valid while WinIsland runs.
    let host = unsafe { &*info.host_api };
    // SAFETY: `host` originated from WinIsland and its ABI header was checked by the helper.
    let Some(context_api) = (unsafe { host.context_api() }) else {
        return PluginResultC::err("context API is unavailable");
    };
    let Some(create_context) = context_api.create else {
        return PluginResultC::err("context create is unavailable");
    };
    if context_api.release.is_none() {
        return PluginResultC::err("context release is unavailable");
    }

    let context = ContextDataV1 {
        priority: PRIORITY_MEDIUM,
        flags: CONTEXT_FLAG_SHOW_COMPACT,
        timeout_ms: 0,
        title: str_to_fixed("Hello WinIsland"),
        body: str_to_fixed("ABI v1 plugin is running"),
        compact_text: str_to_fixed("Hello"),
        ..Default::default()
    };
    let mut context_id = INVALID_ID;
    // SAFETY: Inputs and output remain valid until the synchronous call returns.
    let result = unsafe { create_context(info.plugin_token, &context, &mut context_id) };
    if result.status != 0 {
        return result;
    }

    let instance = Box::new(Instance {
        token: info.plugin_token,
        context_api,
        context_id,
    });
    // SAFETY: WinIsland treats this pointer as opaque until `destroy`.
    unsafe { out_handle.write(Box::into_raw(instance).cast::<c_void>()) };
    PluginResultC::ok()
}

unsafe extern "C" fn shutdown(handle: PluginHandle) -> PluginResultC {
    if handle.is_null() {
        return PluginResultC::ok();
    }

    // SAFETY: `handle` was created from `Box<Instance>` and has not been destroyed.
    let instance = unsafe { &mut *handle.cast::<Instance>() };
    if instance.context_id != INVALID_ID {
        let Some(release) = instance.context_api.release else {
            return PluginResultC::err("context release is unavailable");
        };
        // SAFETY: The resource belongs to this instance's host-issued token.
        let result = unsafe { release(instance.token, instance.context_id) };
        if result.status != 0 {
            return result;
        }
        instance.context_id = INVALID_ID;
    }
    PluginResultC::ok()
}

unsafe extern "C" fn destroy(handle: PluginHandle) {
    if !handle.is_null() {
        // SAFETY: WinIsland calls destroy once, after shutdown succeeds.
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

## Validate the DLL

Run checks before producing a release binary:

```powershell
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The DLL is written to:

```text
target/release/hello_winisland_plugin.dll
```

If no DLL is produced, confirm `[lib].crate-type = ["cdylib"]`. If the linker is missing, install or repair the Visual Studio MSVC build tools.

## Load it during development

For a quick local check, close WinIsland, place the DLL directly in WinIsland's plugin directory, and restart WinIsland. Root-level DLLs are treated as manually installed plugins. Check the WinIsland log for either:

```text
Loaded ABI v1 plugin: hello-winisland-plugin ...
```

or a validation error naming the DLL.

Manual root DLLs are useful during development, but distributable plugins should use the ZIP format. A packaged update cannot replace a manually installed root DLL automatically; remove the root DLL first.

## Make a change safely

Use `ContextApiV1::update` with the saved token and resource ID. Do not create a new Context for every refresh.

```rust
let updated = ContextDataV1 {
    title: str_to_fixed("Task complete"),
    body: str_to_fixed("The release build finished"),
    compact_text: str_to_fixed("Complete"),
    timeout_ms: 5_000,
    ..Default::default()
};

let result = unsafe {
    instance.context_api.update.unwrap()(
        instance.token,
        instance.context_id,
        &updated,
    )
};
```

An update refreshes ordering and starts a new timeout. A timed-out Context is hidden but remains owned by the plugin until release or successful plugin shutdown.

## Next steps

- Read [ABI and lifecycle](/plugin-dev/abi-lifecycle) before adding threads or storing callback pointers.
- Use [Host services](/plugin-dev/services) to add Media, lyric transforms, translation bundles, or Host State.
- Follow [Packaging and installation](/plugin-dev/packaging) to generate a validated ZIP.

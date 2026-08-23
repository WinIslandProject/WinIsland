# ABI and lifecycle

ABI v1 is a native, in-process contract. The host and plugin exchange only C-compatible values, opaque handles, copied service tables, and borrowed pointers with documented lifetimes. Correct shutdown is part of memory safety: unloading a DLL while one of its threads or callbacks is executing would jump into unmapped code.

## Entry point and descriptor

Every ABI v1 plugin exports exactly this symbol:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn winisland_plugin_entry_v1() -> *const PluginDescriptorV1 {
    &DESCRIPTOR
}
```

The returned descriptor must remain readable and unchanged for the complete DLL lifetime. A `static` descriptor is the normal implementation.

WinIsland rejects a descriptor when:

- the entry symbol is missing or returns null;
- `struct_size` is smaller than the ABI v1 descriptor prefix;
- `abi_version` is not `ABI_VERSION_1`;
- unknown capability bits are set;
- `create`, `shutdown`, or `destroy` is absent;
- the plugin ID is empty or contains characters outside `[a-zA-Z0-9_-]`;
- a package manifest is present and its ID, name, version, author, or description differs from the DLL descriptor.

Fields after a known `struct_size` prefix may be added in a compatible future revision. A plugin must never read fields beyond the size supplied by the other side.

## Capability negotiation

Set capability bits in `PluginDescriptorV1` before loading:

```rust
capabilities: CAPABILITY_CONTEXT | CAPABILITY_MEDIA,
```

During `create`, copy the service tables you need:

```rust
let host = unsafe { &*info.host_api };
let context = unsafe { host.context_api() };
let media = unsafe { host.media_api() };
```

The helpers validate the host ABI header, call `query_interface`, and validate the returned service table's size and version. They return `None` if any part is unavailable. Function slots are still optional, so validate each required slot before creating plugin state.

Declaring a capability does not require every function to be used. Failing to declare it causes corresponding host calls to return an error even if the table was queried successfully.

## Create contract

The host registers the plugin token before calling `create`, so host service calls are valid during initialization.

`create` must:

1. Reject null `create_info` and `out_handle` pointers.
2. Check the `PluginCreateInfoV1` prefix size and ABI version.
3. Reject null `host_api` and `INVALID_ID` tokens.
4. Query and validate every required service and function slot.
5. Construct all state needed by callbacks and worker threads.
6. Write one non-null opaque handle to `out_handle` and return `PluginResultC::ok()`.

An `Ok` result with a null handle is invalid and the plugin is rejected.

If initialization fails before a handle exists, leave `out_handle` null and return an error. If partial initialization requires cleanup, the plugin may write a cleanup handle and return an error. WinIsland then follows the normal `shutdown -> destroy` cleanup sequence for that handle. Do not publish a handle unless `shutdown` knows how to clean every initialized field.

`PluginResultC::err` copies an error message into a fixed UTF-8-safe buffer. Return actionable errors; they are included in the WinIsland log and installation failure message.

## Opaque instance handle

`PluginHandle` is `*mut c_void`. WinIsland stores it but never dereferences it. A common Rust representation is:

```rust
let instance = Box::new(Instance { /* ... */ });
unsafe { out_handle.write(Box::into_raw(instance).cast()) };
```

Borrow it without taking ownership during callbacks and shutdown:

```rust
let instance = unsafe { &mut *handle.cast::<Instance>() };
```

Take ownership exactly once in `destroy`:

```rust
unsafe { drop(Box::from_raw(handle.cast::<Instance>())) };
```

Never reconstruct a `Box` in `shutdown`; shutdown may return an error and be retried, and WinIsland still needs the handle for `destroy` after a later success.

## Shutdown contract

WinIsland marks the plugin as stopping before entering `shutdown`. New Media command dispatch is rejected, and unload is rejected while a Media or Lyrics Transform callback is in flight.

`shutdown` must perform work in this order:

1. Signal every plugin worker to stop.
2. Join every thread that can execute plugin code, invoke a plugin callback, or call a host service.
3. Prevent external callbacks from retaining plugin function or data pointers.
4. Release host resources using their original token and resource IDs.
5. Return success only when no plugin code can run asynchronously.

The function must be safe to retry if a previous shutdown returned an error. WinIsland keeps the DLL and handle loaded after failure and may attempt unload again later. Keep enough state to distinguish already-stopped workers and already-released resources.

Do not wait for a worker while holding a mutex that the worker needs to exit. A typical pattern is to take the join handle out of plugin state, release the state lock, then join.

If a host resource release fails, either retain its ID and return the error for a retry, or prove that continuing is safe. Do not mark an ID invalid before a successful release.

## Destroy contract

`destroy` is called once after shutdown succeeds. It has no result channel and should only release plugin-owned memory that remains in the opaque instance.

At this point:

- workers are joined;
- callbacks can no longer enter the plugin;
- host resources have been explicitly released or revoked by WinIsland;
- calling host services is unnecessary and should be avoided.

After `destroy` returns, WinIsland unloads the library. Every pointer to plugin code, static data, vtables, thread-local state, or callback data becomes invalid.

## Threading and callbacks

Host resource functions are synchronized and may be called from plugin worker threads. They copy borrowed inputs before returning and wake the WinIsland event loop when visible state changes.

Media command callbacks are different:

- WinIsland calls them synchronously on its event-loop thread.
- `callback_data` must remain valid until the Media resource is successfully released.
- A callback may call host services.
- Updating or releasing the same Media resource while its callback is active returns an error.
- A callback should enqueue work and return quickly; blocking it stalls WinIsland input and rendering.

Do not call plugin UI or framework code that assumes the callback runs on a worker thread. If the plugin needs asynchronous work, copy the command into a channel owned by a worker that shutdown can stop and join.

Lyrics Transform callbacks run synchronously on a lyrics-fetch worker, twice per parsed line: a
size query followed by the actual write. Their callback data must remain valid until release
succeeds. Keep conversion bounded, thread-safe, and deterministic between both calls. Release and
unload are rejected while a lyric callback is active.

## FFI data rules

- Every public ABI structure is `#[repr(C)]`.
- Initialize versioned input/output structures with `Default` when available.
- Required pointers must be non-null, correctly aligned, and readable or writable for the entire synchronous call.
- Fixed byte arrays are NUL-terminated UTF-8 fields; `str_to_fixed` truncates without splitting a UTF-8 code point.
- `ByteSliceV1` and `Utf8SliceV1` are borrowed `(ptr, len)` ranges, not NUL-terminated strings.
- Borrowed slices need to live only until the host function returns, unless a field explicitly states otherwise.
- Do not pass Rust references, `String`, `Vec`, trait objects, unwinding panics, or compiler-specific layouts across the ABI.
- Do not allow panic to unwind through an `extern "C"` boundary. Catch it inside the plugin or use an aborting panic strategy.

The trusted-plugin model cannot validate whether a non-null plugin pointer actually refers to enough accessible memory. Pointer validity remains the plugin's responsibility.

## Versioning strategy

There are three related versions:

| Value | Meaning |
|---|---|
| crate `0.6.x` | Rust package release containing ABI v1 definitions |
| `ABI_VERSION_1` | Top-level descriptor and create-info ABI |
| `INTERFACE_VERSION_1` | Version of an individual host service table |

A compatible service-table extension appends fields and increases `struct_size` without changing the v1 prefix. An incompatible layout or lifecycle change requires a new ABI number and entry symbol.

## Migrating from 0.2

ABI v1 is a rewrite, not an in-place upgrade.

| 0.2 pattern | ABI v1 replacement |
|---|---|
| `plugin_get_instance` | `winisland_plugin_entry_v1` returning `PluginDescriptorV1` |
| `PluginVTable` / `PluginInstanceC` | Descriptor lifecycle callbacks plus opaque `PluginHandle` |
| `plugin_set_host_api` / `HostApiC` | `PluginCreateInfoV1.host_api` and `query_interface` |
| `PluginType` providers | Explicit capability bitset |
| plugin-defined Context IDs | Host-issued `ResourceId` |
| global push/clear calls | Service-specific create, update, and release operations |
| unfinished Theme/Shortcut APIs | No ABI v1 equivalent |

Remove all old exports. A DLL should export only ABI v1 entry points and use 0.6 types throughout; mixing layouts is not supported.

## Review checklist

- Descriptor is static and all required lifecycle functions are present.
- Metadata matches `Cargo.toml` and packaged `plugin.yml` exactly.
- Every queried service has a matching capability bit.
- Every host result is checked before updating local ownership state.
- Callback data outlives its registered resource.
- Workers are signaled and joined before shutdown success.
- Shutdown can be retried without double-free or joining a thread twice.
- Destroy frees the instance once and does not run plugin work.
- No panic, Rust-owned layout, or unbounded pointer crosses the ABI.

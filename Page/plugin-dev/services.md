# Host services

ABI v1 exposes six versioned host services. Context, Media, i18n, Widget, and Lyrics Transform create resources owned by the plugin token. Host State returns a snapshot and creates no resource.

## Query and validate a service

Declare the capability in `PluginDescriptorV1`, query the table during `create`, then validate every function slot the plugin requires:

```rust
let host = unsafe { &*info.host_api };
let Some(context_api) = (unsafe { host.context_api() }) else {
    return PluginResultC::err("context API is unavailable");
};
let (Some(create), Some(update), Some(release)) = (
    context_api.create,
    context_api.update,
    context_api.release,
) else {
    return PluginResultC::err("context API is incomplete");
};
```

Service tables are copied values containing function pointers. Keep the copied table in plugin state. The function pointers remain valid while WinIsland runs, but they must not be called after plugin shutdown has completed.

Every fallible service function returns `PluginResultC`. Check `status` before storing, replacing, or discarding local ownership state:

```rust
let result = unsafe { update(token, resource_id, &data) };
if result.status != 0 {
    return result;
}
```

## Current host limits

Limits protect the WinIsland process from accidental unbounded resource use. They are host policy, not ABI constants, and may change in later WinIsland releases.

| Resource | Per-plugin limit | Data limit |
|---|---:|---:|
| Context | 64 active resources | Fixed fields: title 255, body 511, compact text 127 UTF-8 bytes plus NUL |
| Media | 4 active resources | Cover up to 16 MiB each; 32 MiB total cover data per plugin |
| Translation bundle | 16 active bundles | 1 MiB per bundle; 4 MiB total per plugin |
| Translation pairs | 4,096 per bundle | Key/value up to 64 KiB each; language code up to 64 bytes |
| Widget | 16 active resources | Span up to 6 columns by 3 rows; stable key up to 63 ASCII bytes |
| Lyrics transformer | 4 active resources | Transformed output up to 256 KiB per line |

An update is included in the same quota as the resource it replaces. Releasing a resource returns its count and memory budget.

## Context service

Context is for compact, glanceable plugin state such as a build result, timer, recording status, or ongoing task.

### Data model

```rust
let context = ContextDataV1 {
    priority: PRIORITY_HIGH,
    flags: CONTEXT_FLAG_SHOW_COMPACT,
    timeout_ms: 10_000,
    title: str_to_fixed("Deployment finished"),
    body: str_to_fixed("Production is healthy"),
    compact_text: str_to_fixed("Deployed"),
    ..Default::default()
};
```

Priorities are ordered `LOW < MEDIUM < HIGH`. WinIsland displays the highest-priority compact Context; equal-priority resources are ordered by their latest update time. Unknown priority values and flag bits are rejected.

`CONTEXT_FLAG_SHOW_COMPACT` makes the resource eligible for compact-island display. If `compact_text` is empty, the title is used. The body is rendered as secondary text when present.

`timeout_ms = 0` means no timeout. A nonzero timeout starts when the resource is created or updated. Expiry hides the Context but does not release it or return its quota; the plugin still owns the ID.

### Create, update, and release

```rust
let mut id = INVALID_ID;
let result = unsafe { context_api.create.unwrap()(token, &context, &mut id) };
if result.status != 0 {
    return result;
}

let updated = ContextDataV1 {
    title: str_to_fixed("Deployment verified"),
    compact_text: str_to_fixed("Healthy"),
    ..context
};
let result = unsafe { context_api.update.unwrap()(token, id, &updated) };

let result = unsafe { context_api.release.unwrap()(token, id) };
```

Keep the ID unchanged when update fails. Mark it `INVALID_ID` only after release succeeds. Creating and immediately releasing before WinIsland renders is supported; event coalescing prevents stale text from appearing.

### Context errors

Typical failures are an empty title, unknown priority/flags, a wrong token, an ID owned by another plugin or service, and the 64-resource limit.

## Media service

A plugin Media resource supplies the media information WinIsland actually displays. It is independent of the SMTC setting. The most recently created or updated plugin Media resource is active; releasing it selects the next most recent plugin resource, then falls back to SMTC when none remain.

### Display data

```rust
let cover = std::fs::read("cover.png").unwrap_or_default();
let media = MediaSourceDataV1 {
    flags: MEDIA_FLAG_PLAYING,
    duration_ms: 180_000,
    position_ms: 12_000,
    title: str_to_fixed("Plugin Track"),
    artist: str_to_fixed("Plugin Artist"),
    album: str_to_fixed("Plugin Album"),
    cover: ByteSliceV1::from_slice(&cover),
    ..Default::default()
};
```

The title is required. Artist and album may be empty. Cover bytes must contain a decodable PNG or JPEG for display; WinIsland copies them before returning. An empty slice clears the cover.

When `MEDIA_FLAG_PLAYING` is set, WinIsland advances the displayed position from `position_ms` using elapsed time. Send an update after seek, pause/resume, track changes, or authoritative position corrections. `duration_ms = 0` represents unknown duration.

### Optional controls

Controls are opt-in. Set only commands the plugin can handle and provide a callback whenever any control bit is nonzero:

```rust
media.available_controls = MEDIA_CONTROL_TOGGLE_PLAY
    | MEDIA_CONTROL_PREVIOUS
    | MEDIA_CONTROL_NEXT
    | MEDIA_CONTROL_SEEK;
media.on_command = Some(on_media_command);
media.callback_data = state_ptr;
```

```rust
unsafe extern "C" fn on_media_command(
    callback_data: *mut std::ffi::c_void,
    resource_id: ResourceId,
    command: *const MediaCommandV1,
) {
    if callback_data.is_null() || command.is_null() {
        return;
    }
    let command = unsafe { &*command };
    if command.struct_size < std::mem::size_of::<MediaCommandV1>() as u32 {
        return;
    }

    match command.command {
        MEDIA_COMMAND_TOGGLE_PLAY => { /* enqueue toggle */ }
        MEDIA_COMMAND_PREVIOUS => { /* enqueue previous */ }
        MEDIA_COMMAND_NEXT => { /* enqueue next */ }
        MEDIA_COMMAND_SEEK => {
            let target_ms = command.position_ms;
            // enqueue seek for `resource_id`
        }
        _ => {}
    }
}
```

WinIsland calls the callback synchronously on its event-loop thread. Keep it short and enqueue slow work. `callback_data` must stay valid until release succeeds. A seek drag remains bound to the Media resource that was active when dragging started, so use the callback's `resource_id` rather than assuming the plugin's newest resource.

Updating or releasing the same resource from inside its callback returns `media callback is in progress`. Other host service calls are allowed. Unload also waits until no Media callback is in flight.

### Media errors

Media calls reject an empty title, unknown flags or controls, controls without a callback, null cover data with nonzero length, covers larger than 16 MiB, total cover quota overflow, a wrong owner/type, and update/release during a callback.

## Lyrics Transform service

Lyrics Transform post-processes lyrics fetched by WinIsland. Declare
`CAPABILITY_LYRICS_TRANSFORM`, query `lyrics_transform_api()`, and register a callback resource:

```rust
let transformer = LyricsTransformerDataV1 {
    on_transform: Some(transform_lyrics),
    callback_data: state_ptr,
    ..Default::default()
};
let mut transformer_id = INVALID_ID;
let result = unsafe {
    lyrics_api.register.unwrap()(token, &transformer, &mut transformer_id)
};
```

WinIsland calls the transformer once per parsed line, in registration order, after a lyrics fetch
completes and before the result is cached. This makes a Simplified-to-Traditional converter a
small text-only plugin: pass the input line through OpenCC and return the converted UTF-8 text.

The callback uses two passes. On the first call, `output` is null and `output_capacity` is zero;
write the required byte length to `out_len`. On the second call, copy at most `output_capacity`
bytes to `output` and update `out_len` with the actual length. Return an error on invalid pointers,
conversion failure, or insufficient capacity.

`LyricsTextV1` includes `line_time_ms`, the borrowed UTF-8 line, and
`LYRICS_TEXT_FLAG_WORD_SYNCED`. Word-synchronised output must retain the input's Unicode character
count. WinIsland then maps the original per-word boundaries into the transformed UTF-8 bytes, so
highlight timing remains intact even when byte sequences change. A different character count is
rejected for that line; other lines continue through the transformer chain.

Callbacks run synchronously on a lyrics-fetch worker and may call other host services. Keep them
bounded: every line is limited to 256 KiB of output. `callback_data` must remain valid until release
succeeds. Release and plugin unload return an error while the callback is active. Release the
transformer during `shutdown`; newly registered transformers apply from the next lyrics fetch.

## Widget service

Widget resources render through WinIsland's `DrawApiV1`, so plugins do not link Skia or another
graphics library. Declare `CAPABILITY_WIDGET`, query `widget_api()`, and give every configurable
widget a stable key:

```rust
let widget = WidgetDataV1 {
    key: str_to_fixed("status"),
    span_cols: 2,
    span_rows: 1,
    title: str_to_fixed("Status"),
    on_draw: Some(draw_widget),
    ..Default::default()
};

let mut widget_id = INVALID_ID;
let result = unsafe { widget_api.create.unwrap()(token, &widget, &mut widget_id) };
```

The key is combined with the descriptor's plugin ID for saved layout identity. It must contain
1-63 ASCII letters, digits, `_`, or `-`, be unique within the plugin, and remain unchanged in
`update` and future plugin releases. Keyed widgets appear in **Settings > Widgets** with a live
preview from their real draw callback. Users can drag them into the island grid, rearrange them,
or remove them back to the widget library; the selected slot persists across restarts.

An empty key is accepted for compatibility with plugins built before API 0.5. Those widgets keep
legacy automatic placement in the first free slot and do not appear in the Settings library.

Implement the draw callback like this:

```rust
unsafe extern "C" fn draw_widget(
    callback_data: *mut std::ffi::c_void,
    ctx: *const WidgetDrawContextV1,
) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: WinIsland supplies the context and keeps it valid for this call.
    let ctx = unsafe { &*ctx };
    let Some(draw) = (unsafe { ctx.draw_api() }) else {
        return;
    };
    let (Some(round_rect), Some(text), Some(circle)) =
        (draw.draw_round_rect, draw.draw_text, draw.draw_circle)
    else {
        return;
    };
    let _ = callback_data;

    // Coordinates are logical; the host applies the island scale and alpha.
    unsafe { round_rect(ctx, 0.0, 0.0, ctx.width, ctx.height, 12.0, 0x28FFFFFF) };
    unsafe { text(ctx, 16.0, 16.0, Utf8SliceV1::borrowed("Ready"), 18.0, 1, 0xFFFFFFFF) };
    unsafe { circle(ctx, ctx.width - 14.0, 14.0, 4.0, 0xFF34C759) };

    // save/restore/translate keep a plugin-local transform stack.
    unsafe { draw.save.unwrap()(ctx) };
    unsafe { draw.translate.unwrap()(ctx, 8.0, 0.0) };
    unsafe { draw.draw_rect.unwrap()(ctx, 0.0, ctx.height - 3.0, 24.0, 2.0, 0x40FFFFFF) };
    unsafe { draw.restore.unwrap()(ctx) };
}
```

Colors are `0xAARRGGBB`, `draw_text`'s `y` is the text top (ascent line), and `draw_image`
takes non-premultiplied RGBA8 pixels with the host applying the context alpha. The full draw
surface is `draw_rect`, `draw_round_rect`, `draw_circle`, `draw_line`, `draw_arc`, `draw_text`,
`measure_text`, `draw_image`, and the `save` / `restore` / `translate` transform stack.

The draw callback runs synchronously on the render thread. Use logical coordinates relative to
the widget, keep the callback short, never retain the context, and release the resource during
shutdown. Widget calls reject invalid spans, unknown flags, missing callbacks, duplicate or
invalid keys, key changes during update, wrong ownership, and the per-plugin resource limit.

## Translation service

Translation bundles add plugin-owned keys to WinIsland's existing translation lookup. Register one bundle per language and retain every returned ID.

```rust
let pairs = [
    TranslationPairV1 {
        key: Utf8SliceV1::borrowed("hello.title"),
        value: Utf8SliceV1::borrowed("Hello"),
    },
    TranslationPairV1 {
        key: Utf8SliceV1::borrowed("hello.status"),
        value: Utf8SliceV1::borrowed("Running"),
    },
];

let mut bundle_id = INVALID_ID;
let result = unsafe {
    i18n_api.register_bundle.unwrap()(
        token,
        Utf8SliceV1::borrowed("en_us"),
        pairs.as_ptr(),
        pairs.len() as u32,
        &mut bundle_id,
    )
};
```

WinIsland copies the language, keys, and values during registration. The slices may be dropped after the call returns. Keys must be nonempty; values may be empty. Use unique, plugin-prefixed keys to avoid overriding another plugin or application string accidentally.

Built-in language codes currently include `en_us`, `zh_cn`, and `es_es`. A bundle affects lookup only while its language matches the selected WinIsland language. For the same key and language, the most recently registered active plugin bundle wins. Releasing it reveals an older bundle or the built-in translation again.

```rust
let result = unsafe { i18n_api.release_bundle.unwrap()(token, bundle_id) };
```

Release bundles in reverse registration order during shutdown. WinIsland also removes remaining bundles after successful plugin shutdown.

### Translation errors

Registration rejects an empty bundle, null pair array, more than 4,096 pairs, an empty language or key, invalid UTF-8, oversized strings, a bundle larger than 1 MiB, per-plugin count/total quota overflow, and missing capability.

## Host State service

Host State is a snapshot, not a subscription. Initialize the output structure so `struct_size` is present, then call `get`:

```rust
let mut state = HostStateV1::default();
let result = unsafe { host_state_api.get.unwrap()(token, &mut state) };
if result.status == 0 {
    let is_playing = state.is_playing != 0;
    let title = read_fixed(&state.media_title);
    let theme = read_fixed(&state.theme);
}
```

A plugin can use a local fixed-buffer reader:

```rust
fn read_fixed(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
```

The snapshot contains the media currently displayed by WinIsland, including active plugin Media, and the current theme string (`light` or `dark`). Media fields may be empty when nothing is displayed. Do not cache the result as an event stream; query again when the plugin needs a fresh value.

Host State validates the token, capability, output pointer, and output `struct_size`. It creates no `ResourceId` and requires no release.

## Resource cleanup pattern

Keep IDs in the instance and clear each only after successful release:

```rust
fn release_resource(
    id: &mut ResourceId,
    release: unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC,
    token: PluginToken,
) -> PluginResultC {
    if *id == INVALID_ID {
        return PluginResultC::ok();
    }
    let result = unsafe { release(token, *id) };
    if result.status == 0 {
        *id = INVALID_ID;
    }
    result
}
```

For multiple resources, stop callbacks/workers first, then release resources in reverse dependency order. If one release fails, preserve the unreleased IDs and return the error so shutdown can be retried.

## Diagnosing host errors

`PluginResultC::into_result()` converts a result to `Result<(), String>` on the Rust side. Useful error text includes:

| Error | Meaning |
|---|---|
| `invalid plugin token` | Token is zero, stale, or not issued to this loaded instance |
| `capability was not declared` | Descriptor omitted the service capability |
| `resource was not found` | ID was released, revoked during shutdown, or never existed |
| `resource is owned by another plugin` | Token or service type does not match the ID |
| `media callback is in progress` | Defer update/release until the callback returns |
| `... limit reached` / `... exceed ... limit` | Release existing resources or reduce copied data |

Log failures with the operation and resource ID, but do not log secrets or raw cover/translation payloads.

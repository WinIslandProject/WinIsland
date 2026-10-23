# WinIsland Architecture

## Overview

WinIsland is a Windows desktop application that creates a Dynamic Island overlay — a translucent, always-on-top island that displays media playback info, lyrics, and audio visualization. Built entirely in Rust with Skia rendering.

- **Window system**: `WindowSystem` and `PlatformEvent` isolate the application's window and event-loop code from winit; the Windows implementation uses winit + DirectComposition with a companion Windows Composition backdrop window
- **Rendering**: Skia Ganesh on D3D12, with premultiplied-alpha DXGI composition swap chains
- **Media integration**: Windows SMTC (System Media Transport Controls) via COM
- **Audio visualization**: cpal (loopback capture) + realfft (6-band spectrum)
- **Plugin system**: Native C ABI DLLs loaded via libloading
- **Language**: English & Chinese (i18n via custom .lang files)

---

## Directory structure

```
crates/
├── winisland-core/    Platform-independent domain layer (no Windows API, no Skia, no winit, no UI)
│   ├── config/        AppConfig, widget model, grid placement, version migration
│   ├── context/       Plugin context manager
│   ├── i18n/          Translation catalogue + plugin translation bundles
│   ├── lyrics/        Lyrics model, LRC parsing, matching, online providers
│   ├── anim.rs        Keyed animation pool
│   ├── physics.rs     Spring physics for smooth animations
│   ├── persistence.rs Config parse/migrate/atomic write (the config path is injected)
│   ├── plugin_settings.rs Plugin settings page model
│   └── widgets.rs     Plugin widget model (PluginWidget, WidgetManager)
├── winisland-plugin-api/  ABI v2 types, draw protocol, SDK, and optional packager
├── winisland-plugin-package/  Manifest, ZIP activation, signing, and marketplace catalog
├── winisland-plugin-host/  Per-instance service tables, loader, lifecycle, resource registry, draw validation and replay
├── winisland-render/      Rendering values, Painter, images, text, D3D12 targets, and frame lifecycle
├── winisland-platform/    OS-neutral capability traits, window/event contracts, and value types; no dependencies or unsafe
└── winisland-platform-windows/  Windows window/event loop, backdrop, shell, metrics, display, audio, media, notification, and input implementations

src/                 Application crate "WinIsland"; it depends on winisland-core, never the reverse
├── core/              Application-side scheduling and state
│   ├── audio.rs       FFT spectrum and capture scheduling through AudioProvider
│   ├── persistence.rs Config path adapter — resolves ~/.winisland/config.toml, forwards to winisland-core
│   └── smtc.rs        Media state, lyrics, selection, and polling through MediaProvider
├── icons/             Custom vector path icons (arrows, controls, music, settings)
├── plugin/inventory.rs Installed-plugin list, enable state, and file removal for settings UI
├── ui/island.rs       Main draw_island() composition and island views
├── ui/expanded/       Expanded island views
│   ├── music_view.rs  Music player page (album art, controls, progress)
│   └── widget_view.rs Widget/page view for additional content
├── utils/             Utilities
│   ├── animations.rs  Animation curve helpers
│   ├── backdrop.rs    Dynamic color background effects
│   ├── blur.rs        Motion blur sigma calculation
│   ├── color.rs       Theme color helpers
│   ├── cover.rs       Cover image decode through winisland-render
│   ├── mouse.rs       Geometry helpers using DisplayProvider
│   ├── scroll.rs      Scroll container helpers
│   ├── settings_ui/   Settings UI components drawn through Painter
│   └── updater.rs     Nightly release check + download
└── window/
    ├── app.rs         Main App state, input, frame scheduling, and orchestration
    ├── app/events.rs  AppHandler implementation consuming PlatformEvent
    ├── app/system.rs  Tray, plugin installation, and shell notifications
    ├── app/v2.rs      ABI v2 resource snapshots and prepared widget frames
    └── settings/      Separate settings window
```

`winisland-core` is a separate crate so that the domain layer cannot reach the Windows API, Skia or the window system. It declares no platform or rendering dependency, and the application injects the platform values it needs: the system locale through `i18n::set_system_locale_provider`, the CJK conversion through `lyrics::set_simplify_hook`, and the config path through `persistence::load_config_at` / `save_config_at`.


---

## Rendering pipeline

`src/main.rs` calls `platform::window().run(&mut app)`. The Windows platform owns the winit event loop, its proxy and message hook, and both application windows. Its adapter converts winit callbacks into `PlatformEvent` values for the application's `AppHandler` in [events.rs](src/window/app/events.rs):

```
Resumed → WindowSystem::create_overlay creates backdrop before the owned foreground window
           (transparent, topmost, skip-taskbar)
           → create a hardware D3D12 device and shared Skia DirectContext
           → create independent composition swap chains for the island and settings windows

on_about_to_wait() [display refresh rate while active, throttled while idle]:
  1. Enforce topmost position
  2. Handle tray events
  3. Check config changes on a timed interval
  4. Process pending plugin installs
  5. Update cursor hit-test & auto-hide state
  6. Update seeking, borders, lyrics transitions
  7. Compute spring targets, update all springs
  8. Request redraw if animating
  9. Return the earliest app/settings deadline; the platform applies winit WaitUntil or Wait

RedrawRequested → winisland_render::Renderer::frame() → ui::island::draw_island():
  1. Compute dt, motion blur sigmas
  2. Get current MediaInfo from SMTC
  3. Get spectrum from AudioProcessor
  4. Draw background (default, glass, or dynamic)
  5. Draw album art (rounded/cover fit)
  6. Draw lyrics with transitions
  7. Draw spectrum visualizer bars
  8. Draw progress bar
  9. Draw mini controls (play/pause/prev/next)
  10. Flush with Present access, submit on the shared D3D12 queue, and present through DXGI
```

The platform normalizes input, resize, DPI, theme, file drop, redraw, and lifecycle callbacks. The settings window handles DPI size requests during the same scale-change callback through `WindowSystem::request_inner_size`. The event-loop proxy coalesces repeated wake requests; the platform message hook reports DWM composition changes as `PlatformEvent::CompositionChanged`. Window operations use `WindowId` and platform-owned value types. `native_surface()` supplies the renderer with an HWND and an opaque keepalive for the window. On exit, the app releases the host backdrop before the renderer and then destroys the overlay window; `main.rs` flushes the logger after `run()` returns.

Each style draws its background differently:
- **glass**: Companion Windows Composition window with a clipped host-backdrop brush
- **dynamic**: Cached blurred album art rendered by the active Skia backend
- **default**: Solid black

D3D12 is the only rendering backend. `winisland-render` owns Skia, image handles, font caches,
the D3D12 device, and frame presentation. Plugin drawing reaches it only through validated ABI v2
draw lists replayed by `winisland-plugin-host`; plugin callbacks run on their worker threads.
Each frame starts with an unclipped transparent clear and
isolates the drawing callback's canvas state. Resizing waits for GPU work and releases back-buffer
references before calling ResizeBuffers. Renderer failures invalidate both windows' GPU caches
and recreate their targets together. The companion backdrop window remains independent.

---

## SMTC integration

[SMTC](src/core/smtc.rs) uses Windows `GlobalSystemMediaTransportControlsSessionManager`:

- Polls session properties every 300ms (title, artist, thumbnail, position, duration)
- On song change: triggers async lyrics fetch + thumbnail download
- Auto-allow list: known music apps are automatically registered
- Handles seek/play/pause/skip commands from the UI
- Periodically refreshes (every 30th poll ~9s) to catch new apps

---

## Plugin system

Plugins are trusted in-process DLLs loaded by `winisland-plugin-host` through
`libloading`. Each DLL exports `winisland_plugin_entry_v2()`, returning a
`PluginDescriptorV2` with metadata, capabilities, `create`, `shutdown`,
`destroy`, and optional `on_tick`. The descriptor receives a host-issued token
and an instance-owned `PluginHostV2` table. Its `query_interface` exposes eleven
capability-gated tables: Context, Media, I18n, HostState, Widget,
LyricsTransform, Settings, Text, Image, Store, and Log. The wire contract lives
in `winisland-plugin-api`; the host owns the registry, resource table, and
implementation. `winisland-plugin-api` has no default external dependencies.

`App` creates one `PluginHost` and loads enabled ABI v2 plugins on startup.
`src/plugin/inventory.rs` supplies the settings list and enable/uninstall file
operations. ZIP extraction, manifest validation, marketplace data, signing,
staging, and backup/rollback activation live in `winisland-plugin-package`.
Installation validates `abi-version: 2` and DLL descriptor metadata before
activation. V1 has no runtime compatibility path.

Plugin resources belong to their token. Host services validate capability,
ownership, generation, and quotas; shutdown revokes the token's resources.
The host's worker runs tick, media-command, host-state, settings-change, and
lyric-transform callbacks. Lyrics are transformed after fetch and retain word
timing boundaries only if the replacement has the same character count.
Context, media, settings, and widget snapshots feed the existing application
models. Album art is decoded and supplied through the Image service.

A widget worker submits a complete draw list. The host validates the entire
list, resolves owned images, and prepares immutable drawing commands before
rendering. `src/ui/expanded/widget_view.rs` and the settings preview replay
those commands with host-side clipping, scaling, and alpha. No plugin callback
runs on the render thread. Invalid lists are rejected; repeated malformed
widget frames can disable that widget. The widget's logical size comes from
the configured expanded grid; collapse animation scales the replay without
changing that size, so text layout remains stable in the settings preview.

Unload joins the host worker, calls plugin `shutdown`, then `destroy`, and only
then unloads the DLL. A failed shutdown, including cleanup of a partial
`create`, keeps the DLL and host service tables allocated until process exit so
remaining plugin threads can finish safely.
The plugin must join its own threads before reporting successful shutdown.
Release builds still use `panic = "abort"`: a panic in an `extern "C"` plugin callback can
terminate the process. The host writes an active-plugin marker before callbacks;
on the next start it disables plugins named by leftover markers and reports
them. The first process still exits on callback panic.

---

## Windows API usage

| Area | Owner |
|------|-------|
| SMTC sessions, timeline, commands, thumbnails | `crates/winisland-platform-windows/src/media/` |
| WASAPI process capture, CoreAudio volume and meter | `crates/winisland-platform-windows/src/audio/` |
| WNF, Event Log, WinRT toast listener | `crates/winisland-platform-windows/src/notify/` |
| Registry, locale, app activation, tray, process lock | `crates/winisland-platform-windows/src/shell/` |
| System metrics, WMI brightness, monitor and cursor queries, input hooks | `crates/winisland-platform-windows/src/{metrics,display,input}.rs` |
| Window ownership, styles, backdrop, input adaptation and event loop | `crates/winisland-platform-windows/src/{window/,backdrop.rs}` |
| D3D12, DXGI presentation and Skia | `crates/winisland-render/` |

`winisland-platform` carries only traits and value types, including `WindowSystem`, `AppHandler`, and `PlatformEvent`. The application adapter in `src/platform.rs` selects the Windows implementation. `winit` is confined to `winisland-platform-windows`; application code calls the platform contract. COM and WinRT initialization are owned by platform resources, which release their handles on drop. Every unsafe block needs a `// SAFETY:` explanation.

---

## Configuration

Stored as TOML at `~/.winisland/config.toml`:

- Window dimensions (compact/expanded)
- Visual style (default/glass/dynamic)
- Language (auto/en/zh)
- SMTC settings (auto-allow, lyric sources)
- Audio visualization (gate threshold)
- Auto-hide and auto-start behavior

---

## Build & test

```bash
# Development
cargo check                           # Fast type-checking
cargo clippy --workspace -- -D warnings  # Lint (warnings are errors)
cargo fmt --all                       # Format

# Release
cargo build --release                 # Production build (LTO, abort on panic)

# Test
cargo test                            # Run all tests
```

Build requirements: Windows SDK, LLVM/clang (via Visual Studio or `choco install llvm ninja`).

### Version

`[workspace.package] version` in the root `Cargo.toml` is the single source of truth; both `WinIsland` and `winisland-core` inherit it through `version.workspace = true`, so `core::config::APP_VERSION` still reports the application version. The `release.yml` workflow takes its version as a manual input, so that input has to be bumped together with the manifest. Read the current value from `Cargo.toml`; do not rely on a number written here.

---

## Where `unsafe` is allowed

`unsafe` exists only where the code owns a platform handle, crosses the C ABI, or drives the GPU. Everywhere else it is a defect, not a style choice.

| Crate | Allowed locations | Why |
|---|---|---|
| `winisland-platform-windows` | entire crate | Owns Win32, COM, WinRT, and winit handles |
| `winisland-render` | `src/backend/**` only | Owns the D3D12 device, swap chains, and Skia GPU contexts |
| `winisland-plugin-host` | `src/{abi,loader,services}/**`, `src/lifecycle*` | Implements the C ABI tables and the plugin loader |
| `winisland-plugin-api` | `src/{abi,sdk}/**` | Declares the ABI types and wraps them for plugin authors |
| `winisland-plugin-calendar` | `src/lib.rs` plugin entry only | Built-in plugin; its `extern "C"` entry points are the only FFI surface |
| `winisland-core` | none | Domain layer; must compile without any platform dependency |
| `winisland-platform` | none | Contracts and value types only; must stay free of `unsafe` |
| `src/**` (application) | none | Application code calls the platform contract instead |

Rules that apply everywhere:

1. Every `unsafe` block has a `// SAFETY:` comment on the line above it. The comment names the invariant and who upholds it. "This is safe" is not a comment.
2. Every `unsafe extern "C"` entry point validates its pointer arguments before dereferencing them and returns a `PluginStatus` instead of panicking.
3. New `unsafe` outside the table above requires an architecture decision, not a code review comment.

Two known violations are recorded rather than fixed: `src/window/settings/mod.rs` calls `EnumDisplayDevicesW` directly instead of going through `winisland-platform-windows`, and `winisland-plugin-calendar` keeps its `extern "C"` entry points in the crate root. Do not add more.

### Documentation rules

Documentation in this directory is read by agents as ground truth, so it must not contain facts that expire.

- Do not write version numbers, line counts, file counts, or line references. Point at the file instead: "see `Cargo.toml`", not "currently 1.3.9".
- Directory listings describe intent, not an exhaustive inventory. Treat the tree as authoritative when they disagree, and correct the listing in the same change that moves a file.
- When a rule names a file path, that path must exist. A rule that points at a deleted file is worse than no rule.

# WinIsland Architecture

## Overview

WinIsland is a Windows desktop application that creates a Dynamic Island overlay — a translucent, always-on-top island that displays media playback info, lyrics, and audio visualization. Built entirely in Rust with Skia for GPU-accelerated rendering.

- **Window system**: winit + DirectComposition
- **Rendering**: skia-safe on a shared D3D12 Ganesh context
- **Media integration**: Windows SMTC (System Media Transport Controls) via COM
- **Audio visualization**: cpal (loopback capture) + realfft (6-band spectrum)
- **Plugin system**: Native C ABI DLLs loaded via libloading
- **Language**: English & Chinese (i18n via custom .lang files)

---

## Directory structure

```
src/
├── core/              Core business logic
│   ├── audio.rs       Audio loopback capture + FFT spectrum
│   ├── config.rs      AppConfig struct and defaults
│   ├── i18n.rs        Translation system (key-value .lang files)
│   ├── lyrics.rs      Async lyrics fetcher (NetEase, lrclib, local .lrc)
│   ├── persistence.rs Config save/load (~/.winisland/config.toml)
│   ├── render.rs      Main draw_island() — all Skia rendering lives here
│   └── smtc.rs        SMTC session manager — polls media info, handles commands
├── icons/             Custom Skia path icons (arrows, controls, music, settings)
├── plugin/            Native plugin system
│   ├── loader.rs      NativePlugin — wraps DLL via libloading, C ABI vtable
│   ├── manager.rs     PluginManager — RwLock registry, discover/install/unload
│   ├── types.rs       Host-side Rust types mirroring C ABI structs
│   └── zip_loader.rs  Plugin package extraction + manifest validation
├── ui/expanded/       Expanded island views
│   ├── music_view.rs  Music player page (album art, controls, progress)
│   └── widget_view.rs Widget/page view for additional content
├── utils/             Utilities
│   ├── animations.rs  Animation curve helpers
│   ├── autostart.rs   Registry-based auto-start
│   ├── backdrop.rs    Dynamic color background effects
│   ├── blur.rs        Motion blur sigma calculation
│   ├── color.rs       Adaptive island border color from screen pixels
│   ├── font.rs        Font manager with caching
│   ├── glass.rs       Frosted glass effect (GDI capture + blur + dark overlay)
│   ├── mouse.rs       Global cursor position, hit-test, fullscreen detection
│   ├── physics.rs     Spring physics for smooth animations
│   ├── scroll.rs      Scroll container helpers
│   ├── settings_ui/   Skia-rendered settings UI components
│   ├── updater.rs     Nightly release check + download
│   └── win32.rs       Raw Win32 API wrappers (topmost, window styles, etc.)
└── window/
    ├── app.rs         Main App struct — event loop, state, input, orchestration
    ├── tray.rs        System tray icon + context menu
    └── settings/      Separate settings window
```

---

## Rendering pipeline

The application uses winit's `ApplicationHandler` and `WaitUntil` scheduling in [app.rs](src/window/app.rs):

```
resumed() → create window (transparent, topmost, skip-taskbar)
           → create D3D12 device, queue, and shared Skia DirectContext
           → create a DirectComposition swap chain and GPU surfaces

about_to_wait() [display refresh rate while active, throttled while idle]:
  1. Enforce topmost position
  2. Handle tray events
  3. Check config changes on a timed interval
  4. Process pending plugin installs
  5. Update cursor hit-test & auto-hide state
  6. Update seeking, borders, lyrics transitions
  7. Compute spring targets, update all springs
  8. Request redraw if animating
  9. Schedule the next deadline from animation, playback, interaction, or idle state

RedrawRequested → draw_island():
  1. Compute dt, motion blur sigmas
  2. Get current MediaInfo from SMTC
  3. Get spectrum from AudioProcessor
  4. Draw background (default, glass, dynamic, or mica)
  5. Draw album art (rounded/cover fit)
  6. Draw lyrics with transitions
  7. Draw spectrum visualizer bars
  8. Draw progress bar
  9. Draw mini controls (play/pause/prev/next)
  10. Flush the GPU surface → present the DirectComposition swap chain
```

Each style draws its background differently:
- **glass**: GDI screen capture → cached GPU surfaces → Skia blur → dark multiply blend
- **dynamic**: Cached GPU-blurred album art with animated movement
- **mica**: Downscaled desktop capture blurred on cached GPU surfaces
- **default**: Solid black

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

Plugins are trusted native DLLs loaded via `libloading` with versioned C ABI v1:

```
DLL exports: winisland_plugin_entry_v1() -> *const PluginDescriptorV1

PluginDescriptorV1:
  ABI version + struct size
  metadata: PluginMetadataC (id, name, version, author, description)
  capability bitset (Context, Media, I18n, HostState, Widget, LyricsTransform)
  create(create_info, out_handle) -> PluginResultC
  shutdown(handle) -> PluginResultC
  destroy(handle)

PluginCreateInfoV1:
  host-issued PluginToken
  HostApiV1 with query_interface()

Host services issue ResourceId values. Context, Media, translation, and Widget
resources are owned by PluginToken, validated on every operation, and revoked
after a successful shutdown. Plugins may call host services from worker threads;
resource changes wake the winit event loop. shutdown must stop and join all plugin
threads before the DLL can be destroyed and unloaded.

LyricsTransform resources register bounded UTF-8 line callbacks. The host runs
them once after lyrics are fetched and preserves word-synchronised timing byte
boundaries when the transformed Unicode character count is unchanged.

Widget rendering is synchronous and render-thread only: `draw_widget_page`
(src/ui/expanded/widget_view.rs) places plugin widgets into free grid slots and
invokes their `on_draw` callback on every frame. The plugin draws exclusively
through the host-provided `DrawApiV1` drawing operations (src/plugin/manager.rs) — logical
coordinates relative to the slot, host-applied scale/alpha, and a plugin-local
transform stack — so plugins never touch the host Skia canvas directly.
```

Plugin packages are `.zip` files with a YAML manifest, one declared entry DLL,
optional dependencies/assets, and optional signature metadata. Installation uses
bounded staging extraction and backup/rollback directory activation.

---

## Windows API usage

| Area | APIs |
|------|------|
| SMTC | `GlobalSystemMediaTransportControlsSessionManager` |
| COM | `CoInitializeEx`, `CoUninitialize` |
| Audio | `IMMDeviceEnumerator`, `IAudioMeterInformation` |
| Window | `SetWindowPos` (topmost), extended styles (WS_EX_TOOLWINDOW, WS_EX_NOACTIVATE, WS_EX_LAYERED, WS_EX_TRANSPARENT) |
| Rendering | D3D12, DXGI flip swap chains, DirectComposition, Skia Ganesh |
| GDI | `GetDC`, `CreateCompatibleDC`, `BitBlt`, `GetDIBits`, `StretchBlt` |
| DWM | `DwmEnableBlurBehindWindow` (deprecated), `DwmSetWindowAttribute` |
| IME | `ImmGetContext`, `ImmSetCompositionWindow` |
| Registry | Auto-start registration |
| Locale | `GetUserDefaultLocaleName` for language auto-detect |
| Shell | `SetCurrentProcessExplicitAppUserModelID` |

All calls are in `unsafe` blocks with detailed `// SAFETY:` comments.

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

# WinIsland Architecture

## Overview

WinIsland is a Windows desktop application that creates a Dynamic Island overlay — a translucent, always-on-top island that displays media playback info, lyrics, and audio visualization. Built entirely in Rust with Skia rendering.

- **Window system**: winit + DirectComposition, with a companion Windows Composition backdrop window
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
│   └── widgets.rs     Plugin widget model (PluginWidget, WidgetManager)
├── winisland-plugin-api/  Plugin C ABI types + optional packager
└── winisland-render/      Rendering values, Painter, images, text, D3D12 targets, and frame lifecycle

src/                 Application crate "WinIsland"; it depends on winisland-core, never the reverse
├── core/              Application-side logic that still owns platform calls
│   ├── audio.rs       Audio loopback capture + FFT spectrum
│   ├── persistence.rs Config path adapter — resolves ~/.winisland/config.toml, forwards to winisland-core
│   ├── plugin_settings.rs Plugin settings page model
│   └── smtc.rs        SMTC session manager — polls media info, handles commands
├── icons/             Custom vector path icons (arrows, controls, music, settings)
├── plugin/            Native plugin system
│   ├── loader.rs      NativePlugin — wraps DLL via libloading, C ABI vtable
│   ├── manager.rs     PluginManager — RwLock registry, discover/install/unload
│   ├── types.rs       Host-side Rust types mirroring C ABI structs
│   └── zip_loader.rs  Plugin package extraction + manifest validation
├── ui/island.rs       Main draw_island() composition and island views
├── ui/expanded/       Expanded island views
│   ├── music_view.rs  Music player page (album art, controls, progress)
│   └── widget_view.rs Widget/page view for additional content
├── utils/             Utilities
│   ├── animations.rs  Animation curve helpers
│   ├── autostart.rs   Registry-based auto-start
│   ├── backdrop.rs    Dynamic color background effects
│   ├── blur.rs        Motion blur sigma calculation
│   ├── cjk.rs         Traditional to simplified Chinese conversion (LCMapStringEx)
│   ├── color.rs       Adaptive island border color from screen pixels
│   ├── glass.rs       Frosted glass effect (GDI capture + blur + dark overlay)
│   ├── locale.rs      System locale lookup (GetUserDefaultLocaleName)
│   ├── mouse.rs       Global cursor position, hit-test, fullscreen detection
│   ├── scroll.rs      Scroll container helpers
│   ├── settings_ui/   Settings UI components drawn through Painter
│   ├── updater.rs     Nightly release check + download
│   └── win32.rs       Raw Win32 API wrappers (topmost, window styles, etc.)
└── window/
    ├── app.rs         Main App struct — event loop, state, input, orchestration
    ├── backdrop.rs    Shared Windows Composition host-backdrop window
    ├── tray.rs        System tray icon + context menu
    └── settings/      Separate settings window
```

`winisland-core` is a separate crate so that the domain layer cannot reach the Windows API, Skia or the window system. It declares no platform or rendering dependency, and the application injects the platform values it needs: the system locale through `i18n::set_system_locale_provider`, the CJK conversion through `lyrics::set_simplify_hook`, and the config path through `persistence::load_config_at` / `save_config_at`.


---

## Rendering pipeline

The application uses winit's `ApplicationHandler` and `WaitUntil` scheduling in [app.rs](src/window/app.rs):

```
resumed() → create foreground and backdrop windows (transparent, topmost, skip-taskbar)
           → create a hardware D3D12 device and shared Skia DirectContext
           → create independent composition swap chains for the island and settings windows

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

Each style draws its background differently:
- **glass**: Companion Windows Composition window with a clipped host-backdrop brush
- **dynamic**: Cached blurred album art rendered by the active Skia backend
- **default**: Solid black

D3D12 is the only rendering backend. `winisland-render` owns Skia, image handles, font caches,
the D3D12 device, and frame presentation. The plugin ABI v1 adapter retains a hidden Skia
re-export until its drawing bridge is replaced. Each frame starts with an unclipped transparent clear and
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
| Rendering | D3D12 + Skia Ganesh; DXGI + DirectComposition presentation |
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

### Version

`[workspace.package] version` in the root `Cargo.toml` is the single source of truth (currently 1.3.9); both `WinIsland` and `winisland-core` inherit it through `version.workspace = true`, so `core::config::APP_VERSION` still reports the application version. The `release.yml` workflow takes its version as a manual input, so that input has to be bumped together with the manifest.

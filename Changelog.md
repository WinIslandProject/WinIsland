# Changelog

### v1.3.2
- Added optional secondary lyrics in the compact island with adaptive two-line layout
- Improved several animations
- Fixed delayed or inconsistent playback progress after seeking
- Fixed known issues

### v1.3.1
- Added word-synced LRC lyrics with smooth per-character highlighting
- Added QQ Music as a lyrics source
- Added a compact CPU and memory usage widget
- Replaced the native Windows volume flyout with the Dynamic Island volume indicator when using volume keys
- Fixed known issues
- Fixed a crash issue when quickly switching songs(#130)
- Optimized performance and resource usage

### v1.3.0
- Fixed known bugs
- Improved the notification system
- Unified Nightly and Stable installer identities to prevent duplicate installations
- Improved the update system
- Added a plugin marketplace
- Added a widget system for the compact island
- Fixed compact lyrics being clipped when using larger fonts (#145)

### v1.2.9
- Fixed notification text and app icon display issues (#129)
- Optimised GPU performance utilisation (#136)
- Improved the settings interface and widget layout editor
- Added G3 continuous corners to the Dynamic Island
- Fixed some window issues
- Improved the lyrics system
- Hid the music page when no music is available
- Fixed excessive Windows notification database writes when notification display is enabled
- Fixed occasional GPU and memory usage spikes during music playback
- Fixed slight text jitter during island animations
- Fixed exaggerated lyric width rebound after restoring a hidden island
- Added a resource usage widget for CPU and RAM monitoring

### v1.2.8
- Fixed the Dynamic Island still auto-hiding in full-screen mode while a live activity (e.g. music) is playing (#125)
- Fixed the Dynamic Island position drifting when using MyDockFinder and other dock tools (#126)
- Fixed the progress bar stuttering back when seeking on the system media player (#127)
- Refactored the codebase for better maintainability

### v1.2.7
- Migrated from software rendering to D3D12 hardware rendering
- Fixed an issue where the application would still auto-hide in full-screen mode even when the auto-hide option was disabled 
- Fixed a lyrics display issue when looping a single track
- Added a feature to redirect to the source application when clicking on a notification
- Fixed positional alignment issues when using MyDockFinder

### v1.2.6
- Improve island hiding settings
- Improve glass rendering and interface spacing consistency
- Simplify appearance settings and choose the expansion direction from available screen space
- Optimize performance and resource usage
- Fix notification display compatibility

### v1.2.5
- Add an optional fully hidden mode
- Restrict audio visualization to the active media process when Windows supports process loopback, with system audio fallback
- Fix auto-hide reveal behavior for compact overlays

### v1.2.4
- Add optional notification display
- Package Stable and Nightly releases as installers
- Keep the island out of the taskbar

### v1.2.3
- Add Kugou as a lyrics source, including improved lookup for English songs
- Improve island dragging and upward hide interactions
- Improve settings navigation, numeric input, and option menus
- Refresh settings sidebar icons
- Keep the music page open when media is paused

### v1.2.2
- Add configurable time, calendar, and settings widgets
- Support drag-and-drop placement with a square snapping grid
- Select the default expanded page based on music playback state

### v1.2.1
- Add optional right-click long-press drag to reposition the island (#92)
- Enable window resizability to allow dynamic scale updates (#94)
- Allow music album cover to scale smoothly on play/pause click
- Allow island interaction when other apps are fullscreen (#95)
- Auto-rebuild audio stream on device change (#90)
- Use custom font for ASCII-only text (#89)
- Quote autostart path and sync on startup (#88, #89)
- Improve update checker reliability and beta version comparison (#89)

### v1.2.0
- Resolve stable update check failures
- Distinguish between Stable and Beta update channels in the update available dialog title
- Add manual update check button to settings UI

### v1.1.0
- Restore Mica background style
- Adjust glass style capture position to horizontal offsets
- Fix static frosted glass and Mica background updates with periodic redraws
- Rename Dynamic Color to Album Cover and replace dominant color background with a blurred, zoomed cover art fluid effect
- Fix play/pause button transition afterimages
- Fix lyric text vertical jitter during expand/collapse transitions


### v1.0.0
- Initial release

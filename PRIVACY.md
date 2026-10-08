# Privacy Policy

English | [简体中文](./PRIVACY-zh.md)

Effective date: October 8, 2026

This policy explains how WinIsland handles information. It applies to every WinIsland build published by the WinIslandProject, including GitHub releases, nightly builds, and packaged builds.

## Summary

- WinIsland runs entirely on your computer. It has no accounts, advertising, analytics, telemetry, or crash reporting service.
- The WinIsland project does not operate any server that receives your data, and we never sell or share your data.
- A few features connect to third-party services: update checks, online lyrics, and the plugin marketplace. Each one is described below and can be turned off or avoided.

## Information processed on your device

WinIsland reads the following information to display it on the island. It is processed in memory on your computer and is not sent to the WinIsland project.

| Information | Why it is used | How to control it |
| --- | --- | --- |
| Now-playing media (title, artist, album, artwork, playback state and position, and the app that is playing) | Show music controls, artwork, progress, and lyrics | **SMTC Control**, and the list of allowed media apps |
| Audio output of the media app and the system | Animate the audio visualizer while a media app is active. Audio is analyzed in memory and is never recorded, saved, or transmitted | **SMTC Control** |
| Windows notifications (app name, icon, title, and text) | Show incoming notifications on the island. Off by default | **Notification Display** |
| Volume keys (Volume Up, Volume Down, Mute) | Replace the system volume flyout. Only these three keys are acted on; every other keystroke is passed through untouched and never read or recorded | **System Volume and Brightness Controls** |
| Text you copy to the clipboard | Detect a copied link and ask whether to open it. Clipboard contents are not stored or transmitted | **Ask to Open Copied Links** |
| Whether a microphone or camera is in use | Show a privacy indicator. WinIsland never captures audio from the microphone or video from the camera | |
| CPU, memory, GPU, network, and disk usage | Resource usage widgets | Remove the widgets |
| Foreground window and full-screen state | Hide the island while full-screen apps are running | Auto-hide settings |

## Information stored on your device

| Location | Contents |
| --- | --- |
| `%USERPROFILE%\.winisland\config.toml` | Your settings, including any custom font path, local lyrics folder, and media app lists |
| `%USERPROFILE%\.winisland\logs\` | Diagnostic logs. They may contain track titles and artists, lyrics lookups, and error messages. Logs are never uploaded automatically. If you attach them to an issue, review them first |
| `%USERPROFILE%\.winisland\updates\` | Installers downloaded by the updater |
| `%APPDATA%\WinIsland\plugins\` | Installed plugins and their files |

Deleting these folders removes all data WinIsland has stored.

## Network connections

WinIsland only connects to the internet for the features below. Like any internet request, these connections reveal your IP address to the service being contacted. They do not include any identifier for you or your device.

### Update checks

When **Check for Updates** is on (default), WinIsland periodically asks GitHub (`api.github.com` and `github.com`) whether a newer release is available. An installer is downloaded only after you confirm the update, and its SHA-256 checksum is verified before it runs. GitHub's privacy statement applies to these requests.

### Online lyrics

When **Show Lyrics** is on and **Lyrics Mode** is set to **Online** (both default), WinIsland sends the title and artist of the current track, and for some providers its duration, to look up lyrics. The request goes to the selected provider, and to the others as fallbacks if it finds nothing:

- AMLL (`api.amll.dev`)
- LRCLIB (`lrclib.net`)
- NetEase Cloud Music (`music.163.com`)
- QQ Music (`y.qq.com`)
- Kugou (`kugou.com`)

These services are operated by third parties, and their own privacy policies apply. To stop these requests, set **Lyrics Mode** to **LRC Folder** or turn off **Show Lyrics**.

### Plugin marketplace

When you open the plugin marketplace or install a plugin from it, WinIsland downloads the catalog, icons, and plugin packages from GitHub (`WinIslandProject/PluginMarketplace` and the repositories it lists).

### Links you open

Links you choose to open, including copied links you confirm through **Ask to Open Copied Links**, open in your default browser.

## Plugins

Plugins are extensions that you install yourself. They run with the same permissions as WinIsland and may process data or connect to the internet on their own, for example to fetch weather. The WinIsland project does not control third-party plugins. Review a plugin and its author's privacy practices before installing it, and remove the plugin from the **Plugins** page to stop it.

## Your choices

Because your data stays on your computer, you remain in control of it. You can turn off the features above in Settings and delete stored data at any time by removing the folders listed in this policy. The WinIsland project has no copy of your data to access, correct, or delete.

## Children's privacy

WinIsland does not collect personal information from anyone, including children.

## Changes to this policy

When WinIsland's data handling changes, this file is updated and the effective date above is revised. The full history is available in this repository's commit log.

## Contact

Questions about this policy can be asked in [GitHub Issues](https://github.com/WinIslandProject/WinIsland/issues). Please report security or privacy vulnerabilities privately as described in the [Security Policy](SECURITY.md).

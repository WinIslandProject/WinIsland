# Packaging and installation

WinIsland distributes plugins as ZIP archives with one root-level `plugin.yml` and one declared entry DLL. The archive may contain dependency DLLs and assets, but WinIsland treats only `entry` as a plugin.

## Publish to the plugin marketplace

Marketplace plugins must be open source in a public GitHub repository with a GitHub-detected SPDX license. Add `.github/workflows/release.yml` to that repository:

```yaml
name: Release WinIsland plugin

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write
  id-token: write
  attestations: write

jobs:
  release:
    uses: WinIslandProject/PluginMarketplace/.github/workflows/build-plugin.yml@main
```

The reusable workflow runs check, strict Clippy, formatting verification and the official packager, then publishes a GitHub Release with build provenance. Publish by pushing a version tag such as `v1.0.0`.

After the first release succeeds, add one `plugins/<plugin-id>.toml` file in a pull request to [WinIslandProject/PluginMarketplace](https://github.com/WinIslandProject/PluginMarketplace):

```toml
schema = 1
id = "example-clock"
repository = "owner/example-clock"
asset = "*.winisland-plugin.zip"
categories = ["widget", "utility"]
min_winisland_version = "1.2.9"
```

The ID must match both the file name and `plugin.yml`. Later plugin updates need no marketplace pull request: publish a new valid GitHub Release and the catalog refresh will discover it. See the marketplace [contribution guide](https://github.com/WinIslandProject/PluginMarketplace/blob/main/CONTRIBUTING.md) for the complete review rules.

## Add the packager

Enable the optional `packager` feature as a development dependency:

```toml
[dev-dependencies]
winisland-plugin-api = { version = "0.6", features = ["packager"] }

[[example]]
name = "pack"
path = "package.rs"
```

Create `package.rs`:

```rust
use winisland_plugin_api::packager::PluginPackager;

fn main() {
    PluginPackager::from_cargo()
        .expect("read Cargo.toml")
        .build()
        .expect("build plugin package");
}
```

Run it from the plugin project root:

```powershell
cargo run --example pack
```

The packager performs these steps:

1. Runs `cargo build --release`.
2. Locates the built library using `[lib].name`, or the package name with `-` replaced by `_`.
3. Copies the entry DLL and requested extra directories into a temporary staging directory.
4. Computes DLL hashes.
5. Generates and validates `plugin.yml`.
6. Optionally signs the manifest payload.
7. Writes `target/<package-name>-<version>.zip` unless an output path was configured.

If the project root contains `icon.png`, `icon.jpg`, `icon.jpeg`, or `icon.webp`, the packager includes the first match as the plugin icon. It also includes the first matching `README.md`, `README.markdown`, or `README.txt`. Use `.icon("path")` and `.readme("path")` to override automatic discovery.

## Cargo metadata and descriptor matching

`PluginPackager::from_cargo()` reads:

| Cargo field | Manifest/descriptor field |
|---|---|
| `package.name` | Default `id` and `name` |
| `package.version` | `version` |
| First `package.authors` value | `author` |
| `package.description` | `description` |
| `package.repository` | `github-link` |
| `lib.name` | Entry DLL filename |

The generated manifest's `id`, `name`, `version`, `author`, and `description` must match `PluginMetadataC` exactly. WinIsland loads the DLL in staging and rejects the package before stopping an installed version when these values differ.

If human-readable name or ID should differ from Cargo defaults, configure both sides explicitly:

```rust
PluginPackager::from_cargo()
    .unwrap()
    .id("example-clock")
    .name("Example Clock")
    .author("Example Author")
    .description("Shows the current time in WinIsland")
    .build()
    .unwrap();
```

```rust
metadata: PluginMetadataC::new(
    "example-clock",
    "Example Clock",
    env!("CARGO_PKG_VERSION"),
    "Example Author",
    "Shows the current time in WinIsland",
),
```

Plugin IDs must be 1 to 63 ASCII bytes and match `[a-zA-Z0-9_-]+`. Treat the ID as stable after release because it identifies the installation directory and update target.

## Manifest format

A generated manifest looks like this:

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
dll_hashes:
  - 75f1cd58a8bbf6dd32a68415c13e4065a827b603ab70542032fdd722d98a4f4d
```

Required host fields:

| Field | Rule |
|---|---|
| `id` | Stable ASCII plugin ID matching the descriptor |
| `name` | Nonempty, at most 127 UTF-8 bytes, matching the descriptor |
| `author` | Nonempty, at most 127 UTF-8 bytes, matching the descriptor |
| `version` | Nonempty, at most 31 UTF-8 bytes, matching the descriptor |
| `description` | Nonempty, at most 255 UTF-8 bytes, matching the descriptor |
| `github-link` | Nonempty, at most 2,048 UTF-8 bytes |
| `abi-version` | Must be `1` |
| `entry` | One root-level `.dll` filename, at most 255 bytes |

Optional presentation fields:

| Field | Rule |
|---|---|
| `icon` | Safe relative `.png`, `.jpg`, `.jpeg`, or `.webp` path; displayed in the plugin list and details panel |
| `readme` | Safe relative `.md`, `.markdown`, or `.txt` path; displayed in the details panel, at most 1 MiB when read by the host |

Declared presentation files must exist in the archive. When either field is omitted, WinIsland also looks for the conventional root filenames listed above, so existing packages can add an icon or README without changing their manifest.

Packager metadata may also include `dll_hashes` and `signature`. The current WinIsland host deserializes the required fields but does not enforce hashes, signer identity, or Ed25519 signature verification. A signature therefore records build metadata; it is not currently an installation trust decision. Distributors must state this clearly.

## Include dependencies and assets

Add an extra directory with `include_dir`:

```rust
PluginPackager::from_cargo()
    .unwrap()
    .icon("branding/plugin-icon.png")
    .readme("docs/PLUGIN.md")
    .include_dir("assets")
    .include_dir("runtime")
    .build()
    .unwrap();
```

Included paths must be nonempty relative paths composed of normal path components. Absolute paths, `.` components, and `..` traversal are rejected. The directory is copied recursively with its relative path preserved.

Put dependency DLLs beside the entry DLL or in the location expected by the plugin's own loading logic. WinIsland calls `LoadLibrary` only for `entry`; dependency resolution remains a Windows loader/plugin responsibility.

Do not include signing keys, `.env` files, build credentials, debug databases containing sensitive paths, or unrelated build output.

## Override build inputs

The builder supports explicit paths when defaults do not fit:

```rust
PluginPackager::new("Example Clock")
    .id("example-clock")
    .version("1.0.0")
    .author("Example Author")
    .description("Shows the current time in WinIsland")
    .github_link("https://github.com/example/example-clock")
    .dll_name("example_clock")
    .dll_path("target/release/example_clock.dll")
    .output("target/example-clock-1.0.0.zip")
    .build()
    .unwrap();
```

`build()` still invokes `cargo build --release`. `dll_path` changes which output is copied; it does not skip compilation.

## Optional signing

The packager can load an Ed25519 PKCS#8 PEM key from a file or environment variable:

```rust
PluginPackager::from_cargo()
    .unwrap()
    .signing_key_env("WINISLAND_PLUGIN_SIGNING_KEY")
    .build()
    .unwrap();
```

or:

```rust
.signing_key_path("signing_key.pem")
```

Prefer an environment secret in CI. Never commit a private key. Key-loading builder methods currently log a warning and continue unsigned when loading fails, so inspect build output and the generated manifest when a signature is required by your release process.

The signature covers canonical JSON containing manifest fields and DLL hash values, excluding `signature` itself. Again, WinIsland does not currently verify it during installation.

## Archive validation limits

Before extraction, WinIsland validates the archive from the same open file handle used for extraction:

- at most 4,096 ZIP entries;
- at most 256 MiB uncompressed per entry;
- at most 512 MiB total uncompressed data;
- `plugin.yml` at the archive root and no larger than 1 MiB;
- the exact case-sensitive root `entry` named by the manifest;
- no symlinks, absolute paths, drive/ADS colons, empty components, `.` or `..` components;
- no path component longer than 255 bytes or ending in a dot/space;
- no Windows device names such as `CON`, `NUL`, `COM1`, or `LPT1`;
- no paths that collide after Windows-style separator and case normalization.

Actual bytes written are counted again during extraction. Validation failure removes the staging directory.

## Installation transaction

Open **Settings > Plugins** and drop the ZIP onto the installation area. Installation proceeds as follows:

1. A background thread validates and extracts the package into a hidden staging directory.
2. On the WinIsland event-loop thread, the staged entry DLL is loaded only for descriptor validation.
3. Manifest and descriptor metadata are compared before the installed plugin is touched.
4. The staged validation DLL is unloaded.
5. The old plugin, if loaded from the package destination, must complete shutdown.
6. The old directory is renamed to a hidden backup.
7. Staging is renamed atomically to the final `<plugin-dir>/<id>` directory.
8. The entry DLL is loaded again from its final path and initialized.
9. On success, the backup is removed. On failure, the new directory is removed/moved, the backup is restored, and the old plugin is reloaded.

The validation and final load are intentionally separate. On Windows, renaming the directory of a loaded DLL succeeds, but the module path retained by the loader still points to the old staging path. Loading the final instance from its destination ensures relative resources and delayed dependencies resolve against the installed location.

If a newly created plugin instance cannot stop after initialization fails, WinIsland keeps its DLL loaded rather than deleting code that may still be executing. The installation reports a non-stoppable instance error and does not pretend rollback completed.

After installation, WinIsland refreshes the installed-plugin list and asks for an application restart. Plugins can be enabled or disabled from either the list or details panel; that state is persisted by plugin ID and takes effect after restart. Disabled packaged plugins are skipped before their DLL is opened. A manually installed root DLL must still be opened far enough to read its descriptor and determine its ID.

Clicking a plugin opens its details panel with the icon, metadata, repository link, enable switch, and README. README files are parsed as CommonMark with GFM tables, task lists, and strikethrough enabled. Headings, paragraphs, emphasis, lists, block quotes, code, tables, separators, and web links are rendered by WinIsland. Remote images are not downloaded by the settings page; their alternative text and source link remain available.

## Manual DLL versus package install

WinIsland also discovers root-level `.dll` files in the plugin directory. These are manual installations without `plugin.yml`.

- Manual DLLs are useful for local development.
- A ZIP update cannot replace a loaded manual root DLL with the same plugin ID.
- Remove the manual DLL and restart WinIsland before installing the packaged version.
- Packaged plugins live under `<plugin-dir>/<id>/` and are updateable transactionally.

## Inspect the output

List archive contents:

```powershell
tar -tf target/hello-winisland-plugin-0.1.0.zip
```

Inspect the manifest without extracting:

```powershell
tar -xOf target/hello-winisland-plugin-0.1.0.zip plugin.yml
```

Expected minimum contents:

```text
hello_winisland_plugin.dll
plugin.yml
```

Build and lint the plugin itself before publishing:

```powershell
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
cargo run --example pack
```

Test installation, restart discovery, update from an older package, failed initialization rollback, all declared Media controls, and shutdown while workers are active.

## Troubleshooting

### `winisland_plugin_entry_v1` is missing

Confirm the exact function name, `extern "C"`, `#[unsafe(no_mangle)]`, and `cdylib` crate type. Inspect exports with Visual Studio tools if needed:

```powershell
dumpbin /exports target/release/hello_winisland_plugin.dll
```

### Descriptor and manifest metadata do not match

Compare all five matched fields: ID, name, version, author, and description. Cargo's first author value and `[lib].name` may differ from assumptions. Rebuild the DLL before rebuilding the ZIP.

### Entry DLL is missing

Check `[lib].name`, `.dll_name(...)`, `.dll_path(...)`, and the `entry` value. The entry must be at the archive root and its case must match exactly.

### Existing plugin cannot shut down

The update is stopped before directory replacement. Fix worker cancellation, callback lifetime, and retryable resource release in `shutdown`. WinIsland intentionally keeps the old DLL loaded when shutdown fails.

### New plugin cannot initialize

Read the full rollback message. It distinguishes initialization failure, failed-directory removal, backup restoration failure, and failure to reload the previous plugin. Fix the first reported plugin error before retrying.

### Package is signed but WinIsland gives no trust indication

This is expected in ABI v1: host-side signer trust and signature/hash enforcement are not implemented. Distribute through a trusted channel and publish independent checksums when authenticity matters.

## Release checklist

- Increment plugin version and update descriptor metadata together.
- Verify Cargo metadata and `PluginMetadataC` match exactly.
- Run check, strict Clippy, and release build.
- Build the package from a clean source checkout.
- Inspect ZIP contents and `plugin.yml`.
- Ensure no secrets or unrelated files are included.
- Test clean install, restart, update, rollback, and uninstall/shutdown behavior.
- Document requested capabilities and why the plugin needs them.
- State that the plugin executes in-process without sandboxing.
- Publish the ZIP and checksum through a controlled release channel.

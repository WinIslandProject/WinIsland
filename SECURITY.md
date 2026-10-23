# Security Policy

## Supported Versions

This project follows a rolling release model. Only the latest stable release line receives security updates. Older stable releases, Nightly, and Beta builds are not guaranteed to receive security updates.

| Version | Supported |
| ------- | ------------------ |
| Latest stable releases | :white_check_mark: |
| Older stable releases | :x: |
| Nightly / Beta | :warning: Reports welcome, but no guaranteed security updates |

## Reporting a Vulnerability

Report vulnerabilities privately through GitHub Security Advisories: Repository → Security → Report a vulnerability. There is no security email address.

Please include:

- Affected version
- OS and environment
- Reproduction steps
- Impact
- PoC, screenshots, or logs
- Whether you want public credit

What to expect:

- Acknowledgement within 3 business days
- An initial assessment within 7 business days
- If accepted, a fix released in a future stable version
- If declined, a written reason
- Coordinated public disclosure after the fix ships

Do not disclose unreported vulnerabilities in public issues.

A report is most useful when it names the affected crate or module. The trust boundaries described below define which behaviour counts as a vulnerability: anything that crosses one of them without the stated check is a defect, not a feature request.

### Scope

In scope:

- The plugin loader, ABI boundary, package extraction, and marketplace verification
- The updater and any network fetch that installs or executes code
- Memory safety defects in `unsafe` blocks and FFI entry points
- Path traversal, symlink, or zip handling in package extraction
- Unsafe deserialisation of configuration, manifests, or catalogues

Out of scope:

- A plugin that misbehaves while holding capabilities the user granted it. Native plugins are trusted code by design; see the trust boundary below.
- Denial of service through a locally modified configuration file
- Anything that requires an already-compromised user account or administrator rights
- Reports produced only by a fuzzer without a minimal reproduction

## Plugin Security Model

### Trust Boundary

WinIsland loads native plugins (DLLs) into the host process through C ABI v2 (`winisland-plugin-api`). Plugins run with the same privileges as the host and are not sandboxed at the OS level. A malicious or buggy plugin can execute arbitrary code, read and write user files, and make network requests.

The ABI boundary is a correctness boundary, not a security boundary. It prevents a plugin from corrupting host state by accident. It cannot stop a plugin that intends to.

### Plugin Distribution

There are two installation paths with different trust models, and they must not be confused.

**Marketplace.** WinIsland verifies an Ed25519 signature over the marketplace catalogue against a public key compiled into the binary. Each catalogue entry carries the package size and SHA-256, which the host checks after download, and the package metadata must match the signed catalogue entry. A package's own `signature` field is not consulted on this path; the catalogue signature is the trust anchor.

**Local ZIP.** Dragging a ZIP into the window or opening one from disk extracts and installs it after manifest and archive validation. This path does not verify a signature, does not verify `dll_hashes`, and does not check signer identity. Treat every locally installed plugin as code the user chose to run.

### Host API Capabilities

A plugin declares its capabilities in the descriptor returned by its ABI v2 entry point. The host masks them against `KNOWN_CAPABILITIES_V2` at load time and rejects unknown bits. Each capability gates a service table, and every call re-checks it:

```
CAP_CONTEXT       context data
CAP_MEDIA         current track metadata
CAP_I18N          translation strings
CAP_HOST_STATE    host playback state
CAP_WIDGET        widget rendering
CAP_LYRICS        lyric text transformation
CAP_SETTINGS      settings pages
CAP_TEXT          text measurement
CAP_IMAGE         image resources
CAP_STORE         plugin storage
CAP_INPUT         input events
CAP_COMMAND       media commands
CAP_SURFACE       interactive surfaces
CAP_EVENTS        host events
CAP_MEDIA_SESSION media session access
CAP_SYSTEM        system services
CAP_ACTIVITY      activity indicators
```

Capabilities are declared by the plugin, not approved by the user. There is no per-plugin permission prompt, and a plugin that requests a capability gets it. `CAP_HOST_STATE`, `CAP_SYSTEM`, and `CAP_MEDIA_SESSION` expose the most host state; review the source of a plugin before installing it outside the marketplace.

### Crash Isolation

A plugin callback that panics is not caught. Release builds use `panic = "abort"`, so a panic inside a plugin callback terminates the process. `catch_unwind` is used only inside the SDK wrapper that invokes a plugin's lyric transform callback, so a panic there returns an error instead of unwinding across the ABI.

What the host does provide is attribution and recovery:

- Before each callback the host writes a marker file naming the plugin, and removes it afterwards.
- If the process dies with a marker present, the next start disables every plugin named by a surviving marker, writes a recovery report, and tells the user.
- Disabled plugins stay disabled until the user re-enables them. There is no automatic blacklist beyond this.

### ABI Compatibility

Two version numbers are in play and they are not the same:

- The plugin descriptor declares an ABI version. The loader rejects anything other than `ABI_VERSION_2`.
- Every service table begins with a `TablePrefix` carrying `struct_size` and an interface version. `query_interface` returns null when a plugin asks for a version newer than the host provides, and both sides validate `struct_size` before reading a table.

ABI v1 is not supported. A v1 package is rejected at install time with a message telling the user to upgrade the plugin. Breaking changes require a new ABI version, never a silent semantic change to an existing table.

## Dependency and Supply Chain

- Dependencies are pinned through `Cargo.lock`, which is committed.
- There is currently no automated advisory scanning. Run `cargo audit` locally before a release, and treat a new dependency as a security-relevant change.
- Adding a dependency requires an architecture decision; see `.agents/ARCHITECTURE.md` and the agent rules in `AGENTS.md`.

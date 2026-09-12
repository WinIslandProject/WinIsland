# Security Policy

## Supported Versions

This project follows a rolling release model. Only the latest stable release line receives security updates. Older stable releases, Nightly, and Beta builds are not guaranteed to receive security updates.

| Version | Supported |
| ------- | ------------------ |
| Latest stable releases | :white_check_mark: |
| Older stable releases | :x: |
| Nightly / Beta | :warning: Reports welcome, but no guaranteed security updates |

## Reporting a Vulnerability

Please report vulnerabilities privately via:

- GitHub Security Advisories: Repository → Security → Report a vulnerability
- Email: nothing in there XD (maybe we will create email in future)

Please include:

- Affected version
- OS and environment
- Reproduction steps
- Impact
- PoC / screenshots / logs
- Whether you want public credit

We will:

- Acknowledge receipt within 1 weekend days (i don't think we can maybe, but we also will do it quickly if we can)
- Provide an initial assessment within 7 business days
- If accepted, fix it as soon as possible and release it in a future stable version
- If declined, explain why
- Coordinate public disclosure after a fix is released

Please do not disclose unreported vulnerabilities in public issues.

## Plugin Security Model

### Trust Boundary

WinIsland loads native plugins (DLLs) into the host process through a versioned C ABI (`winisland-plugin-api`). Plugins run with the same privileges as the host and are not sandboxed at the OS level. This means:

- A malicious or buggy plugin can execute arbitrary code, read/write user files, and make network requests.
- A plugin crash can take down the entire host process unless explicitly isolated.

### Plugin Distribution

- Plugins distributed through the official marketplace MUST be signed.
- The host verifies plugin signatures before loading.
- Unsigned plugins can only be loaded manually with an explicit user override.

### Host API Permissions

Plugins access host capabilities through `HostApiV1::query_interface`. The following interfaces are exposed:

| Interface | ID | Sensitivity |
|---|---|---|
| Context | 1 | Low — plugin context display |
| Media | 2 | Medium — current track metadata |
| I18N | 3 | Low — translation strings |
| Host State | 4 | High — requires explicit permission |
| Widget | 5 | Low — widget rendering |
| Lyrics Transform | 6 | Low — lyric text processing |

Plugins requiring `INTERFACE_HOST_STATE` MUST declare it in their manifest. The host prompts the user on first load.

### Crash Isolation

- Plugin entry points are wrapped in `catch_unwind`.
- A panicking plugin is disabled and reported, but does not crash the host.
- Repeated crashes result in automatic plugin blacklisting.

### ABI Compatibility

- Every interface has an explicit version.
- The host rejects plugins requesting an unsupported interface version.
- Breaking changes require a new interface ID, never a silent semantic change.

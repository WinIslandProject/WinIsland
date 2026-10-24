---
name: pr-check
description: Run the checks CI runs for a WinIsland pull request. Use after any code change, before committing, or when the user asks to build, lint, format, or verify the workspace.
license: GPL-3.0
metadata:
  project: WinIsland
---

Run these from the repository root. Report the real output; do not describe what the commands would print.

## 1. Format (seconds)

```bash
cargo fmt --all
```

CI runs `cargo fmt --all -- --check`. Write the formatted file first, then check, so a failure means a real problem rather than an unformatted edit.

## 2. Lint and type-check (one command)

```bash
cargo clippy --workspace -- -D warnings
```

Clippy type-checks the workspace, so this replaces `cargo check`; running both pays the compile cost twice. Fix every warning. Do not silence one with `#[allow(...)]`.

## 3. Release build (only when asked)

```bash
cargo build --release
```

CI runs a release build on every PR that changes Rust code, so run it locally before opening one. Skip it while iterating.

## Reading the result

- A warning is a failure. CI treats them as errors, so a local run that only warns still blocks the PR.
- Report the exit code. "It should compile" is not a result.
- Do not run `cargo test`. The project accepts no tests; the command compiles the workspace a second time in the test profile and runs nothing.

## Common failures

| Symptom | Cause |
|---|---|
| `link.exe not found` | Visual Studio build tools or LLVM/clang missing |
| Skia link error on a fresh clone | `skia-safe` downloads prebuilt binaries on first build; needs network |
| Slow first build | Skia and the D3D12 stack dominate it; later builds are incremental |
| `failed to select a version` | `Cargo.lock` out of sync; run `cargo update -p <crate>` rather than hand-editing |

`ninja` is not required. `skia-safe` 0.99 uses prebuilt binaries and no build script in this workspace invokes it.

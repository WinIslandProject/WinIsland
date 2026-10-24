# WinIsland — AI Agent Guide

WinIsland is a **Dynamic Island for Windows**, written in Rust. It replicates the iPhone Dynamic Island experience on Windows, integrating with System Media Transport Controls (SMTC) to display now-playing media, real-time lyrics, audio spectrum visualization, and more.

## Quick start

```bash
cargo fmt --all                          # Format
cargo clippy --workspace -- -D warnings  # Lint and type-check; replaces cargo check
cargo build --release                    # Production build, only when asked
```

Clippy type-checks the workspace, so running `cargo check` as well only pays the compile cost twice. `cargo test` is not useful here: the project accepts no tests.

## Where to look

Read in this order. Stop when you have what you need; do not read all of it for a small change.

| File | Read it when |
|------|--------------|
| [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) | You need to know which crate owns a concern, or where `unsafe` is allowed |
| [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md) | You are writing Rust and need naming, import, error, or rendering conventions |
| [CONTRIBUTING.md](CONTRIBUTING.md) | You are preparing a pull request |
| [.agents/skills/pr-check/SKILL.md](.agents/skills/pr-check/SKILL.md) | You changed code and need to verify it |
| [.agents/skills/commit-push/SKILL.md](.agents/skills/commit-push/SKILL.md) | The user asked you to commit |
| [.agents/skills/pr-write/SKILL.md](.agents/skills/pr-write/SKILL.md) | You are writing a PR description |

## Hard rules

These are absolute. Nothing else in this file overrides them.

1. **Never commit** unless the user explicitly asks in this turn.
2. **Never modify changelog files** (`Changelog.md`, `Changelog-zh.md`). Only maintainers update them.
3. **Never add tests** — no unit, integration, snapshot, UI, or `#[cfg(test)]` tests. The project rejects them.
4. **Never create README or documentation files** unless the user asks.
5. **Never add `unsafe`** outside the crates listed in [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md). If it is genuinely required there, put a `// SAFETY:` comment immediately above naming the invariant and who upholds it, and list it separately in your reply.
6. **Never add `#[allow(...)]` to silence a warning.** A warning is a signal to restructure. If suppression is genuinely required, explain it in the PR.
7. **Never add `unwrap()`, `expect()`, or `panic!()`** outside `main` startup paths.
8. **Never commit secrets.** Do not log keys, tokens, or credentials.
9. **Do not change capture exclusion casually.** `crates/winisland-platform-windows/src/capture.rs` sets `WDA_EXCLUDEFROMCAPTURE` so the island does not appear in its own backdrop; the magnifier fallback and the liquid glass style depend on that ordering. Read the module before touching it.
10. **Pull requests follow the template** in [`.github/pull_request_template.md`](.github/pull_request_template.md). An unfilled template gets the PR closed.

## Conventions

Match the code around you before writing new code. If the neighboring files disagree with any document here, the code wins and the document is stale.

- **Naming, imports, errors, rendering**: [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md).
- **Which crate owns a concern**: [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md).
- **Comments**: explain why, not what. No comment that restates the code.
- **Dependencies**: check `Cargo.toml` and neighboring imports before adding a crate. Adding one is an architecture decision; see Dependency changes below.
- **Files**: prefer editing an existing file over creating a new one.
- **Emoji**: none, unless the user asks.

## Change discipline

These rules exist because over-broad, unreviewed agent changes are the main source of structural debt in this repository. They apply to every task unless the user explicitly overrides a specific rule.

### Scope budget

- State the files and crates you intend to touch before editing. If the change spans more than one crate, wait for confirmation first.
- Keep one task within 5 files and 2 crates. If a change must cross more boundaries, stop and report why each boundary is required. Do not expand scope on your own.
- Do not perform opportunistic refactors, reformatting, import reordering, or rename passes. Change only what the task requires.

### No net addition without removal

- When replacing an implementation, delete the code it replaces. Do not keep the old path "just in case", as a fallback branch, or as a compatibility layer.
- Do not add functions, types, modules, config keys, or feature flags that have no call site.
- Before reporting completion, state what the change removed. If the answer is "nothing", re-examine the change.

### Never relax your own constraints

- Do not modify `AGENTS.md`, `.agents/**`, `.github/workflows/**`, or `CODEOWNERS` unless the task is explicitly about those files.
- Do not make checks pass by editing CI configuration, path filters, skip markers, or test selection. Fix the code instead.
- Do not add `[ci-skip]`, `[docs]`, or any equivalent marker to a PR title or commit message.

### Dependency changes are architectural decisions

- Do not add, remove, upgrade, or enable features on any dependency in `Cargo.toml` unless the user explicitly asks.
- If a dependency change is required, state the alternative considered, the binary size impact, and whether it reaches `winisland-plugin-api` (plugin authors compile against that crate).
- Do not edit `Cargo.lock` by hand; let cargo regenerate it.

### Verify before reporting

- Run `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` after any code change, and report the real result.
- Never claim a change compiles, passes, or works without running it. "Should be fine" is not verification.
- Never invent command output. Quote the terminal, not what you expected the terminal to print.
- For UI changes, describe how a person can observe the effect, because this project has no automated tests.

### No throwaway scaffolding

- Do not create temporary test directories, comparison harnesses, verification crates, or `#[cfg(test)]` modules. Hard rule 3 forbids tests; this also forbids the temporary equivalents.
- Do not leave `examples/`, debug binaries, or scratch files behind. If a file exists only to prove your change works, delete it before reporting.

### Commit messages

A commit message describes what the commit changes and why. Nothing else.

- No workflow position or task tracking: no `phase 1`, `P0`, `step 3`, `stage 2`, `batch 4`, `part 2`, or any localized equivalent.
- Do not number a series of commits as a plan. If a change needs several commits, name each by its content.
- The format, types, scopes, and body rules live in [.agents/skills/commit-push/SKILL.md](.agents/skills/commit-push/SKILL.md). Load that skill rather than reconstructing the format from memory.

### Reporting

- List the files you actually changed, using their real paths.
- State partial work as partial. Never present an unfinished task as complete.
- When a request conflicts with these rules, name the conflict and ask. Do not silently choose one side.
- Do not write numbers that expire into documentation: versions, line counts, file counts, or line references. Read them from `Cargo.toml` or the tree at the time of use.


# WinIsland — AI Agent Guide

WinIsland is a **Dynamic Island for Windows**, written in Rust. It replicates the iPhone Dynamic Island experience on Windows, integrating with System Media Transport Controls (SMTC) to display now-playing media, real-time lyrics, audio spectrum visualization, and more.

You are working in a codebase you did not design. Read before you write.

## Quick start

```bash
cargo fmt --all                          # Format
cargo clippy --workspace -- -D warnings  # Lint and type-check; replaces cargo check
cargo build --release                    # Production build, only when asked
```

Clippy type-checks the workspace, so also running `cargo check` pays the compile cost twice. `cargo test` compiles the workspace in the test profile and runs nothing: this project accepts no tests.

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

1. **Never commit, tag, push, or open a PR** unless the user explicitly asks for it in this turn.
2. **Never modify changelog files** (`Changelog.md`, `Changelog-zh.md`). Only maintainers update them.
3. **Never add tests** — no unit, integration, snapshot, UI, or `#[cfg(test)]` tests. The project rejects them.
4. **Never create README or documentation files** unless the user asks.
5. **Never relax a rule to make a check pass.** Do not edit this guide, `.agents/**`, `.github/workflows/**`, or `CODEOWNERS` unless the task is explicitly about those files; do not change CI configuration, path filters, skip markers, or test selection to get green. Fix the code, or report that you cannot.
6. **Never silence a diagnostic.** No `#[allow(...)]`, no suppression pragma, and no catch-all arm that exists because handling the case was harder. `let _ =` on a fire-and-forget call is the project convention and is fine; discarding a result that can fail is not. Do not truncate with `as`: use `TryFrom` or a bounds check when the value may not fit. A warning is a signal to restructure; if suppression is genuinely required, explain it in the PR.
7. **Never add `unwrap()`, `expect()`, or `panic!()`** outside `main` startup paths.
8. **Never add `unsafe`** outside the crates listed in [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md). If it is genuinely required there, put a `// SAFETY:` comment immediately above naming the invariant and who upholds it, and list it separately in your reply.
9. **Do not change capture exclusion casually.** `crates/winisland-platform-windows/src/capture.rs` sets `WDA_EXCLUDEFROMCAPTURE` so the island does not appear in its own backdrop; the magnifier fallback and the liquid glass style depend on that ordering. Read the module before touching it.
10. **Never commit, log, or paste secrets**, keys, tokens, or credentials.
11. **Pull requests follow the template** in [`.github/pull_request_template.md`](.github/pull_request_template.md). An unfilled template gets the PR closed.
12. **Do not write facts that expire** — versions, line numbers, file counts, dates — into tracked documentation. Point at the source instead.

## Conventions

Match the code around you before writing new code. When a document and the code disagree, the code is right; name the stale document.

- **Naming, imports, errors, rendering**: [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md).
- **Which crate owns a concern, where `unsafe` is allowed**: [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md).
- **Comments**: explain why, not what. No comment that restates the code.
- **Dependencies**: check `Cargo.toml` and neighbouring imports before adding a crate. Adding one is an architecture decision; see [Dependency changes](#dependency-changes-are-architectural-decisions).
- **Files**: prefer editing an existing file over creating a new one.
- **Emoji**: none, unless the user asks.

## Before you edit

- Read the file you are changing and at least one neighbour. Match them, not your habits.
- State the files and crates you intend to touch, and why, before editing. A change that needs more than 5 files or 2 crates: stop and ask.
- If the request is ambiguous, ask one question rather than guessing and building the wrong thing.

## While you edit

These rules exist because over-broad, unreviewed agent changes are the main source of structural debt in this repository. They apply to every task unless the user explicitly overrides a specific rule.

### Scope budget

- Keep one task within 5 files and 2 crates. If a change must cross that boundary, or spread across unrelated modules, stop and report why each boundary is required. Do not expand scope on your own.
- Change what the task needs. No drive-by reformatting, renaming, import sorting, comment rewriting, or opportunistic refactors.
- Delete what you replace. No fallback path, compatibility shim, dead flag, or unused config key kept "just in case".
- No new abstraction before a third call site. No function, type, module, config key, or feature flag without a call site.
- Before reporting completion, state what the change removed. If the answer is "nothing", re-examine the change.

### Dependency changes are architectural decisions

- Do not add, remove, upgrade, or enable features on any dependency in `Cargo.toml` unless the user explicitly asks.
- If a dependency change is required, state what it replaces, its build and runtime cost, whether it reaches `winisland-plugin-api` (plugin authors compile against that crate), and why the standard library or an existing dependency is not enough.
- Do not edit `Cargo.lock` by hand; let cargo regenerate it.

### No throwaway scaffolding

- Do not create temporary test directories, comparison harnesses, verification crates, or `#[cfg(test)]` modules. The no-tests rule also forbids the temporary equivalents.
- Do not leave `examples/`, debug binaries, or scratch files behind. If a file exists only to prove your change works, delete it before reporting.

## Before you report

### Verify

- Run `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` after any code change, and report the real result, including failures and the exit code.
- Never claim a change compiles, passes, or works without running it. "Should be fine" is not verification.
- Never invent command output. Quote the terminal, not what you expected the terminal to print.
- Unverified is not done. If part of the change cannot be checked, name that part.
- For UI changes, describe how a person can observe the effect, because this project has no automated tests.

### Report

- List the files you actually changed, using real paths.
- State partial work as partial. Never present an unfinished task as complete.
- When a request conflicts with these rules, name the conflict and ask. Do not silently choose one side.

### Commits

A commit message describes what the commit changes and why. Nothing else.

- Format, types, scopes, and body rules live in [.agents/skills/commit-push/SKILL.md](.agents/skills/commit-push/SKILL.md). Load that skill rather than reconstructing the format from memory.
- Subject: imperative, lowercase, no trailing period, 72 characters or fewer.
- No workflow position or task tracking: no `phase 1`, `P0`, `step 3`, `stage 2`, `batch 4`, `part 2`, or any localized equivalent.
- No `[ci-skip]`, `[docs]`, or equivalent marker in a commit message or PR title.
- Never `git add -A` when the tree spans unrelated changes; stage related files together.

## Project-specific

- **Verify with**: `cargo fmt --all`, then `cargo clippy --workspace -- -D warnings`. `cargo build --release` only when asked. CI parity and failure modes: [.agents/skills/pr-check/SKILL.md](.agents/skills/pr-check/SKILL.md).
- **Ownership boundaries**: `winisland-core` holds the domain layer and never depends on a platform crate; `winisland-platform` holds OS-neutral contracts with no `unsafe`; `winisland-platform-windows` owns the Win32 surface; `winisland-render` owns the GPU; `src/` is the application and depends on `winisland-core`, never the reverse. The full table is in [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md).
- **Do not touch**: `Changelog.md`, `Changelog-zh.md`, `Cargo.lock` (by hand), and CI or agent configuration unless the task is explicitly about them.
- **Conventions live in**: [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md). The code around you wins over any document.

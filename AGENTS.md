# WinIsland — AI Agent Guide

WinIsland is a **Dynamic Island for Windows**, written in Rust. It replicates the iPhone Dynamic Island experience on Windows, integrating with System Media Transport Controls (SMTC) to display now-playing media, real-time lyrics, audio spectrum visualization, and more.

## Quick start

```bash
cargo check          # Fast type-check
cargo clippy --workspace -- -D warnings   # Lint (warnings as errors)
cargo fmt --all      # Format all files
cargo build --release   # Production build
```

## Key files to read

| File | What it contains |
|------|-----------------|
| [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) | Project structure, module responsibilities, rendering pipeline, plugin system, Windows API usage |
| [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md) | Coding conventions, naming, unsafe safety comments, Skia patterns |
| [.agents/skills/pr-check/SKILL.md](.agents/skills/pr-check/SKILL.md) | Build and lint automation for AI agents |
| [.agents/skills/commit-push/SKILL.md](.agents/skills/commit-push/SKILL.md) | Commit message generation for AI agents |

## Agent behavior rules

1. **Never commit changes** unless the user explicitly asks you to.
2. **Never create README or documentation files** unless the user asks.
3. **Do not use emoji** unless the user explicitly requests it.
4. **Always look at existing code conventions** before writing new code — mimic imports, patterns, and styles from neighboring files.
5. **Check dependencies exist** before using a library — look at Cargo.toml or neighboring imports first.
6. **Follow security best practices** — never log or commit secrets/keys.
7. **No unnecessary comments** — do NOT add comments in code unless asked.
8. Prefer editing existing files over creating new ones.
9. **WDA_EXCLUDEFROMCAPTURE is intentionally NOT set** — see glass.rs doc comments for rationale.
10. **Do not modify changelog files** (`Changelog.md`, `Changelog-zh.md`) — only project maintainers can modify them, unless explicitly requested by the user.
11. **Do not add tests** — this project does not accept unit, integration, snapshot, or UI tests.
12. **Pull requests must follow the PR format** — see [`.github/pull_request_template.md`](.github/pull_request_template.md) for the required template and guidelines.

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

### Suppressions require justification

- Do not add `#[allow(...)]` to silence a clippy warning. A warning is a signal to restructure, not to suppress. If suppression is genuinely required, put the reason on the same line and call it out separately in your reply.
- Do not add `unsafe`. If it is genuinely required: keep it inside a crate allowed by [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md), place a `// SAFETY:` comment immediately above explaining which invariant is upheld and by whom, and list it separately in your reply.
- Do not add `unwrap()`, `expect()`, or `panic!()` outside `main` startup paths.

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

- Do not create temporary test directories, comparison harnesses, verification crates, or `#[cfg(test)]` modules. Rule 11 already forbids tests; this also forbids the temporary equivalents.
- Do not leave `examples/`, debug binaries, or scratch files behind. If a file exists only to prove your change works, delete it before reporting.

### Commit messages

- A commit message describes what the commit changes and why. Nothing else.
- Do not encode workflow position, planning state, or task tracking in a commit message. No `phase 1`, `phase-2`, `P0`, `P1`, `step 3`, `stage 2`, `batch 4`, `part 2`, or any localized equivalent, in the subject or the body.
- Do not number a series of commits as a plan. If a change needs multiple commits, name each by its content.
- Follow the Conventional Commits format from [`.agents/skills/commit-push/SKILL.md`](.agents/skills/commit-push/SKILL.md): `<type>(<scope>): <description>`.
- Subject: imperative, lowercase, no trailing period, 72 characters or fewer.
- Body: explain why the change is needed and what it replaces. Do not restate the diff, do not list files, do not narrate the process.
- Do not commit unless the user explicitly asks.

### Reporting

- List the files you actually changed, using their real paths.
- State partial work as partial. Never present an unfinished task as complete.
- When a request conflicts with these rules, name the conflict and ask. Do not silently choose one side.
- Do not write numbers that expire into documentation: versions, line counts, file counts, or line references. Read them from `Cargo.toml` or the tree at the time of use.


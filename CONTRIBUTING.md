# Contribution Guide

English | [简体中文](CONTRIBUTING-zh.md)

Thank you for your interest in the WinIsland project! This document will help you understand how to contribute.

## 1. PR Contribution Scope

For non-project-members, your PR can cover the following areas:

1.1. Issues already labelled `accepted` — you may submit a PR for these.

1.2. Documentation, comments, code cleanups (e.g. fixing clippy warnings), minor UI tweaks, and other small, well-defined changes.

1.3. Feature PRs with a large diff (new features, refactoring core modules, etc.) require a detailed design proposal in the PR first; project members will then review it.

1.4. **Changelog**: Do not modify changelog files (`Changelog.md`, `Changelog-zh.md`) in your pull request. Only project maintainers are allowed to update these files.

⛔ For PRs that fall outside this scope, project members **have the right to reject them directly**.

> Our principle: **Any contribution should deliver more value to the project than the effort required to review it**. Please communicate with project members before starting work to avoid conflicting directions.
>
> (Of course, if there has already been discussion in an issue, feel free to jump right in :P Just let us know :D)

## 2. Development Environment Requirements

- **Rust**: the toolchain must support edition 2024 (installation via [rustup](https://rustup.rs/) is recommended). The workspace sets `edition = "2024"`; there is no `rust-version` floor, so use a current stable toolchain.
- **Git**: latest version
- **Windows**: WinIsland depends heavily on Windows APIs. Develop on Windows 10/11 (x86_64 or ARM64).

After the initial clone, run:
```bash
cargo build
```

The first build compiles Skia from prebuilt binaries and may download them, so it needs network access and takes several minutes.

## 3. Code Standards

The project has one source of truth for conventions: [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md). This section covers only what a contributor needs before opening a PR.

### 3.1 Rust Code Style

**3.1.1 Formatting**: must be run before committing.
```bash
cargo fmt --all
```

**3.1.2 Linting**: clippy must pass with no warnings.
```bash
cargo clippy --workspace -- -D warnings
```

A warning is a signal to restructure, not to suppress. Do not add `#[allow(...)]` to make the check pass; if a suppression is genuinely required, explain it in the PR.

**3.1.3 Naming**: file, function, and variable names are `snake_case`; types are `PascalCase`; constants and statics are `SCREAMING_SNAKE_CASE`. The full table, including module and value-type rules, is in [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md).

**3.1.4 Comments**: explain why, not what. `unsafe` blocks require a `// SAFETY:` comment naming the invariant and who upholds it. Do not add comments that restate the code.

**3.1.5 Tests**: this project does not accept unit, integration, snapshot, or UI tests. Do not add `#[cfg(test)]` modules either.

### 3.2 Where Code Goes

The workspace is layered, and a change that reaches the wrong crate will be rejected regardless of how well it works.

| Change | Goes in |
|---|---|
| Domain logic with no platform dependency | `crates/winisland-core/` |
| Platform contracts and value types | `crates/winisland-platform/` |
| Win32, COM, WinRT, winit, audio, media | `crates/winisland-platform-windows/` |
| Skia, D3D12, drawing primitives, text | `crates/winisland-render/` |
| Plugin ABI types and the plugin SDK | `crates/winisland-plugin-api/` |
| Plugin loading, services, draw-list replay | `crates/winisland-plugin-host/` |
| Package format, signing, marketplace | `crates/winisland-plugin-package/` |
| Application state and UI | `src/` |

Application code under `src/**` must not depend on `winit`. Skia calls belong in `crates/winisland-render/`; application UI draws through `Painter`. The architecture and the allowed `unsafe` locations are documented in [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md).

**3.2.1 `unsafe`**: every `unsafe` block needs a `// SAFETY:` comment. New `unsafe` outside the crates listed in [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) requires an architecture decision before the PR is opened.

**3.2.2 Async**: background work uses `pollkit::Job` for fire-and-forget tasks with a pollable result, or `tokio::spawn` for async work. Cross-thread communication with the window layer goes through the platform contract; application code does not touch the event loop directly.

### 3.3 Rendering and UI

- Drawing code uses `winisland_render` value types and `Painter`. Do not write into pixel buffers by hand.
- New icons are Skia paths under `src/icons/`; do not hardcode SVGs elsewhere.
- New or modified UI follows Apple's design language and matches the existing island, not a new visual style.
- UI changes need a screenshot or a description of the animation in the PR. There are no automated UI tests, so a reviewer cannot verify appearance from the diff.

### 3.4 Dependencies

- Adding, removing, or upgrading a dependency is an architecture decision, not a convenience. State the alternative you rejected, the effect on binary size, and whether the dependency reaches `winisland-plugin-api` (plugin authors compile against that crate).
- Do not edit `Cargo.lock` by hand.
- `winisland-plugin-api` must keep its default feature set dependency-free.

## 4. Git Workflow

### 4.1 Branch Naming

Branches must match `<prefix>/<description>`, where the description is ASCII letters, digits, dots, underscores, or hyphens. `ci/validate-pr-branch.yml` enforces this and closes non-conforming PRs automatically.

Allowed prefixes: `feat`, `fix`, `hotfix`, `refactor`, `chore`, `docs`, `test`, `ci`, `build`, `perf`, `style`, `revert`, `release`, `deps`, `security`.

Do not open a PR from `main` or `master`.

### 4.2 Commit Conventions

This project enforces [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<scope>): <description>
```

The scope is optional. The type is lowercase and must be one of:

| Type | Use for |
|---|---|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only |
| `style` | Formatting with no behaviour change |
| `refactor` | A change that neither fixes a bug nor adds a feature |
| `perf` | A performance improvement |
| `chore` | Build, tooling, dependencies, releases |
| `ci` | CI configuration |
| `revert` | Reverting a previous commit |
| `security` | A security fix |

A commit message describes what the commit changes and why. Nothing else:

- Subject: imperative, lowercase, no trailing period, 72 characters or fewer.
- Body: explain why the change is needed and what it replaces. Do not restate the diff, do not list files, and do not narrate the process.
- Do not encode workflow position or task tracking in a message. No `phase 1`, `P0`, `step 3`, `stage 2`, or `part 2` markers, in the subject or the body.
- Do not number a series of commits as a plan. If a change needs several commits, name each by its content.

**Examples**:
```
feat(smtc): support custom SMTC app filtering
fix(render): keep the rounded corner inside the island bounds
docs(contributing): describe the layer each crate owns
```

### 4.3 Commit Checks

There are no Git hooks in this repository. Checks run in CI:

| Workflow | Runs |
|---|---|
| `pr-check.yml` | `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings` |
| `pr-build.yml` | `cargo build --release`, `cargo test` |
| `validate-pr-branch.yml` | Branch naming, for PRs from forks |

Run the same commands locally before pushing. A PR whose title contains `[ci-skip]` or `[docs]` skips the first two workflows; do not use either marker.

### 4.4 What if a check fails?

1. Formatting fails → run `cargo fmt --all`, then commit the result.
2. Clippy fails → fix the code. Do not add `#[allow(...)]`.
3. The build fails → fix it before opening the PR. CI runs a release build, which enables optimisations the debug build does not.

Self-check before pushing:

```bash
cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo build --release
```

### 4.5 Pull Request Process

1. Fork the repository and create a branch:
   ```bash
   git checkout -b feat/your-feature
   ```

2. Develop and self-check with the command in 4.4.

3. Commit:
   ```bash
   git add <files>
   git commit -m "feat(scope): description"
   ```

   Stage related changes together rather than everything at once, so each commit stands on its own.

4. Push your branch and open a PR.

    - Title: 70 characters or fewer, in the same `<type>(<scope>): <description>` form as a commit subject.
    - Fill in the PR template. A PR with an unchecked template, or one that leaves "Summary" and "Motivation" empty, will be closed.
    - If the change touches configuration (`crates/winisland-core/src/config/`), describe backward compatibility and the migration path.
    - If the change touches the plugin ABI, say so explicitly. ABI changes are reviewed separately.
    - Keep the diff to what the description claims. A PR that also reformats unrelated files or moves unrelated code will be asked to split.

5. Respond to review. Pushing a new commit dismisses existing approvals, so a review round starts again.

## 5. Code Review Standards

### 5.1 Must Meet

- CI passes: `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --release`.
- The change does not break existing SMTC monitoring, window behaviour, or plugin loading.
- Every new `unsafe` block has a `// SAFETY:` comment and sits in a crate that [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) allows.
- The change does not add tests, and does not add temporary verification scaffolding either.
- The change deletes what it replaces. Leaving the old path in place "just in case" is a review blocker.

### 5.2 Should Meet

- The diff stays inside the scope the description claims.
- UI changes include a screenshot or a description of the animation, since there is no automated way to verify appearance.
- New behaviour is observable: say how a reviewer can trigger it.

### 5.3 How to Review a PR You Did Not Write

Start from the PR description, then check that the diff matches it.

1. Does the file list match the claimed scope? A "lyrics font" change that touches seven crates needs one sentence per crate explaining why.
2. Read every new `unsafe`, `#[allow(...)]`, `unwrap()`, and `Cargo.toml` line first. These are where the cost lands later.
3. Ask what the change removed. A change that only adds is the most common source of structural debt.
4. Run the branch locally for anything that changes behaviour. There is no test suite to fall back on.

## 6. FAQ

### 6.1 How do I run WinIsland?
```bash
cargo run --package WinIsland
```
> Only one instance can run at a time; the second exits immediately.

### 6.2 Too many clippy warnings?

Fix them. `cargo clippy --fix --allow-dirty` applies the mechanical ones, but read the result: the tool rewrites code, and it does not always preserve intent. Never silence a remaining warning with `#[allow(...)]`.

### 6.3 How do I test audio visualization?

Play audio that SMTC recognises, then watch the island. The spectrum path is `src/core/audio.rs` and the capture backend is in `crates/winisland-platform-windows/src/audio/`. Temporary logging is fine while you work, but remove it before committing.

### 6.4 How do I test a plugin?

Build the plugin as a `cdylib`, package it with the packager, then drag the resulting ZIP onto the island window. Locally installed plugins are not signature-checked, so only load plugins you built or trust. Plugin development is documented in [Page/plugin-dev.md](Page/plugin-dev.md).

### 6.5 I do not use AI tools. Does this repository assume I do?

No. [AGENTS.md](AGENTS.md) and [.agents/](.agents/) exist so that AI agents produce changes that fit the project, and they are also a useful summary for humans. Nothing in this guide requires an AI tool, and no contribution is judged on whether one was used.

## 7. Code of Conduct

- Respect all contributors and stay kind and professional.
- Accept constructive feedback.
- Help new contributors understand the Windows APIs and the rendering pipeline.
- Discuss design before writing a large change. A rejected 3,000-line PR costs everyone more than a five-minute conversation.

---

Thank you once again for your contribution! We look forward to your PR! If you have any questions, feel free to ask in an Issue or reach out to project members.

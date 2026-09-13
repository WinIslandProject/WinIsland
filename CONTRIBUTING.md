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

- **Rust**: 1.80+ (installation via [rustup](https://rustup.rs/) is recommended)
- **Git**: latest version
- **Windows**: WinIsland has a strong dependency on Windows APIs. Developing on Windows 10/11 (x86_64 or ARM64) is recommended.

After the initial clone, run:
```bash
cargo build
```

## 3. Code Standards

### 3.1 Rust Code Style

**3.1.1 Formatting**: Must be run before committing:
```bash
cargo fmt --all
```

**3.1.2 Linting**: All clippy checks must pass with no warnings allowed:
```bash
cargo clippy --workspace -- -D warnings
```

**3.1.3 Naming conventions**:
- File names: `snake_case` (e.g. `audio_capture.rs`)
- Functions/variables: `snake_case` (e.g. `get_media_info`)
- Structs/enums/traits: `PascalCase` (e.g. `MediaInfo`, `AudioProcessor`)
- Constants/statics: `SCREAMING_SNAKE_CASE` (e.g. `MAX_SAMPLE_RATE`)

**3.1.4 Comment conventions**:
- Complex logic or `unsafe` blocks require inline comments explaining the reasoning.
- Avoid meaningless comments (e.g. ones that simply repeat what the code says).

**3.1.5 Windows-specific**:
- All Win32 API calls must be wrapped inside `unsafe {}` blocks.
- Code involving windows, audio, or SMTC must pay attention to thread safety.

**3.1.6 Tests**:
- Do not add unit, integration, snapshot, or UI tests to this project.

### 3.2 Rendering (Skia)

- When modifying drawing logic in `src/core/render.rs`, ensure the Skia surface has been properly initialized.
- Drawing code should use the 2D APIs provided by `skia_safe`; do not manually write into pixel buffers.
- Adding new icons: define Skia paths under `src/icons/`; do not hardcode SVGs elsewhere.

### 3.3 Async Code

- All async tasks should be started with `tokio::spawn`, for example updaters and audio capture.
- When interacting with the winit event loop, use `tokio` channels or `winit::event_loop::EventLoopProxy` for cross-thread communication.

### 3.4 UI Guidelines
- All new/modified UI should follow Apple Design guidelines and maintain consistency with existing UI styles.

## 4. Git Workflow

### 4.1 Branch Naming

- `feat/` — new feature
- `fix/` — bug fix
- `hotfix/` — urgent bug fix
- `refactor/` — refactoring
- `chore/` — chores (dependency updates, build config, etc.)
- `docs/` — documentation updates
- `test/` — tests
- `ci/` — CI changes
- `build/` — build system changes
- `perf/` — performance improvements
- `style/` — code style changes
- `revert/` — revert changes
- `release/` — release branches
- `deps/` — dependency updates
- `security/` — security fixes

### 4.2 Commit Conventions

This project enforces [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). Format:

```
<type>: <subject>
<type>(scope): <subject>
```

`type` must be lowercase and can be one of:

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Code formatting (no logic change)
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `perf`: A performance improvement
- `chore`: Build process, dependencies, etc.
- `ci`: CI configuration changes
- `revert`: Reverting a previous commit

**Examples**:
```
feat(smtc): support custom SMTC app filtering
fix(render): fix rounded corner drawing issue in extended mode
docs(contributing): supplement Skia rendering guidelines
```

### 4.3 Commit Checks

The repository has automated checks configured:

- `pre-commit`: runs `cargo fmt -- --check` to ensure correct formatting.
- `commit-msg`: validates that the commit message follows the Conventional Commits format.
- CI: runs clippy, format checks, and builds once more.

### 4.4 What if my commit is blocked?

1. `cargo fmt` fails → run `cargo fmt --all`, then `git add` again.
2. Commit message does not conform → rewrite it using the `<type>: description` format.
3. Run self-checks beforehand: `cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo build`

### 4.5 Pull Request Process

1. Fork the repository and create a branch:
   ```bash
   git checkout -b feat/your-feature
   ```

2. Develop and self-check:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo build --release   # ensure the release build succeeds
   ```

3. Commit:
   ```bash
   git add .
   git commit -s -m "feat(scope): feature description"
   ```

4. Push your branch and open a PR.
    - Keep the PR title concise (≤70 characters). The description should include a summary of changes, testing instructions, and related issues.
    - If you changed configuration items (`config.rs`), describe backward compatibility.

## 5. Code Review Standards

### 5.1 Must Meet

- ✅ All CI checks pass
- ✅ `cargo fmt` produces no diff
- ✅ `cargo clippy` produces no warnings (`-D warnings`)
- ✅ `cargo build --release` succeeds
- ✅ Feature is complete and does not break existing SMTC monitoring or window behavior
- ✅ `unsafe` blocks have adequate justification and their safety has been reviewed

### 5.2 Should Meet

- Adequate comments and documentation
- If there are UI changes, provide screenshots or describe animation effects (spring parameters, etc.)

## 6. FAQ

### 6.1 How do I run WinIsland?
```bash
cargo run --package WinIsland --bin WinIsland --profile dev
```
> Note: Only one instance can run at a time (protected by a Windows mutex).

### 6.2 Too many clippy warnings?
```bash
cargo clippy --fix --allow-dirty
```

### 6.3 How do I test audio visualization?
- Play any audio that SMTC can recognize.
- Check whether `src/core/audio.rs` behaves correctly (you can add temporary print statements during development).

## 7. Code of Conduct

- Respect all contributors, and remain kind and professional.
- Accept constructive feedback.
- Help new contributors understand Windows APIs and Skia usage.

---

Thank you once again for your contribution! We look forward to your PR! If you have any questions, feel free to ask in an Issue or reach out to project members.

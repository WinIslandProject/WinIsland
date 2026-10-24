---
name: commit-push
description: Write a Conventional Commit message from the staged diff and commit it. Use when the user says commit, /commit, commit changes, or asks to create a git commit.
license: GPL-3.0
metadata:
  project: WinIsland
---

## Before writing anything

```bash
git status --short
git diff --cached --stat
```

If nothing is staged, stage related files together first. Do not `git add -A` when the working tree spans unrelated changes; split them.

Never commit unless the user explicitly asked in this turn.

## Format

```
<type>(<scope>): <description>

<body, optional>
```

### Type

| Type | Use for |
|---|---|
| `feat` | A new feature |
| `fix` | A bug fix |
| `refactor` | Neither fixes a bug nor adds a feature |
| `perf` | A performance improvement |
| `style` | Formatting with no behaviour change |
| `docs` | Documentation only |
| `chore` | Build, tooling, dependencies, releases |
| `ci` | CI configuration |
| `revert` | Reverting a previous commit |
| `security` | A security fix |

### Scope

Use the module or crate the change actually touches. Prefer the crate name when the change lives in one crate, and omit the scope when it spans several.

| Scope | Area |
|---|---|
| `core` | `crates/winisland-core/` |
| `platform` | `crates/winisland-platform/` or `winisland-platform-windows/` |
| `render` | `crates/winisland-render/` |
| `plugin` | `crates/winisland-plugin-*/` or `src/plugin/` |
| `settings` | `src/window/settings/` |
| `window` | `src/window/` |
| `ui` | `src/ui/` |
| `lyrics` | Lyrics model, providers, or matching |
| `smtc` | Media session integration |
| `audio` | Capture, spectrum, output devices |
| `ci` | `.github/workflows/` |
| `agents` | `AGENTS.md`, `.agents/` |

### Description

- Imperative mood: `add`, not `added` or `adds`.
- Lowercase, no trailing period.
- 72 characters or fewer.
- Describe the change, not the process.

### Body

Only when the subject cannot carry the reason. Explain why the change is needed and what it replaces. Wrap at 72 columns. Do not restate the diff, do not list files, and do not narrate the steps you took.

## What a message must not contain

- Workflow position or planning state: no `phase 1`, `P0`, `P1`, `step 3`, `stage 2`, `batch 4`, `part 2`.
- A number standing in for a plan. If a change needs several commits, name each by its content.
- Emoji, unless the user asked for them.

## Examples

```
feat(lyrics): shape Bangla lines before measuring text
fix(render): keep the rounded corner inside the island bounds
refactor(plugin): move the built-in calendar into a workspace crate
ci: gate PR checks so required contexts always report
docs(agents): document where unsafe is allowed
```

## After committing

```bash
git log --oneline -1
git status --short
```

Confirm the tree is clean and report the hash. Push only if the user asked.

---
name: pr-write
description: Draft a WinIsland pull request description that matches the repository template. Use when the user asks to open a PR, write a PR description, or prepare a branch for review.
license: GPL-3.0
metadata:
  project: WinIsland
---

## Before writing

```bash
git log --oneline origin/master..HEAD
git diff --stat origin/master...HEAD
git diff origin/master...HEAD
```

Read the actual diff. A description written from the commit messages alone will miss what the branch really changes.

Check the branch name first. `ci/validate-pr-branch.yml` closes any PR whose branch does not match `<prefix>/<description>`, where the description is ASCII letters, digits, dots, underscores, or hyphens. Allowed prefixes: `feat`, `fix`, `hotfix`, `refactor`, `chore`, `docs`, `test`, `ci`, `build`, `perf`, `style`, `revert`, `release`, `deps`, `security`.

## Template

Fill [.github/pull_request_template.md](../../../.github/pull_request_template.md) exactly. Do not reorder or drop sections.

- **Pre-submission checklist**: tick the first three. Leave the fourth ("I confirm that I haven't read the checkboxes") unticked; it exists as a trap.
- **Change type**: exactly one box. Use the same type as the commit subject.
- **Scope**: every box that applies.
- **Related issues**: `Closes #N` when the PR should close it, `Refs #N` otherwise. Write `None.` when there is no issue.
- **Description**: keep the four headings. Delete the HTML comments and write real prose under each.

## What each section has to say

**Summary**: one sentence. What changes for the user or the maintainer.

**Motivation and context**: why this is needed now. Name the concrete problem: the failing behaviour, the misleading document, the check that never runs. "Improves code quality" is not a motivation.

**Changes**: group by file or subsystem. One bullet per meaningful change, not per file. A reviewer should be able to map each bullet to a hunk.

**UI changes**: a screenshot, or "None." If the change is visible, a reviewer cannot verify it from the diff because there are no UI tests.

## Rules

- Do not claim a check passed unless it ran. Quote the result or omit the claim.
- Do not describe work that is not in the diff. Deferred work goes under "Notes for reviewers", not under "Changes".
- If the diff is larger than the description accounts for, either explain the extra files or split the PR.
- State what the change removes. A PR that only adds is the most common source of structural debt.
- No emoji.
- Keep the title in `<type>(<scope>): <description>` form, 70 characters or fewer.

## Notes for reviewers

Add this section when the PR needs context a reviewer would otherwise have to reconstruct:

- Behaviour that is intentionally unchanged but looks changed.
- A known gap this PR does not close.
- Follow-up work that must happen after merge.
- Anything the author is unsure about. Naming the uncertainty is better than hiding it.

## Opening the PR

```bash
gh pr create --base master --head <branch> --title "<type>(<scope>): <description>" --body-file <file>
```

Write the body to a file rather than passing it inline; multi-line Markdown does not survive shell quoting reliably.

After opening:

```bash
gh pr view <number> --json state,mergeable,mergeStateStatus,statusCheckRollup
```

Report the check results. A PR that is `BLOCKED` on a required context is not ready, even if the code is.

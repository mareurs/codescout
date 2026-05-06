---
specialist: docs-lotus-frog
scope: project
slug: experimental-docs-lifecycle
created: 2026-05-06
updated: 2026-05-06
tags: [experiments-branch, documentation, graduation, cherry-pick]
---

**Lesson:** Every feature commit on `experiments` must carry its docs in the same commit; graduation to `master` has a four-step doc migration checklist.

**Why:** This project keeps experimental docs isolated under `docs/manual/src/experimental/` with a visible ⚠ callout. When a feature graduates, the doc must move, the callout must drop, SUMMARY.md must gain the entry, and the experimental index must lose it — all atomically in the cherry-pick commit.

**How to apply:**

### Adding a feature to `experiments`

- Create `docs/manual/src/experimental/<feature-name>.md` — user-facing prose, with this callout at the very top:
  ```
  > ⚠ Experimental — may change without notice.
  ```
- Add one line to `docs/manual/src/experimental/index.md` linking to the new page.
- Both changes go in the **same commit** as the feature code.
- **Bug fixes are exempt** — no experimental doc needed.

### Removing a feature from `experiments`

- Delete `docs/manual/src/experimental/<feature-name>.md`.
- Remove its entry from `docs/manual/src/experimental/index.md`.
- Both changes go in the **same commit** as the revert/removal.

### Graduating a feature (`experiments` → `master`)

Use `--no-commit` to bundle the doc migration into the cherry-pick commit:

```bash
git cherry-pick --no-commit <sha>
```

Then make exactly four doc changes before committing:

1. `git mv docs/manual/src/experimental/<name>.md docs/manual/src/<target-chapter>/<name>.md`
2. Remove the `> ⚠ Experimental` callout from the top of the moved file.
3. Add the page to `docs/manual/src/SUMMARY.md` under the right chapter.
4. Remove the feature's entry from `docs/manual/src/experimental/index.md`.

**Rebase caveat:** After cherry-picking to `master`, the graduation commit has extra doc changes relative to the original `experiments` commit. Git will **not** auto-drop the original during `git rebase master` on `experiments`. Must drop it manually:

```bash
git checkout experiments
git rebase master          # original commit NOT auto-dropped
git rebase -i master       # drop it from the list
```

---
specialist: docs-lotus-frog
scope: project
slug: experimental-docs-lifecycle
created: 2026-05-06
updated: 2026-08-06
tags: [experiments-branch, documentation, graduation, cherry-pick, unreleased-callout]
---

**Lesson:** New-subsystem docs go **straight into the main manual** carrying a byte-identical unreleased callout — not staged under `docs/manual/src/experimental/` for later migration. The callout comes off at **release**, not at the merge to `master`. The staging-then-move flow this memory originally prescribed was measured at **0/62 compliance** and is retired.

**Why:** Two separable decisions were conflated in the original version of this memory — *where the page lives while unreleased* and *whether the page is marked unstable*. Evidence from 2026-08-06 split them:

- **Staging location — the flow failed.** A 387-commit cohort (62 `feat` commits, ten new subsystems: worktree overlay, catalog GC/rehome, entry citations, `link_scan`, `graft`, constitution trackers, `edit_markdown` miss diagnostics, …) landed with **zero** pages created under `experimental/`, and `experimental/index.md` still claimed *"No features are currently in flight as of 2026-06-16"* seven weeks later. Not partial compliance — zero. The failure point is always the *move* step, which happens long after the author who understood the feature has moved on. Frog Heuristic 4 applies to this memory itself: a checklist nobody follows is aspirational documentation, and documenting reality beats documenting intent.
- **The marker — the flow was right.** Dropping the callout along with the staging directory lost the one thing a reader needs: *"can I use this today?"* A page in the main manual with no marker silently claims availability. That signal is worth keeping independently of where the file sits.

**How to apply:**

### Adding a new subsystem on `experiments`

- Write the page in its **permanent** home (`docs/manual/src/concepts/<name>.md` or `tools/<name>.md`) and wire it into `SUMMARY.md` under the right chapter. Same commit as the feature code.
- Insert this blockquote immediately after the H1, **byte-identical every time** — uniformity is what makes removal mechanical later:
  ```
  > ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
  > crates.io; the API may change without notice. The full cohort is listed under
  > `[Unreleased]` in
  > [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).
  ```
  Bump the version number in the text to whatever the last shipped release is.
- Add the entry to `CHANGELOG.md` `[Unreleased]`. That is the one canonical cohort list — the page links to it rather than restating it.
- **Bug fixes are exempt** — no page, no callout.
- `experimental/index.md` is no longer a hand-maintained inventory. It points at the commit range (`git log master..experiments`) as the mechanical source of truth, plus a table mapping the current cohort to its permanent manual pages.

### Removing a feature before release

- Delete the page, drop its `SUMMARY.md` line and its `CHANGELOG.md` `[Unreleased]` entry. Same commit as the revert.

### Merging `experiments` → `master`

Callouts **stay**. `master` is not crates.io — a reader on master still cannot `cargo install` the feature, so the callout's claim is still true. Nothing doc-side to migrate; the pages are already home. See `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*, whose documentation gate lists this explicitly.

### Cutting a release

Remove every callout **in the same commit as the version bump** — otherwise the published docs tell readers a feature they can now install is unavailable. Mechanical, because the text is uniform:

```bash
grep -rl 'Unreleased — on the `experiments` branch only' docs/manual/src/
# delete the five-line blockquote from each hit
```

This is step **1b** of `docs/RELEASE.md` § *Release Cycle*.

**What carried over unchanged:** the *same-commit* discipline (docs land with the code, never "later" — the original memory's strongest claim, and the one the cohort also violated) and the bug-fix exemption.

**Retired:** the four-step graduation checklist (`git mv` + drop callout + add SUMMARY + remove index entry bundled into `cherry-pick --no-commit`) and its rebase caveat about the original commit not auto-dropping. Both were consequences of staging under `experimental/`. If a future session reinstates staging, they come back — `git log -- .buddy/memory/docs-lotus-frog/experimental-docs-lifecycle.md` has the text.

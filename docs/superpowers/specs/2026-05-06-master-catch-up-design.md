# Master Catch-Up Design

**Date:** 2026-05-06
**Branch:** experiments → master
**Type:** One-time wholesale FF merge

## Context

`experiments` has accumulated 263 commits since master's last sync. All features are working, tests pass. The accumulated work is stable enough to advance master wholesale rather than cherry-pick one commit at a time.

This is a one-time catch-up, not a workflow change. Cherry-pick remains the standard for future individual feature graduation. This batch merge is used when the branch accumulates a large, stable, coherent body of work.

## Pre-conditions

- `master` is a strict ancestor of `experiments` — fast-forward is possible with no conflicts
- `git merge --ff-only` will be used to enforce this (fails if FF is not possible)
- One commit has a stale "wip" label (`dffd1ce`) but contains complete, working code

## Steps

### 1. Add CLAUDE.md rule (on experiments, before merge)

Add to the Branch Strategy section:

> **`experiments` is never deleted.** After any merge to `master`, `experiments` continues from the same commit — no recreation, no force-reset.

Commit on `experiments`:
```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE): add experiments-branch persistence rule"
```

### 2. Pre-flight

```bash
cargo test && cargo clippy -- -D warnings
```

Must pass before proceeding. If either fails, fix on `experiments` before merging.

### 3. FF merge

```bash
git checkout master
git merge --ff-only experiments
```

### 4. Push master

```bash
git push
```

### 5. Post-merge state

After FF: `master == experiments` (identical SHAs). No rebase needed. Both branches point to the same tip. Future experimental work continues on `experiments` as before.

## Non-goals

- No version bump or crates.io publish (separate task, later)
- No squashing of history
- No cleanup of the "wip" commit label

## CLAUDE.md branch strategy (after change)

```
### Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch.** Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP.
- **`experiments` is never deleted.** After any merge to `master`, `experiments` continues from the same commit — no recreation, no force-reset.
- Never commit directly to `master` for in-progress or exploratory work.
```

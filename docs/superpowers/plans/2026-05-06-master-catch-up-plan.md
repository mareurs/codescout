# Master Catch-Up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Advance `master` to match `experiments` via a fast-forward merge, and add a branch persistence rule to CLAUDE.md.

**Architecture:** Single fast-forward merge — no conflicts, no history rewriting. CLAUDE.md rule is committed on `experiments` first so it arrives on `master` as part of the merge.

**Tech Stack:** git, cargo

---

## Task 1: Add CLAUDE.md branch persistence rule

**Files:**
- Modify: `CLAUDE.md` (Branch Strategy section)

- [ ] **Step 1: Verify we are on `experiments`**

```bash
git branch --show-current
```

Expected: `experiments`

- [ ] **Step 2: Edit CLAUDE.md**

In `CLAUDE.md`, find the Branch Strategy section. The current content is:

```
### Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch** (or a dedicated feature branch). Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP (`cargo build --release` + `/mcp` restart).
- Never commit directly to `master` for in-progress or exploratory work.
```

Add one bullet after the cherry-pick line:

```
### Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch** (or a dedicated feature branch). Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP (`cargo build --release` + `/mcp` restart).
- **`experiments` is never deleted.** After any merge to `master`, `experiments` continues from the same commit — no recreation, no force-reset.
- Never commit directly to `master` for in-progress or exploratory work.
```

- [ ] **Step 3: Verify the change**

```bash
grep "never deleted" CLAUDE.md
```

Expected: `- **\`experiments\` is never deleted.**...`

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE): add experiments-branch persistence rule"
```

Expected: `[experiments <sha>] docs(CLAUDE): add experiments-branch persistence rule`

---

## Task 2: Pre-flight — tests and clippy

- [ ] **Step 1: Run tests**

```bash
cargo test
```

Expected: `test result: ok. N passed; 0 failed`

If any test fails: **stop**. Fix the failure on `experiments` before proceeding.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings, exit code 0.

If clippy reports warnings: **stop**. Fix them on `experiments` before proceeding.

---

## Task 3: Fast-forward merge and push

- [ ] **Step 1: Confirm fast-forward is still possible**

```bash
git merge-base --is-ancestor master experiments && echo "FF possible" || echo "DIVERGED — stop"
```

Expected: `FF possible`

If `DIVERGED`: stop and investigate. Do not force a merge.

- [ ] **Step 2: Switch to master**

```bash
git checkout master
```

Expected: `Switched to branch 'master'`

- [ ] **Step 3: Fast-forward merge**

```bash
git merge --ff-only experiments
```

Expected: `Fast-forward` in output, no merge commit created.

If the command fails with "Not possible to fast-forward": abort (`git merge --abort` if needed), return to `experiments`, and investigate.

- [ ] **Step 4: Verify master tip matches experiments**

```bash
git log --oneline -3
git rev-parse master
git rev-parse experiments
```

Expected: both SHAs identical.

- [ ] **Step 5: Push master**

```bash
git push
```

Expected: `master -> master` in output, no rejections.

- [ ] **Step 6: Verify post-merge state**

```bash
git log --oneline master..experiments
```

Expected: no output (branches are identical).

```bash
git log --oneline experiments..master
```

Expected: no output.

---

## Summary

Three commits will land on `master` beyond what was there before — 263 accumulated experiments commits plus the CLAUDE.md rule added in Task 1. No code was changed during this plan; all changes were made on `experiments` before the merge.

After completion: `master == experiments`. Future experimental work continues on `experiments` as before. A release (version bump + `cargo publish`) is a separate task, not part of this plan.

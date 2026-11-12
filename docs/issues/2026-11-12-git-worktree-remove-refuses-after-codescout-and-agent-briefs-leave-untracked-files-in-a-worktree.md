---
id: '3dde1548c3e5c310'
kind: bug
status: open
title: git worktree remove refuses after codescout and agent briefs leave untracked files in a worktree
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-12
severity: medium
---

# BUG: git worktree remove refuses after codescout and agent briefs leave untracked files in a worktree

## Summary

Every worktree where a codescout write tool ran ends with untracked files that no `.gitignore` rule covers, so `git worktree remove <path>` refuses ("contains modified or untracked files") and each cleanup needs `--force` and a human decision. A subagent brief that names a scratch report outside `.superpowers/` adds one more. After one forced removal, git had already deregistered the worktree but the directory could not be deleted ("Directory not empty"), leaving a dangling directory under `.worktrees/`.

## Class fit

No class on the roster says that a tool leaves per-checkout state that nothing ignores or cleans up; the nearest, `IC-17` (a shared resource carries no owner), is about peers, not about files. The template says to use the escape hatch rather than force a fit, so this is `cluster/unclassified`.

## Symptom (Effect)

- `git worktree remove .worktrees/<name>` exits 128: `contains modified or untracked files, use --force to delete it`.
- The skill `finishing-a-development-branch` forbids forcing on the agent's own initiative, so the agent must stop and ask the operator once per worktree. The operator said on 2026-11-12: "we can't leave dangling codescout/superpowers trash lying around".
- Leaving the worktree behind also blocks `git branch -d`, because the branch stays checked out.

## Reproduction

Observed three times in one session (2026-11-11 and 2026-11-12); the minimal trigger is not isolated.

1. `git worktree add .worktrees/x -b y experiments`, enter it, activate it in codescout with `read_only=false`, and run codescout write and shell tools in it.
2. `git -C .worktrees/x status --porcelain -uall` lists untracked files.
3. `git worktree remove .worktrees/x` refuses.

## Environment

- codescout on `experiments`, Linux, git worktrees under `/.worktrees/` (ignored by `.gitignore` line 7).
- Three worktrees affected: `tracker-lifecycle-plan-a`, `tracker-lifecycle-plan-b`, `lifecycle-grace-ledger-scope`.

## Root cause

Two causes, both verified by the probes in Hypotheses tried.

- **Refusal (fixed):** the first write tool call in a checkout creates `.codescout/native-provenance-v1.counter` and `.lock`, and no `.gitignore` rule covered them. Fix 1 covers them.
- **Leftover directory (not fixed):** the first `symbols` call on a Rust file starts a `codescout mux … rust-analyzer` process with `--cwd` set to the worktree, and rust-analyzer runs `cargo check` with `target/` inside the worktree. Neither process stops when another project is activated or when the worktree is removed. `git worktree remove` deregisters the worktree and then fails on `Directory not empty` because `cargo check` writes `target/` again.
- **Second trap (not fixed):** after the active project's worktree is removed, `workspace(action="activate")` fails for every path, the main checkout included, with `No such file or directory (os error 2)`. Creating an empty directory at the old path lets activation succeed again.

Other verified facts:

- Before fix 1, `.gitignore` had no rule for `.codescout/native-provenance-v1.counter` or `.codescout/native-provenance-v1.lock`. The lock name is defined in `src/tools/native_provenance_fence.rs` (`LOCK_NAME`). Both files appeared untracked in all three worktrees.
- Not a cause: `.superpowers/`, `.codescout/write.lock`, `.codescout/write.lock.holder`, `.codescout/private-memories/`, `scripts/__pycache__/` and `target/` are all ignored, so they do not block removal.
- `.superpowers-fix-report.md` was untracked and not ignored. A subagent brief written by the controller named that path for the implementer's scratch report. That one is a dispatch defect of the superpowers workflow, not of codescout.

## Evidence

- `tracker-lifecycle-plan-a`: `git status --porcelain -uall` gave `?? .codescout/embeddings`, `?? .codescout/native-provenance-v1.counter`, `?? .codescout/native-provenance-v1.lock`. The entry `.codescout/embeddings` is untracked even though `.gitignore` line 80 ignores `.codescout/embeddings/`; a rule with a trailing slash matches only directories, so it may have been a file or a symlink. Not verified before the worktree was removed.
- `tracker-lifecycle-plan-b`: `git worktree remove --force` exited 255 with `failed to delete '.../tracker-lifecycle-plan-b': Directory not empty`. `git worktree list` no longer named it, but the directory still held `.codescout` and `target`. It needed `rm -rf` of the exact path. Why files reappeared during the removal is unknown.
- `lifecycle-grace-ledger-scope`: `git status --porcelain --ignored -uall` listed untracked `.codescout/native-provenance-v1.counter`, `.codescout/native-provenance-v1.lock`, `.superpowers-fix-report.md`; ignored `.codescout/private-memories/`, `.codescout/write.lock`, `.codescout/write.lock.holder`, `.superpowers/`, `scripts/__pycache__/`, `target/`. `--force` removal left no directory this time.

## Hypotheses tried

Three probes on 2026-11-12, each on a fresh worktree of `experiments`, each followed by `git status --porcelain --ignored -uall`.

1. **Pinned with `workspace=`, never activated.** After `grep` the only file was `.codescout/write.lock`. After `create_file` the files `.codescout/native-provenance-v1.counter`, `.lock`, `write.lock` and `write.lock.holder` existed, so a write tool creates the fence files. `run_command echo` added nothing. With fix 1 in place all four show as ignored (`!!`), and a plain `git worktree remove` (no `--force`) succeeded and left no directory.
2. **Activated with `read_only=false`.** Activation added the ignored `.codescout/libraries.json`. A plain removal while this worktree was the active project succeeded and left nothing. A later read call did not recreate the directory, and `create_file` failed with `No such file or directory`.
3. **Activated, then `symbols` on a `.rs` file.** This started `codescout mux --cwd <worktree> --idle-timeout 180 -- rust-analyzer` and a `cargo check --manifest-path <worktree>/Cargo.toml`, and `target/` filled up (52 009 bytes of ignored paths). Both processes were still running after I activated the main checkout. A plain `git worktree remove` then exited 255: `failed to delete '<path>': Directory not empty`. `git worktree list` no longer named it, and the directory held a fresh `target/` that the running `cargo check` had written.

Not reproduced: the untracked `.codescout/embeddings` entry of plan-a, and a recreated `.codescout` in plan-b (probe 3 recreated only `target/`).

## Fix

Proposals; item 1 is applied, items 2 and 3 are not:

1. **Applied 2026-11-12:** `/.codescout/native-provenance-v1.counter` and `/.codescout/native-provenance-v1.lock` are in `.gitignore`, with the `crates/*/` twin rules that the gate `every_root_codescout_ignore_rule_has_its_crates_twin` requires (find the commit with `git log -S native-provenance-v1 -- .gitignore`). `git check-ignore -v` matches all three paths, and `tests/hook_config.rs` passes (12 of 12). This removes the refusal caused by those two files. It does not cover `.codescout/embeddings` (untracked in plan-a, cause unverified), a scratch file named outside `.superpowers/`, or the leftover directory after a forced removal, so the bug stays open.
2. Brief templates for subagents must name scratch files only under `.superpowers/` (git-ignored). Applies to the superpowers workflow, not to this repo.
3. Consider a documented safe-removal recipe or a helper that checks `git status --porcelain --ignored`, removes only regenerable files, and then runs `git worktree remove`.

## Tests added

N/A — not fixed. A regression check would be a test that creates a worktree, runs a codescout write tool in it, and asserts `git status --porcelain` is empty.

## Workarounds

Before removing a worktree that ever served `symbols`, `references` or a cargo run: activate the main checkout, run `pgrep -af <worktree path>`, and stop the listed processes by PID. Then run a plain `git worktree remove <path>`. If you removed the active project's worktree first and activation now fails, `mkdir` the old path, activate the main checkout, and delete the empty directory.

For forced removal: `git worktree remove --force <path>` after `git status --porcelain --ignored -uall` shows only regenerable files. If git answers "Directory not empty", confirm with `git worktree list` that git deregistered it, then `rm -rf` the exact path and run `git worktree prune`.

## Resume

Fix 1 is done and the leftover-directory cause is verified. Next: fix 3 as code, not only a recipe. Candidates: a codescout action that stops the mux and its cargo children for a path (or does so when the active project changes away from a worktree), and `workspace(activate)` that no longer fails when the current project's directory is gone. Fix 2 (brief templates, in the superpowers plugin) is still open. Still unexplained: the untracked `.codescout/embeddings` entry in plan-a and the recreated `.codescout` in plan-b.

## References

- `.gitignore` lines 7, 80, 210; `src/tools/native_provenance_fence.rs` (`LOCK_NAME`).
- `docs/trackers/tracker-organization-session-log.md` entry W-3 (the work that was cleaned up).
- Memory `worktree-merge-catalog-reconciliation` covers merging worktrees, not removing them.

---
id: '3dde1548c3e5c310'
kind: bug
status: mitigated
title: git worktree remove refuses after codescout and agent briefs leave untracked files in a worktree
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-12
opened: 2026-11-12
severity: medium
unverified: The --stop path has had no real use (no mux was running at first use). Fix 2 (superpowers brief templates) is open and the .codescout/embeddings symlink rule is not fixed. macOS and Windows CI lanes are not checked.
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
- **Leftover directory (mitigated by `scripts/worktree-remove.sh`, 2026-11-12; codescout still does not stop the mux itself):** the first `symbols` call on a Rust file starts a `codescout mux … rust-analyzer` process with `--cwd` set to the worktree, and rust-analyzer runs `cargo check` with `target/` inside the worktree. Neither process stops when another project is activated or when the worktree is removed. `git worktree remove` deregisters the worktree and then fails on `Directory not empty` because `cargo check` writes `target/` again.
- **Second trap (fixed, bug `d291b97d9e69edef`):** after the active project's worktree is removed, `workspace(action="activate")` fails for every path, the main checkout included, with `No such file or directory (os error 2)`. Creating an empty directory at the old path lets activation succeed again.

Other verified facts:

- The untracked `.codescout/embeddings` entry is a symlink to the main checkout's `.codescout/embeddings` directory (seen 2026-11-12 in the worktree of this fix: `embeddings -> /home/marius/work/claude/codescout/.codescout/embeddings`, created at the same minute as `cc_session_id`). The rule `.codescout/embeddings/` (`.gitignore:88`) has a trailing slash and matches only a directory, so git lists the symlink as untracked and refuses a plain removal. What creates the symlink is not identified.

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

Not reproduced: a recreated `.codescout` in plan-b (probe 3 recreated only `target/`). The untracked `.codescout/embeddings` entry of plan-a is explained: see Root cause.

## Fix

Item 1 and item 3 are applied; item 2 is open:

1. **Applied 2026-11-12:** `/.codescout/native-provenance-v1.counter` and `/.codescout/native-provenance-v1.lock` are in `.gitignore`, with the `crates/*/` twin rules that the gate `every_root_codescout_ignore_rule_has_its_crates_twin` requires (find the commit with `git log -S native-provenance-v1 -- .gitignore`). This removes the refusal caused by those two files.
2. **Open:** brief templates for subagents must name scratch files only under `.superpowers/` (git-ignored). Applies to the superpowers plugin, not to this repo.
3. **Applied 2026-11-12 (approach A of `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md`):** `scripts/worktree-remove.sh` lists the holders of a worktree, stops the stoppable ones only with `--stop`, runs a plain `git worktree remove`, prunes, and verifies that the directory is gone. The operator page is `docs/conventions/removing-a-worktree.md`. The second trap (activation after removal) is fixed under bug `d291b97d9e69edef`.

First real use, 2026-11-12, on the worktree of this fix: the first run listed one holder, the `claude` process of the session itself (kind `other`, a blocker), exit 3. After the session left the worktree the script found no holders, git refused, and the script exited 6 and listed one untracked entry, the `.codescout/embeddings` symlink. After I deleted that symlink (not its target) the script exited 0. The directory, the registration and the branch were then gone, and `pgrep` found no process that names the path. The mux was not running at that time, so the `--stop` path had no real use.

## Tests added

- `tests/worktree-remove.sh`: a shell suite of 180 cases (fixture `/proc`, real-process cases, `git` shims). `tests/worktree_remove_script.rs` (`the_worktree_remove_suite_passes`, Linux only) runs it under `cargo nextest`.
- For the activation part: see bug `d291b97d9e69edef`.
- Not covered: a test that runs a codescout write tool in a worktree and asserts that `git status --porcelain` is empty; the `.codescout/embeddings` symlink would make it fail today.

## Fix provenance

- **SHA:** `5044debcb` (`experiments`)
- **patch-id:** `9264d93dd2b6`
- **SHA:** `16ea63f80` (`experiments`)
- **patch-id:** `754e565f9d31`
- **SHA:** `1f78b5462` (`experiments`)
- **patch-id:** `bcab0fc94cc6`
- **SHA:** `0ed026ea4` (`experiments`)
- **patch-id:** `7d99a1d84bcb`
- **SHA:** `5edd2280b` (`experiments`)
- **patch-id:** `5a31db54a63c`
- **SHA:** `221aba482` (`experiments`)
- **patch-id:** `8a910dd3890a`
- **SHA:** `bbf491003` (`experiments`)
- **patch-id:** `bbbe54aabedd`
- **SHA:** `60f3b6e05` (`experiments`)
- **patch-id:** `c49c5fc00f7b`
- **SHA:** `2cd58c6d5` (`experiments`)
- **patch-id:** `617325cce9ec`

## Workarounds

Run `scripts/worktree-remove.sh <path>` from outside the worktree and from a session whose cwd is not inside it. Without `--stop` it only lists holders. Ask the occupant before `--stop`. If git refuses, the script lists the untracked files and never forces; for the `.codescout/embeddings` symlink, delete the link itself (`rm <worktree>/.codescout/embeddings`, no trailing slash) and run the script again. Details: `docs/conventions/removing-a-worktree.md`.

## Resume

Open items:

- Fix 2: subagent brief templates in the superpowers plugin (ask the operator before looking there).
- The `.gitignore` rule for `.codescout/embeddings` has a trailing slash and misses the symlink. Candidate: add `.codescout/embeddings` without the slash, with its `crates/*/` twin rule, and find what creates the symlink.
- The recreated `.codescout` in plan-b is unexplained.
- Parked script follow-ups: scrub UTF-8 C1 control characters (U+0080 to U+009F) from printed command lines (one-line fix `cmd=${cmd//$'\xc2'[$'\x80'-$'\x9f']/ }`); a very large `--wait` overflows the deadline arithmetic (the safe direction); the exit-7 `ls -A` listing prints raw file names; the survivor message says "SIGKILL is never used".
- Check that CI is green on the macOS and Windows lanes (the Rust wrapper is Linux-only; not run locally).
- Use the `--stop` path on a worktree that has a live mux, and record what happened.

## References

- `.gitignore` lines 7, 80, 210; `src/tools/native_provenance_fence.rs` (`LOCK_NAME`).
- `docs/trackers/tracker-organization-session-log.md` entry W-3 (the work that was cleaned up).
- Memory `worktree-merge-catalog-reconciliation` covers merging worktrees, not removing them.

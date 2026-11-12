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
unverified: 'The --stop path has been used on three real trees (2026-11-12). Not observed: a SIGTERM sent to a live child or cargo tree, and the survivor exit (5). The ancestor blocker (bug 786b1826bab82b18) is tested on fixtures only. The .codescout/embeddings symlink is created by two codescout-companion hooks; stopping that is the claude-plugins issue. macOS and Windows CI lanes are not checked.'
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

- The untracked `.codescout/embeddings` entry is a symlink to the main checkout's `.codescout/embeddings` directory (seen 2026-11-12 in the worktree of this fix: `embeddings -> /home/marius/work/claude/codescout/.codescout/embeddings`, created at the same minute as `cc_session_id`). The rule `.codescout/embeddings/` (`.gitignore:88`, before `652f12d88`) had a trailing slash and matches only a directory, so git lists the symlink as untracked and refuses a plain removal. Two codescout-companion hooks create the symlink: `hooks/worktree-activate.mjs` (loop `for (const asset of ['embeddings'])`, line 128) and `hooks/session-start.mjs` (the same loop, line 394, which runs when a session starts inside a worktree). Both run only when the worktree's `.codescout` is a real directory and not a symlink, and both skip an `embeddings` entry that already exists. This repo tracks files under `.codescout/` (for example `.codescout/librarian.toml`, `.codescout/repo.toml` and `.codescout/system-prompt.md`), so every checkout of it has a real `.codescout` directory and gets the link. Which of the two hooks made the link in a given worktree is not determined; both make the same link. Read on 2026-11-12 in the hooks' source in `claude-plugins`; no hook was run. The archived bug `docs/issues/archive/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md` found the link first and calls it a no-op (the claim is not verified here: `src/retrieval/config.rs` `resolve_sqlite_dir` still defaults the sqlite-vec store to `<root>/.codescout/embeddings`). The design `docs/superpowers/specs/2026-08-13-worktree-semantic-search-design.md` (line 254) and its plan (line 1044) already call this link a no-op for Qdrant-backed search and say to drop it, but the hooks still create it.

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

Item 1 and item 3 are applied; item 2 was checked and has no defect to fix. A fourth change, applied 2026-11-12, covers the `.codescout/embeddings` symlink: the two `.gitignore` rules lost their trailing slash (`652f12d88`).

1. **Applied 2026-11-12:** `/.codescout/native-provenance-v1.counter` and `/.codescout/native-provenance-v1.lock` are in `.gitignore`, with the `crates/*/` twin rules that the gate `every_root_codescout_ignore_rule_has_its_crates_twin` requires (find the commit with `git log -S native-provenance-v1 -- .gitignore`). This removes the refusal caused by those two files.
2. **Closed, no defect found (2026-11-12):** the superpowers templates `7.0.0` to `7.0.2` (read in the caches of the four profiles) never name `.superpowers-fix-report.md`. They take a report path from the controller, and `sdd-workspace` puts every artifact under `.superpowers/sdd/<plan>/` with a self-ignoring `.gitignore`. The path in this bug came from a brief the controller wrote. Not checked: other plugin versions, other harnesses.
3. **Applied 2026-11-12 (approach A of `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md`):** `scripts/worktree-remove.sh` lists the holders of a worktree, stops the stoppable ones only with `--stop`, runs a plain `git worktree remove`, prunes, and verifies that the directory is gone. The operator page is `docs/conventions/removing-a-worktree.md`. The second trap (activation after removal) is fixed under bug `d291b97d9e69edef`.

First real use, 2026-11-12, on the worktree of this fix: the first run listed one holder, the `claude` process of the session itself (kind `other`, a blocker), exit 3. After the session left the worktree the script found no holders, git refused, and the script exited 6 and listed one untracked entry, the `.codescout/embeddings` symlink. After I deleted that symlink (not its target) the script exited 0. The directory, the registration and the branch were then gone, and `pgrep` found no process that names the path. The mux was not running at that time, so the `--stop` path had no real use then.

Second use, 2026-11-12, on a probe worktree (`.worktrees/stop-probe`, branch `probe/worktree-stop-20261112`, created from `experiments` for this test and deleted after it). A `symbols` call pinned to the worktree (`workspace=<path>`) started a mux (`--cwd` the worktree), rust-analyzer, and a `cargo check` with its `rustc`, `sccache` and build-script children. Results:

- Without `--stop`: exit 3 and 15 holders listed. The two mux processes of the main checkout (`--cwd` the parent directory of the worktree) were not listed.
- With `--stop`, first run: exit 4, nothing signalled. The blockers were `make`, `cc`, `cc1` and `as` (kind `other`), the C compile of a build script (`tikv-jemalloc-sys`). An earlier scan had also listed an `sh .../configure` as `other`. So a worktree in the middle of a cold build that compiles C code cannot be stopped in one run; the refusal has no side effect and the run can be repeated.
- With `--stop`, second run: four holders (the mux and three rust-analyzer processes; the `cargo check` had ended by then), exit 0. The directory and the registration were gone, no process named the path, and both main-checkout mux processes kept their PIDs (`765668`, `3018812`). I did not observe the signals: the script prints no line per signalled process, and the mux had an idle timeout of 180 s, so I cannot rule out that it ended by itself in that window.
- Not exercised: a SIGTERM to a live `cargo`/`rustc` tree, and the survivor exit (5).

Both follow-ups were made on 2026-11-12 (commits in Fix provenance). First, `--stop` prints one stderr line per signal it sends. Second, a new kind `child`: an `other` holder that has a holder of a stoppable kind among its ancestors (found through `PPid:` in `/proc/<pid>/status`, at most 64 steps) is stoppable. The script's own ancestors, and a `codescout` that is not a mux, are never a `child`. Both exceptions came from the final review of the branch, which showed that the first version would have signalled them.

Third use, 2026-11-12, on a fresh probe worktree with a cold build: the row `sh .../tikv-jemalloc-sys/.../configure`, which was `other` in the second use, was listed as `child`, and the first `--stop` run exited 0 in 8.2 s with the line `signalled <pid> mux with SIGTERM`. A scan took 2.45 s without a `child` row and 3.72 s with one, on a busy machine during the build (the scan timeout is 10 s). Only the mux was signalled in that run: no signal sent to a `child`, and no survivor exit (5), was observed.

A holder that is an ancestor of the script is a blocker too, whatever its kind: `--stop` exits 4 before any signal (bug `786b1826bab82b18`, fixed 2026-11-12, commits in Fix provenance).

## Tests added

- `tests/worktree-remove.sh`: a shell suite (passed=266 assertions on 2026-11-12, with the cases for the ancestor blocker listed in bug `786b1826bab82b18`; fixture `/proc` trees with `cwd`, `cmdline` and `status` files, real-process cases, `git` shims). `tests/worktree_remove_script.rs` (`the_worktree_remove_suite_passes`, Linux only) runs it under `cargo nextest`.
- For the signal lines and the `child` kind: `case_stop_prints_each_signal_it_sends`, `case_a_descendant_of_a_stoppable_holder_is_a_child`, `case_without_a_status_file_a_descendant_stays_other`, `case_an_ancestor_walk_that_cannot_finish_leaves_other`, `case_the_script_parent_never_makes_a_child`, `case_a_stoppable_ancestor_of_the_script_makes_no_child`, `case_a_codescout_server_under_a_stoppable_holder_stays_a_blocker`, `case_the_ancestor_walk_cap_is_exactly_64`, `case_a_descendant_that_is_not_a_holder_is_not_listed` and `case_stop_ends_a_c_compile_child_and_removes` (real processes). The two cases for the script's ancestry and for a non-mux `codescout` failed before their fix (exit 5 and kinds `cargo,child,child` and `cargo,child`).
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

- **SHA:** `652f12d88` (`experiments`)
- **patch-id:** `600069586b7d`
- **SHA:** `1f9b1d9ba` (`experiments`)
- **patch-id:** `7c7098362df0`
- **SHA:** `009955716` (`experiments`)
- **patch-id:** `26c6ee573c39`
- **SHA:** `b176e6655` (`experiments`)
- **patch-id:** `d4065dca2d7c`
- **SHA:** `a1a0b28d2` (`experiments`)
- **patch-id:** `591b08f8c84f`
- **SHA:** `2e3bdbb30` (`experiments`)
- **patch-id:** `65e62ea427a3`
- **SHA:** `2740f4798` (`experiments`)
- **patch-id:** `501ff47461ef`

## Workarounds

Run `scripts/worktree-remove.sh <path>` from outside the worktree and from a session whose cwd is not inside it. Without `--stop` it only lists holders. Ask the occupant before `--stop`. If git refuses, the script lists the untracked files and never forces; for the `.codescout/embeddings` symlink, delete the link itself (`rm <worktree>/.codescout/embeddings`, no trailing slash) and run the script again. Details: `docs/conventions/removing-a-worktree.md`.

## Resume

Open items:

- Fix 2 is closed: no defect found in the superpowers templates (see Fix, item 2).
- The `.codescout/embeddings` symlink is created by two companion hooks (see Root cause). Whether to stop creating it is tracked in the `claude-plugins` issue `docs/issues/2026-11-12-worktree-hooks-link-the-legacy-embeddings-store-into-every-worktree.md`, and the operator must decide it. The `.gitignore` rule is fixed: `.codescout/embeddings` and `crates/*/.codescout/embeddings` have no trailing slash now, so git ignores the symlink too (checked in a scratch repo: the old rule listed it as untracked, the new rule did not).
- The recreated `.codescout` in plan-b is unexplained. A possible cause, not checked: `session-start.mjs` creates `.codescout` as a symlink to the main checkout's directory when a session starts in a worktree that lacks it.
- Parked script follow-ups: scrub UTF-8 C1 control characters (U+0080 to U+009F) from printed command lines (one-line fix `cmd=${cmd//$'\xc2'[$'\x80'-$'\x9f']/ }`); a very large `--wait` overflows the deadline arithmetic (the safe direction); the exit-7 `ls -A` listing prints raw file names; the survivor message says "SIGKILL is never used".
- Check that CI is green on the macOS and Windows lanes (the Rust wrapper is Linux-only; not run locally).
- Open after the `child` work: a SIGTERM sent to a live `child` or `cargo` tree and the survivor exit (5) have not been observed on a real tree; no mutation probe covers the `seen` set or the parent exclusion of the ancestry walk; the `lacks` assertions on the signal lines were never seen failing; the ancestry set stops after 64 steps.

## References

- `.gitignore` lines 7, 80, 210; `src/tools/native_provenance_fence.rs` (`LOCK_NAME`).
- `docs/trackers/tracker-organization-session-log.md` entry W-3 (the work that was cleaned up).
- Memory `worktree-merge-catalog-reconciliation` covers merging worktrees, not removing them.
- `claude-plugins:docs/issues/2026-11-12-worktree-hooks-link-the-legacy-embeddings-store-into-every-worktree.md`: the hooks that create the `.codescout/embeddings` symlink (split out of this bug).
- `docs/issues/archive/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`: the archived bug that calls that link a no-op.

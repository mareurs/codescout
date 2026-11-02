---
id: '35bcf086deeb7cbe'
kind: bug
status: mitigated
title: 'BUG: rust-analyzer''s file watcher walks the 109 directory symlinks in .codescout/resident-private without end, so documentSymbol stays null and RSS passes 15 GB; files.excludeDirs does not stop the walk'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-10-31
opened: 2026-10-31
related:
- docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md
- docs/issues/2026-10-30-references-treats-every-new-mux-connection-as-a-cold-server-and-re-asks-for-5-s.md
severity: high
unverified: 'The move has not been done, so no run shows the nulls ending. That the watcher follows symlinks rests on reading rust-analyzer `master` (`crates/vfs-notify`) and notify 8.2.0 (`follow_symlinks: true` by default), not the shipped 1.97.1 source, plus one subagent measurement of 34,425 inotify watches on `resident-private` directories. The null rate in `usage.db` began on 2026-10-23, before any of these links, so the links explain only part of the nulls.'
---

## Summary

On this checkout, rust-analyzer answers `textDocument/documentSymbol` with null for as long as it runs. Every codescout call that needs symbols then fails. Its file-watcher thread, `notify-rs inoti`, uses about 87% of one core in the kernel and never settles. The mux's rust-analyzer grew from 11 GB to 15 GB RSS in 150 s.

The cause is `.codescout/resident-private/`. It holds 32,136 real directories and 109 symlinks to directories. Most of those links point back into the same tree. The watcher meets the same experiment directories again through each parent. Setting `files.excludeDirs` for this directory does not stop the walk.

## Class fit

The setting is named `files.excludeDirs`, so it reads as a full exclusion of a directory. It removes the directory only from file loading. The watcher still walks it. The guard covers less than its name says. This bug differs from the class's other members in one way: the guard belongs to a third-party tool, and codescout only supplies its input.

## Symptom (Effect)

- `symbols`, `references` and `edit_code` report "the language server answered textDocument/documentSymbol with null … on every attempt".
- The null answer does not end. It lasted 15 minutes in the session where this was found. It lasted 28 asks in a row (280 s) in a standalone run.
- rust-analyzer holds 5 to 15 GB RSS and one core at about 95% CPU.
- The mux's rust-analyzer and the mux itself later vanished. `earlyoom` and the kernel log show no kill. The cause of the exit is not known.

## Reproduction

1. Start `rust-analyzer` with `RA_LOG=info` as a child of your own script. Do not use the mux.
2. Send `initialize` with the repo as `rootUri`, `window.workDoneProgress` and `workspace.workspaceFolders` as capabilities, and `files.excludeDirs` as codescout builds it. This matches the handshake in `src/lsp/mux/process.rs`.
3. Send `initialized` and `didOpen` for `src/tools/run_command/attribution.rs`.
4. Ask `documentSymbol` every 10 s. Read `/proc/<pid>/task/*/stat` every 5 s.
5. Result: the first answer returns symbols at 2 s, before the workspace loads. Every later answer is null from 12 s on.

Single-variable runs on 2026-10-31 (90 to 300 s each, one run per row):

| run | tree | config | watches | `notify-rs inoti` | `documentSymbol` after 12 s |
|---|---|---|---|---|---|
| A2 | real repo | default | 32,582 then 48,816 | busy from 61 s to the end | null in 28 of 28 asks |
| B | real repo, `resident-private` excluded | default | 32.6k then 48,816 | busy | null in 12 of 12 asks |
| G | real repo, `resident-private` excluded | light | 32.6k then 48,816 | busy | null in 8 of 8 asks after the first |
| F | `git archive HEAD` copy | light | 290 | idle from 30 s | symbols in 9 of 9 asks |
| H1 | F plus a copy of `resident-private` with its symlinks | light | 36,218 to 37,111 | busy from about 76 s | null in 8 of 8 asks |
| H2 | H1 with the 109 directory symlinks deleted | light | 32,426, fixed | idle from 31 s | null once at 12 s, then symbols in 7 of 7 asks |
| V1 | real repo after the 109 links were deleted there | default | 48,816, fixed from 30 s | 36%, 21%, 20%, 8% of a core in successive 16 s windows | null until 52 s, then symbols in 6 of 6 asks |

"Light" means `checkOnSave`, `cargo.buildScripts` and `procMacro` are off.

## Environment

- Branch `experiments`, checked at or after `7693bf73d`.
- rust-analyzer 1.97.1 (`8bab26f 2026-07-14`). Linux 7.2.8 on btrfs. `fs.inotify.max_user_watches` is 524,288.
- `.codescout/resident-private/` is in `.gitignore`. It is local experiment evidence. It holds 30 run directories, 278,380 files, 29,154 `.rs` files and 6.7 GB.

## Root cause

Established:

- One thread made 1,000,195 `inotify_add_watch` calls in 87.7 s, about 11,400 per second, under `strace -f`. 1,000,191 of the paths were distinct. 999,428 were under `.codescout/resident-private`. The deepest path had 26 components.
- The tree has 109 directory symlinks. 57 resolve into `matched-workflow-20261031`, 15 into `workflow-transfer-20261030`, 14 into `prechecked-repair-20261030` and 10 into each of `compile-mechanism-r2-20261029` and `compile-mechanism-20261029`. One resolves into the repo's `scripts`. I found no link whose target is an ancestor of the link. I did not test for indirect cycles.
- H1 against H2 changes one thing, the symlinks in a scratch copy. The spin and the null answers disappear with them.
- `files.excludeDirs` lowers RSS (run B measured about 5.4 GB to 756 MB; the controlled pair under Resume measured 10.2 GB to 3.9 GB, so the run B figure does not hold), because rust-analyzer then does not load the 29,154 `.rs` files. It does not lower the watch walk. Runs B, G and D all excluded the directory and still walked it.

Not established:

- That rust-analyzer's `notify` follows symlinks. `notify` 8.2.0 in the cargo registry has a `follow_links` option that its recursive walk passes to `WalkDir`. That is not necessarily the version inside rust-analyzer, and I did not check its setting.
- Why the walk never finishes. A walk with no cycle but heavy fan-out is finite. The 1,000,195 calls were still rising when the run stopped.

## Evidence

- Process data from the live mux server: `notify-rs inoti` at 99.9% CPU, kernel time 253.5 s against user time 27.3 s, while the main `LspServer` thread used 1.6 s in total.
- `strace -f -c` over 120 s: 1,790,921 `statx`, 603,185 `getdents64`, 348,017 `openat`, 296,269 `inotify_add_watch`.
- `src/lsp/init_options.rs` builds `files.excludeDirs` from `git worktree list` under the root. It excludes `.worktrees` whole. `.codescout/resident-private` is not a worktree, so it is not in the list.

## Hypotheses tried

- `earlyoom` or the kernel OOM killer ended the old server. Ruled out for rust-analyzer: `earlyoom` killed one `rustc` at 11:37, and the kernel log shows no kill.
- The inotify limit. Ruled out: 48,812 watches against a limit of 524,288.
- File churn in the repo. Ruled out: a few files changed in 10 s.
- The light config explains F. Ruled out by G, which has the same config and the real tree.
- The size of `resident-private` alone. Partly true for memory, as run B shows. False for the spin, because an exclusion does not remove it.

## Fix

Applied on 2026-10-31 to the data, not to the product. This is a mitigation.

- I removed the 109 directory symlinks from `.codescout/resident-private`. `RESTORE-DIR-SYMLINKS.tsv` and `restore-dir-symlinks.sh` in that directory restore them. I tested the restore on a scratch copy: it rebuilt 109 links identical to the manifest. `README-RESTORE-SYMLINKS.md` there explains the removal. The 146 file symlinks were not touched. Directory and file counts match the counts before the removal, plus the 3 new files.
- Before the delete, no process had its working directory in the tree, and no file had changed in the last 30 minutes.
- Validation run V1 on the real tree used the default config and no exclusion. `documentSymbol` was null until 52 s and returned symbols in 6 of 6 asks from 62 s on. The null window of about 55 s is close to the 53.5 s cold-start window in the archived sibling bug. The watch count stayed at 48,816. `notify-rs inoti` stayed the busiest thread, but its use fell from 36% to 8% of a core over four 16 s windows. Before the fix it stayed near 95%. RSS reached 9.5 GB and stayed flat from 77 s.

The directory is gitignored, so no commit carries the removal. Experiment sessions may create new directory symlinks. No script in this repo creates them.

Not done:

1. DONE 2026-10-31 (product, memory only): `initialization_options` in `src/lsp/init_options.rs` now takes the local-evidence directories that exist under the root. `local_evidence_dirs` returns `.codescout/resident-private` only when it exists, so other projects still send no options. The exclusion lowers RSS; see the controlled pair under Resume (10.2 GB to 3.9 GB). Run B's 5.4 GB to 756 MB was not reproduced. It does not stop the watcher walk or the null answers. The live mux was restarted on the new build on 2026-10-31 and sends the option (captured).
3. DONE 2026-11-01 (repo side; the move itself is NOT done): scratch is to live outside the repository tree. `scripts/resident-scratch.sh <name>` creates and prints `~/.cache/codescout-resident/<repo>/<name>`, keyed on the main checkout. `scripts/resident-scratch-migrate.sh` is a dry run that `--apply` turns into a move of closed directories, with link retargeting and pointer files. The convention is `docs/conventions/resident-scratch-location.md`. Why a move and not more exclusion: one rust-analyzer held 34,425 of its 51,028 inotify watches on `resident-private` directories although `files.excludeDirs` was sent (measured by a subagent, 2026-11-01, matching `/proc/<pid>/fdinfo` inodes against `find`). The hint now names the deepest directory that holds a strict majority of the links, because the first link found was a bystander under `.worktrees`. No real data has been moved: four experiment directories were written on 2026-11-01, and memories name their paths.
2. DONE 2026-11-01 (product, diagnosis only; first shipped 2026-10-31 and did not fire on the real tree, see Resume): `src/lsp/watch_fanout.rs` walks the workspace root without `.gitignore` and without following links. It stops at 1,000,000 entries. The first bound, 50,000, counted files as well as directories and stopped the scan before the first link; commit `824548df5` raised it. When it finds 20 or more directory symlinks, the null-exhausted `documentSymbol` error in `src/lsp/client.rs` adds one sentence to its hint: the count, the first link, and that rust-analyzer has been seen to walk such links without end. It runs on that failure path only, so healthy calls pay nothing. It does not fix the spin. A scan that finds nothing, or stops at its limit with nothing, adds no sentence and proves no absence.
3. Product: avoid rust-analyzer's server-side watcher. This needs a design decision and is untested.

## Tests added

Six tests in `src/lsp/init_options.rs` cover the exclusion list: the directory alone, with worktrees, named twice, outside the root or the root itself, a server that is not rust-analyzer, and `local_evidence_dirs` on a missing and an existing directory. Two mutants went red: no `is_dir` check, and no root guard. Sixteen tests in `src/lsp/watch_fanout.rs` (one puts 60,000 files in the root before 25 links, and was red on the old 50,000-entry bound with `find` returning `None`; four are for the majority rule: links spread over leaf directories, a bystander, exactly half, the root itself; five mutants of that rule were run with `./scripts/mutation-probe.sh`, four killed by tests and one, "top-level only", hung until killed because its loop never ends), 16 in `tests/test_resident_scratch.py` for the two scripts in a disposable repo (ten mutants of the scripts, all caught; the file runs in the light Python lane of `./scripts/gate.sh`), and two in `src/lsp/client.rs` (a null answer with 25 links names them; a control without links keeps the old hint) cover the diagnosis. Six mutants run with `./scripts/mutation-probe.sh` were killed: re-entering `.git`, counting every symlink, no entry bound, no minimum, losing the lower bound, and a probe that never finds. All tests cover the exclusion list and the diagnosis, not the spin. The spin has no regression test: it needs a decision on the fix first.

## Workarounds

- Adding `.codescout/resident-private` to `files.excludeDirs` lowers memory by about 7 times. It does not stop the spin or the null answers.
- Deleting the directory symlinks ended the spin in a scratch copy (H2). It was applied to the real tree on 2026-10-31. See Fix.

## Resume
- RE-MEASURE ON A FRESH SERVER, 2026-11-02 (session `1a35461f-23c2-41e6-ba35-052481f7dd96`, after an `/mcp` reconnect; checkout at `8302a1f04`): a `symbols` call started a new rust-analyzer (pid 679273) on the real root, and a read-only script sampled it at 251 s of age. RSS 4,087,876 kB (about 4.09 GB), 109 threads, 16,541 inotify watches. A 10 s per-thread CPU sample: `LspServer` 8.5% of one core, `notify-rs inoti` 0.8%, `VfsLoader` 0.5%. `find .codescout/resident-private -type l -xtype d` found 0. `symbols` on `src/lsp/watch_fanout.rs` returned 7 symbols and 17 tests, and a second call returned `probe` at lines 112 to 118; neither was null.
  - Against the 2026-11-01 sample below: RSS is within 2% (4,158 MB then), and the watches are 874 higher (15,667 then). The cause of the extra watches was not looked into.
  - One sample after one restart. It does not show that the nulls stay away under load or over hours, so the record stays `mitigated`.
  - Method limits: the script reads `/proc/<pid>/status`, `fdinfo` and `task/*/stat`, and matches the process by the name `rust-analyzer` with the real root as its cwd, first match only. Its CPU figures are per thread name; the 2026-11-01 figure was a total over all threads, so the two are not directly comparable.

- FRESH MUX AFTER THE MOVE, 2026-11-01 (operator approved killing mux 972038): the new rust-analyzer (pid 2316124) at 254 s held 4,158 MB RSS, flat over a 10 s sample, with 15,667 inotify watches. `documentSymbol` for `src/lsp/client.rs` returned 11 symbols with ranges, not null. CPU in that 10 s was 270 ticks, about 27% of one core, `LspServer` 96 ticks, the rest spread over rayon workers; no `notify-rs` thread was in the top five. The old process had held 16 GB. One sample after one restart: it does not show that the nulls stay away under load or over hours. The record stays `mitigated`: two scratch directories stayed in-tree, new in-tree scratch is not prevented by the repo, and the nulls that began on 2026-10-23 had another cause (nested worktrees).

- THE MOVE, 2026-11-01 (about 15:40 local): with the operator's go-ahead, `scripts/resident-scratch-migrate.sh --apply --force-referenced` moved 34 directories from `.codescout/resident-private` to `/home/marius/.cache/codescout-resident/codescout/`. Two stayed because a file changed in the last hour: `blind-challenger-20261101` and `worktree-cleanup-20261101`. Links were NOT retargeted (7,294 links inside moved directories keep their old absolute target and dangle), because a seal can cover link text. Memory paths were updated first, in 15 files (6 committed at `2e5112a83`; the rest are untracked or carry a peer's uncommitted edits and are left uncommitted). A pointer file `<name>.moved` is left at each old path.
  - In-tree directory links after the move: 0 (was 1,918). Another session had also written `README-RESTORE-SYMLINKS.md`, `restore-dir-symlinks.sh` and `RESTORE-DIR-SYMLINKS.tsv` into the tree; they are not mine.
  - Sealed directories: link counts unchanged (`fixed-checker-transfer-20261101` 71, `fixed-acceptance-population-20261101` 0). File counts exceed the memory's seal counts by 2 each beyond `__pycache__` (700 vs 698, 3,094 vs 3,092); the extras are verification files dated 12:26 to 12:36 today, before the move. `closure.py verify` was NOT run, because it writes into a sealed directory.
  - Controlled probes after the move, same method as above: with NO exclusion option rust-analyzer settled at 4.26 GB from 30 s (was 10.2 GB, 150 s before the move); with the option, 4.16 GB. So the move gives what the exclusion gave, and the option no longer matters once the scratch is out of the tree.
  - Live rust-analyzer (pid 972205, started before the move): 16 GB RSS, 15,606 inotify watches, about 12% of one core, `LspServer` the top thread. It has not been restarted, so it says nothing about the new state.
  - Notified: Codex thread `01a15ca9-f939-7603-acb5-01c6b02f68e2` (rollout grew) and the Claude session "Codex effort steering experiment branch" (pid 3944213). Not checked: whether any running experiment reads an old path.
  - Not done: the 5 stale call-graph edges under `resident-private`; a restart of the shared mux; retargeting links.

- Check, 2026-11-01, live hint: on a rust-analyzer that answered null for `src/lsp/client.rs` and `src/lsp/watch_fanout.rs`, the warning carried the base hint with no sentence about directory symlinks, although `.codescout/resident-private` held 1,876 directory symlinks (`find -type l -xtype d`). Cause: the 50,000-entry bound. A read-only Python replay of `find` on the real root: bound 50,000 gave 50,000 visited, truncated, 0 links (so `find` returns `None`); no bound gave 311,890 entries, 1,879 links, first link at entry 97,906, in 1.2 s. The replay is not the Rust function, and the order of directory entries is assumed equal. The Rust test with 60,000 root files confirms the same outcome for the old bound. Fixed at `824548df5`.
  - Not checked: the new hint on a live null answer. The running server needs a rebuild and `/mcp` first, and the mux must restart if it still runs the old image.
  - The null answer itself was present at that time on a 1,837 s old rust-analyzer with 5.0 GB RSS and `notify-rs inoti` at about 1.7% of a core. Its cause is not found.

- Check, 2026-10-31, after the rebuild and a kill of the old mux (the mux outlives `/mcp`, so a reconnect alone left the old rust-analyzer running):
  - Captured the `initialize` request of a fresh mux on the real root. It carries `"initializationOptions":{"files":{"excludeDirs":[".codescout/resident-private",".worktrees"]}}`.
  - Controlled pair, two rust-analyzer processes started directly at the same time on the real root, same binary: no option settled at 10.2 GB RSS after 180 s; with the option it settled at 3.9 GB after 60 s. Both reported `Loaded 463 packages`.
  - The live server's rust-analyzer held 5.0 GB at 253 s, between the two. Not explained.
  - 10 s CPU sample of the live process: `LspServer` about 9.5% of a core, no `notify-rs` thread in the top five. Sampler joined threads by name, so the split is approximate. The watcher spin was not seen.
  - `resident-private` held 146 symlinks and 29,157 real `.rs` files at that time.
  - Not checked: that rust-analyzer's status text names the exclusion (only its first 25 lines were saved); whether the spin returns under load.

State on 2026-10-31: mitigated. The 109 links are gone from the real tree, and V1 shows a finite null window and a quiet watcher.

Open:

- V1 is one run of 120 s. The watcher was still at 8% of a core at the end. Check it again over a longer run.
  - Longer check, 2026-10-31, on the live mux server after `/mcp` reconnect. One `rust-analyzer` process had run 1622 s, with 9.2 GB RSS and 109 threads.
  - A 10 s sample of `/proc/<pid>/task/*/stat` showed `notify-rs` at 2 ticks, which is 0.2% of one core (`CLK_TCK` is 100). The `LspServer` thread used 125 ticks, which is 12.5% of one core.
  - `symbols` on `src/tools/symbol/symbols.rs` returned 34 symbols with signatures. `references` on `LspLegHealth/note_start_failure` returned 7 locations. Neither returned the null error.
  - `find .codescout/resident-private -type l -xtype d` found no directory symlink, so none came back.
  - This is one sample, 27 minutes after the start. It shows the watcher quiet at that moment. It does not show that new links cannot bring the spin back.
- The mitigation is local and unversioned. A new experiment can bring the links back. Product options 1 to 3 under Fix are the durable answer, and none is chosen.
- The bug stays `mitigated`, not `fixed`: no product change and no regression test exist.


**Recurred, 2026-11-01 (session `76494b3d-4b99-49c6-9924-4fd3a7cee027`).** The mitigation no longer holds.

- `symbols` and `edit_code` on `src/tools/grep.rs` returned the null error on every attempt for more than 3 minutes after a `/mcp` reconnect.
- The live rust-analyzer (pid 2435941, cwd the real root, 1.5 GB RSS) had one thread, `notify-rs inoti`, at 98.1% of a core in `top -H`.
- `find .codescout/resident-private -type l -xtype d` counts 1,876 directory symlinks: 1,806 with an mtime of 2026-10-31 and 70 of 2026-11-01. The longer check above, also 2026-10-31, had found none, so they came later that day.
- They come from the effort-steering experiment trees. Example: `fixed-checker-transfer-20261101/review/mutant06/matched-workflow-20261031 -> /home/marius/work/claude/codescout/.codescout/resident-private/matched-workflow-20261031` (11:12 today).
- Not done: I did not delete or move them. They are another session's experiment data. Not checked: whether the experiment scripts that create them live in this repo.

## References

- `docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md`: the same family. Nested worktrees stretched the null window. The fix there was the exclusion in `src/lsp/init_options.rs`.
- `docs/issues/2026-10-30-references-treats-every-new-mux-connection-as-a-cold-server-and-re-asks-for-5-s.md`
- `src/lsp/init_options.rs`, `src/lsp/mux/process.rs`

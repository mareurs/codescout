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
unverified: That rust-analyzer's notify watcher follows symlinks is inferred from the path shape and from one run per variant (H1, H2, V1). Its source was not read. The mitigation is local and unversioned, V1 ran 120 s, and no product fix or regression test exists.
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
- `files.excludeDirs` lowers RSS from about 5.4 GB to 756 MB, because rust-analyzer then does not load the 29,154 `.rs` files. It does not lower the watch walk. Runs B, G and D all excluded the directory and still walked it.

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

1. Product: add `.codescout/resident-private` to `files.excludeDirs` in `src/lsp/init_options.rs`. In run B the exclusion cut RSS from about 5.4 GB to 756 MB. V1 holds 9.5 GB because rust-analyzer loads the 29,154 `.rs` files.
2. Product: warn when a directory symlink fan-out sits under the workspace root.
3. Product: avoid rust-analyzer's server-side watcher. This needs a design decision and is untested.

## Tests added

N/A. Nothing is fixed. A regression test needs a decision on the fix first.

## Workarounds

- Adding `.codescout/resident-private` to `files.excludeDirs` lowers memory by about 7 times. It does not stop the spin or the null answers.
- Deleting the directory symlinks ended the spin in a scratch copy (H2). It was applied to the real tree on 2026-10-31. See Fix.

## Resume

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

## References

- `docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md`: the same family. Nested worktrees stretched the null window. The fix there was the exclusion in `src/lsp/init_options.rs`.
- `docs/issues/2026-10-30-references-treats-every-new-mux-connection-as-a-cold-server-and-re-asks-for-5-s.md`
- `src/lsp/init_options.rs`, `src/lsp/mux/process.rs`

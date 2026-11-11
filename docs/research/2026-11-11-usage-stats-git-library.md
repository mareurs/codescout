---
kind: research
status: active
title: "Usage statistics — do we need libgit2 (git2), the git CLI, or gix"
tags:
  - usage-stats
  - frictions
  - git
---

Archived analyst report from the 2026-11-11 statistics pass. The summary and the decisions are in [the statistics tracker](../trackers/frictions/statistics.md). The scripts named below are in `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz`. They read pickles and a scratch directory that no longer exist, so treat them as method, not as a rerunnable job.

# Do we need libgit2 (git2 crate) in codescout, or the `git` CLI, or gix?

Read-only analysis, 2026-11-11. Repo: /home/marius/work/claude/codescout (HEAD ad5fbcfc9 at start; peers commit while I measured).
Machine: 64 cores, git 2.56.0, hyperfine 2.0.0, cargo 1.99.1, load average during the final benchmark batch: 10.04 before, 11.54 after (both below 12).
Every number below was measured in this session unless it says "read from source" or "hypothesis".

All scratch files are in /home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/2e6121be-ee04-4e7a-974f-3717997500f4/scratchpad/ (called $SP below), prefix `git_`.

---------------------------------------------------------------------------------------------------

## 1. INVENTORY

Method: `grep -rn git2` and `grep -rn 'Command::new("git")'` over src/, crates/, tests/; production vs test split by the first `#[cfg(test)] mod` line (script $SP/git_count.py; it mis-classified src/agent/mod.rs because that file has `#[cfg(test)] mod x;` declarations at line 4-6, corrected by hand below). Each production body was read with read_file/symbols this session.

### Counts

| what | production | test-only | comment lines | total |
|---|---|---|---|---|
| lines containing `git2::` (21 files) | 32 | 62 | 3 | 97 |
| distinct production functions that open a libgit2 repo | 18 (one of them, `git::diff_tree_to_tree`, has no production caller: only its own 4 tests) | - | - | - |
| `Command::new("git")` lines in src/ (14 files) | 5 | 20 | 0 | 25 |
| `Command::new("git")` lines in tests/ (integration tests) | 0 | 22 | 0 | 22 |

The user's figure "93 git2 sites" is close to my 94 code lines (97 lines minus 3 comments); the 1-line difference is regex and counting convention, not checked line by line. The 25 src CLI sites match exactly: 5 production, 20 test.

Production CLI sites are only these 5 lines, but they spawn more than 5 processes per use: observe.rs runs 5 spawns per observed call, merge_worktree runs 2.

### Production git2 sites (18 functions)

| # | file:line | enclosing symbol | git operation | purpose | frequency class |
|---|---|---|---|---|---|
| 1 | src/observations/store.rs:112 | `checkout` | `Repository::discover` + `workdir()` + canonicalize | map a path to its checkout root (observation store key) | per tool call, Linux only. Called from `dispatch_at` (2 calls in the mismatch test when `context.root` is set), `Request::emit` (1) and `store()` (1) = at least 4 discovers per call; more for the terminal event (read from source, not traced at runtime) |
| 2 | src/observations/store/policy.rs:17 | `directory` | `discover` + `repo.path()` canonicalize | find the git dir of the per-checkout policy latch (linked worktree uses its own git dir) | per observation store open / policy load (once per store per process, plus `load` per `ensure`) |
| 3 | src/agent/mod.rs:925 | `resolve_head_sha` | `open` + `revparse_single("HEAD")` + `short_id()` | HEAD short sha for project activation (WIN-14) | once per workspace activation (3 call sites: lines 1047, 1190, 1967) |
| 4 | src/agent/mod.rs:934 | `probe_has_git_remote` | `open` + `remotes()` | cache `has_git_remote` on `ActiveProject` | once per workspace activation (lines 333, 1048, 1984) |
| 5 | src/retrieval/index_state.rs:260 | `head_commit_full` | `discover` + `head()` + `peel_to_commit` | HEAD oid written into `.codescout/index-state.json` and compared | per index write, per `git_sync_status` |
| 6 | src/retrieval/index_state.rs:376 | `behind_count` | `discover` + `graph_ahead_behind` | how many commits the index is behind | only when HEAD != indexed commit; callers: `workspace(status)` (tools/config/mod.rs:677), semantic/index.rs:896, :1081 |
| 7 | src/git/mod.rs:8 (via dashboard/api/project.rs:46) | `open_repo` -> `git_info` | `discover` + `head()` + `statuses(None)` | dashboard shows branch and `dirty` | per dashboard project-API request (feature `dashboard`) |
| 8 | src/git/mod.rs:33-57 | `diff_tree_to_tree` | `diff_tree_to_tree` + `find_similar(renames)` | name-status diff between two commits | NONE in production (grep + references: only its 4 tests call it) |
| 9 | src/librarian/tools/doctor.rs:7751 | `scan_archived_fix_sha_unresolvable` | `open` + `revparse_single(sha)` per pointer | is a declared fix SHA still an object | per `librarian doctor` (55 calls in usage.db, avg 6,967 ms) |
| 10 | src/librarian/tools/doctor.rs:8305-8350 | `paths_touched_since` | `revwalk` + `Sort::TIME` + `find_commit` + `diff_tree_to_tree` per in-window commit, cap 4000 commits | which cited files changed recently | per `doctor` (called at doctor.rs:8589 with a pathspec of citing files) |
| 11 | src/librarian/tools/doctor.rs:8455 | `scan_open_bug_cited_from_source_walking` | `open` (hands repo to #10) | as #10 | per `doctor` |
| 12 | src/librarian/catalog/gc.rs:188 | `detect_move_candidates` | `open` + `revwalk` + `push_head` (all reachable commits, oids only) | detect "repo was moved here" | per `doctor` (doctor.rs:1280) |
| 13 | src/librarian/tools/reindex.rs:198 | `backfill_commits` | `open` + `revwalk` TOPOLOGICAL\|REVERSE + `find_commit` for every commit | fill the `commits` table | per reindex target (reindex.rs:748; 25 `reindex` calls in usage.db, avg 26,294 ms) |
| 14 | src/librarian/tools/audit_doc_refs/mod.rs:793 | `git_head_commit` | `open` + `head()` + `peel_to_commit` | 8-char HEAD for the audit record | per `audit_doc_refs` (27 calls, avg 4,896 ms) |
| 15 | src/librarian/tools/audit_doc_refs/mod.rs:886 | `build_tracked_dirs` | `open` + `index()` + iterate every entry | tracked-directory set for the gitignore check (replaces a `git ls-files` subprocess by design, see docs/issues/archive/2026-08-06-audit-doc-refs-gitignore-cap-ignores-the-index.md) | per `audit_doc_refs` |
| 16 | src/librarian/tools/legibility_scan/mod.rs:611 | `git_head` | `open` + `revparse_single("HEAD")` | full HEAD sha | per legibility scan (rare/manual) |
| 17 | src/librarian/tools/gather.rs:311 | `gather_git_log` | `discover` + `find_branch` + `revwalk` + `Sort::TIME` + `find_commit` | `doc(gather)` git_log source, default limit 20 | per gather with a git_log source (1 `doc gather` call in usage.db) |
| 18 | src/librarian/tools/gather.rs:533 | `last_changed` | `open` + `blame_file` | last-change commit/time of a path | per gathered config path (rare) |

### Production CLI sites (5 lines)

| # | file:line | enclosing symbol | git operation | purpose | frequency class |
|---|---|---|---|---|---|
| A | src/tools/native_provenance_fence.rs:192 | `canonical_checkout` | `git -C dir rev-parse --show-toplevel` | canonical checkout root for the provenance fence | per call: `provenance_fence_context::scope` (src/tools/provenance_fence_context.rs:169) runs for every `run_command` and every write-class call, `#[cfg(unix)]` only. In usage.db that is at least 51,365 run_command + 16,810 edit_file/edit_code/create_file = 68,175 of 118,278 rows (57.6%), more once `doc` writes count (read from source, not traced at runtime) |
| B | src/tools/run_command/observe.rs:129 (helper `git`, used 3 ways) | `toplevel`, `fingerprint` | `rev-parse --show-toplevel` (1x), then `rev-parse HEAD` + `status --porcelain=v1 -z --no-renames --untracked-files=all` before AND after (4x) = 5 spawns, env GIT_OPTIONAL_LOCKS=0, 2 s kill timeout each, tokio::process | detect "tree changed while a call declared `read`" | per `run_command` that declared `effect=read`, foreground, with no @cmd/@file/@tool/@bg handle. usage.db: 19,204 read-declared rows, 11,659 carry `observed_changed` (22.7% of 51,365 run_command rows). Not cfg-gated, so also Windows |
| C | src/librarian/tools/delete.rs:259 | `file_git_state` | `git -C dir status --porcelain=v1 --ignored -- name` | what git can restore if the file is deleted (dry run) | per `doc delete` dry run (12 `delete` calls in usage.db). Synchronous on purpose (catalog mutex held) |
| D | src/librarian/tools/mv.rs:29 | `files_mentioning` | `git -C root grep --untracked -l -F -e stem` | files whose text cites a moved doc | per `doc move` (235 calls, avg 447 ms), 2-3 spawns per call (lines 112, 117, 569) |
| E | src/librarian/tools/merge_worktree.rs:175 | `check_rebase_invariant` | `git -C main merge-base --is-ancestor` x2 | rebase-before-merge check | per `librarian merge_worktree` (39 calls, avg 55 ms) |

### `run_command` observe.rs and attribution.rs: what git work runs per call, by what mechanism

observe.rs (read from source, lines 1-232): only for calls that declared `effect=read`, are foreground and name no buffer handle. Mechanism: the `git` CLI via tokio::process, 5 spawns (1 toplevel + HEAD + status before, HEAD + status after). It is not libgit2. Sum of measured medians on this repo at low load: 1.47 + 2 x (1.73 + 22.2) = 49.3 ms of spawn+git work per observed call. Interaction with usage.db: observed-read rows have p50 108 ms against 40-60 ms for unobserved-read rows (confounded: the unobserved rows include buffer queries; not a clean estimate).

attribution.rs (read from source, `wip_author_diagnostic_in`): NOT on every call. Stage 0 returns immediately when exit_code == 0 and the output has nothing failure-shaped, and when disabled. Otherwise it spawns `python3` on an embedded `scripts/attribute-red.py`, which itself runs `git -C root status --porcelain` through Python `subprocess` (attribute-red.py:116). So there is no git2 and no Rust-side git spawn in attribution: the git process belongs to the Python engine. usage.db (rows with attribution_ms, since 2026-11-01): 1,727 of 51,365 run_command rows (3.4%). attribution_ms: silent n=1,549 p50 150 ms, p90 247 ms, p99 737 ms; hinted n=177 p50 8,104 ms, p90 16,704 ms, p99 27,595 ms (the transcript scan, not git); timed_out n=1 at 30,012 ms. Share of the call latency (attribution_ms / latency_ms): p50 0.56, p90 0.91 over all 1,727 rows; for short commands (latency minus attribution under 1 s, n=1,335): attribution p50 151 ms of latency p50 273 ms, share p50 0.67. Context: `python3 -I -c pass` alone is 74.1 ms and `git status --porcelain` is 20.3 ms (hyperfine, 20 runs), so for a silent attribution the git part is about 14% and Python start-up dominates (hypothesis for the rest: engine imports).

### The five hottest production paths, by frequency

1. Provenance fence `canonical_checkout`: 1 CLI spawn per run_command / write-class call (>= 68,175 rows of 118,278), Unix only.
2. Observation store `checkout`: >= 4 libgit2 discovers per tool call on Linux (every call, 118,278 rows).
3. `run_command` observe: 5 CLI spawns per read-declared call (11,659 rows observed).
4. Activation: 2 libgit2 opens + revparse per `workspace(activate)` (430 activate rows + 189 rows with no action) and `head_commit_full` per index write / status.
5. Attribution engine (Python + `git status`): per red `run_command` (1,727 rows).
Warm paths: `librarian doctor` (history walk x2), `audit_doc_refs` (index read + HEAD), `doc move` (git grep), `workspace(status)` (git_sync_status).

---------------------------------------------------------------------------------------------------

## 2. WHY BOTH

### History (git log, docs, memories)

- `git2 = "0.20"` has been in Cargo.toml since the initial commit 01842913c (2026-02-25), and survived the librarian-mcp merge d48bf9922 (2026-05-16). No ADR chooses libgit2 over the CLI (grep of docs/adrs and docs/conventions: none).
- The recorded decision is a rule, not an ADR: WIN-14 in docs/trackers/windows-platform-support.md:82, commit dd3bff8f7 / bcc712ae (2026-06-09), "fix(agent): resolve_head_sha via libgit2, no git spawn". Quote (src/agent/mod.rs:919-923): "Uses libgit2 (no subprocess): on this project's locked-down Windows VDI, every `CreateProcessW` is taxed by EDR injection, and a raw `git rev-parse` with no timeout could hang activation outright." Same text in src/librarian/tools/legibility_scan/mod.rs:605-608. The tracker (lines 22, 50) names the primary driver as a locked-down VDI whose EDR/AV injects into spawned processes. docs/adrs/2026-07-25-embedding-transport-boundary.md:58 calls it "git-spawn elimination" and notes `legibility_scan::git_head` was left unconverted for six days. docs/trackers/capability-proposals.md:766-767: "`resolve_head_sha` and `probe_has_git_remote` already use `git2`, so the dependency exists; do not shell out."
- So libgit2 is the project's deliberate no-subprocess channel, and the CLI sites are later additions where a library cannot do the job (`git grep` in mv.rs has no libgit2 equivalent; `status` with `GIT_OPTIONAL_LOCKS=0` and a kill-on-timeout in observe.rs; oldest hit in the first 15 results of `git log -S 'Command::new("git")' -- src` is merge_worktree 365c2df81, 2026-07-17; I did not page further back).
- No per-spawn EDR cost is recorded anywhere I found (grep of the tracker, spec and session log): the premise is documented, not measured here.

### Defects attributable to libgit2

- usage.db (ids <= 299175, 118,278 rows, 2026-10-12..2026-11-10): `error_msg LIKE '%git2%' OR '%libgit2%' OR '%class=%code=%'` outside run_command: 0 rows. err_family values touching git or repo: 0 (the 47 families are il3_*, path_not_found, worktree_activate_required, etc.; none is a git library failure). `worktree_activate_required` (132 rows) is codescout's own gate, not libgit2.
- Docs: no issue names a libgit2 defect (ownership, safe.directory, worktree quirk, status slowness). Closest: docs/issues/archive 2026-07-17-catalog-dead-rows-no-gc mentions "git2 binding error ... swallowed" as a hypothetical cause only. Performance, not correctness: .codescout/memories/research/doctor-latency-20261109.md records "The git history walk (24.3% of the first profile)" (52.2 G instructions) and "Whether turning off libgit2 strict hash verification would cut the (now smaller) walk. Not tried." The fix e9b4bb7b2 made the walk ask only about citing files. docs/issues/2026-10-16-doctor-recently-touched-walk-returns-a-bare-set-at-its-cap.md: walk hits its 4,000-commit ceiling every call (5,582 commits against 4,000 on 2026-09-09).
- NEW, found by me (not in docs/issues or docs/trackers, grep of git_info and "dirty"): `dashboard/api/project.rs::git_info` computes `dirty = repo.statuses(None)...!s.is_empty()`. `statuses(None)` includes ignored files. Verified with a scratch repo ($SP/git_sem/ignoredonly: a committed `.gitignore` with `target/` and an ignored `target/junk.o`): CLI `git status --porcelain` is empty, libgit2 `statuses(None)` returns `1 entries [Status(IGNORED):target/]`, so the dashboard reports a clean checkout as dirty whenever any ignored dir exists. Also on a sparse checkout it reports `WT_DELETED` for out-of-cone files (section 4).

### Defects attributable to CLI spawning

- WIN-14 itself (EDR spawn tax, unbounded `.output()` with no timeout); WIN-1 (run_command spawn hang under EDR), WIN-2 (taskkill/tasklist spawns replaced by Win32 calls), WIN-15 (GPU probe spawn) are all the same hazard class (docs/trackers/windows-platform-support.md:69-83).
- `index.lock` contention: observe.rs sets GIT_OPTIONAL_LOCKS=0 so `git status` does not take index.lock against a peer's commit (docs/issues/archive/2026-09-24-residual-run-command-observed-effect-class.md:73-74; test `a_snapshot_never_rewrites_the_index_it_is_observing`).
- Missing binary: no production preflight for `git` exists (grep: nothing in src/embed/preflight.rs or src/agent/mod.rs; the `Command::new("git")` there are tests). Every production CLI site fails soft: observe -> no observation; delete -> `"unknown"`; mv -> `None` ("UNCOVERED" comment at mv.rs:35-45 admits the spawn-failure branch is untested); merge_worktree -> skips the check; fence -> `canonical_checkout(root).or_else(|_| root.canonicalize())`. Many attribution tests self-skip with "skipping: git unavailable". Windows under wine: ci.yml:896-906 says the wine profile's WINEPATH holds only git's `cmd`, not `usr\bin`.
- `git grep` exit-code trap (mv.rs:36-48): exit 1 is a real zero, any other non-zero is a failed scan.

---------------------------------------------------------------------------------------------------

## 3. RUNTIME COST (benchmark)

Scratch crate: $SP/git_bench (git2 0.20.4 resolved to libgit2-sys 0.18.8+1.9.7 offline; the repo lock has libgit2-sys 0.18.3+1.9.2. Features: default-features=false, vendored-libgit2, same as the repo). Release build. 3 warm-ups then 20 iterations (10 for the heavy rows, marked n). libgit2 rows include `Repository::open` per iteration (as production does); CLI rows are `std::process::Command::output()` with stdin null, so spawn is included. CLI runs set GIT_OPTIONAL_LOCKS=0.
Target: this repo (9,271 commits, 4,339 loose + 81,704 packed objects in 3 packs, 43 linked worktrees, a split commit-graph chain in .git/objects/info/commit-graphs/, no fsmonitor / untrackedCache configured: `git config --get-regexp` found nothing).
Commands: `bash $SP/git_runbench.sh <label> [filter]`, output $SP/git_bench_<label>.txt.

### Authoritative batch: R2all (load average 10.04 before, 11.54 after; file $SP/git_bench_R2all.txt)

| operation | git2 median / p90 ms | CLI median / p90 ms | CLI command | ratio (CLI/git2) |
|---|---|---|---|---|
| discover (from subdir) + workdir | 0.17 / 0.18 | 1.47 / 1.87 | rev-parse --show-toplevel | 8.6x |
| open(root) | 0.14 / 0.15 | - | - | - |
| HEAD full oid (open + revparse) | 0.26 / 0.28 | 1.73 / 1.96 | rev-parse HEAD | 6.7x |
| HEAD short id | 0.32 / 0.35 | 1.83 / 2.03 | rev-parse --short HEAD | 5.7x |
| remotes (probe_has_git_remote) | 0.17 / 0.18 | 1.75 / 2.05 | git remote | 10x |
| config read user.name | 0.14 / 0.15 | 1.48 / 1.72 | config --get | 10.6x |
| worktree list (39 linked worktrees at that moment) | 0.38 / 0.39 | 4.31 / 4.54 | worktree list --porcelain | 11x |
| is_path_ignored | 0.25 / 0.27 | 2.89 / 3.19 | check-ignore -q | 11.6x |
| index read + iterate all entries (468k bytes of paths) | 5.80 / 6.23 | 3.90 / 4.14 | ls-files -z | 0.67x (CLI faster) |
| ahead/behind HEAD vs HEAD~50 | 1.47 / 1.73 | 3.83 / 4.17 | rev-list --count b..h | 2.6x |
| is-ancestor (merge_worktree uses CLI) | 1.44 / 1.49 | 3.67 / 4.38 | merge-base --is-ancestor | 2.5x |
| status incl. untracked, default opts (dashboard `statuses(None)`) | 51.10 / 53.28 | 22.52 / 23.25 | status --porcelain=v2 -z --untracked-files=all | 0.44x (CLI 2.3x faster) |
| status incl. untracked, no ignored | 49.53 / 50.77 | 22.22 / 25.66 | status v1 with observe.rs flags | 0.45x |
| status tracked only | - | 5.74 / 5.96 | status -uno | - |
| status of one file (delete dry run) | 6.53 / 7.17 | 5.23 / 5.67 | status --porcelain=v1 --ignored -- file | 0.8x |
| tree diff HEAD~5..HEAD (no renames) | 9.40 / 9.84 | 3.98 / 4.17 | diff-tree -r --name-status -z | 0.42x |
| tree diff HEAD~5..HEAD (+renames, as git/mod.rs) | 9.51 / 10.12 | 4.01 / 4.25 | + -M | 0.42x |
| tree diff HEAD~200..HEAD (no renames) | 9.66 / 10.32 | 9.86 / 10.46 | diff-tree -r --name-status -z | 1.0x |
| tree diff HEAD~200..HEAD (+renames) | 263.51 / 278.54 | 76.53 / 80.81 | + -M | 0.29x |
| revwalk last 200 commits (Sort::TIME, +summary, as gather.rs) | 196.52 / 199.89 | 3.94 / 4.45 | log -n 200 --format=%H | 0.02x (CLI 50x faster) |
| revwalk ALL commits oids only (gc.rs) | 193.82 / 197.12 | 15.00 / 16.11 | rev-list HEAD | 0.08x |
| revwalk ALL commits TOPO\|REVERSE + find_commit (reindex backfill) n=10 | 244.44 / 255.38 | 130.96 / 135.45 | log --topo-order --reverse --format=%H%x00%ct%x00%s | 0.54x |
| doctor-like walk: <=4000 commits, diff in-window vs parent, NO pathspec, n=10 | 1743.95 / 1766.39 | 116.35 / 121.67 | log -n 4000 --since=7.days.ago --name-only --format= -z | 0.07x (see note) |
| blame_file src/librarian/tools/gather.rs, n=10 | 1980.55 / 2008.40 | 19.45 / 19.84 | blame --porcelain | 0.01x (CLI 100x faster) |

Note on the doctor row: production passes a pathspec of citing files (doctor.rs:8589, e9b4bb7b2), which my row does not; the Sort::TIME fixed cost below is the part that survives. The pathspec variant was not measured.

Revwalk scaling (R2all, n=10, git2 oids only vs CLI): Sort::TIME takes 193.85 / 191.14 / 196.82 / 195.66 / 195.38 ms for the first 1 / 20 / 200 / 1000 / 5000 commits, i.e. a constant ~195 ms regardless of N. Unsorted (`Sort::NONE`) takes 0.36 / 0.77 / 5.21 / 19.68 / 113.19 ms. CLI `log -n N`: 1.95 / 2.51 / 3.77 / 5.67 / 11.29 ms. So with Sort::TIME libgit2 pays the full-history cost even to read one commit (hypothesis for the mechanism: it walks the whole graph eagerly under TIME sorting; not proven, I did not read libgit2's revwalk.c). `gather_git_log` (default limit 20) and `paths_touched_since` both use Sort::TIME.

Pure spawn floor (R2all): 50 sequential `git --version` = 61.8 ms total = 1.24 ms/spawn; 50 sequential `git rev-parse --git-dir` in this repo = 70.0 ms = 1.40 ms/spawn; 50 sequential `/bin/true` = 28.1 ms = 0.56 ms/spawn. `git rev-parse HEAD` in /tmp (not a repo) median 1.30 ms.

Earlier batches A-M (all at load average 40-117, i.e. above 12) were repeated as R2all at load ~10-11; ratios agreed (e.g. discover 0.19 vs 2.02 ms, status 83.7 vs 56.6 ms, revwalk 200 213 vs 4.2 ms, blame 2129 vs 21.5 ms). Files $SP/git_bench_A.txt .. M.txt are the superseded runs.

### Follow-up experiments

- libgit2 strict hash verification off (`git2::opts::strict_hash_verification(false)`, process-global; load 40-48, so only the ratio counts): revwalk TIME 200 commits 213 -> 151 ms, ALL commits 208 -> 148 ms, blame 2129 -> 1312 ms, doctor-like walk 1935 -> 1243 ms, tree diff HEAD~5 11.8 -> 8.5 ms. About 27-38% better, still 30-37x slower than the CLI on the 200-commit walk. ($SP/git_bench_G2nostrict.txt, H2nostrict, D2nostrict)
- Commit-graph (hyperfine, 20 runs): `git log -n 200` 4.3 ms with commit-graph vs 7.3 ms with `-c core.commitGraph=false`; `git rev-list HEAD` 12.5 ms vs 127.5 ms (memory 8.7 vs 36.8 MiB). The repo has a split commit-graph chain; libgit2's all-commits walk (194 ms) is close to git's no-graph walk (127.5 ms), which suggests, but does not prove, that libgit2 gets no benefit from the graph chain. ($SP/git_hf_commitgraph.json)
- Linked worktree, real case (/home/marius/work/claude/codescout/.worktrees/ecc-bfs-identity-20261110): git2 HEAD 0.36 ms vs CLI 1.68 ms; status incl. untracked git2 37.37 ms vs CLI 16.05 ms; `status -uno` 5.37 ms.
- Peak RSS (wait4 rusage, whole process, $SP/git_rss.txt): git_bench discover group 9.9 MB; status group 11.4 MB; Sort::TIME scale group 47.8 MB; blame group 92.4 MB. CLI: `git status` 10.4 MB, `git log -n 200` 11.4 MB, `git blame` 21.3 MB. CPU: the status group (23 libgit2 statuses) 4.4 s CPU of which 2.9 s sys.

### gix 0.81 in-process (third option; $SP/git_build/gx/src/bin/gxbench.rs, load 11.6; gix 0.84 does not resolve offline: gix-imara-diff is not in the local registry, so 0.81)

| operation | gix median / p90 ms | compare |
|---|---|---|
| discover(subdir)+workdir | 0.23 / 0.24 | git2 0.17, CLI 1.47 |
| HEAD oid (open + head_id) | 0.34 / 0.35 | git2 0.26, CLI 1.73 |
| remotes | 0.21 / 0.23 | git2 0.17 |
| revwalk first 200 (default sort) | 1.25 / 1.43 | git2 TIME 196.5, CLI 3.94 |
| revwalk first 200, ByCommitTime | 1.93 / 1.96 | same |
| revwalk ALL commits oids only | 3.25 / 3.77 | git2 193.8, CLI 15.0 |
| index read + iterate | 3.69 / 3.81 | git2 5.80, CLI ls-files 3.90 |
| status incl. untracked (index-worktree iter) | 22.56 / 24.52 | git2 51.1, CLI 22.5 |
| tree diff HEAD~5..HEAD (no renames) | 4.69 / 5.14 | git2 9.40, CLI 3.98 |
| tree diff HEAD~200..HEAD (no renames) | 9.05 / 9.52 | git2 9.66, CLI 9.86 |
Peak RSS of the whole gxbench run: 33.3 MB. Blame and rename-tracking diff were not benchmarked with gix.

---------------------------------------------------------------------------------------------------

## 4. SEMANTICS AND RISK

Verified on scratch repos under $SP/git_sem (built by $SP/git_sem_setup.sh and $SP/git_sem_ignored.sh; libgit2 side = $SP/git_bench/src/bin/sem.rs; CLI side = $SP/git_sem_cli.sh; HOME pointed at a scratch dir so global config is controlled).

| scenario | libgit2 (as production uses it) | CLI (git 2.56.0) | matters for codescout? |
|---|---|---|---|
| linked worktree (`.git` file) | open, discover, HEAD, index, status, revwalk, diff, blame, worktrees() all OK; `repo.path()` = `base/.git/worktrees/wt`, `workdir()` = the worktree root; `is_worktree=true` | same results | yes, and OK: this repo had about 40 linked worktrees (`git worktree list` showed 43 lines at the start). policy.rs relies on the per-worktree git dir (comment: "Linked worktrees use their own Git dir") and libgit2 returns it. Real worktree in this repo opened fine (section 3) |
| worktree created with `worktree.useRelativePaths=true` (git 2.48+ feature) | OK | OK | OK |
| linked worktree whose admin dir was deleted | `open` ERR "failed to resolve path ... worktrees/wtbroken" | `fatal: gitfile does not point to a valid repository` | same failure either way |
| sparse checkout (cone) | `statuses` reports `WT_DELETED dirB/b.txt` for every out-of-cone file (false dirty); HEAD/revwalk/diff fine | status is clean | only the dashboard `git_info` uses libgit2 status; observe/delete use the CLI so they are right |
| sparse index (`index.sparse`) | `repo.index()` and `statuses` ERR "unsupported mandatory extension: 'sdir'" | works | audit_doc_refs `build_tracked_dirs` degrades to an empty set (documented fallback at mod.rs:870-880); dashboard shows not-dirty |
| reftable ref storage (`git init --ref-format=reftable`) | `open` and `discover` ERR "unsupported extension name extensions.refstorage" | works | every libgit2 site then takes its not-a-repo branch; doctor would say "is not a git repository", which is misleading. Rare today (hypothesis: becomes more common as git changes its default) |
| SHA-256 repo | `open` ERR "unknown object format 'sha256'" | works (64-hex ids) | same as reftable |
| partial clone `--filter=blob:none` | open/HEAD/revwalk/tree diff OK; reading a missing blob ERR "object not found - no match for id" (blame) | `git cat-file -p HEAD:dirA/a.txt` lazily fetched the blob and succeeded | only `last_changed` (blame) touches blobs; rare |
| shallow clone | opens, `is_shallow=true`, walk 1 commit, blame OK; `HEAD^` fails in both | same | doctor's SHA check and move detection would see a truncated history in both |
| `includeIf "gitdir:..."` config | honoured (read `user.email` from the included file) | honoured | not used by production code (no config reads) |
| fsmonitor / untracked cache | ignored by libgit2 (config accepted, no effect) | would use them | repo does not enable them, so nothing to lose today. libgit2 status is 2.3x slower than CLI on this repo regardless |
| packed-refs only, separate-git-dir, `.git` file | OK | OK | OK |
| hooks | libgit2 never runs hooks | runs hooks on commit etc. | production never creates commits; only tests do |
| safe.directory / ownership check | NOT VERIFIED. `git2::opts::set_verify_owner_validation` exists (compiled in sem.rs); no foreign-owned repo found on this machine (find over /opt /srv /usr/share /var/lib /home) and I could not fake one | git refuses with "dubious ownership" | hypothesis: both refuse a repo owned by another user (shared checkouts, sandboxes, containers with another uid); parity not tested |

Worktree use in doctor.rs and index_state.rs: both open `cp.git_root` / `discover(root)`; in a linked worktree these resolve to the worktree's own HEAD and index (verified above), which is what the per-worktree `.codescout/index-state.json` logic needs.

What matters for how codescout uses libgit2 (read-only opens, HEAD, discover, remotes, revwalk, blame, index read): linked worktrees (fine), commit-graph / Sort::TIME performance (bad), sparse-index and reftable (fail soft, with misleading text), partial clones (blame only). The ones that would matter more if it did writes (hooks, signing, includeIf for identity) do not apply because production never writes.

---------------------------------------------------------------------------------------------------

## 5. BUILD AND SIZE COST

Method: clean target dir per measurement, `nice -n 19`, CARGO_BUILD_JOBS=8, `--offline`, cwd = the scratch dir so the repo's `.cargo/config.toml` (sccache wrapper + mold) is NOT applied (my first attempt ran with cwd in the repo and picked up sccache: those results, files $SP/git_build/results_*.txt without the "2", are DISCARDED). Wall, user and sys come from wait4 rusage of the whole cargo process tree ($SP/git_rusage.py). Profiles copied from the repo's Cargo.toml: dev = opt 0, debug 1; release = opt 3, lto off, codegen-units 16, strip, panic abort. Commands: `bash $SP/git_build_run.sh <hello|g2|gx> <dev|release> [split]` and `bash $SP/git_build_marginal.sh`; results $SP/git_build/results2_*.txt. Load average 14-22 during the builds.

| crate | profile | wall s | CPU s (user+sys) | binary bytes | delta vs hello |
|---|---|---|---|---|---|
| (a) hello | release | 0.4 | 0.4 | 337,424 | - |
| (a) hello | dev | 0.2 | 0.2 | 4,587,528 (unstripped) | - |
| (b) git2 (repo features) | release, total | 22.8 (15.1 + 7.7) | 84.1 (63.3 + 20.8) | 1,459,688 | +1,122,264 (1.07 MiB) |
| (b) git2 | dev, total | 20.9 (9.8 + 11.1) | 82.4 (38.0 + 44.4) | 13,831,904 (unstripped) | +9.2 MB |
| (c) gix 0.81, features sha1+revision+index+status+blob-diff+blame+parallel | release | 19.3 | 121.9 | 3,843,200 | +3,505,776 (3.34 MiB) |
| (c) gix | dev | 16.0 | 92.8 | 58,074,568 (unstripped) | +53.5 MB |

libgit2-sys's share (step 1 `cargo build -p libgit2-sys`, which also builds libz-sys, cc, pkg-config): release 15.1 s wall / 63.3 CPU-s of 22.8 s / 84.1 CPU-s = 66% of wall, 75% of CPU; dev 9.8 s / 38.0 CPU-s of 20.9 s / 82.4 CPU-s = 47% of wall, 46% of CPU. Marginal over what codescout already compiles (the `url`/ICU stack is a direct dependency of codescout itself, Cargo.lock line 660, and of reqwest/rmcp): second measurement, release libgit2-sys 15.2 s / 63.7 CPU-s plus git2 crate and a tiny main 2.8 s / 5.4 CPU-s (url 5.6 s / 15.3 CPU-s excluded); dev libgit2-sys 8.8 s / 35.7 CPU-s plus git2 crate 2.5 s / 3.3 CPU-s. So git2's own marginal build is about 69 CPU-s release / 39 CPU-s dev, once per cold target directory; the 56 CPU-s release user time is the C compile (-O3) of libgit2.
gix: 156 locked packages against 51 for git2 (including the shared url/ICU stack), 122 CPU-s release. Its marginal CPU is about 1.8x git2's marginal; wall time is lower because Rust crates parallelize better than one build script.

Size context: the live /home/marius/work/claude/codescout/target/release/codescout is 43,767,744 bytes (ls -l; its feature set is unknown to me), so git2's +1.07 MiB is about 2.6% of it and gix's +3.34 MiB (subset of features, unused functions pruned by the linker) about 8%.

Gate context: memory research/gate-speed-20261101 records lane compile times from one gate run (LEAN 56 s, DEFAULT 1 m 31 s, "cold build about 1 m 30 s per profile") and does NOT record libgit2-sys. A standalone dev libgit2-sys is 8.8 s wall at 8 jobs, so the upper bound is about 16% of the LEAN and 10% of the DEFAULT compile if it sat on the critical path; whether it does was not measured (hypothesis: it is a leaf build script that runs in parallel with other crates, so the real share is smaller). It only recompiles on a cold target dir, not on incremental rebuilds. CI uses rust-cache and sccache, and sccache does not cache the C objects from a build script (read from ci.yml, not measured).

C toolchain needs (libgit2-sys 0.18.8 + libz-sys): a C compiler (`cc` crate: gcc/clang, MSVC cl.exe, or MinGW gcc), optional `pkg-config` binary to find system zlib; NOT cmake (the build ran without invoking it, though cmake exists on this box), NOT openssl (libgit2-sys deps in Cargo.lock are cc, libc, libz-sys, pkg-config; default-features=false drops https and ssh; the openssl-sys in Cargo.lock comes from native-tls via other crates). Runtime: the scratch g2 binary links the system `libz.so.1` dynamically (ldd: libz.so.1, libgcc_s, libc), because libz-sys found zlib through pkg-config; the gix binary has only libgcc_s and libc. I did not ldd the real codescout binary. Platforms: .github/workflows/ci.yml:392-396 builds and tests on ubuntu-latest, macos-latest and windows-latest (MSVC), plus a windows-gnu MinGW cross lane with wine (line 646+); all of them already compile libgit2-sys, so no new platform risk from keeping it. I did not run any CI lane.

---------------------------------------------------------------------------------------------------

## 6. REMOVAL COST

### If libgit2 were dropped in favour of the CLI (production)

- 18 functions in 14 files rewrite from library calls to spawns (about 32 production lines of `git2::` become process plumbing). Hot per-call sites (#1-#6) go from 0.14-0.26 ms to 1.4-1.8 ms each (measured medians), i.e. about +5 ms per tool call just for #1 on Linux (4 discovers x (1.47 - 0.17) ms), plus a spawn on every activation; or they would need a hand-written `.git` reader (an existing pure-Rust walk-up exists: `librarian::current_project::lookup_git_root`, 8 lines, but it only finds the root, it does not read HEAD/refs/packed-refs). Heavy sites get FASTER with the CLI (revwalk 195 -> 4-15 ms, blame 1,980 -> 19 ms, doctor walk), status 51 -> 22 ms.
- Error handling changes: today a library error is a typed `git2::Error` mapped to `None`/empty/"not a git repository" at each site; with the CLI each site must distinguish spawn failure (git missing), non-zero exit (not a repo vs real failure), timeout and non-UTF-8 output. There is no existing helper beyond observe.rs's `git()` (adds a 2 s kill timeout) so a shared `run_git(dir, args, timeout)` would be new code (M).
- git missing: no production preflight exists (section 2). All 5 existing CLI sites already fail soft. A CLI-only design would make git a hard runtime dependency for activation (HEAD sha), doctor and the observation store; agents in sandboxes without git would lose HEAD sha, `has_git_remote` and the whole observations store key (`checkout`) unless the pure-Rust fallback is kept.
- Windows: reintroduces CreateProcessW under EDR on the VDI at activation (WIN-14), at 6 per-call sites, and in doctor. The EDR cost per spawn is not measured anywhere I found; this is the project's own recorded reason for libgit2.
- Tests: 62 test-only `git2::` lines in 15 files build fixtures: src/git/mod.rs (init_repo, commit_file helpers, 4 diff tests), src/librarian/catalog/gc.rs (helpers at 1218-1240, tests at 1258-1325), src/retrieval/index_state.rs:391-402 (helper), src/librarian/tools/append_entry.rs:1688-1761 (12 lines: commit_all, upstream setup with `init_bare` and `set_upstream`), src/librarian/tools/doctor.rs (25 lines: commits with controlled `git2::Time` at 13053-13608, move tests at 18367 and 18507, 20478), src/agent/declared_member_tests.rs:72-75, tests/observation_store.rs:13,694-697 and tests/observation_time.rs:7, and one-line `Repository::init(dir)` in src/tools/edit_file/tests.rs:3934, src/tools/symbol/tests.rs:11223, src/server/native_context_tests.rs:579 and :696, src/observations/server_tests.rs:10, src/tools/run_command/attribution.rs:828. They could use `git init -q` and `git -c user.name=.. commit` (22 integration-test sites in tests/ already do exactly that, so the pattern exists) with `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` for the doctor tests; one shared test helper would replace the 5 duplicated commit helpers. Effort M for tests alone. Per-fixture cost goes from in-process to spawn (1.2-2 ms per git call, measured spawn floor).

### If CLI calls were replaced by the library

| site | could move to git2? | saving | verdict |
|---|---|---|---|
| A fence `canonical_checkout` (1 spawn per run_command/write call) | yes: `Repository::discover(dir)`, `workdir()`, canonicalize (same as observations `checkout`) | 1.47 -> 0.17 ms per call (about 1.3 ms; over the 68,175+ affected rows in usage.db about 90 s in 29 days); removes a failure mode (it already falls back to canonicalize) | low value, effort S |
| B observe.rs (5 spawns per read-declared call) | status yes, but libgit2 status is 2.3x slower (51 vs 22.5 ms, twice per call), it has no GIT_OPTIONAL_LOCKS=0 equivalent proof, no kill-on-timeout, and false-dirty on sparse checkouts | spawns saved 5 x ~1.5 ms = 7.5 ms, status cost added ~57 ms | do not move |
| C delete.rs file_git_state | `status_file` exists | 6.53 vs 5.23 ms (no gain) | do not move |
| D mv.rs git grep | libgit2 has no grep | - | cannot |
| E merge_worktree is-ancestor x2 | `graph_descendant_of` | 3.67 -> 1.44 ms x2 on a rare path | optional, effort S |

---------------------------------------------------------------------------------------------------

## 7. VERDICT

### Matrix per cluster

| cluster | verdict | measured evidence | risk | effort |
|---|---|---|---|---|
| Per-call / activation hot path (observations `checkout`, policy `directory`, `resolve_head_sha`, `probe_has_git_remote`, `head_commit_full`; sites #1-#5) | KEEP git2 | 0.14-0.32 ms vs 1.47-1.83 ms CLI (6-10x); >= 4 discovers per call | none new; fails soft on reftable/sha256/sparse-index | - |
| Provenance fence `canonical_checkout` (site A) | optional MOVE to git2 (or the existing pure-Rust walk-up) | 1.47 vs 0.17 ms, 68,175+ rows | low; must keep the canonicalize fallback | S |
| `run_command` observe (site B) | KEEP CLI | libgit2 status 51.1 ms vs CLI 22.5 ms; gix status 22.6 ms | libgit2 false-dirty on sparse checkout | - |
| Attribution engine | out of scope (Python + CLI) | silent p50 150 ms, python3 start alone 74 ms | - | - |
| librarian doctor (#9-#12) | KEEP git2 for SHA lookups (`revparse_single` ~54 calls); the two history walks are the only place libgit2 is badly slower: Sort::TIME walk ~195 ms fixed, all-commit walk 194 ms vs CLI 15 ms | if the walks moved to the CLI or to gix: ~0.4 s of a ~7 s doctor (hypothesis: sum of the two measured walk floors, not measured in situ) | CLI reintroduces EDR spawn on a manual, rare tool; gix is pre-1.0 | M (CLI), L (gix) |
| append_entry | no production git2 or CLI site (grep: all 12 git2 lines are at or after line 1688, inside `mod tests`, which starts at 471) | - | - | S (tests to CLI if the library is ever dropped) |
| index_state (#5, #6) | KEEP git2 | 0.32 ms / 1.47 ms vs 1.83 / 3.83 ms | - | - |
| gc.rs `detect_move_candidates` (#12) and reindex `backfill_commits` (#13) | KEEP for now; candidates for the CLI or gix | 194 vs 15 ms (oids), 244 vs 131 ms (full backfill) | rare/manual | S-M |
| gather.rs (#17, #18) | KEEP; `last_changed` blame is 1,980 ms vs 19 ms CLI per path; fix is cheaper than a migration: use Sort::NONE or the CLI for blame | blame 100x, Sort::TIME 195 ms floor | rare (1 gather call in usage.db) | S |
| audit_doc_refs (#14, #15) | KEEP git2 | index read 5.8 ms vs `ls-files` 3.9 ms; HEAD 0.3 ms | sparse-index falls back to an empty set (documented) | - |
| dashboard `git_info` (#7) | FIX (bug, not a migration): `dirty` is true for any ignored dir and for out-of-cone sparse files; use `StatusOptions` without ignored (49.5 ms) or the CLI (22.5 ms, `-uno` 5.7 ms) | verified on scratch repo | feature `dashboard` only | S |
| mv.rs `git grep`, delete.rs status, merge_worktree is-ancestor (C, D, E) | KEEP CLI (D has no libgit2 equivalent) | - | missing-git branch is untested (mv.rs comment) | - |
| `git::diff_tree_to_tree` (#8) | REMOVE (dead in production; 4 tests only) or keep as test-only | 9.4 ms vs 4.0 ms CLI | - | S |
| Tests that build fixtures with git2 (62 lines, 15 files) | MOVE to CLI via one shared helper only if the dependency is ever dropped; otherwise leave | - | spawn cost per fixture | M |

### Overall recommendation (hypotheses marked)

Keep git2. Dropping it does not pay: it saves about 1.07 MiB (2.6% of a 43.8 MB binary) and about 69 CPU-s (release) / 39 CPU-s (dev) on a cold target directory, but it would make the 6 per-call/activation sites 6-10x slower (0.14-0.32 ms -> 1.5-1.8 ms each) and reintroduce the process-spawn hazard that WIN-14 removed on the EDR-taxed VDI, plus require a git binary that no preflight checks today. The library also has no recorded defect: 0 matching usage.db rows, 0 matching docs. The CLI is the better tool only where libgit2 is slow: status (2.3x), history walks (15-50x), blame (100x), and those are rare, manual or already CLI paths. gix is the only option that gives both properties (in-process, CLI-class speed: revwalk 200 commits 1.25 ms, status 22.6 ms, tree diff 9 ms) but costs +3.34 MiB, a 156-package tree (122 CPU-s release), pre-1.0 API churn (0.84 was not even resolvable offline), and a rewrite of 18 sites and 62 test lines (effort L); I would only spike it for the doctor/reindex/gather history walks if their latency matters. Cheapest real wins found: (1) fix `git_info` dirty semantics (bug, S); (2) stop using `Sort::TIME` where a fixed 195 ms floor is paid for 1-20 commits, or measure doctor with the pathspec variant first (hypothesis: ~0.4 s of ~7 s); (3) optionally move the fence's `canonical_checkout` spawn onto the library (S, about 1.3 ms per call). Do not move observe.rs to libgit2.

---------------------------------------------------------------------------------------------------

## NOT VERIFIED

- EDR cost per spawn on the Windows VDI (the premise of WIN-14); no measurement exists in the docs I searched, and I measured only on Linux. Windows and macOS behaviour of libgit2 vs git was not run; CI evidence was read, not executed.
- safe.directory / ownership behaviour of libgit2 vs git (no foreign-owned repo available).
- Why libgit2's Sort::TIME walk costs a constant ~195 ms (eager full-history walk is a hypothesis; libgit2 source not read) and whether it ignores the split commit-graph chain (suggested by the numbers, not proven).
- Per-call counts of libgit2 discovers and CLI spawns are read from source bodies, not traced at runtime (no strace); the "at least 4 per call" for observations is from `dispatch_at` + `emit` + `store` only.
- doctor with its production pathspec (my doctor-like row had none); the share of doctor time spent in libgit2 in situ; callgrind share beyond the memory note.
- libgit2-sys's share of the real gate's compile time (not recorded in memory research/gate-speed-20261101, and I did not build the repo: scratch crates only).
- `ldd` and feature set of the real codescout binary (did not run ldd on the live binary); whether the real build links system zlib.
- gix: blame, rename-tracking diff, worktree list, reftable/sha256/sparse-index support were not tested; the build used gix 0.81, not 0.84.
- git 3.0 defaulting to reftable (outside this session's evidence).
- usage.db rows cover ids 180,898-299,175 only (118,278 rows, 2026-10-12..2026-11-10); older history is not in the table. attribution_ms exists only from 2026-11-01.

## Scripts and outputs (all under $SP)

git_count.py (prod/test classifier), git_usage.py (usage.db percentiles), git_bench/ (crate: src/main.rs benchmark, src/bin/sem.rs semantics check), git_runbench.sh, git_bench_*.txt (A..M superseded, R2all authoritative, *nostrict), git_sem_setup.sh / git_sem_ignored.sh / git_sem_cli.sh and git_sem/ (scratch repos), git_build/{hello,g2,gx} + git_build_run.sh + git_build_marginal.sh + git_rusage.py + results2_*.txt, git_gxbench.sh + git_build/gxbench_out.txt, git_rss.sh + git_rss.txt, git_hf_commitgraph.json, git_git2_sites.txt (raw grep of every git2 line).

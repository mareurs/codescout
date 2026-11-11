---
id: '489e013ddfd6c3fc'
kind: bug
status: fixed
title: call_graph serves cached edges that an edit made outside codescout never invalidates
tags:
- cluster/unclassified
- navigation
closed: 2026-11-11
opened: 2026-11-11
owner: marius
related:
- a73c49456056fb92
- '58a45860b802a477'
- '4e7ec7bfc047ac7b'
severity: medium
unverified: 'Measured by hand in two scratch crates (callers direction, rust-analyzer, default transport) before the first fix, and by test in both directions with a mock LSP. The epoch (second fix) is verified by unit tests, by the mock-LSP callers test and by a manual re-run on the live server (crate stale7: a caller added in an unnamed file and a caller in a file created after the entry was cached are both reported; the schema shows the epoch column). A git checkout in both directions (adding and removing callers in files the result does not name) was run once on the live server in that scratch crate; it was not run on a large repository. That re-run did not show that an unchanged tree is still served from the cache; only the unit tests show that, and the 2026-11-12 timing pairs in ## Fix are consistent with it (a second identical call took 101 to 375 ms against 173 to 51 879 ms) without proving it. The epoch cost was timed with the real function in a release build, once, on one machine (btrfs, warm page cache, 9 calls per case) with the old cap of 5000: see the table in ## Fix. The new cap of 20000 is verified by one test that a tree above the OLD cap stores an epoch; its worst-case cost is extrapolated and not measured, and the live server was not re-run with it. The narrowed invalidation (writes to files that cannot hold a call keep the cache) is verified by two tests and a mutation check; the live server was not re-run with it, and the 1.3 times longer life is an estimate from one day of write counts on one checkout, not a measured hit rate. A cold cache, another filesystem, a loaded machine and a second session were not measured, and the 131 to 280 ms first-call penalty is attributed to a cold inode cache without that being verified. The saving of a cache hit was measured on 4 symbols of one repository, once each, on a loaded shared machine (one pair is a 52 s outlier whose cause I did not find); how often a hit happens in a shared checkout was not measured. The probe lives only in a scratch copy and is not committed. Languages other than Rust, Python and TypeScript (timing only; the behavior was tested on Rust only) were not run. The epoch covers only files of the seed''s language family, so a caller written in another language family is not detected by an external edit; a codescout write to a source file of any language still clears the cache. A walk cut short by its cap makes the epoch unknown, which turns the cache off for that call; it happens above 20000 files of the seed''s language, and a registry-sized tree (72310 Rust files) is still above it. Skipping the epoch for callees was judged unsafe from the code (re-export chains) and not reproduced. The mechanism of the spurious ts edge is inferred, not traced. The link to 4e7ec7bfc047ac7b is a lead, not a finding. Severity and the status fixed are my proposal; the owner may reopen it.'
---

# BUG: call_graph serves cached edges that an edit made outside codescout never invalidates

## Summary

`call_graph` caches the edges it resolves in `.codescout/call_edges.db`, keyed by the position of the target symbol. The cache is invalidated only by codescout's own write tools. A file changed by anything else (`cargo fmt`, `git checkout`, an editor, a peer without codescout) leaves the cached edges in place, so `call_graph` keeps answering with the old lines, omits a caller added later, and in one run reported an edge that does not exist. `references` and `symbols` on the same files were correct at the same time.

## Class fit

The nearest class is `cluster/gate-keyed-on-unobservable-event`: the invalidation is keyed on an event (a codescout write) that an external edit never produces. I did not read that class's definition against this case, so I use `cluster/unclassified`. It differs from `a73c49456056fb92` (a stale copy inside the language server) because here the server is current and codescout's own store is the stale one.

## Symptom (Effect)

After an external edit of a caller file, `call_graph(symbol, direction="callers")` returns the callers as they were when the cache entry was written: old line numbers, no new caller, and a mix of old lines with new symbol ranges that produced a spurious edge. No warning or age is shown.

## Reproduction

Scratch crate `stale5` (rust-analyzer, default mux transport), session `5f715d8f`, with the fix `43c4ca9df` live in the server. `lib.rs`: `target_fn`, `same_file_caller` (calls it), `pub mod other;`. `other.rs`: `use crate::target_fn;`, `full_path_caller` (calls `crate::target_fn(5)`), `imported_caller` (calls `target_fn(6)`). Every state was read from the files; every step used `workspace=<crate>`.

| # | Step | `call_graph(target_fn, callers)` | `references` / `symbols` |
|---|---|---|---|
| 1 | Baseline, files synced | 3 edges: `lib.rs:6`, `other.rs:4`, `other.rs:8` (correct) | `references` 5 locations, correct |
| 2 | External rewrite of `other.rs`: 3 comment lines prepended; the calls are now on lines 7 and 11 | 4 edges: `other.rs:4`, `other.rs:8` (**old lines**) and `7: full_path_caller → imported_caller (depth=2, ts)` (**this call does not exist**) | `references` correct (`4`, `7`, `11`); `symbols` correct (`full_path_caller` L6-8, `imported_caller` L10-12) |
| 3 | Same `call_graph` call again after those two | identical to step 2 | |
| 4 | Control: move `.codescout/call_edges.db` away (scratch crate only), same call | 3 edges: `lib.rs:6`, `other.rs:7`, `other.rs:11` (**correct**) | |
| 5 | External append of `late_caller` (calls `target_fn(9)` on line 15) | 3 edges, **`late_caller` missing** | `references` lists `other.rs:15` |
| 6 | Control: a codescout write to `other.rs` (`edit_file`, a comment on line 15), same call | 4 edges, `15: late_caller → target_fn` present | |

Step 4 shows that the stale answer comes from the cache file and not from the language server. Step 6 shows that codescout's own writes do invalidate it. Steps 2 and 3 show no self-healing: the answer stayed the same after `references` and `symbols` had refreshed everything else.

The spurious edge in step 2 is consistent with a stale line (7, which was the line `pub fn imported_caller` in the old file) being attributed to the enclosing function in the NEW symbol ranges (`full_path_caller`, L6-8). That mechanism is my inference from the output; I did not trace it.

### Re-run with the rebuilt binary (commit `5a7936984`, release binary of 23:01, `signatures_json` present in the binary)

New scratch crate `stale6` (callers direction, same layout as `stale5`, plus `unrelated.rs` that holds no call at first), default transport, `workspace=<crate>` on every call. Every state read from the files.

| # | Step | `call_graph(target_fn, callers)` |
|---|---|---|
| 1 | Baseline | 3 edges: `lib.rs:6`, `other.rs:4`, `other.rs:8` (`ts`, the server was still starting) |
| 2 | External rewrite of `other.rs`: 3 lines prepended | 3 edges at the CURRENT lines: `lib.rs:6`, `other.rs:7`, `other.rs:11`, now from `lsp`. No spurious edge. (Before the fix: lines `4` and `8` and an invented `full_path_caller → imported_caller`.) |
| 3 | External append of `late_caller` (line 15) to `other.rs` | 4 edges, `15: late_caller → target_fn` present. (Before the fix: missing.) |
| 4 | External rewrite of `unrelated.rs`, a file the cached result does not name: `sneaky` calls `target_fn` on line 6 | still 4 edges, **`sneaky` missing**; `references` lists `unrelated.rs:6`. **The residual, reproduced by hand.** |
| 5 | Control: external change to a NAMED file (`other.rs`, one blank line) | 5 edges, `unrelated.rs:6: sneaky → target_fn` now present |
| 6 | Migration on a real old database: `call_graph` on `stale5`, whose `call_edges.db` was written by the previous binary | works; `call_edge_resolutions` now has `signatures_json`; the one row was recomputed and signed |

Steps 2, 3 and 6 confirm the fix on a real server. Step 4 against step 5 shows the limit exactly: the entry is invalidated by a change in a file that it names and not by a change in a file that it does not.

### Re-run with the epoch (commit `702a0f737`, release binary of 23:52, live MCP server after `/mcp`)

New scratch crate `stale7` (callers direction, rust-analyzer, default transport, `workspace=<crate>` on every call, every state read from the files). `lib.rs` declares `pub mod other; pub mod unrelated; pub mod later;`; `later.rs` does not exist at first. Proof that the live server has the epoch code: after the first call, `pragma_table_info('call_edge_resolutions')` lists `epoch`, and the one row has both `epoch` and `signatures_json` set.

| # | Step | `call_graph(target_fn, callers, full)` |
|---|---|---|
| 1 | Baseline | 3 edges: `lib.rs:6`, `other.rs:4`, `other.rs:8` (`ts`, server starting) |
| 2 | External rewrite of `unrelated.rs` ONLY, a file the cached result does not name: `sneaky` calls `target_fn` on line 8 | 4 edges, `unrelated.rs:8: sneaky → target_fn` present, now from `lsp`. **The residual is closed** (`stale6` step 4: missing). No named file changed, so only the epoch can have invalidated the entry. |
| 3 | External creation of `later.rs`, a file that did not exist when the entry was written: `new_file_caller` on line 4 | 5 edges, `later.rs:4: new_file_caller → target_fn` present |
| 4 | Control: move `.codescout/call_edges.db` away, same call | the same 5 edges |

Not covered by this re-run: that an UNCHANGED tree is still served from the cache (the unit tests cover the hit; the live server gave no observable hit marker), a second session, another language.

### Re-run through `git checkout` (same crate `stale7`, 2026-11-12)

Two branches of the scratch repository: `b-state` (`unrelated.rs` and `later.rs` hold no call) and `a-state` (`unrelated.rs` has `sneaky`, `later.rs` has `new_file_caller`; the two branches differ in exactly those two files, from `git diff --stat`). Only `git checkout` changed the files; the server was not told. Every call pinned with `workspace=<crate>`; the cache row's epoch read from `call_edges.db` before each call.

| # | Step | Epoch in the cache row before the call | `call_graph(target_fn, callers, full)` |
|---|---|---|---|
| 1 | On `b-state`, first call | none (empty) | 3 edges: `lib.rs:6`, `other.rs:4`, `other.rs:8` (`ts`, server starting) |
| 2 | `git checkout a-state`: adds two callers in two files the cached result does not name | `62782c9e` | 5 edges: also `later.rs:4: new_file_caller` and `unrelated.rs:8: sneaky`, from `lsp` |
| 3 | `git checkout b-state`: removes them again | `fe20de41` | 3 edges, neither of the two |

The row's epoch differs after each checkout, as it should. This covers a checkout that adds callers (step 2) and one that removes them (step 3), on one crate and one run.
## Environment

Linux 7.2.15-zen1, codescout HEAD `e922462e9` (fix `43c4ca9df` included, release binary rebuilt 22:24), rust-analyzer. The earlier unexplained omission of `other.rs::use_it` from `call_graph` (noted in `a73c49456056fb92`) fits this cause: that cache entry was written before `other.rs` existed and its key never changed. I did not re-run that sequence.

## Root cause

Read at the bytes:

- `one_hop_at` (`src/tools/symbol/call_graph/mod.rs`, around line 460): builds `ResolutionKey { symbol, file, line, col, direction }` from the seed's definition, calls `EdgeCache::lookup_resolution`, and returns the cached edges at once unless an old payload lacks endpoint identity. Nothing compares the files that the cached edges mention with the disk.
- The key holds the position of the TARGET symbol only. An edit that does not move the target (any change in a caller file) keeps the key valid.
- `invalidate_file` (`call_edges/cache.rs:191`) deletes the rows of one file. Its only callers outside tests are `Agent::invalidate_call_edges` and `invalidate_call_edges_for` (`src/agent/mod.rs`), and those are called from `edit_code` (rename, remove, replace, insert), `edit_file`, `create_file` and `edit_markdown`. Nothing calls it for a change that codescout did not make.
- The store is a file on disk (`.codescout/call_edges.db`, 32 KB in the run), so a stale entry outlives the session.

NOT established: the cross-session persistence was read from the code and not run; the `callees` direction was run only as a test, not by hand. (`invalidate_file` does clear the completed resolutions: its second statement deletes every resolution of the project, read 2026-11-11.)

## Evidence

The tool outputs in the table, copied from the session.

## Hypotheses tried

- The language server is stale: **refuted**. `references` and `symbols` were correct in the same state, and removing the cache file alone fixed `call_graph`.
- The cache is never invalidated at all: **refuted** by step 6.

## Fix

Partly done 2026-11-11, commit `5a7936984` (see `## Fix provenance`), by candidate (1): a cached resolution is revalidated against the files it names.

- `call_edge_resolutions` gets a column `signatures_json`. `store_resolution` writes into it the `(len, mtime)` of every file the result names (`files_behind`: the definition it started from, every call-site file, every next-hop definition), using the same `disk_signature` as the LSP client (now `pub(crate)`).
- `lookup_resolution` returns a miss when any of those files differs from its stored signature, including a file that was absent and now exists. An entry without signatures (stored before this change) is a miss. A corrupt signature record is a miss. A corrupt edge payload is still an error.
- `apply_schema` adds the column to a database that already has the table (`ALTER TABLE`, tolerating a second process that added it first).
- Codescout's own write tools keep invalidating through `invalidate_file`; nothing about them changed.

**Residual (fixed by the project-level epoch, see below):** an entry is valid while the files it NAMES are unchanged. A caller added in a file that the result does not mention (a new file, or an existing file with no call of the symbol) does not change any signed file, so the entry is still served and the caller is missing. That is the case that first showed the problem (`other.rs` did not exist when the entry was written). It had a red test (see `## Tests added`). Options considered: (a) a project-level epoch (implemented, below): a hash of `(relative path, len, mtime)` over the source files of the seed's language, computed once per `call_graph` call and stored with each resolution; (b) do not reuse a `callers` resolution across calls; (c) a file watcher in codescout, which does not exist today (`mark_file_dirty` marks only codescout's own writes).

Cost of (a), measured 2026-11-11 on this repo (`epoch-cost.py`, tracked plus untracked files minus ignored): 547 `.rs` files; one `stat` pass over them 1.5 ms (warm cache, 5 runs); over 4226 files of four languages 14 ms; `git ls-files` for the 7283 listed files 22 ms. That was a proxy; the real function is timed in the next section. Every codescout write already deletes ALL resolutions of the project (`invalidate_file`'s second statement), so an epoch adds invalidation only for edits made outside codescout. A hash, not "newest mtime and count", because a replaced file with an older mtime (`cp -p`, `rsync -t`) would not raise the newest mtime. If the walk hits its scan cap the epoch is unknown and the resolution should not be reused.

Small accepted window: the signatures are read after the analysis, so a file that changes while one analysis runs is stored with its new signature beside edges from its old content.

**Epoch, implemented 2026-11-11 (option (a), second commit, see `## Fix provenance`):**

- `scan_budget::language_epoch(root, seed_lang, cap)` returns a SHA-256 over the sorted `(relative path, len, mtime_ns)` of every source file of the seed's language family (TypeScript includes JavaScript). It walks with `scan_walk` and the same `ScanBudget` as the `callers` fallback. It returns `None` when the walk is cut short by its cap or hits a walk error.
- `call_edge_resolutions` gets a column `epoch`. `EdgeCache` carries an `EpochCheck`: `Off` (no check, the default), `Unknown` (never a hit, nothing stored) and `Known(hash)` (a hit only when the stored epoch is equal). A row stored without an epoch is a miss under `Known`. `apply_schema` adds the column to an old database the same way as `signatures_json`.
- `CallGraph::call` computes the epoch once, before the direction loop, on a blocking thread, and passes it to `CachedResolver`.

Limits, all deliberate: any edit to a same-language file invalidates every cached resolution of that language, not only the ones that name it. A caller in another language family is not detected. When the walk is cut short the cache is off for that call, so the answer is correct but slow.

**Measured cost of the epoch, 2026-11-12.** The real `language_epoch` (so the real `scan_walk`, stat and SHA-256), release build, one run of an `#[ignore]` probe in a scratch copy (not committed): 9 calls per case, warm page cache, btrfs on this machine, single-threaded walk. `files walked` is every file the walk visits; `family` is the files of the seed's language family.

| Tree | Seed | Files walked | Family | Cap 5000 | Median | First call |
|---|---|---|---|---|---|---|
| codescout | rust | 7073 | 562 | Some | 29.9 ms | 29.9 ms |
| codescout | python | 7073 | 423 | Some | 28.9 ms | 28.8 ms |
| claude-code | typescript | 2455 | 2360 | Some | 18.6 ms | 146.9 ms |
| opencode | typescript | 4559 | 1607 | Some | 25.2 ms | 131.0 ms |
| hermes-agent | typescript | 10114 | 2708 | Some | 46.7 ms | 173.3 ms |
| hermes-agent | python | 10114 | 4586 | Some | 51.2 ms | 258.1 ms |
| codex | rust | 8777 | 5096 | **None** | 47.7 ms | 279.5 ms |
| `~/.cargo/registry/src` | rust | 156679 | 72310 | **None** | 48.7 ms | 82.7 ms |

Without the cap the same two `None` cases give `Some` in 50.9 ms (codex) and 986 ms median, 2012 ms first call (cargo registry). The cost is about 4 to 6 microseconds per walked file.

What this shows: a typical project pays 18 to 52 ms per `call_graph` call, once per call, on every call (the epoch is computed before the cache is consulted, so a hit pays it too). The cap decides the rest: with `MAX_WORKSPACE_FILES_SCAN = 5000` a tree with more than 5000 files of the seed's language gets `None`, so the cache is off for it. `codex` (5096 Rust files) is one such tree, on this machine. The cap was written for the `callers` fallback, which parses files; the epoch only stats and hashes them, and its cost without the cap is 51 ms for `codex`. Decided 2026-11-12 by the owner: the epoch has its own cap, `MAX_EPOCH_FILES_SCAN = 20_000` files of the seed's language family, in `src/tools/symbol/call_graph/mod.rs` (classified `NOT_A_CAP` for `tests/result_caps.rs`: past it the epoch is unknown and the answer is computed afresh, nothing is shortened). The table above was measured with the shared cap of 5000. Under 20000 the `codex` tree (5096 files) gets an epoch, at the measured 51 ms. The worst case at the new cap, about 250 to 300 ms per call, is an extrapolation from 4 to 6 microseconds per walked file and was not measured at 20000.

Two more observations, not acted on. The epoch is applied to both directions (`one_hop_at` calls `with_epoch` for every resolution), so a `callees` resolution is also invalidated by an unrelated same-language edit. I first read that as waste, on the assumption that only `callers` results depend on files they do not name. Checked 2026-11-12 and not safe to assume: the signatures cover the definition, the call-site files and the next-hop definitions, but not the files a call passes through to reach its definition (a glob import or a re-export chain, for example `use crate::prelude::*` where `prelude.rs` re-exports the callee). An edit to such a file can change what a `callees` call resolves to without changing any signed file. I reasoned this from the code and did not reproduce it, so it is a mechanism and not a finding. Skipping the epoch for `callees` is therefore NOT done. And the first call on a tree that had not been touched recently took 131 to 280 ms against a 19 to 52 ms median: I attribute that to a cold inode cache (the file `stat` calls), which I did not verify, because dropping the caches needs root.

**Saving of a cache hit, measured 2026-11-12.** `call_graph(symbol, direction=callers, max_depth=2)` on the codescout repository itself through the live server, each call twice in a row (a cold call, then the same call), latency read from `tool_calls.latency_ms` in `.codescout/usage.db`. `call_edges.db` of the project was empty before the run, so every first call was cold. One run per pair, on a shared machine with other sessions running builds.

| Symbol | Callers found | Cold | Same call again | Saved |
|---|---|---|---|---|
| `detect_language` | 56 | 51 879 ms | 375 ms | 51 504 ms |
| `is_compiled_twin` | 33 | 966 ms | 184 ms | 782 ms |
| `disk_signature` | 22 | 410 ms | 210 ms | 200 ms |
| `scan_walk` | 35 | 173 ms | 101 ms | 72 ms |

The first `language_family` call (44 callers, run before the table as a warm-up) took 998 ms; the same call after the table was wiped took 422 ms and left 5 rows in `call_edge_resolutions` (the seed and four depth-2 nodes), all with `epoch` set, so a hit covers the depth-2 nodes too.

What this shows: in all four pairs the saving was larger than the epoch's own cost on this repository (about 30 ms), and a hit still costs 100 to 375 ms, so a hit is not free (the epoch, the signature checks and the placement of each next node, which is not cached). The saving ranged from 72 ms to 52 s; the 52 s cold call is one outlier, and its output says `truncated at depth 2`. I did not find out why it was slow. What it does not show: how often a hit happens. Between two of my queries `call_edges.db` went from 4 rows to 0, because `invalidate_file` deletes every resolution of the project on any codescout write and other sessions write here all the time. In a shared checkout the cache is short-lived whatever the epoch does.

**Invalidation by codescout writes, narrowed 2026-11-12 (owner: "ok, go" to a finer invalidation).** `EdgeCache::invalidate_file` deleted every cached resolution of the project on any write by a codescout write tool (`edit_file`, `create_file`, `edit_code`, `edit_markdown`). It now does so only when `can_hold_a_call(file)`: false for a file `detect_language` does not recognize (TOML, JSON, plain text, a file with no extension) and for markdown, true for every recognized language (HTML and CSS included). The raw `call_edges` rows of the written file are still deleted in every case.

Measured on this checkout, `usage.db`, successful `edit_file`, `create_file` and `edit_code` calls in the 24 hours before 2026-11-12, by extension of the `path` argument: 1565 writes with a path; `rs` 700, `py` 439, `md` 304, `txt` 57, `sh` 23, `json` 14, `toml` 11, the rest 17. 396 of them (25%) are in files the new rule skips. That is about one write in four that no longer clears the cache, so on this checkout a cached call graph can live about 1.3 times as long. That is an estimate from the write mix, not a measured hit rate, and the writes in `.rs` files (44%) still invalidate, now through the epoch as well.

Not done, deliberately: deleting only the resolutions of the written file's language family. It would also spare the 439 `py` writes, but the epoch covers only the seed's own family, so a codescout write that adds a Java caller of a Kotlin function would no longer clear a cached Kotlin result. That is a regression against today's behavior for codescout's own writes, so it needs its own decision.

## Fix provenance

Fixed in four commits. The limits that remain are listed in `## Fix` and in the `unverified` field.

- **SHA:** `5a7936984` (`experiments`): the signatures of the files a resolution names.
- **patch-id:** `ae42b2606d11`
- **SHA:** `702a0f737` (`experiments`): the project-level epoch; closes the residual case.
- **patch-id:** `1a8c59aee0d6`
- **SHA:** `c5da750ac` (`experiments`): the epoch has its own file cap (20000), so a tree above 5000 source files keeps its cache.
- **patch-id:** `bd9ade2b251a`
- **SHA:** `59e15ae0c` (`experiments`): a codescout write to a file that cannot hold a call no longer clears the cache.
- **patch-id:** `3540df6c9d06`

## Tests added

In `src/tools/symbol/call_graph/identity_tests.rs` (real parser, real edge cache, the real `CallGraph` tool, mock LSP; direction `callees`): `call_graph_does_not_serve_stale_edges_after_a_file_changed_outside_codescout` (was red under `#[ignore]`, now green) and its control `call_graph_is_current_once_a_write_invalidates_the_cached_edges`. The shared body `callees_after_a_change_made_outside_codescout` resolves `entry`, rewrites `src/lib.rs` from the test so that `entry` stays where it was and a second call appears, and resolves again from a new context over the same cache file.

In `src/tools/symbol/call_edges/cache.rs`, six unit tests on `EdgeCache`: hit while every named file is unchanged; miss when the definition file changed; miss when a call-site file changed with the SAME length (mtime only); miss when a file that was absent appears; miss for a row without signatures; an old database gets the column, the migration is idempotent, and its rows are misses. The existing `completed_resolution_reports_corrupt_payload_as_an_error` still holds.

Verified 2026-11-11: `cargo test --lib -- tools::symbol::call_edges tools::symbol::call_graph` 113 passed, 0 failed, 0 ignored; clippy with `-D warnings` clean; `./scripts/gate.sh` green (9442 tests, FMT, CLIPPY, LEAN, DEFAULT all 0).

Not covered by an automated test: the `callers` direction against a real language server (re-run by hand after the fix, steps 1 to 5 of the re-run); a second session through a real process boundary; a language other than Rust.

**The residual test is now green.** The text below is how it was added (red, `#[ignore]`); the epoch commit removed the `#[ignore]`. It was added 2026-11-11 in the same file, direction `callers`, with a mock whose scripted `references` reports a new caller on the second call (as rust-analyzer does for a file it sees by its own watcher): `call_graph_reports_a_caller_added_in_a_file_the_cached_result_does_not_name` (was `#[ignore]` and red: the second answer was `["a_caller"]` and lacked `b_caller`; green with the epoch) and its control `call_graph_reports_a_new_caller_once_a_write_invalidates_the_cached_edges` (passes). The test file's `context` helper was split into `symbols_client` and `tool_context` so a test can script `references`; its behavior is unchanged (`tools::symbol::call_graph`: 58 passed, 1 ignored).


**Epoch tests, 2026-11-11.** In `src/tools/symbol/scan_budget.rs`, five tests on `language_epoch`: it changes when a source file is added, touched or removed; it changes when a file is replaced by one with an OLDER mtime; it ignores files of other languages; it is `None` when the walk is cut short by its cap; a TypeScript seed covers JavaScript files. In `src/tools/symbol/call_edges/cache.rs`, three tests: a resolution is a hit only under the epoch it was stored under; an unknown epoch is never a hit and stores none; a resolution stored without an epoch is a miss under a known epoch. The migration test now checks both columns.

Verified 2026-11-11: `cargo test --lib -- tools::symbol::scan_budget tools::symbol::call_edges tools::symbol::call_graph` 135 passed, 0 ignored. On a clean `git archive HEAD` extract with my four files overlaid (with a throwaway `git init`, because one test calls `git ls-files`): `cargo clippy --workspace --all-targets` with `--features local-embed -D warnings` and with `--no-default-features -D warnings` both exit 0; `cargo nextest run --workspace --no-default-features --no-fail-fast` 9359 passed, 0 failed; `cargo nextest run --workspace --no-fail-fast` 9453 passed, 0 failed. `./scripts/gate.sh` on the shared tree was NOT green: it stopped at compile because another session's uncommitted edits to `src/tools/symbol/edit_code.rs` and `tests.rs` leave `replace_would_rename_reason` and `owned_hint` unused. I did not touch those files.


**Cap test, 2026-11-12.** `a_tree_above_the_workspace_scan_cap_still_stores_its_resolutions_under_an_epoch` in `identity_tests.rs`: a project with `MAX_WORKSPACE_FILES_SCAN + 1` Rust files, one `callees` call, then a read of `call_edge_resolutions`: every stored row must have `epoch` set. Red before the change (`left: 0, right: 1`: the row was stored without an epoch), green after. It does not test the value 20000 itself; the cut-off of `language_epoch` is tested in `scan_budget.rs`.

Verified 2026-11-12 on the shared tree: `./scripts/gate.sh` CLIPPY 0, LEAN 0 (9530 passed), DEFAULT 0 (9624 passed), Python light lane 1836 passed; FMT refused (`ownership-refused`) for `src/tools/run_command/tests.rs`, which is another session's, and for my `identity_tests.rs`, which I formatted with `rustfmt --edition 2021` after checking that its whole uncommitted diff was mine (one hunk, 0 deletions). `cargo test --test result_caps` 71 passed.


**Invalidation test, 2026-11-12.** In `cache.rs`, `a_write_to_a_file_that_cannot_hold_a_call_keeps_the_resolutions` (a write to `notes.md`, `config.toml`, `data.json` and `Dockerfile` keeps the resolution; a write to `caller.py` deletes it). In `identity_tests.rs`, `a_codescout_write_to_a_markdown_file_keeps_the_stored_resolutions`, through `Agent::invalidate_call_edges_for` after a real `call_graph` call (`NOTES.md` keeps the row, `src/lib.rs` deletes it). Both were red before the change (`left: None`; `left: 0, right: 1`). With `can_hold_a_call` forced to `true` (the old behavior) both fail again on the same assertions; I restored it and checked that `true ||` is gone.

Verified 2026-11-12 on the shared tree: `./scripts/gate.sh` CLIPPY 0, LEAN 0 (9533 passed), DEFAULT 0 (9627 passed), Python light lane 1836 passed. FMT was `ownership-refused` for `cache.rs` (the history has a coverage gap, not a named owner); I checked that its whole uncommitted diff was mine and ran `rustfmt --edition 2021` on that file only.

## Workarounds

Delete `.codescout/call_edges.db`, or write to the changed file with a codescout write tool. Cross-check a surprising `call_graph` result with `references`.

## Resume

Fixed in two commits (see `## Fix provenance`): named-file signatures (`5a7936984`) and the project-level epoch for the residual case. The record keeps its limits in `## Fix`: any same-language edit invalidates the language's cache, callers in another language family are not detected, and a capped walk turns the cache off for that call.

Next, if the owner wants more: (1) skipping the epoch for `callees` was checked and not done, for the reason in `## Fix`; (2) check whether this explains the label/bytes mismatch in `4e7ec7bfc047ac7b`, a lead I have not established.

Related: `a73c49456056fb92`, `58a45860b802a477`, `bug-fix-session-log:F-236`.

**Merge note, 2026-11-12.** Three other branches change the same files and conflict with `experiments` (`git merge-tree --write-tree`, nothing written to any tree): `fix/ecc-bfs-identity-20261110` (`identity_tests.rs`, `mod.rs`), `feat/ecc-callgraph-identity-20261109` (`cache.rs`, `mod.rs`) and `feat/ecc-navigation-truth-20261109` (`mod.rs`). Only the first conflict is caused by my commits: merged against `78729bfb3~1` it is clean, the other two conflict there too. For that first branch the three files are byte-identical to `dc769e430` ("integrate declaration-aware traversal") on `experiments`, and its tip `537fa820d` is not an ancestor of `experiments`, so it is the pre-integration source of an already integrated commit and has nothing to add on those files. Its conflict is one hunk in the `use` block of `mod.rs` and an add/add of `identity_tests.rs` (my helpers and tests against the copy that was identical). Its commits carry `Session-Id: codex:01a1bf68-e650-7f20-911e-b53f86597c27`, a thread that was not among the 5 loaded Codex threads at 2026-11-11T23:02Z, so no message was sent. Not checked: the detached review and mutation copies of these worktrees, and whether the other two branches are also pre-integration sources of `4776e44bd` and `3fe613bdd`.

## References

- `src/tools/symbol/call_graph/mod.rs`: `one_hop_at`
- `src/tools/symbol/call_edges/cache.rs`: `EdgeCache::lookup_resolution`, `invalidate_file`
- `src/agent/mod.rs`: `invalidate_call_edges`, `invalidate_call_edges_for`

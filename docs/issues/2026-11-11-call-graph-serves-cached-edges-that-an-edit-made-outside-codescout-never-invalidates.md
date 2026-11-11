---
id: '489e013ddfd6c3fc'
kind: bug
status: open
title: call_graph serves cached edges that an edit made outside codescout never invalidates
tags:
- cluster/unclassified
- navigation
opened: 2026-11-11
owner: marius
related:
- a73c49456056fb92
- '58a45860b802a477'
- '4e7ec7bfc047ac7b'
severity: medium
unverified: Measured by hand in one scratch crate (callers direction, mux transport, before the fix) and by test in the callees direction with tree-sitter edges and a mock LSP, after it. The callers direction was not re-run after the fix and no test covers it. A caller added in a file the cached result does not mention is NOT fixed and has no test. A git checkout, a second session and other languages were not run. The mechanism of the spurious ts edge is inferred, not traced. The link to 4e7ec7bfc047ac7b is a lead, not a finding. Severity is my proposal.
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

**Residual, not fixed:** an entry is valid while the files it NAMES are unchanged. A caller added in a file that the result does not mention (a new file, or an existing file with no call of the symbol) does not change any signed file, so the entry is still served and the caller is missing. That is the case that first showed the problem (`other.rs` did not exist when the entry was written). It is not covered by a test. Options, none tried: (a) a project-level epoch (for example the newest mtime and the count of the source files, one walk per call), at a cost per call and with unrelated edits invalidating entries; (b) do not reuse a `callers` resolution across calls; (c) a file watcher in codescout, which does not exist today (`mark_file_dirty` marks only codescout's own writes).

Small accepted window: the signatures are read after the analysis, so a file that changes while one analysis runs is stored with its new signature beside edges from its old content.

## Fix provenance

This is a partial fix: the record stays `open` for the residual case in `## Fix`.

- **SHA:** `5a7936984` (`experiments`)
- **patch-id:** `ae42b2606d11`

## Tests added

In `src/tools/symbol/call_graph/identity_tests.rs` (real parser, real edge cache, the real `CallGraph` tool, mock LSP; direction `callees`): `call_graph_does_not_serve_stale_edges_after_a_file_changed_outside_codescout` (was red under `#[ignore]`, now green) and its control `call_graph_is_current_once_a_write_invalidates_the_cached_edges`. The shared body `callees_after_a_change_made_outside_codescout` resolves `entry`, rewrites `src/lib.rs` from the test so that `entry` stays where it was and a second call appears, and resolves again from a new context over the same cache file.

In `src/tools/symbol/call_edges/cache.rs`, six unit tests on `EdgeCache`: hit while every named file is unchanged; miss when the definition file changed; miss when a call-site file changed with the SAME length (mtime only); miss when a file that was absent appears; miss for a row without signatures; an old database gets the column, the migration is idempotent, and its rows are misses. The existing `completed_resolution_reports_corrupt_payload_as_an_error` still holds.

Verified 2026-11-11: `cargo test --lib -- tools::symbol::call_edges tools::symbol::call_graph` 113 passed, 0 failed, 0 ignored; clippy with `-D warnings` clean; `./scripts/gate.sh` green (9442 tests, FMT, CLIPPY, LEAN, DEFAULT all 0).

Not covered: the residual above (a caller in an unmentioned file); the `callers` direction (measured by hand only, before the fix; not re-run after it); a second session through a real process boundary; a language other than Rust; the new binary was not run against the scratch crate.

## Workarounds

Delete `.codescout/call_edges.db`, or write to the changed file with a codescout write tool. Cross-check a surprising `call_graph` result with `references`.

## Resume

Partly fixed by `5a7936984` (see `## Fix provenance`). Status stays `open` because the residual case (a caller added in a file the cached result does not mention) is not fixed, and it is the case that started this investigation.

Next: (1) re-run the manual `callers` sequence of the table with the rebuilt binary (steps 2, 3 and 5 should now show current lines and `late_caller`); (2) with the owner, choose among the residual options and write the red test for it (a mock LSP that reports a caller in a new file on the second call); (3) check whether this explains the label/bytes mismatch in `4e7ec7bfc047ac7b`, a lead I have not established.

Related: `a73c49456056fb92`, `58a45860b802a477`, `bug-fix-session-log:F-236`.

## References

- `src/tools/symbol/call_graph/mod.rs`: `one_hop_at`
- `src/tools/symbol/call_edges/cache.rs`: `EdgeCache::lookup_resolution`, `invalidate_file`
- `src/agent/mod.rs`: `invalidate_call_edges`, `invalidate_call_edges_for`

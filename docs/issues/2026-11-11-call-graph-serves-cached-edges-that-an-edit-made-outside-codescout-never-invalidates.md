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
unverified: Measured in one scratch crate, rust-analyzer, mux transport, callers direction only, one run per step. The callees direction, a git checkout, a second session and other languages were not run. The mechanism of the spurious ts edge is inferred, not traced. Whether invalidate_file clears completed-resolution entries was not read. The link to 4e7ec7bfc047ac7b is a lead, not a finding. No fix designed or tried. Severity is my proposal.
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

NOT established: whether `invalidate_file` also clears the completed-resolution entries (the control in step 6 shows the effect for `edit_file`, not the mechanism); the cross-session persistence was read from the code and not run; the `callees` direction was not run.

## Evidence

The tool outputs in the table, copied from the session.

## Hypotheses tried

- The language server is stale: **refuted**. `references` and `symbols` were correct in the same state, and removing the cache file alone fixed `call_graph`.
- The cache is never invalidated at all: **refuted** by step 6.

## Fix

Pending, for the owner. Candidates, none tried: (1) store, with each cached resolution, the `(len, mtime)` signature of every file its edges mention, and treat the entry as a miss when one differs (the same signature the LSP client already uses, `disk_signature` in `src/lsp/client.rs`); (2) invalidate from the point where the LSP client detects drift (`resync_all_drifted`), which couples the client to the cache; (3) stop reusing a completed resolution across calls, at the cost of an LSP round trip per hop. Candidate (1) also covers `git checkout` between sessions, which (2) would not.

## Tests added

None.

## Workarounds

Delete `.codescout/call_edges.db`, or write to the changed file with a codescout write tool. Cross-check a surprising `call_graph` result with `references`.

## Resume

Open, measured on the mux transport with the new binary, one crate, callers direction. Severity `medium` is my proposal: a silent wrong answer (stale lines, a missing caller, an invented edge) from the tool that agents use for impact analysis, with no age or warning, and it persists across sessions. Next: write the failing test (a resolution cached, then an external write to a caller file, then a second `call_graph`), then pick the fix with the owner. Check whether this explains the label/bytes mismatch in `4e7ec7bfc047ac7b`, which was observed in call-graph work on a retained worktree; I did not establish that. Related: `a73c49456056fb92`, `58a45860b802a477`.

## References

- `src/tools/symbol/call_graph/mod.rs`: `one_hop_at`
- `src/tools/symbol/call_edges/cache.rs`: `EdgeCache::lookup_resolution`, `invalidate_file`
- `src/agent/mod.rs`: `invalidate_call_edges`, `invalidate_call_edges_for`

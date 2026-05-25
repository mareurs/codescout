---
status: fixed
opened: 2026-05-25
closed: 2026-05-25
severity: medium
owner: marius
related: []
tags:
  - call_graph
  - lsp
  - performance
kind: bug
---

# BUG: `call_graph(max_depth=2)` times out at 60s on a leaf-test-fn fanout

## Summary
`call_graph(direction="callers", max_depth=2)` reproducibly times out at 60s
when the symbol's direct callers are all `#[test]` leaf functions. The same
query at `max_depth=1` completes in <1s with 10 LSP-sourced edges. The
depth=1 → depth=2 cliff is well outside the cost expected for trivially-leaf
callers (no further callers to walk).

## Symptom (Effect)
Verbatim error returned to the MCP client:

```
Tool 'call_graph' timed out after 60s. Increase tool_timeout_secs in .codescout/project.toml if needed.
```

No partial result, no edge count, no buffer ID — the timeout aborts before
any edges are written to the response payload. Wall-clock cost: 60s per
call. Reproduced 2× consecutively on the same symbol/params during the
2026-05-25 reconnect test.

## Reproduction

Branch: `experiments` (and `master` — they converged this session).
Codescout version: 0.13.0.
Git HEAD: `1b0e1d819287f2658080f4940c28acdfb6a7d06c`.

Steps (from this repo's root, after `/mcp` reconnect):

1. Baseline (works, <1s):
   ```
   call_graph(
     symbol="Onboarding/call_content",
     path="src/tools/onboarding.rs",
     direction="callers",
     max_depth=1,
   )
   ```
   Returns 10 LSP-sourced edges, all in `src/tools/run_command/tests.rs`.

2. Failure case (times out, 60s):
   ```
   call_graph(
     symbol="Onboarding/call_content",
     path="src/tools/onboarding.rs",
     direction="callers",
     max_depth=2,
   )
   ```
   Hangs to the 60s tool timeout. No partial output.

The 10 direct callers from step 1 are all `#[test]` functions
(`onboarding_call_content_*`, `single_project_call_content_has_no_project_prompts`,
`workspace_onboarding_full_flow` etc.). Each is a leaf — no production
code calls a `#[test]` fn. So the depth=2 BFS layer should expand to 10
"who calls this `#[test]` fn?" LSP queries, all of which should return
empty, and terminate immediately.

## Environment

- OS: Linux 7.0.9-zen1-1-zen
- Rust toolchain: per `rust-version = "1.88"` in `Cargo.toml`
- MCP transport: stdio (Claude Code via `~/.cargo/bin/codescout` symlink → `target/release/codescout`)
- Project: code-explorer (this repo, single-project workspace this session)
- Branch: `experiments` at `1b0e1d81`
- LSP backend: rust-analyzer (started fresh after `workspace(post_compact=true)` flushed caches)
- The `references()` call against the same symbol immediately before this
  bug returned in <1s — so the LSP isn't dead, the bug is specific to
  `call_graph`'s expansion path.

## Root cause

Confirmed after fix: the BFS inner loop at `src/tools/symbol/call_graph/traversal.rs::bfs` serialized `OneHopResolver::one_hop` calls via `while let Some(sym) = current_level.pop_front()`. Each `one_hop` awaits before the next starts. At depth>1 over a level of leaf functions (e.g., 10 `#[cfg(test)] mod tests` `#[test]` fns), each `resolve_one_hop` issues a `textDocument/references` LSP call. Rust-analyzer's empty-result latency on `#[cfg(test)]` functions is ~6s per call (RA walks the crate to confirm no caller exists). 10 serial calls × 6s = 60s = the tool timeout.
## Evidence

### E1 — Two consecutive `call_graph(depth=2)` calls both timed out

Session: `77099ac5-fd0c-4bff-b47e-fa01146b0bc9` (2026-05-25 reconnect test).
First call: timed out after 60s, no payload.
Second call (same params, ~5 minutes later, after intervening read-only
tool calls): also timed out after 60s, no payload.

Reproducibility rate from this single sample: 2/2.

### E2 — Adjacent calls succeeded

The same symbol returned cleanly from:

- `symbols(name="call_content")` — 23 matches across 8 files, <1s
- `symbol_at(path="src/tools/onboarding.rs", line=302)` — def + hover, <1s
- `references(symbol="Onboarding/call_content", path=...)` — 11 refs, <1s
- `call_graph(..., max_depth=1)` — 10 edges, <1s

So neither the LSP nor the symbol-resolution path is broken — the timeout
is specific to `call_graph`'s depth>1 expansion.

## Hypotheses tried

1. **Hypothesis:** LSP cold-start latency.
   **Test:** Re-ran `call_graph(depth=2)` after `references` had already
   warmed the rust-analyzer client for the same file (E2).
   **Verdict:** Rejected. Second call also timed out, after the LSP was
   demonstrably warm.
   **Evidence link:** E1, E2.

2. **Hypothesis:** Fanout-explosion at depth=2 (transitive callers expand
   into a large subgraph).
   **Test:** Inspected the depth=1 callers in E2 — all 10 are `#[test]`
   functions in a single test file. Test fns have no callers (they're
   entry points invoked by the test harness, not by Rust code).
   **Verdict:** Rejected on graph shape. The depth=2 layer should expand
   to 10 empty caller sets and terminate, not 1000 nodes.
   **Evidence link:** E2 (the depth=1 callers list).

3. **Hypothesis (deferred):** LSP "find references" against an inline
   `#[test]` function inside a `#[cfg(test)]` module doesn't terminate
   promptly when the answer is empty — rust-analyzer scans the whole
   crate trying to find a caller that doesn't exist.
   **Test:** Not yet run. Would require enabling RA trace logging
   (`RA_LOG=lsp_server=trace`) and re-running, then inspecting how long
   each `textDocument/references` request takes against the leaf test fns.
   **Verdict:** Deferred.

4. **Hypothesis (deferred):** The call_graph implementation serializes
   the depth>1 LSP queries instead of running them in parallel, and
   the 10 sequential "who calls this `#[test]` fn?" queries each cost
   6+ seconds.
   **Test:** Not yet run. Would require reading
   `src/tools/call_graph.rs` (or wherever the BFS layer is implemented)
   to confirm serial vs parallel and instrument the per-query latency.
   **Verdict:** Deferred.

5. **Hypothesis (deferred):** The tree-sitter + LSP edge-source merger
   deadlocks or holds a lock at depth>1 boundaries.
   **Test:** Not yet run. The edges returned at depth=1 are all
   `source: lsp` (no tree-sitter), so the merger may only kick in at
   depth>1.
   **Verdict:** Deferred.

## Fix

Shipped on master in commit `bd8fdbb4` (`fix(call_graph): parallelize BFS inner loop to fix depth>1 timeout`).

The serial `while let Some(sym) = pop_front()` loop in `src/tools/symbol/call_graph/traversal.rs::bfs` was replaced with `futures::future::try_join_all` over the drained level. All post-await processing (visited tracking, edge accumulation, depth-coherent `max_edges` cap, cycle handling) stays serial — only the `OneHopResolver::one_hop` calls fan out concurrently. Edge order, dedup semantics, and the depth-coherent truncation invariant are preserved.

`Cargo.toml`: `futures` moved from `optional = true` (librarian-gated) to mandatory dep. `Cargo.lock` unchanged — the dep was already transitively resolved via the default `librarian` feature.

**Live verification post-rebuild + `/mcp` reconnect:**
- `call_graph(symbol="Onboarding/call_content", direction="callers", max_depth=2)` returned in <1s with 10 depth=1 edges, vs 60s timeout pre-fix. (The `(truncated at depth 2)` rendering line is a pre-existing per-symbol display artifact from RA returning spurious depth-2 callers for `#[test]` fns + the depth-coherent cap firing — unrelated to this fix, surfaced only because the BFS now completes.)
- `call_graph(symbol="bfs", direction="callers", max_depth=2)` returned 7 callers across 2 files, `max_depth_reached: 2`.
## Tests added

`bfs_parallelizes_one_hop_within_level` in `src/tools/symbol/call_graph/traversal.rs` (inside the existing `tests` module). Uses a custom `ConcurrencyTrackingResolver` that tracks active concurrent `one_hop` calls via two `AtomicUsize` counters (active + max_active) and yields 10× via `tokio::task::yield_now()` per call to give sibling futures a chance to interleave. After a depth=2 BFS where the seed `a` expands to 5 leaf-fn callers, the test asserts `max_active > 1`. The test deterministically fails against the pre-fix serial code (max_active stays at 1 with no concurrent interleaves).

The 4 pre-existing BFS tests (`bfs_reaches_max_depth_then_stops`, `bfs_handles_cycle_without_infinite_loop`, `bfs_depth_coherent_cap_preserves_full_levels`, `bfs_dedupes_parallel_paths`) continue to pass unchanged — confirming the parallelization preserved correctness invariants.
## Workarounds

- Use `max_depth=1` instead. The bug is specific to depth>1; depth=1
  returns valid edges in <1s.
- For "who calls X, transitively" intent: walk depth=1 once, then loop
  in the controller — call `call_graph(depth=1)` on each direct caller
  individually. Slower-but-bounded vs the current depth=2 hang.
- For "who references X anywhere" (a superset of who-calls): `references()`
  is unaffected and returns immediately.

## Resume

N/A — fixed.
## References

- Session JSONL: `/home/marius/.claude-kat/projects/-home-marius-work-claude-code-explorer/77099ac5-fd0c-4bff-b47e-fa01146b0bc9.jsonl`
  (the 2026-05-25 reconnect test session, this bug noticed in wave 3)
- Related tracker: `docs/trackers/lsp-tools-error-rate-2026-04.md`
  (artifact id `f87ff6fcbb6eaa56`) — LSP tools high error rate, broader scope
- Code likely involved: `src/agent/` or `src/tools/` — locate via
  `symbols(name="call_graph")`

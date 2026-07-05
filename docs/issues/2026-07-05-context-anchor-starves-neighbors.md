---
status: open
opened: 2026-07-05
closed:
severity: medium
owner: marius
related: []
tags: [librarian, context, link-graph, budget]
kind: bug
---

# BUG: context(anchor_id) — a large anchor starves its neighbors; max_tokens not applied to the anchor

## Summary
`librarian(action="context", anchor_id=…)` correctly gathers the anchor's link
neighborhood as candidates, but the packer spends the whole token budget on the
anchor's own body: a large anchor returns `included_ids=[anchor]` and zero
neighbors — at `max_tokens=800` AND at the default 4000, with byte-identical
output. Hub artifacts (the ones with the most links) are exactly the ones whose
neighborhoods become unreachable.

## Symptom (Effect)
With the audit log (80 KB body) carrying 3 outgoing `cites` edges (verified via
`artifact(graph)`), `context(anchor_id="59ebeebb6ed05c89")` returned
`included_ids=["59ebeebb6ed05c89"]` and 17,906 bytes of markdown — identical at
`max_tokens=800` and at the default. Neighbors (f2ecdd76a6189efb,
e522954737601d13, c43df94e69ca915f) absent.

## Reproduction
Seed edges (`librarian(action="link_scan", write=true)`), then
`librarian(action="context", anchor_id="59ebeebb6ed05c89", max_tokens=800)` —
observe single-id `included_ids` and ~17.9 KB markdown.

## Environment
codescout `experiments` @ 1d10c072 (first session with a populated link graph —
the starvation was invisible while the graph was empty).

## Root cause
Candidate gathering is correct (`src/librarian/tools/context.rs:124-138` —
anchor + outgoing dsts + incoming srcs). Hypothesis (unverified in detail): the
downstream packer includes the anchor first — apparently un-trimmed and
over-budget — and exhausts `max_tokens` before any neighbor is packed. Needs a
read of the packing loop below `candidate_ids`.

## Evidence
Live session 2026-07-05: `artifact(graph, id=59ebeebb…, depth=1)` → 4 nodes /
3 `cites` edges; `context(anchor_id=…)` at two budgets → same 17,906-byte
result, `included_ids` = anchor only.

## Hypotheses tried
1. **Budget too small at 800.** Test: re-ran with default 4000. **Verdict:
   rejected as the whole story** — byte-identical output; budget change had no
   effect at all, so the anchor isn't being trimmed to budget either.

## Fix
Plan: reserve a fraction of the budget for neighbors (e.g. anchor capped at
50%, or anchor included as preview/heading-map when oversized), and honor
`max_tokens` for the anchor itself. Not implemented.

## Tests added
N/A — not yet fixed.

## Workarounds
Use `artifact(action="graph", id=…)` + targeted `artifact(get, heading=…)` per
neighbor instead of anchor-mode context for large hubs.

## Resume
Read the packing loop downstream of `candidate_ids` in
`src/librarian/tools/context.rs` (below line 138); confirm anchor-first
un-trimmed inclusion; decide reserve-ratio vs preview-fallback; add a corpus
test: large anchor + 3 neighbors → all 4 ids in `included_ids`.

## References
`src/librarian/tools/context.rs:124-138`; seeded graph from
`librarian(link_scan)` (experiments:dc35c70e).

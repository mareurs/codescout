---
kind: bug
status: open
title: 'doc(action="list_stale") on an augmented tracker routes to tracker-conventions and is served only its preamble: the result names the tracker in items[].abs_path and no section declares the shape'
tags:
- cluster/declared-not-wired
- guides
- section-grain-delivery
- topic-routing
opened: 2026-10-17
closed:
owner: marius
severity: low
related:
- docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
- docs/issues/2026-10-16-observed-shapes-gate-skips-every-doc-shape.md
---

# BUG: a real doc.list_stale reaches tracker-conventions and gets no section

## Summary

`doc(action="list_stale")` returns `{count, threshold_hours, items: [{id, kind, title, abs_path, ...}], ...}`
(`src/librarian/tools/refresh_stale.rs`). `names_path_containing` reads `abs_path` inside `items[]`, and an
augmented tracker lives under `docs/trackers/`, so `LibrarianAdapter::relevant_guide_topic` picks
`tracker-conventions` for the call. No `tracker-conventions` section declares `doc.list_stale`, so the
caller is served only the topic preamble, with the line "no section declares this call's shape".

## Class fit

A route (the result-based topic selector) that reaches a declaring topic whose sections were never
written for the call: the router, the matcher and the sections each behave as specified, and the shape
the router sends here has no section, so the delivery is the preamble alone.

## Symptom (Effect)

A session that lists stale augmented trackers receives the `tracker-conventions` preamble (891 B in the
measurement below) rather than whatever `librarian` serves for this action (unmeasured whether any
`librarian` section declares `doc.list_stale`; the result's own `next_step` names `doc(gather)` and
`doc(update, commit_refresh=true)`). Nothing errors.

## Reproduction

In-process test server (`make_server` in `src/server.rs`), `call_tool` helper, 2026-10-17:

1. `doc(action="create", rel_path="docs/trackers/zz-measure.md", kind="tracker", title=..., body=...)`
2. `doc(action="augment", id=<id>, augment={prompt: "keep it", params: {}})`
3. `doc(action="list_stale", threshold_hours=0)`

The guide block that rides the result begins
`<!-- auto-injected get_guide('tracker-conventions') preamble — no section declares this call's shape. -->`.

## Environment

codescout 0.15.0, branch `experiments` + `53a53782`, 2026-10-17.

## Root cause

`names_path_containing` (`src/util/librarian_response.rs`) scans `abs_path`/`rel_path` per item of an
`items` array, written for `find`. `list_stale` reuses the key `items` with `abs_path`, so it matches by
coincidence of shape. Measured 2026-10-17 in the same run, for comparison: `doc(gather)`, `doc(graph)`,
`doc(event_create)` and `doc(event_list)` against the same tracker name no tracker path and route to
`librarian` (the `event_list` call is served `librarian` § Event Log, `graph` § Relationship Map).

The sibling ledger entry's runtime table lists `list_stale` with the actions whose result has "no path
key"; that row is wrong, which is why this was not found there.

## Evidence

The reproduction output above; Gate 2 (`every_observed_shape_of_a_declaring_topic_has_a_section`) found the
shape once it resolved census rows renamed from `artifact.*` to `doc.*`, and its probe `{"abs_path": ...}`
stands for exactly this routing for `doc.list_stale`.

## Hypotheses tried

1. **Hypothesis:** the gate's synthetic probe over-reports, as it does for the write shapes.
   **Test:** a real `list_stale` call against an augmented tracker (above). **Verdict:** rejected: the
   real result routes to `tracker-conventions`.

## Fix

Not started, and not designed here. The choice is the same one the sibling entry records: serve
`doc.list_stale` from a `tracker-conventions` section (the sections would have to be written and declared),
or stop `list_stale`'s `items[].abs_path` from routing here (the router is a decision), or declare the
`librarian` guide's refresh section as the answer and let the preamble stand. When one is chosen, delete
`doc.list_stale` from `TRACKER_DOC_SHAPES_UNSERVED` (`src/prompts/mod.rs`).

## Tests added

None. `every_observed_shape_of_a_declaring_topic_has_a_section` holds the state open by waiving
`doc.list_stale` under `tracker-conventions`, with this entry named in the reason.

## Workarounds

`get_guide("librarian")` and `get_guide("tracker-conventions")` return the whole topic on request.

## Resume

Read `src/librarian/tools/refresh_stale.rs` (the result shape) and `LibrarianAdapter::relevant_guide_topic`
(`src/librarian/adapter.rs`) first; decide the route, then delete the waiver token.

## References

- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`
- `docs/issues/2026-10-16-observed-shapes-gate-skips-every-doc-shape.md`
- `src/prompts/mod.rs` `TRACKER_DOC_SHAPES_UNSERVED`

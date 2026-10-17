---
kind: bug
status: fixed
title: 'doc(action="list_stale") on an augmented tracker routes to tracker-conventions and is served only its preamble: the result names the tracker in items[].abs_path and no section declares the shape'
tags:
- cluster/declared-not-wired
- guides
- section-grain-delivery
- topic-routing
opened: 2026-10-17
closed: 2026-10-17owner: marius
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

Fixed 2026-10-17 by a routing change plus a split, with no new guidance written.

Re-verified first with a real call (in-process test server): `doc(create, docs/trackers/..., augment)`
then `doc(list_stale, threshold_hours=0)` shipped an 891 B `tracker-conventions` preamble ("no section
declares this call's shape"), as this entry records.

Of the three options, a section declared in `tracker-conventions` was not taken: that guide holds no
text about list_stale or the refresh cycle (grep of `list_stale|commit_refresh|gather` finds nothing
about it), and writing some would be new guidance. The content a list_stale caller needs already
exists in the `librarian` guide: the refresh cycle (gather, synthesize, update with
`commit_refresh=true`) and the stale check.

- **Routing:** `names_tracker_path` (`src/librarian/adapter.rs`) returns false for a result carrying
  `threshold_hours`, a key only `list_stale` emits. `find` returns the same `items[].abs_path` shape
  without it and still routes to `tracker-conventions`. The exclusion is in the adapter, not in
  `names_path_containing`, so no `path~` predicate changes meaning.
- **Mis-homed content:** after the routing change the call was served `librarian` § Augmentation
  Lifecycle, which declares `doc.list_stale` but holds neither the refresh cycle nor the stale check:
  both sat under the `### Changing ONE entry` subsection, declared for `doc.update_entry` only (an
  earlier split left them there). They are now their own section, `### Refreshing an augmented
  artifact — the cycle and the stale check` (about 0.5 KB, under the 2.5 KB cap), declared
  `serves: doc.list_stale`. Moved, not duplicated.

Not done: `doc.gather` is declared in § Augmentation Lifecycle but its caller also needs that cycle
(step 1 is `gather`). Declaring the new section for `doc.gather` as well would change what a `gather`
call ships, which this entry does not cover; left as a follow-up decision.

Merge note: the observed-shapes branch (`worktree-agent-ab7a7e0e672e14d3a`) waives `doc.list_stale`
under `tracker-conventions` (`TRACKER_DOC_SHAPES_UNSERVED`) because its Gate 2 probe is
`{"abs_path": ...}`, which the router now treats differently from a real `list_stale` result. After
merging, give that probe a real `list_stale` result shape (with `threshold_hours`), and delete the
waiver token; this branch has neither the gate nor the token, so nothing was edited in `mod.rs`.

## Tests added

Red before the fix, green after:

- `librarian::adapter::tests::a_list_stale_result_is_not_a_tracker_operation_even_though_its_items_name_trackers`
  (a `list_stale` result does not route; the same `items` without `threshold_hours`, a `find`, still does)
- `server::guide_hint_tests::list_stale_on_an_augmented_tracker_ships_the_refresh_cycle_not_a_preamble`
  (real create-with-augment then `list_stale`: RED at first with `got 891 B: <!-- auto-injected
  get_guide('tracker-conventions') preamble`; RED again after the routing change alone with `got 1725
  B: ... § Augmentation Lifecycle`, which lacked the cycle; green once the section was split. It also
  asserts the section does not ship a second time.)

## Workarounds

`get_guide("librarian")` and `get_guide("tracker-conventions")` return the whole topic on request.

## Resume

Nothing owed here. See the merge note under Fix for the observed-shapes branch's probe and waiver token,
and the `doc.gather` follow-up.

## Fix provenance

- **SHA:** `40e2a621` (`worktree-agent-a666e4cdd398d0f53`)
- **patch-id:** `44c3c448d759`

## References

## References

- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`
- `docs/issues/2026-10-16-observed-shapes-gate-skips-every-doc-shape.md`
- `src/prompts/mod.rs` `TRACKER_DOC_SHAPES_UNSERVED`

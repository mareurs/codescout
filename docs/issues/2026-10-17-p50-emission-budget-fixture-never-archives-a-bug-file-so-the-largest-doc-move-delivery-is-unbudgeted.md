---
kind: bug
status: open
title: 'the p50 emission-budget fixture moves docs/specs to docs/specs, so archiving a bug file (about 9.6 KB of tracker-conventions sections after a doc.move) is outside every session-level byte budget'
tags:
- cluster/instrument-omits-the-dimension-that-grows
- guides
- section-grain-delivery
opened: 2026-10-17
closed:
owner: marius
severity: low
related:
- docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
---

# BUG: no session-level byte budget covers a bug-file archive move

## Summary

`a_p50_session_stays_under_the_committed_emission_byte_ceiling` (`src/server.rs`) sums every block
after the primary across the p50 session's six artifact/librarian shapes. Its `move` step relocates
`docs/specs/...` to `docs/specs/p50-fixture-moved.md`, a path under neither `docs/issues/` nor
`docs/trackers/`. Once `doc.move` routes to `tracker-conventions` for tracked paths (2026-10-17), a
bug-file archive ships about 9.6 KB of sections after the move, and that delivery is in no budget:
the p50 ceiling cannot see it because the fixture never makes the call.

## Class fit

The instrument sums the blocks of the shapes its fixture issues, and the dimension that just grew
(the sections a `move` of a tracked path draws) is one the fixture does not exercise, so it can stay
green while that delivery grows without bound; the instrument omits the dimension that grows.

## Symptom (Effect)

A session that archives bug files receives about 9.6 KB of `tracker-conventions` sections on the
first archive move, in addition to the `librarian` section served at the pre-call hold, and no
test notices if a later edit adds 5 KB to those sections. Each section is held to 2,500 B by Gate 5
and the whole set to the auto-inject bound by `tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`,
but there is no total budget for the move.

## Reproduction

```
grep -n 'p50-fixture-moved' src/server.rs
```

The `new_rel_path` names `docs/specs/`, so `names_path_containing` never matches on that call.

## Environment

codescout 0.15.0, branch `experiments` + the 2026-10-17 `doc.move` routing fix.

## Root cause

The fixture was written when `move` could not route to `tracker-conventions`; its path choice was
harmless then. The routing change made the same call shape deliver different bytes depending on the
path, and the fixture holds one path.

## Evidence

`src/server.rs`, `a_p50_session_stays_under_the_committed_emission_byte_ceiling`, the `move` step
(`"new_rel_path": "docs/specs/p50-fixture-moved.md"`). Section sizes measured 2026-10-17 from
`src/prompts/guides/tracker-conventions.md`: 0.8 + 2.1 + 2.2 + 1.4 + 1.7 + 1.3 KB.

## Hypotheses tried

None. Observed by reading the fixture.

## Fix

Not started. Either add a bug-archive step (create under `docs/issues/`, move to
`docs/issues/archive/`) to the p50 fixture and re-derive its ceiling (a spec amendment, since the
ceiling is derived, not chosen), or add a separate budget test for the archive flow with its own
derived ceiling, which leaves the p50 figure untouched. The second is smaller and keeps the p50
population honest: the p50 session does not archive bugs.

## Tests added

None.

## Workarounds

None needed: each section is individually capped and the set is bounded by the auto-inject limit.

## Resume

Decide between the two options in Fix. Measure the delivery with real `doc` calls (see
`server::guide_hint_tests::moving_a_bug_file_or_a_tracker_ships_the_archive_sections`).

## References

- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`

---
kind: bug
status: fixed
title: 'the p50 emission-budget fixture moves docs/specs to docs/specs, so archiving a bug file (about 9.6 KB of tracker-conventions sections after a doc.move) is outside every session-level byte budget'
tags:
- cluster/instrument-omits-the-dimension-that-grows
- guides
- section-grain-delivery
opened: 2026-10-17
closed: 2026-10-17owner: marius
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

Fixed 2026-10-17 as a SEPARATE worst-case budget test, not a step in the p50 fixture.

Why separate: a p50 session does not archive bug files, and the p50 ceiling is derived from the
shapes a p50 session issues (measured 12,116 B + 128 B). Adding the archive to it would either raise
that ceiling, which the byte-budget policy forbids (they only move down), or hide a regression in the
common case behind the slack the rare case needs. Two populations get two ceilings, and the p50
fixture and its ceiling are untouched.

`server::guide_hint_tests::a_bug_archive_move_stays_under_its_own_emission_byte_ceiling` creates a
bug under `docs/issues/`, makes the held first `doc.move` (asserted held), re-issues it, and sums every
block after the primary on both calls (the librarian section served at the hold plus the six
tracker-conventions sections served after the move). Measured by running it at ceiling 0:
**13,758 B** (hold 2,271 B + executed 11,487 B). Ceiling **13,886 B**, the same 128 B margin the p50
ceiling carries. Asserts no counted block carries the fixture root (so the total is not a property of
this machine's temp path) and that all six sections' distinctive sentences are present (a vanished
section lowers a byte count, so the count alone cannot catch it).

## Tests added

`server::guide_hint_tests::a_bug_archive_move_stays_under_its_own_emission_byte_ceiling`. Observed
RED by mutation: appending 442 B of padding to a served archive section failed it with `a bug-archive
move emitted 14200 B after the primary (hold 2271 B + executed 11929 B), ceiling 13886 B`; the file
was restored and the test passes at 13,758 B.

## Workarounds

None needed: each section is individually capped and the set is bounded by the auto-inject limit.

## Resume

Nothing owed. If the budget fires, shrink or split a `doc.move` section rather than raising the
ceiling. The anonymous tier (no hold) ships fewer bytes and is covered by the same ceiling.

## Fix provenance

- **SHA:** `46bffcf1` (`worktree-agent-a666e4cdd398d0f53`)
- **patch-id:** `ee0a4bac5516`

## References

## References

- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`

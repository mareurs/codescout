---
id: '084e2642586069b3'
kind: bug
status: fixed
title: 'RESIDUAL: Add serves: section declarations to the tracker-conventions guide so the first tracker-path call ships a section, not the whole 39 KB topic'
tags:
- cluster/declared-not-wired
closed: 2026-10-16
unverified: 'TRACKED b9eec23561e3299b — the write-shape sections (update, append_entry, move, delete) cannot reach their tracker-conventions sections, 13 of 24, so those are pull-only. Also not checked against a rebuilt live MCP (cargo rb + /mcp): delivery was driven through in-process doc calls only.'
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add serves: section declarations to the tracker-conventions guide so the first tracker-path call ships a section, not the whole 39 KB topic.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md` (status `fixed`):

> Cost, not correctness. Reachability is fixed, mutation-verified, and confirmed live 2026-08-31 09:49Z. What remains: a session's FIRST tracker-path-naming call still ships tracker-conventions WHOLE (39,106 B) and no section — deliberate, since that route closed 32736ca0 — so the 26x overshoot stands on that one call until tracker-conventions adopts `serves:`. See § Resume item 1.

## Fix

Re-checked against HEAD first, as the parent asked: the caveat had been **partly overtaken**. At `3923da8c` `tracker-conventions` is 60,203 B (the parent said 39,106 B) and, being over `MAX_AUTO_INJECT_GUIDE_BYTES`, its first tracker-path call already shipped a one-line POINTER, not the body. So the live cost was no longer 26x of a section; it was that the guide's content never auto-delivered at all.

What was done (`d34c398e`):

- `tracker-conventions.md` split at `###` where a section would otherwise exceed `MAX_DECLARED_SECTION_BYTES` (2,500 B) and carries `<!-- serves: ... -->` declarations, all **path-qualified** (`doc.create(path~docs/trackers/)` and so on). 24 sections, 11 declared.
- Only `doc.create`, `doc.get`, `doc.find` and `librarian.doctor` produce a result that names a tracker or bug path. Measured 2026-10-16 by making each `doc` call and reading the result. So only those can route here, and only those are declared. The other 13 sections (43 KB) are waived in `prompts::SECTION_WAIVERS` with that reason, because declaring them would be a declaration nothing reaches.
- `librarian.doctor` is not served per violation class (`Shape` cannot qualify on one), so it ships the preamble, as the pointer did.
- `no_two_topics_declare_an_overlapping_shape` is path-aware: a path-qualified shape beside an unqualified one in another topic is a narrowing; two of the same kind stay ambiguous. It was vacuous until now; it fires (verified by mutation) the moment a tracker-conventions declaration drops its `path~`.
- Compatible with the first-use gate (`first_use_guide_gate`, branch `worktree-agent-a60cb9fdae14b958b`): the gate asks `topic_declaring` about the selector with a null result, and a `path~` shape never matches a null result. `no_tracker_conventions_section_is_reachable_without_a_result` pins that.

First call per shape now ships, from a real `doc` call: `create` of a bug 4.1 KB (3 sections), `get` of a bug 3.4 KB, `find` 2.0 KB; `create` of a tracker 7.0 KB. Each is under the 16 KiB bound and under a quarter of the guide.

## Fix provenance

- **SHA:** `d34c398e` (`worktree-agent-a547c2640763139eb`, to be rebased onto `experiments` by the lead)
- **patch-id:** `84e51a5d19e7932fb73397bb887cb57ac08b9962`

## Tests added

- `prompts::guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it` — per routed shape, the sections that must arrive, under the byte bound and under a quarter of the guide.
- `prompts::guide_index::tests::no_tracker_conventions_section_is_reachable_without_a_result` — twin guard for the first-use gate.
- `prompts::guide_index::tests::a_path_qualified_shape_beside_an_unqualified_one_is_a_narrowing_not_an_ambiguity` — the overlap rule, both argument orders.
- `server::guide_hint_tests::the_first_call_of_each_routed_shape_ships_a_tracker_conventions_section` — end to end through real `doc` calls, asserting section BODY text.
- Two existing server tests rewritten, not loosened: `a_declared_section_still_arrives_once_the_content_topic_is_spent` (the fallthrough is now reached on the SECOND `get`) and the oversize-guide test, renamed `an_oversize_guide_ships_sections_not_its_body_and_only_an_explicit_fetch_stamps_the_topic` (no whole-topic guide is over the bound any more, so the pointer path itself is pinned only by the `guide_emit` unit tests, which pass a small bound).

Mutations run, red then restored: guide reverted to `HEAD` fails the new section test, the two rewritten tests and the new end-to-end test; one declaration changed to an unqualified `doc.create` fails both `no_two_topics_declare_an_overlapping_shape` and `no_tracker_conventions_section_is_reachable_without_a_result`.

## References

- `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md` — parent
- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md` — the remainder (router / `check~`)
- `docs/issues/2026-10-16-observed-shapes-gate-skips-every-doc-shape.md` — Gate 2 does not see `doc.*` shapes

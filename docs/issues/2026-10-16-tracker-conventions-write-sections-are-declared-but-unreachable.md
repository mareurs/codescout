---
kind: bug
status: open
title: 'tracker-conventions sections that a write needs are declared-but-unreachable: update, append_entry and move results route to librarian, and doctor cannot be served per violation class'
tags:
- cluster/declared-not-wired
- guides
- section-grain-delivery
- topic-routing
opened: 2026-10-16
closed:
owner: marius
severity: low
related:
- docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md
- docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md
---

# BUG: the half of tracker-conventions that a write needs cannot be delivered by section

## Summary

`tracker-conventions` now declares `serves:` sections (d34c398e), but only for the four call shapes
whose RESULT names a `docs/issues/` or `docs/trackers/` path: `doc.create`, `doc.get`, `doc.find`
and `librarian.doctor`. Every other call that needs the guide is a write whose result names no such
path, so `LibrarianAdapter::relevant_guide_topic` never chooses this topic for it and the
sections written for it (13 of 24, about 43 KB of the 61 KB guide) are waived in
`prompts::SECTION_WAIVERS` and reachable only by an explicit `get_guide("tracker-conventions")`.
`librarian.doctor` does route here but ships only the preamble, because `Shape` cannot qualify on
the violation class.

## Class fit

A declaration (the guide's `serves:` for the archive, entry-id and fix-provenance sections) that
production never reaches: the matcher, the sections and the ledger are each correct, and no call
connects an `update` or a `move` to them, because the router reads the result and those results
carry no path it scans.

## Symptom (Effect)

A session that archives a bug with `doc(action="move", ...)` or appends a tracker entry with
`doc(action="append_entry", ...)` is never served the `tracker-conventions` sections that describe
exactly that flow (stage both halves of the move, re-point citations, allocate entry ids). The call
routes to `librarian` and receives `librarian` § *Archiving / Moving Trackers* or § *Reach for
augmentation*.

## Reproduction

```
git rev-parse HEAD   # at or after d34c398e
cargo test --lib tracker_conventions_ships_sections_to_each_shape_that_routes_to_it
```

That test names the four routed shapes. To see the unrouted ones, make the calls (as done on
2026-10-16 with a throwaway test, since removed): create a tracker under `docs/trackers/`, then call
each `doc` action on it and read which results `names_path_containing(result, "docs/trackers/")`
accepts.

## Environment

codescout 0.15.0, branch `experiments` + d34c398e, in-process test server (`make_server`), 2026-10-16.

## Root cause

`LibrarianAdapter::relevant_guide_topic` (`src/librarian/adapter.rs`) picks the topic from the RESULT
via `names_path_containing` (`src/util/librarian_response.rs`), which reads top-level
`abs_path`/`rel_path`, `items[].rel_path` and `violations[].path`.

measured 2026-10-16 by making each `doc` call against a `docs/trackers/` artifact and testing the
primary block:

| action | result keys | names a tracker path |
|---|---|---|
| `get` | `id, abs_path, ...` | yes |
| `find` | `count, items, ...` (`items[].rel_path`) | yes |
| `create` | `id, abs_path, ...` | yes |
| `update` | `id, updated` | no |
| `append_entry` | `id, artifact_id, reserved, section_written, ...` | no |
| `move` | `id, previous_id, old_abs_path, new_abs_path, ...` | no (keys not scanned) |
| `delete` | `dry_run, deleted, id, would_delete_abs_path, ...` | no |
| `rekey_prefix`, `state_at`, `event_create`, `event_list`, `link`, `augment`, `gather`, `graph`, `list_stale` | no path key | no |

**Correction 2026-10-17:** the `list_stale` entry in that last row is wrong. `list_stale` returns
`items[].abs_path`, which the scan reads, so a real call against an augmented tracker DOES route to
`tracker-conventions` (preamble only); filed as
`docs/issues/2026-10-17-doc-list-stale-on-an-augmented-tracker-routes-to-tracker-conventions-and-gets-only-the-preamble.md`.
`gather`, `graph`, `event_create` and `event_list` were re-measured the same day and name no tracker path.

The adapter's own doc comment claimed `update` and `move` return a top-level path; it is corrected in
the same change. A fixture comment in `server.rs` also claimed an `append_entry` under
`docs/trackers/` would route here; it would not.

## Evidence

The runtime table above. `src/prompts/mod.rs` `TRACKER_SECTION_NEEDS_A_WRITE` carries the reason on
every waiver, so the state is visible where the next author edits.

## Hypotheses tried

1. **Hypothesis:** `path~`-qualified sections for `update`/`append_entry`/`move` would be served.
   **Test:** the table above. **Verdict:** rejected: they could not be chosen, so they are not
   declared. Declaring them is the failure this file records.

## Fix

Not started. Three options, none made here because each changes which guide arrives on a common call
(cf. `32736ca0`, which routed tracker work to this guide on purpose):

1. **Scan `old_abs_path`/`new_abs_path`** in `names_path_containing`. One line plus tests. Fixes `move`
   only, which is the archive flow (about 8 KB of the waived bytes: stage both halves, re-point
   citations, the sweep).
2. **Route on the selector.** `relevant_guide_topic` sees only the result; `selector_key` sees the
   input. A `(selector, kind-of-target)` route would reach `update` and `append_entry`, but the
   target's path is not in either, so the adapter would have to resolve the id to a path first.
3. **A `check~<name>` predicate in `Shape`** so `librarian.doctor` can serve the section that
   remediates the check a violation names, instead of all of them (the eight sections that name a
   doctor check total 31.6 KB, measured 2026-10-16, and the home of
   `terminal_status_without_fix_anchor` is one 15 KB section). A grammar amendment:
   `guide_index::Shape` says widening it requires amending the spec.

## Tests added

None for the unreachable state itself: it is pinned by absence. The routed side is pinned by
`guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it` and
`server::guide_hint_tests::the_first_call_of_each_routed_shape_ships_a_tracker_conventions_section`.

## Workarounds

`get_guide("tracker-conventions")` returns the whole topic; an `update`/`move`/`append_entry` caller
who wants the flow reads it explicitly. Before section grain these sections reached nobody either:
the whole topic was over the auto-inject bound and shipped a pointer.

## Resume

The decision is the router (options 1 and 2) and the grammar (option 3). Whichever is taken, delete the
waiver rows it makes reachable from `SECTION_WAIVERS` and declare the sections; Gate 5 then holds them
to their size cap, so the large ones (`Entry ids` 6.5 KB, the 15 KB `Closing a bug` section) must be
split at `###` first.

## References

- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md` (the residual this follows)
- `src/prompts/mod.rs` `SECTION_WAIVERS`, `TRACKER_SECTION_NEEDS_A_WRITE`

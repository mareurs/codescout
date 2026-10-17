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

**Partial fix, 2026-10-17: option 1 (the `move` / archive flow). Options 2 and 3 are NOT done, so this
bug stays `open`.**

What landed:

- `names_path_containing` (`src/util/librarian_response.rs`) now also scans top-level
  `old_abs_path`/`new_abs_path`, so a `doc(action="move")` of a bug file or tracker routes to
  `tracker-conventions` instead of `librarian`.
- The 15 KB `Closing a bug` section was split at its natural boundary: the fix-provenance half keeps
  that heading (renamed `Closing a bug — patch-id and fix provenance`, 5.5 KB, still waived: its
  call is `update`), and the archive flow became five `###` sections, each under the 2,500 B cap
  (`Archiving a bug — through the catalog, never git mv`, `… stage both halves of the move`,
  `… re-point the citations`, `Verifying the citation sweep`, `Reading move's inbound_path_citations`),
  each declared `serves: doc.move(path~docs/issues/)`. A sixth, `Citing an archive path — where the
  file is, not where it will go`, is the fix-time half and stays waived.
- `Archiving a tracker` (401 B) now declares `serves: doc.move(path~docs/trackers/)`; its waiver row
  is gone, and `TRACKER_SECTION_NEEDS_A_WRITE` no longer names `move`.

What a `move` of a bug file now ships: about 9.5 KB of sections (a bound of 16 KB and a quarter of the
guide are both asserted). That is a deliberate change to which guide arrives on `move`: `librarian`
§ *Archiving / Moving Trackers* used to; it still arrives on the fallthrough once the
`tracker-conventions` sections are spent.

Still open, each a decision rather than an edit:

2. **Route on the selector** for `update` and `append_entry`: the target's path is in neither the
   input nor the result, so the adapter would have to resolve the id to a path first.
3. **A `check~<name>` predicate in `Shape`** so `librarian.doctor` can serve the section that
   remediates the check a violation names (a grammar amendment).

The remaining waived sections (`Entry ids` 6.5 KB, `Closing a bug`, and the entry-level sections) need
option 2 first, and the large ones must be split at `###` before they can be declared.

## Tests added

For the `move` half (2026-10-17), red before the fix, green after:

- `util::librarian_response::tests::a_move_result_names_the_paths_it_moved_between`
- `librarian::adapter::tests::tracker_paths_route_to_the_tracker_guide_and_nothing_else_does`
  (extended: `move` routes, `update` does not)
- `prompts::guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`
  (extended with `doc.move` for a bug file and a tracker; asserts the byte bounds)
- `server::guide_hint_tests::moving_a_bug_file_or_a_tracker_ships_the_archive_sections`
  (real `doc` create + move calls, asserting section body text and that the whole guide does not ship)

The `update`/`append_entry` half is still pinned by absence only. The routed side of the other shapes
is pinned by `the_first_call_of_each_routed_shape_ships_a_tracker_conventions_section`.

## Workarounds

`get_guide("tracker-conventions")` returns the whole topic; an `update`/`move`/`append_entry` caller
who wants the flow reads it explicitly. Before section grain these sections reached nobody either:
the whole topic was over the auto-inject bound and shipped a pointer.

## Resume

`move` is done (see Fix). What remains is the decision on the router for `update`/`append_entry`
(option 2) and on the grammar for `doctor` (option 3). Whichever is taken, delete the waiver rows it
makes reachable from `SECTION_WAIVERS` and declare the sections; Gate 5 then holds them to their size
cap, so the large ones (`Entry ids` 6.5 KB, `Closing a bug` 5.5 KB) must be split at `###` first.

## References

- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md` (the residual this follows)
- `src/prompts/mod.rs` `SECTION_WAIVERS`, `TRACKER_SECTION_NEEDS_A_WRITE`

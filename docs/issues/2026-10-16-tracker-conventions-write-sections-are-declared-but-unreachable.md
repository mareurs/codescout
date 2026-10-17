---
kind: bug
status: open
title: 'tracker-conventions sections that a write needs are declared-but-unreachable: update and append_entry results route to librarian (move was fixed 2026-10-17), and doctor cannot be served per violation class'
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

`tracker-conventions` declared `serves:` sections (d34c398e) for only four call shapes, the ones
whose RESULT names a `docs/issues/` or `docs/trackers/` path: `doc.create`, `doc.get`, `doc.find`
and `librarian.doctor`. A fifth, `doc.move`, was added 2026-10-17 (its result names the file as
`old_abs_path`/`new_abs_path`, which the scan now reads). Every other call that needs the guide is
a write whose result names no such path (`update`, `append_entry`, `delete`, ...), so
`LibrarianAdapter::relevant_guide_topic` never chooses this topic for it and the sections written
for it are waived in `prompts::SECTION_WAIVERS` and reachable only by an explicit
`get_guide("tracker-conventions")`. (Measured 2026-10-16, before the `move` fix: 13 of 24
sections, about 43 KB of the 61 KB guide.)
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

**Partial fix, 2026-10-17: option 1 (the `move` / archive flow). Options 2 and 3 are NOT done, so this
bug stays `open`.**

What landed:

- `names_path_containing` (`src/util/librarian_response.rs`) now also scans top-level
  `old_abs_path`/`new_abs_path`, so a `doc(action="move")` of a bug file or tracker routes to
  `tracker-conventions` instead of `librarian`.
- The 15 KB `Closing a bug` section was split at its natural boundary: the fix-provenance half keeps
  that heading (renamed `Closing a bug — patch-id and fix provenance`, 5.5 KB, still waived: its
  call is `update`), and the archive flow became `###` sections, each under the 2,500 B cap and
  each declared `serves: doc.move(path~docs/issues/)`: `Archiving a bug or tracker — through the
  catalog, never git mv` (0.8 KB), `… stage both halves of the move` (2.1 KB), `… re-point the
  citations` (2.2 KB), `Verifying the citation sweep` (1.4 KB), `Reading move's
  inbound_path_citations` (1.7 KB) and `Why a green gate proves nothing about the sweep` (1.3 KB,
  the explanation that completes the re-point section's "so the grep is the check"). The first
  three also declare `doc.move(path~docs/trackers/)`. A seventh, `Citing an archive path — where
  the file is, not where it will go`, is the fix-time half and stays waived.
- `Archiving a tracker` (0.4 KB) keeps its waiver and has no `serves:`. Its lead instruction is
  `doc(action="update", patch={status: "archived"})`, and `update` still routes to `librarian`, so
  declaring it on `doc.move` served the right text after the wrong call, and let Gate 5 count it
  reachable on a technicality. A tracker move is instead served the three generic archive sections
  above (new-id warning, stage both halves, re-point paths and ids: 5.1 KB), which is what
  `librarian` § Archiving / Moving Trackers told it before the routing change; a guide_index test
  asserts a tracker move does not receive `Archiving a tracker`.

What a `move` ships now, and what it displaces:

- **Identified sessions** (the normal case): the first destructive `doc.move` is held before it
  runs and served `librarian` § Archiving / Moving Trackers (that gate is input-derived and cannot
  see the result); after the identical re-issue executes, `tracker-conventions` adds about 9.6 KB
  for a bug file (about 5.1 KB for a tracker). Nothing is displaced; the cost is that added
  volume. The bug-archive delivery is budgeted by its own test (13,758 B measured, 13,886 B
  ceiling; see the issue linked under References), separate from the p50 session.
- **Anonymous tier** (no session id, so the hold never fires): the move's post-call emission is now
  `tracker-conventions` sections, where it used to be `librarian` § Archiving / Moving Trackers.
  The tracker-conventions sections carry the same stage-both-halves and re-point guidance, so the
  content is replaced rather than lost, but `librarian`'s section is displaced there.

Still open, each a decision rather than an edit:

2. **Route on the selector** for `update` and `append_entry`: the target's path is in neither the
   input nor the result, so the adapter would have to resolve the id to a path first.
3. **A `check~<name>` predicate in `Shape`** so `librarian.doctor` can serve the section that
   remediates the check a violation names (a grammar amendment).

The remaining waived sections (`Entry ids` 6.5 KB, `Closing a bug`, `Archiving a tracker` and the
entry-level sections) need option 2 first, and the large ones must be split at `###` before they can
be declared.

## Tests added

For the `move` half (2026-10-17), red before the fix, green after:

- `util::librarian_response::tests::a_move_result_names_the_paths_it_moved_between`
- `librarian::adapter::tests::tracker_paths_route_to_the_tracker_guide_and_nothing_else_does`
  (extended: `move` routes, `update` does not)
- `prompts::guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`
  (extended with `doc.move` for a bug file and a tracker, the byte bounds, and a negative: a tracker
  move is not served `Archiving a tracker`)
- `server::guide_hint_tests::moving_a_bug_file_or_a_tracker_ships_the_archive_sections`
  (real `doc` create + move calls on a fresh server per case; hard-asserts the pre-call hold fires
  and serves the `librarian` section and no tracker-conventions section; asserts one distinctive
  sentence from each of the six bug-move sections and the three tracker-move sections after the
  re-issue, that the spent `librarian` and create sections do not repeat, and that
  `Archiving a tracker` does not ship)
- `prompts::tests::section_waivers_are_registered_with_real_reasons` (extended: a waiver's heading
  must exist in its guide, so a rename cannot leave a dead row)
- `librarian::tools::mv::tests::the_archive_confirmation_names_staged_ness_and_content_on_every_surface`
  (not new: its tracker-conventions slice now reads the staging section through `GUIDE_INDEX`, since
  the slice anchored on `**Then re-point` absorbed the new heading and `serves:` comment and broke
  the 2,000 B ceiling, which was not raised)

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

- `docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md`
  (no session-level byte budget covers a bug-file archive move)
- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md` (the residual this follows)
- `src/prompts/mod.rs` `SECTION_WAIVERS`, `TRACKER_SECTION_NEEDS_A_WRITE`

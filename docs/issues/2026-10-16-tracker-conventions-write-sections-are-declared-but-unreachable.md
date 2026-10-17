---
kind: bug
status: open
title: 'tracker-conventions sections that a write needs are declared-but-unreachable: the sections only a status-changing update needs stay waived (routing it delays librarian''s anti-wipe sections), and doctor cannot be served per violation class (move, append_entry, update_entry, augment and link were fixed 2026-10-17)'
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

**Second change, 2026-10-17: the writes that returned only `{id, ...}` or the bare `"ok"`.**
Option 2's obstacle was that the target's path is in neither the input nor the result. It did not
need the router to resolve anything: the doc tool already has the catalog and the id, so the result
is given the path before the router reads it (`write_target_id` / `name_the_target`,
`src/librarian/tools/artifact.rs`). Reviewed by Opus the same day; this section records the
reviewed state.

- `append_entry`, `update_entry`, `augment` and `link` (keyed on `src_id`) answer with `abs_path` (+50 B on an object result, +61 B when a bare `"ok"` is promoted, for the probe path; +59 / +70 B at the median `docs/trackers/` path, 45 characters, n = 92).
  `augment` and `link` answered the bare string `"ok"`; they now answer `{"status":"ok","abs_path":…}`,
  the promotion `annotate_write_path` already makes for every other write and `annotate_write_root`
  already made in any repo with linked worktrees. The ADR it follows
  (`docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md`) is cited and amended: its
  Revisit-when named exactly this ("a write tool needs a path it did not take from
  `input['path']`"), and the amendment says what now holds. Option (b) (routing on the input) was
  already measured and rejected there and is not done.
- **The path is the row the write LANDED ON.** The first cut read `args["id"]`, which in a worktree
  session is the main-checkout artifact; the write goes to its overlay shadow, so the result named a
  file the write never touched (reviewer's probe: `artifact_id` of the shadow, `abs_path` of the main
  file). `name_the_target` now asks `worktree::peek_write_target`, which shares `shadow_coordinates`
  with the forking `resolve_write_target` and forks nothing.
- **`update` is not routed, in either spelling of `status`.** The first cut named a status-changing
  update. The engine ships ONE topic per call and stops at the first that ships something
  (`emit_guide_sections`), so librarian § *Choosing a mode* and § *The shrink guard* (the anti-wipe
  warnings) were delayed: reviewer-measured, bug close -> tracker archive -> body edit delivered them
  only after the THIRD call had run; in the anonymous tier (no session identity, so the destructive
  gate holds nothing) a forced status update ran unheld and shipped 9,048 B of tracker-conventions,
  and the next call, a forced wholesale `patch.body`, ran with no warning. I reproduced both by
  re-introducing the naming (mutation P5, below), which kills the two end-to-end tests that pin the
  opposite. Priority rule: routing must not delay the anti-wipe sections. Chosen: drop the
  status-update shape (not "serve the anti-wipe sections first", which is multi-topic emission, an
  engine change). Cost: the three sections only a status change needs (`Settled caveats`,
  `Closing a bug — patch-id and fix provenance`, `Archiving a tracker`) are waived again, and the
  three-way split of `Closing a bug` made in the first cut was reverted, since nothing served it.
- `One entry format` and `Compaction and archival` were moved OFF `append_entry`: they inform body and
  index maintenance (a `body_edits` update), not an append, and the first cut delivered 16,344 B against
  the 16,384 B bound because the test summed section BODIES while the bound counts the wrapper. The
  refusals and counter sections stay on `append_entry` (a prose-ledger append is its main use and the
  router cannot tell prose from params).
- `Declaring an augmentation` (4.1 KB) was split at `###` (1.9 + 2.4 KB), as the other five splits were.
- **The merge warning rides the first `augment`.** The first `augment` of a tracker is claimed by
  tracker-conventions, so librarian § *Augmentation Lifecycle* (the only guide text saying that under
  `merge=true` a `params` array replaces the WHOLE collection) arrives on the SECOND. The `doc` tool
  description states the `merge=false` reset inline but not the `merge=true` array replacement, and
  `augment` (unlike `update`) reports no `entries_removed`. So one sentence now opens
  § *Declaring an augmentation*: "merge=false resets every field you omit, and under merge=true a
  params array replaces the WHOLE collection ... use append_entry / update_entry"; the section is 2.25 KB
  (cap 2.5 KB). Pinned by the end-to-end test on the FIRST augment (deleting it reds). **Follow-up, not
  done:** surface `entries_removed` plus a `warning` in `augment`'s result, as `update` does
  (`update.rs:895-899`); `augment.rs:448` keeps only `.found` from `merge_params` and drops the
  before/after counts.
- **A `link` names the row whose file it wrote**: the source for a plain edge, the destination (as the
  caller passed it) for `supersedes`. From a worktree session `link::call` writes the MAIN
  checkout's destination file; that is filed separately
  (`docs/issues/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md`).
- Naming re-takes the catalog lock after the write; a `merge_worktree` or abandon in that window makes
  the result name the main row's path or none (cosmetic, routing unchanged).

Delivered bytes per call, wrapped blocks as injected (before = first cut b96a56ea):

| call | before | after (measured) | ceiling (measured + 128 B) |
|---|---|---|---|
| `append_entry` on a tracker | 16,344 | 12,928 | 13,056 |
| `update_entry` on a tracker | 9,077 | 9,077 | 9,205 |
| `augment` on a tracker | unrouted (librarian) | 5,261 (4,958 + the merge warning, below) | 5,389 |
| `link` from a tracker or a bug | unrouted (librarian) | 2,591 | 2,719 |
| status-changing `update` | 9,048 (anonymous tier, reviewer) | 0 from tracker-conventions; librarian's 2,683 | n/a |

The ceilings are one constant each (`WRITE_CEILING_*`, `src/prompts/guide_index.rs`), measured plus a
fixed 128 B (the margin the emission-budget fixtures use); the end-to-end test reads the same constant
instead of a literal. **Against the peer's bug-archive move budget** (13,758 B, ceiling 13,886 B,
`a_bug_archive_move_stays_under_its_own_emission_byte_ceiling`): that figure counts the hold (2,271 B)
plus the executed response (11,487 B). Per RESPONSE, the first `append_entry` (12,928 B) already
exceeds the move's executed response, so "the largest real draw is the move" holds only if hold and
re-issue are counted together.

The earlier note that adding `Citing an entry` to `append_entry` would give 16.8 KB was a BODY sum;
wrapped it is about 19.7 KB. It stays on `update_entry` only.

Still waived, each with its reason in `SECTION_WAIVERS`:

- `Settled caveats`, `Closing a bug — patch-id and fix provenance`, `Archiving a tracker`: need a
  status-changing `update` (above). The fix that would serve them is multi-topic emission.
- `One entry format`, `Compaction and archival`: inform `body_edits` updates, which stay librarian's.
- `Entry ids — when two hosts allocate anyway`, `Entry headings — a prefix with no definer, or owned
  outside markdown`: doctor remedies. Serving them per violation class needs a `check~<name>` predicate
  in `Shape` (a grammar amendment). The doctor messages that cite `§ Entry headings` were checked: they
  say an entry has no `## <ID> — <title>` heading, which is `Entry headings — the definition rule`
  (still that heading, now served on `append_entry`); none points at the no-definer child, so no
  pointer needed changing.
- `Citing an archive path`: informs a body write made before the move.

**What the new routing displaces, measured:** the first `append_entry`, `update_entry`, `augment` and
`link` on a tracker now ship tracker-conventions sections where they shipped librarian § *Reach for
augmentation* / § *Changing ONE entry* / § *Augmentation Lifecycle* / § *Relationship Map*. Those arrive
on the NEXT call of the shape, because the fallthrough picks the topic `topic_declaring` names, and that
is the librarian only because it sorts first in the topic map and nothing else declares these shapes
unqualified: pinned per shape by `the_fallback_for_every_routed_write_is_the_librarians_declaration`,
and by the second `append_entry` in the end-to-end test. `augment`'s § *Augmentation Lifecycle* carries
the merge-versus-replace semantics; the `doc` tool description states `merge=false` replaces wholesale
inline, which is why this was judged acceptable and `update` was not.

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

For the second change (red before the fix: the declarations did not exist, and then the reviewed
defects below were reproduced by mutation):

- `prompts::guide_index::tests::tracker_write_shapes_ship_their_sections_and_only_theirs`: for each of
  the 13 sections declared for these writes, the calls that MUST serve it and at least one that must NOT;
  the table is compared as a SET with the guide's declared write-sections; `doc.update` serves nothing
  from the topic on a tracker, a bug or no path; bytes are measured on the WRAPPED blocks
  (`section_block`), bounded by `MAX_AUTO_INJECT_GUIDE_BYTES`, the 1/4-of-guide ratio, a measured
  ceiling per call, and a floor that fails an instrument that stops counting wrappers.
- `librarian::tools::artifact::tests::a_write_names_the_row_it_landed_on_in_a_worktree_session_and_the_artifact_otherwise`:
  one case per action (`append_entry`, `update_entry`, `augment`, `link`), each in a worktree context
  (the overlay shadow's path) and outside one (the artifact's own path).
- `...::an_update_that_changes_status_names_no_path_in_either_spelling` (`patch.status` and the
  top-level `status` shortcut: two sites of one rule).
- `server::guide_hint_tests::a_write_to_a_tracker_or_bug_file_is_routed_to_its_section` (real calls,
  delivered bytes of the first `append_entry`, the librarian fallthrough on its second call, `augment`
  and `link` named, a link from a spec not claimed);
  `a_status_update_keeps_the_librarians_anti_wipe_sections_first` (both spellings, the three-call
  sequence); `the_anonymous_tier_still_gets_the_anti_wipe_sections_after_a_forced_status_update`;
  `a_forced_status_update_is_held_behind_the_anti_wipe_sections_and_then_runs` (the held block IS
  librarian § Choosing a mode and § The shrink guard: option B preserved);
  `the_fallback_for_every_routed_write_is_the_librarians_declaration`.

Mutation probes (`./scripts/mutation-probe.sh`): see the final report for the verdict list; every one
of the 16 below was KILLED. Naming the landed row's id instead of the caller's, the shadow never
found, `augment` or `link` unnamed or naming the wrong end, `update` named, the bare `"ok"` not
promoted, a section widened or its `path~` dropped, each byte bound (ceiling, floor, wrapper-counting,
auto-inject bound, ratio, end-to-end), a waiver drifting, and the librarian ceasing to declare `link`.

Second review round, each KILLED: a `supersedes` link keyed on the source; a plain link keyed on the
destination; `supersedes` resolved through the overlay (observable because the test pre-forks the
destination's shadow); a plain link not resolved through the overlay; the merge warning deleted from
§ Declaring an augmentation (end-to-end, first `augment`); each of the four ceilings at measured - 1
(so each constant is within 128 B of its measurement, and the first `augment` / `append_entry` really
deliver at least the stated bytes); each section delivered twice.

## Workarounds

`get_guide("tracker-conventions")` returns the whole topic; an `update`/`move`/`append_entry` caller
who wants the flow reads it explicitly. Before section grain these sections reached nobody either:
the whole topic was over the auto-inject bound and shipped a pointer.

## Resume

`move`, `append_entry`, `update_entry`, `augment` and `link` are done (see Fix). What remains is
three decisions: multi-topic emission (one call shipping the librarian's anti-wipe sections AND the
tracker-conventions close/archive sections), which would let a status-changing `update` be named;
the grammar for `doctor` (`check~<name>`); and where `One entry format` / `Compaction and archival`
should be delivered if not on an update. Whichever is taken, delete the waiver rows it makes
reachable from `SECTION_WAIVERS` and declare the sections; Gate 5 then holds them to their size cap.

## References

- `docs/issues/2026-10-17-p50-emission-budget-fixture-never-archives-a-bug-file-so-the-largest-doc-move-delivery-is-unbudgeted.md`
  (no session-level byte budget covers a bug-file archive move)
- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md` (the residual this follows)
- `src/prompts/mod.rs` `SECTION_WAIVERS`, `TRACKER_SECTION_NEEDS_A_WRITE`

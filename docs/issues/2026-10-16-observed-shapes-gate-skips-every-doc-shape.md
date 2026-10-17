---
kind: bug
status: fixed
title: 'Gate 2 (every_observed_shape_of_a_declaring_topic_has_a_section) iterates a census of artifact.* shapes, no tool is named artifact any more, so every doc.* shape is skipped'
tags:
- cluster/selector-narrower-than-its-population
- guides
- section-grain-delivery
opened: 2026-10-16
closed: 2026-10-17
unverified: 'Gate 2 still cannot judge shapes added after the census was recorded (doc.rekey_prefix, doc.state_at, doc.graft, librarian.audit_log); the census was not regenerated.'
owner: marius
severity: low
related:
- docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md
---

# BUG: the observed-shapes gate checks the librarian.* rows and none of the doc.* ones

## Summary

`server::guide_hint_tests::every_observed_shape_of_a_declaring_topic_has_a_section` walks
`src/prompts/shape_census.txt` and, for each row, looks up the tool named before the first `.`. The
census rows for the librarian's document tool are spelled `artifact.get`, `artifact.update`,
`artifact.append_entry`, and so on, but the tool has been registered as `doc` since the 2026-09-02
collapse, so `server.tools.iter().find(|t| t.name() == "artifact")` finds nothing and the loop
`continue`s. The gate has therefore verified only the `librarian.*` rows since then, and passes while
checking none of the shapes that carry nearly all librarian traffic (about 10,100 `artifact.*` census
calls against about 830 `librarian.*` ones).

## Class fit

A selector (the census row's tool prefix) narrower than the population it names (every observed call
shape): it runs to completion over the subset whose tool name still resolves and returns green, and
the `doc.*` shapes it never looked at cannot be counted.

## Symptom (Effect)

No failure. Declaring a topic that serves `doc.*` shapes without covering one of them passes this gate.
Noticed while adding `tracker-conventions` as the second declaring topic (d34c398e): the gate did not
ask for a section or waiver for any `doc.*` shape, only for the `librarian.*` ones, which the waiver
`(librarian call shapes)` in `SECTION_WAIVERS` now names.

## Reproduction

```
git rev-parse HEAD   # at or after d34c398e
grep -c '^artifact' src/prompts/shape_census.txt     # rows the lookup cannot resolve
grep -c '^doc' src/prompts/shape_census.txt          # rows that would: 0
```

## Environment

codescout 0.15.0, `experiments` + d34c398e, 2026-10-16.

## Root cause

`src/server.rs`, `every_observed_shape_of_a_declaring_topic_has_a_section`: `let Some(tool) =
server.tools.iter().find(|t| t.name() == tool_name) else { continue; };`, with `tool_name` taken from
the census row. The census is "generated from real transcripts" (its header names the script) at a time
the tool was `artifact`; it was not regenerated for the rename.

measured 2026-10-16: the census lists `artifact.update` 3917, `artifact.get` 3201, `artifact.find` 1622,
`artifact.append_entry` 413 and others, with no `doc.` row at all; the registered tool is `doc`.

## Evidence

The census rows above, and the gate's `continue`. Gate 6 (`every_declared_shape_names_a_real_tool_and_action`)
reads the registry rather than the census, so it is not affected; this is Gate 2 only.

## Hypotheses tried

1. **Hypothesis:** an alias maps `artifact` to `doc`. **Test:** read the gate and `Tool::name`.
   **Verdict:** rejected: no alias; the lookup is a plain name equality.

## Fix

The guard, not the data. Verified 2026-10-17 on `experiments` + `299395e5`: 33 census rows named no
registered tool (rows `artifact.*`, `artifact_augment`, `artifact_event.*`, `artifact_refresh.*`, plus
`read_markdown`, `edit_markdown.*`, `Edit`, `peer.list`, `replace_all`, `run_file`), the census had no `doc.`
row, and the gate `continue`d past all of them. The census itself was NOT regenerated (its generator is a
plan-doc snippet that needs real transcripts); the rows are read through recorded renames instead. Fix in
`src/server.rs` (tests):

- `resolve_census_row` classifies each census row as `Live` (registered, after `CENSUS_RENAMED_SHAPES` /
  `CENSUS_RENAMED_TOOLS`), `Retired` (in `CENSUS_STRAY_TOOLS`, or in `RETIRED_TOOL_NAMES` for the
  2026-09-02 collapse names, so there is one table of dead names) or `Unresolved`, which fails.
- Gate 2 reads the census through `census_shapes()`, reads `artifact.*` rows as the `doc.*` shapes they
  became, panics on `Unresolved`, collects every uncovered shape into one message, and counts the
  `(tool, topic)` pairs it actually JUDGED (after the `declares` check), asserting each of
  (`doc`, `librarian`), (`doc`, `tracker-conventions`), (`librarian`, `librarian`) and
  (`librarian`, `tracker-conventions`) is non-zero.
- A waiver now resolves on the shape as a whole word of the reason (`prompts::waiver_reason_names_shape`,
  `shape_is_waived`), not `contains`: the bare `doc` shape had read as waived through the `doc(action="find")`
  in an unrelated reason. `doc` is named explicitly in the `librarian` "Common Mistakes" waiver and in
  `TRACKER_DOC_SHAPES_UNSERVED`.
- `TRACKER_DOC_SHAPES_UNSERVED` (`src/prompts/mod.rs`) waives the `doc.*` shapes no `tracker-conventions`
  section serves. Measured 2026-10-16/17 by real calls: `append_entry`, `update_entry`, `move`, `delete`,
  `link`, `augment`, `event_create`, `event_list`, `gather` and `graph` name no tracker path, so a real call
  reaches `librarian` (ledger:
  `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`). `doc.list_stale`
  is the exception: its result carries `items[].abs_path`, so a real call on an augmented tracker routes to
  `tracker-conventions` and is served only the preamble. Filed as
  `docs/issues/2026-10-17-doc-list-stale-on-an-augmented-tracker-routes-to-tracker-conventions-and-gets-only-the-preamble.md`
  (open); the waiver names it.

Fix commits, on branch `worktree-agent-ab7a7e0e672e14d3a` (based on `experiments`): `53a53782` (patch-id
`7ee22cbb367f42b2039b86af9fe3294f5decc618`) and the review fixes `e5075a12` (patch-id
`b52d9d53b2beee1196fcbb116395562ca3bb5e41`).

What this does not cover: Gate 2 sees only the shapes the census holds, and the census was recorded before
`doc.rekey_prefix`, `doc.state_at`, `doc.graft` and `librarian.audit_log` existed, so those cannot be
judged by it. Gate 6 (`every_declared_shape_names_a_live_tool_and_action`) reads the registry and covers
declarations, not these observations.

## Tests added

In `src/server.rs`: `every_census_row_resolves_to_a_live_tool_or_a_recorded_retirement` (red before the tables
were filled, listing 33 unresolved rows; also floors the census at 60 rows and at 10 / 5 Live rows for `doc` /
`librarian`), `census_renames_and_retirements_are_not_stale` (rename targets registered and landing on a real
action, stray tools not registered and not duplicating `RETIRED_TOOL_NAMES`, the resolver failing an unknown
name or an unknown action under a renamed prefix), and the strengthened
`every_observed_shape_of_a_declaring_topic_has_a_section`. In `src/prompts/mod.rs`:
`waiver_reason_names_a_shape_as_a_whole_word_not_a_substring` and
`shape_is_waived_resolves_the_bare_doc_shape_on_an_explicit_mention`.

Each strengthened assertion was observed red with `scripts/mutation-probe.sh` (2026-10-17, isolated tree):

- (`doc`, `tracker-conventions`) judged: `names_tracker_path` made `false` → Gate 2 red, "judged no `doc` census
  shape against `tracker-conventions`". Same for `librarian` topic via `declares` returning false for it.
- Row floor and Gate 2 reading `census_shapes()`: its filter made `|_| false` → both red, "census parsed to only 0 rows".
- `doc` Live floor and rename-lands-on-a-real-action: `("artifact", "doc")` → `("artifact", "symbols")` → "only 5 census
  rows resolve to a live `doc` shape" and "`artifact.update` resolves to `symbols.update`".
- Rename action validity: `doc.augment` → `doc.augmentt` → "`augmentt` is not one of `doc`'s actions".
- Unknown action under a renamed prefix stays unresolved: `if renamed {` → `if false {` → red.
- Whole-word waiver: removing the one explicit `doc` mention from the `librarian` waiver → Gate 2 red for
  (`doc`, `librarian`); making the helper `contains` instead → `waiver_reason_names_a_shape_as_a_whole_word_not_a_substring`
  red (Gate 2 itself stays green there, which is why the helper has its own test).

## Workarounds

For `tracker-conventions`, a table-driven test over the real routed shapes
(`guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`) covers what
this gate does not.

## Resume

Fixed in the sense the issue named as the guard: the next tool rename fails
`every_census_row_resolves_to_a_live_tool_or_a_recorded_retirement` instead of narrowing Gate 2. Open
remainders: the census is not regenerated and does not hold `doc.rekey_prefix`, `doc.state_at`, `doc.graft`
or `librarian.audit_log`, so Gate 2 cannot judge them (regenerating it would need the transcripts the
generator reads, and is likely to demand sections or waivers for every shape it adds).
`doc.list_stale` was waived until its own issue was fixed; the waiver token is deleted and Gate 2 now
probes that shape with a real result (`threshold_hours`, `probe_as_the_shape_returns` in `src/server.rs`).

## References

- `src/prompts/shape_census.txt`
- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md`
- `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md` (open ledger entry for the write shapes)
- `docs/issues/2026-10-17-doc-list-stale-on-an-augmented-tracker-routes-to-tracker-conventions-and-gets-only-the-preamble.md` (open)

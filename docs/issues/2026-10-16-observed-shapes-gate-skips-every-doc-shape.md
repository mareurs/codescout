---
kind: bug
status: open
title: 'Gate 2 (every_observed_shape_of_a_declaring_topic_has_a_section) iterates a census of artifact.* shapes, no tool is named artifact any more, so every doc.* shape is skipped'
tags:
- cluster/selector-narrower-than-its-population
- guides
- section-grain-delivery
opened: 2026-10-16
closed:
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

Not started. Either regenerate the census under the current tool names (its header points at the
script), or have the gate fail when a census row's tool resolves to nothing, so the next rename is
loud. The second is the guard; the first only refreshes the data.

## Tests added

None.

## Workarounds

For `tracker-conventions`, a table-driven test over the real routed shapes
(`guide_index::tests::tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`) covers what
this gate does not.

## Resume

Read the gate's loop first. Expect a regenerated census to demand sections or waivers for every
`doc.*` shape of both declaring topics; budget for that.

## References

- `src/prompts/shape_census.txt`
- `docs/issues/2026-09-24-residual-tracker-conventions-serves-sections.md`

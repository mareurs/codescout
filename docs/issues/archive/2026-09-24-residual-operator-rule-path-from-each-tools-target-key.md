---
id: 5eedf8cca13b2c98
kind: bug
status: fixed
title: 'RESIDUAL: Capture the operator-rule path predicate from each write tool''s actual target key (not only input[''path''])'
tags:
- cluster/declared-not-wired
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Capture the operator-rule path predicate from each write tool's actual target key (not only input['path']).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` (status `fixed`):

> CLOSED 2026-09-01, first half: `tools::core::tests::a_real_edit_file_write_under_dot_claude_delivers_op_4` drives the REAL EditFile through call_content against an absolute path under `.claude` and asserts the OP-4 block arrives in the returned content — one call, no hand-supplied selector, no hand-fed route() call, with the negative control run FIRST so the once-per-session ledger cannot make its silence vacuous. Writable only because 30b6fc41 inverted Tool::selector_key's default, so EditFile now supplies its own selector and call_content runs the router itself. Mutation-checked: removing the annotation kills it plus two siblings (so it establishes the annotation matters, not this test's unique necessity); forcing the key to rel_path does NOT kill it, because names_path_containing scans both keys. STILL OPEN, second half: the path is captured from input["path"] specifically, so a write tool using a different key for its target gets no annotation and any rule serving it would be dead the same way. edit_file and create_file both use `path`, so no live rule is affected today.

## Fix

A guard test was added; `write_path` was NOT taught a per-tool target key. `path_predicate_defect(tool, selector)` in the test module of `src/server.rs` returns a defect string for a selector carrying a `path~` predicate when the tool is not a write call under the selector's action (read from the tool's own `is_write`), is listed in `WRITE_ROOT_ANNOTATION_EXEMPT`, or has no `path` property in its `input_schema`. The existing gate `every_triggered_rule_names_a_tool_that_can_deliver_it` now calls it for every served selector with a `path~` predicate and panics with the rule id and the defect. `WRITE_ROOT_ANNOTATION_EXEMPT` in `src/tools/core/types.rs` became `pub(crate)` so the check reads the real list. No production behaviour changed, and no live rule is affected: `edit_file` and `create_file` both use `path`. A rule authored on a write tool that keys its target differently now fails the gate at authoring time instead of being silently dead.

## Tests added

- The call to `path_predicate_defect` inside the existing gate `every_triggered_rule_names_a_tool_that_can_deliver_it` (`src/server.rs`) pins that every live `path~` selector names a tool that can deliver it. Today that is exercised only by the live corpus (`edit_file` / `create_file`, both clean); no dead live rule was authored to prove the call site, so its red lives in the fixture test below.
- `a_path_predicate_on_a_tool_that_cannot_deliver_it_is_a_defect` (`src/server.rs`) drives the helper from fixtures: the positive twin `edit_file` and `create_file` with `path~` return `None`; `memory` with action `write` (a write tool with no `path` parameter) is a defect naming "no `path` parameter"; `read_file` with `path~` and `memory` with action `read` are defects naming "not a write call"; `approve_write` (writes and has `path`, but exempt) is a defect naming "exempt"; selectors without a `path~` predicate are never a defect. Control asserts on each fixture pin why it is dead, so a tool gaining or losing a `path` parameter reds the test instead of turning a case vacuous.
- No mutation run is recorded in the commit message.

## Fix provenance

- **SHA:** `2c3dbb53` (`experiments`)
- **patch-id:** `f81ef968c34e86af2f0136c6ad3d1985fdac9f87`

## Resume

Closed on 2026-10-06 by the guard test above. Residual follow-ups, listed and not filed:

- The guard is not the same as teaching `write_path` a per-tool target key, which is what this file's title literally asked for. A future write tool with a non-`path` key would still need `write_path` changed before a `path~` rule on it could ever match; the gate only makes that visible.
- The gate's call site is exercised only by the live corpus, both entries of which are clean; the helper's red is covered by the fixture test alone.

## References

- `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` — parent

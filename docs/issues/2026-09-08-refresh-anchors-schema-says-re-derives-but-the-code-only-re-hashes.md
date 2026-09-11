---
kind: bug
status: taken
tags:
- cluster/doc-contradicted-by-code
claimed_at: 2026-09-11
claimed_by: ec98641c-d5ba-456d-8e4a-10e24d52da16
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: medium
---

# BUG: the memory tool's JSON schema says refresh_anchors "re-derives anchor topics"; it re-hashes an existing set and can never add a newly-mentioned path

## Summary

`memory`'s `input_schema` — the text every MCP client renders when deciding what an
action does — describes `refresh_anchors` as re-deriving. The implementation re-hashes
the anchor set already in the sidecar and never calls `extract_paths`/`seed_anchors`, so
a path newly mentioned in a memory's prose is not picked up. `long_docs` and the manual
page both describe it correctly; only the schema line is wrong, and the schema is the
surface a caller reads first.

## Symptom (Effect)

`src/tools/memory/mod.rs:711`:

```
"description": "Operation to perform; refresh_anchors re-derives anchor topics."
```

A caller who adds a new `src/...` citation to a memory and then calls
`refresh_anchors` expecting the new path to become an anchor gets `"ok"` and no new
anchor. Nothing errors. The sidecar is silently unchanged with respect to that path.

## Reproduction

Add a backticked path to a memory's prose that is not currently in its
`.anchors.toml`, then `memory(action="refresh_anchors", topic=...)`. The sidecar gains
no entry for it. `memory(action="write", ...)` with the same content does add it.

## Environment

codescout 0.15.0, branch `experiments`, at `09ecb58d`.

## Root cause

`src/tools/memory/mod.rs:1126` (the `"refresh_anchors"` arm) calls
`crate::memory::anchors::refresh_hashes` at `:1144`. `refresh_hashes`
(`src/memory/anchors.rs:324-351`) reads the sidecar and `retain_mut`s over
`anchor_file.anchors` — dropping gitignored and unreadable paths, re-stamping
`a.hash` for survivors. It never reads the memory's text, so it cannot derive an anchor
that is not already recorded. The doc comment at `src/memory/anchors.rs:317-323` states
this explicitly ("it never re-seeds").

The correct description already exists twice in the codebase: `long_docs` at
`src/tools/memory/mod.rs:678` says "re-hash a topic's code anchors", and
`docs/manual/src/tools/memory.md` § `action: "refresh_anchors"` is also right.

Measured 2026-09-08: read the arm, `refresh_hashes`, and both correct descriptions.

## Evidence

Three descriptions of one function, one of them wrong:

```
src/tools/memory/mod.rs:711   "refresh_anchors re-derives anchor topics."     <- schema, WRONG
src/tools/memory/mod.rs:678   "re-hash a topic's code anchors"                <- long_docs, right
docs/manual/src/tools/memory.md                                               <- manual, right
```

The wrong one is the one a client renders inline next to the parameter.

## Hypotheses tried

1. **Hypothesis:** "re-derives" is loose phrasing for "re-hashes" and no caller would be
   misled.
   **Test:** compared against `update_anchors_on_write` (`src/memory/anchors.rs:291`),
   which genuinely does re-derive — it re-seeds from new content and then merges.
   **Verdict:** rejected. Two functions in the same module differ on exactly this axis,
   so "re-derive" is a live distinction in this codebase, not loose synonymy.

## Fix

Fixed. `src/tools/memory/mod.rs:711` schema description changed from
"refresh_anchors re-derives anchor topics." to "refresh_anchors re-hashes
anchors only." — under budget (`TOOL_SURFACE_CHAR_BUDGET` had zero headroom at
fix time; the replacement is 2 chars shorter than the original, not longer, so
no other description needed trimming).

Fix SHA: *(recorded at archive time — see Resume)*
Patch-id: *(recorded at archive time — see Resume)*
## Tests added

None added — this is a one-line schema-string correction with no behavior
change, so no new assertion was warranted. `server::tests::tool_surface_under_budget`
(already existing) re-ran green after the edit, confirming the surface stayed
under budget. There is still no test asserting a tool's schema description and
its `long_docs` say the same thing about an action's semantics — noted in
Resume as a possible class, not built here (n=1).
## Workarounds

Use `memory(action="write", content=<full document>)` when the anchor SET needs to
change; use `refresh_anchors` only to re-baseline hashes for an anchor set that is
already correct.

## Resume

N/A — fixed. If a second instance of schema/long_docs disagreement is found
elsewhere, that is when a gate comparing the two becomes worth building.
## References

- `src/tools/memory/mod.rs:711` (wrong), `:678` (right), `:1126`, `:1144`
- `src/memory/anchors.rs:317-323`, `:324-351` (`refresh_hashes`), `:291`
  (`update_anchors_on_write`, which does re-derive)
- `docs/manual/src/tools/memory.md`
- Found while re-deriving all 16 stale memories, `09ecb58d`

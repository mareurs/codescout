---
status: open
opened: 2026-09-02
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md
tags:
  - cluster/doc-contradicted-by-code
kind: bug
---

# BUG: `index`'s description enumerates three actions; the enum, the dispatcher and the callers have four

## Summary

The `index` tool's description reads *"Actions: `build` (…), `status` (…), `cancel` (…)"*.
Its `action` enum is `["build", "status", "cancel", "verify"]`, `verify` dispatches to
`IndexVerify`, and it was called 16 times in the last 30 days. The one action that answers
*"did the index actually cover everything?"* is discoverable only by reading the enum, and
the description that claims to list the actions leaves it out.

## Symptom (Effect)

Wire (`tools/list`, 2026-09-02):

```
description: Semantic index operations. Actions: `build` (build/update the project's semantic
index; pass `scope='lib:<name>'` to index a registered library), `status` (show index stats),
`cancel` (abort an in-flight reindex — no-op if nothing is running).
properties.action.enum: ["build", "status", "cancel", "verify"]
```

usage.db, 30 days to 2026-09-02:

```
SELECT count(*) FROM tool_calls WHERE tool_name='index' AND input_json LIKE '%verify%';  → 16
```

## Reproduction

`git rev-parse HEAD` → `4dc0daa2`. `python3 scripts/probe_tool_surface.py --json`, find
`index`; compare `description` with `inputSchema.properties.action.enum`.

## Environment

Not environment-dependent.

## Root cause

`src/tools/semantic/index.rs:836` (description) names three actions;
`src/tools/semantic/index.rs:848` (enum) and `:879` (`"verify" => IndexVerify.call(…)`) carry
four. `verify` shipped with the fix for
`docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md`;
the enum and dispatcher were updated, the sentence that enumerates them was not.

No test relates a description's enumerated actions to the enum. `all_tools_have_valid_schemas`
checks shape only; `tool_descriptions_stay_under_budget` checks length only.

Measured 2026-09-02: wire dump; `src/tools/semantic/index.rs:836/848/879`; usage.db count.

## Evidence

### Wire
`tools_list.json` in the session scratchpad,
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/`.

### Same-surface comparison
Of the tools whose description enumerates actions, `workspace` names 3/3, `library` 2/2,
`edit_code` 4/4, `index` **3/4** (computed over the wire dump, 2026-09-02).

## Hypotheses tried

1. **Hypothesis:** `verify` is intentionally undocumented (internal/probe action).
   **Test:** 16 calls in 30 days from agent sessions; `IndexVerify` is a public tool struct.
   **Verdict:** rejected.

## Fix

Plan, not implemented: add `` `verify` (walk the project and report files the index
skipped) `` to the description at `src/tools/semantic/index.rs:836` (+~60 chars; fund from the `patch`
trim in the sibling bug). Gate: for every tool whose description contains `Actions:`, assert
every value of `properties.action.enum` appears in the description — `index` fails today,
the other three pass. Scope the gate to descriptions that *enumerate*; `artifact` and
`librarian` describe by theme and would be false positives.

## Tests added

None yet. Owed: the enumeration gate above.

## Workarounds

Read the enum; `index(action="verify")` works.

## Resume

Edit `src/tools/semantic/index.rs:836`; add the gate beside `tool_descriptions_stay_under_budget` in
`src/server.rs`; `cargo test --lib index`.

## References

- `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md` — where `verify` came from.
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section.
- `docs/trackers/issue-clusters.md` `IC-11`.

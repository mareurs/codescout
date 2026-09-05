---
id: 3f0e7733ae77c707
kind: bug
status: open
title: 'BUG: docs/architecture/ is in neither present_tense_surfaces'' inclusion list nor its exclusion rationale'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`present_tense_surfaces()` in `tests/doc_tool_refs.rs` enumerates the doc surfaces whose tool calls
must name live tools. It walks `docs/manual/`, `src/prompts/guides/`, and seven named files.
**`docs/architecture/` is in neither the inclusion list nor the documented exclusion rationale** — it
falls through silently. `docs/architecture/augmented-artifacts.md` currently carries **10 dead-tool
call sites** in present tense, and CLAUDE.md directs readers there as authoritative.

## Symptom (Effect)

A present-tense architecture reference prescribes calls to tools that no longer exist, and the gate
named `a_documented_call_names_a_live_tool` is green.

## Reproduction

At `93fd8deb` on `tool-collapse`:

```
grep -nw 'artifact_refresh\|artifact_augment\|artifact' docs/architecture/augmented-artifacts.md
```

Live call syntax at `:213` (`artifact_refresh(action="gather", id)`) and `:253`; six
`artifact_augment(...)` forms; tool-surface lists at `:11` and `:355`; plus stale
`artifact(update, ...)`. Then read `tests/doc_tool_refs.rs:246-280` and confirm `docs/architecture/`
appears in neither the walk nor the exclusion comment.

Measured 2026-09-02 by the Opus task review of `f7b7ff33`.

## Environment

Branch `tool-collapse` at `93fd8deb`. The scope list is shared with `experiments`; the *staleness* is
branch-local, since Tasks 1–6 are what renamed and deleted the tools named in that file.

## Root cause

`present_tense_surfaces()` is an **enumerated allowlist**. Its doc-comment explains why certain
directories are excluded — `docs/issues/`, `docs/plans/`, `docs/superpowers/`, archives — on the
sound principle that a historical record *should* name the tools that existed when it was written.

`docs/architecture/` is not in that rationale. It is not deliberately excluded and not included; it
simply never came up. So the exclusion list, which exists to make the scope decisions legible,
**documents the reasoning for every directory except the one that is wrong**.

That is the sharp edge: a reader auditing coverage reads the exclusion comment, finds it thoughtful
and complete-looking, and has no prompt to ask which directories appear in *neither* list.

## Why `IC-14` and how it differs from its two siblings

The guard's name states the property — *a documented call names a live tool* — and its
implementation covers an enumerated subset. Three open bugs now share that class on three different
axes, and they are **not** re-files of each other:

| bug | what narrows | fix |
|---|---|---|
| `bee04240275ee7d9` | a *citation filter* inside a scanned file (`!c.tool.contains('_')`) | change the filter |
| `db80a4adc712c971` | *file type* — every scanner is markdown-scoped, prose moved into YAML | add a YAML reader |
| this one | *directory enumeration* — a markdown dir in no list | add the directory |

Same class, three mechanisms, three independent fixes. Filed separately for that reason.

## Evidence

### The file is authoritative, not incidental

`CLAUDE.md` § *Docs* points readers at `docs/architecture/` for component details. A stale call there
is read as instruction, which is the same property that makes `docs/augmentations/*.yaml` worse than
ordinary staleness — except this one is prose a human reads rather than a prompt a model consumes.

### A plain tool list is invisible even where the gate does reach

`:11` and `:355` are tool-surface *lists*, not call forms. `doc_tool_refs`' regex anchors on
`tool(param=`, so a list of bare names carries nothing to match — meaning adding the directory to the
walk would catch `:213` and `:253` but **not** `:11` or `:355`. Say so in the fix rather than
declaring the directory covered.

## Hypotheses tried

1. **Hypothesis:** `docs/architecture/` is deliberately excluded as historical, like `docs/plans/`.
   **Test:** read the exclusion rationale in `present_tense_surfaces()`.
   **Verdict:** rejected — it names four other paths and not this one.
   **Evidence:** § Root cause.

2. **Hypothesis:** this is a re-file of `db80a4adc712c971`.
   **Test:** compared root causes — that bug's mechanism is "every scanner is markdown-scoped".
   **Verdict:** rejected. These files *are* markdown; the gap is directory enumeration, and the fixes
   do not overlap.
   **Evidence:** § Why `IC-14`.

## Fix

1. **Add `docs/architecture/` to `present_tense_surfaces()`**, and expect it to red immediately on
   `:213` and `:253` — that RED is the acceptance criterion, not a problem to route around.
2. **Repair the file's call sites** once the collapse programme's renames have settled. Doing it
   before Task 13 means doing it twice.
3. **Record what the addition does not reach** — the bare tool lists at `:11` and `:355`. A directory
   added to the walk is not the same as a file made correct, and conflating them is how this class
   keeps recurring.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: with `docs/architecture/` in the walk and the file
unrepaired, `a_documented_call_names_a_live_tool` must fail naming `artifact_refresh`.

## Workarounds

Treat `docs/architecture/` as unverified when reading it for current tool syntax. The authoritative
surfaces are the schema and `get_guide("librarian")`.

## Resume

Add `docs/architecture/` to `present_tense_surfaces()`' walk and confirm the expected RED before
touching the prose. Then audit the rest of `docs/` for directories in neither the inclusion list nor
the exclusion rationale — this bug is one instance and the enumeration was never a closed set.

## References

- Found during the Opus task review of `f7b7ff33` (Task 6 of the tool-surface-collapse plan),
  2026-09-02, as review finding I3.
- Siblings in the same class, different mechanisms: `bee04240275ee7d9`, `db80a4adc712c971`.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a gate that is silent about a
  directory it never walked is a negative result that does not name its scope.


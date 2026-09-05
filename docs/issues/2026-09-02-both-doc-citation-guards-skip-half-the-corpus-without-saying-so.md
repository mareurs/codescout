---
id: bee04240275ee7d9
kind: bug
status: open
title: 'BUG: both doc-citation guards silently skip 49% of the corpus, and neither reports a denominator'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`tests/doc_tool_refs.rs` carries two guards over documented tool calls. **Both silently skip 304 of
618 anchored citations — 49% of the corpus** — because each excludes non-matching tokens by
`continue` rather than by failing. The excluded half is every citation naming `artifact`, the tool
this branch renamed to `doc`. Neither guard reports the skip; both are green.

## Symptom (Effect)

A documented call naming a dead tool, or a parameter that does not exist on it, passes both guards
provided the tool's name contains no underscore. No error, no warning, no count.

## Reproduction

At `5da2537d` on `tool-collapse`, replicate the scanner and bucket its output by tool name:

```
618 anchored citations across 130 surfaces
304 of them (49%) name `artifact`
```

`a_documented_call_names_a_live_tool` skips them at:

```rust
if !c.tool.contains('_') { continue; }
```

`a_documented_tool_parameter_exists_on_that_tool` skips them at `schemas.get("artifact") → None →
continue`.

Measured 2026-09-02 by the Opus task review of `5da2537d`, by replicating the scanner against the
live corpus — not inferred from reading.

## Environment

Branch `tool-collapse` at `5da2537d`. The **ratio** is branch-specific (this branch renamed
`artifact` → `doc`), but **the mechanism is not**: the no-underscore rule skips any single-word tool
name on any branch.

## Root cause

Two independent exclusions, each individually reasonable, whose union is half the corpus.

1. **The no-underscore rule is a population filter that decayed.** It was written when every
   librarian tool was snake_case (`artifact_event`, `artifact_augment`, `artifact_refresh`), so
   "contains `_`" separated tool names from ordinary English words in prose. Renaming the
   most-cited tool to a single word (`doc`) put **both** the old name and the new one outside the
   filter — the old one because `artifact` has no underscore, the new one likewise. The filter did
   not become wrong; the population moved.

2. **A `schemas.get(...)` miss is treated as "not a tool" rather than "not a live tool".** For a
   name that never was a tool, skipping is right. For a name that *was* one and has been deleted —
   exactly the case a liveness guard exists to catch — skipping is the failure.

## Why `IC-14` and not `IC-2`

`IC-2` (*a gate keyed on an event it cannot observe substitutes a proxy*) was considered first and
**rejected on its admission test**: the discriminator here is not unobservable. The tool registry is
present in the same test — `a_documented_tool_parameter_exists_on_that_tool` already calls
`schemas.get(...)`. The underscore rule is not a proxy for something the gate cannot see; it is a
*narrowing* of a property the gate could check directly. That is `IC-14`: the guard's name
("a documented call names a live tool") states the property, and its implementation covers a subset,
with its own green result concealing the remainder.

## Evidence

### The two skips, and why neither can report

Both are `continue` inside a loop that accumulates offenders. A skipped citation contributes nothing
to the offender list and nothing to any count, so the guard cannot distinguish "checked and clean"
from "not checked". There is no denominator anywhere in the output.

### The renaming interaction is what makes it acute now

The collapse programme renames or deletes `artifact`, `artifact_event`, `artifact_augment`,
`artifact_refresh`. Three of those four contain underscores and are therefore checked; the fourth
and most-cited does not. So the guard's coverage over this programme's own blast radius is
**inversely** related to how often a name is cited.

## Hypotheses tried

1. **Hypothesis:** the skipped citations are caught by another gate.
   **Test:** checked `present_tense_surfaces()` and the `audit_doc_refs` lint for overlapping
   coverage of anchored call forms naming `artifact`.
   **Verdict:** rejected — `present_tense_surfaces()` matches an anchored call form and is silent on
   prose that does not use it; nothing else buckets by tool liveness.
   **Evidence:** § Reproduction.

2. **Hypothesis:** dropping the no-underscore rule is a safe fix.
   **Test:** not run. It is the obvious change and it is the one to measure first, because the rule
   exists to suppress false positives on English words used in call-shaped prose.
   **Verdict:** deferred — **measure the false-positive rate before removing it.** A guard that
   fires on every occurrence of `note(` or `find(` in prose will be disabled by its third user,
   which is a worse end state than the present under-coverage.

## Fix

Not chosen; the ordering matters more than the choice.

- **Replace the heuristic with the registry.** The live tool list is available. A citation whose
  tool is in the registry is checked; one that is not is *reported as unrecognised* rather than
  skipped. This turns the silent half into output.
- **Then** decide what to do with the unrecognised bucket — it is the false-positive population the
  underscore rule was suppressing, and its size is unknown until measured.
- **Emit a denominator either way.** `checked N of M citations` in the assertion message costs one
  line and makes every future narrowing visible. The absence of that line is why this survived.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: a documented call naming a dead single-word tool must
fail the liveness guard. Today it passes.

## Workarounds

When renaming or deleting a tool, do not rely on these guards for prose coverage. Grep the corpus
for the old name directly, with word boundaries, and remember that a tool name may also be a SQL
table name (`"artifact"` is both).

## Resume

Replicate the scanner and bucket all 618 anchored citations by tool name, then re-run with the
no-underscore rule removed and count the new failures — that number is the false-positive population
the rule was hiding, and it decides whether the registry swap is a one-line change or needs an
allowlist. Do not remove the rule before measuring it.

## References

- Found during the Opus task review of `5da2537d` (Task 5 of the tool-surface-collapse plan),
  2026-09-02, as review finding M4.
- `docs/trackers/issue-clusters.md` § `IC-14`, and § `IC-2` (considered and rejected — see above).
- `CLAUDE.md` § *Testing Discipline* — "A count of a defect population must arrive with its unit or
  not at all", and the population-vs-member law: a guard computed over a filtered population cannot
  speak for the members it filtered out.


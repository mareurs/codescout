---
id: '4f4e1478e0a7ba2e'
kind: bug
status: open
title: 'BUG: param_probe does not recurse, so nesting a schema key silently removes it from guard reach'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`param_probe::sweep` iterates `schema["properties"]` and does **not** recurse into nested object
properties. So moving a parameter inside a nested object silently removes it from the probe's
reach — no error, no warning, no change in the probe's own pass/fail. Task 4's `artifact_event`
fold moved **nine** previously-probed keys inside a new `event` object, and all nine are now
unguarded.

The probe's stated job is to catch action-labelled schema keys the tool's `Args` does not honor
(`IC-15`). Its actual coverage is *top-level* action-labelled keys. The name and the coverage
differ, and nothing in the output says so.

## Symptom (Effect)

A schema key labelled for an action, nested inside an object, is never probed. Concretely, both
halves measured 2026-09-02 by mutation at `0c68cdc0`:

```
bogus_unhonored_key labelled "event_create: …" INSIDE event.properties   → probe GREEN (missed)
identical key at TOP LEVEL                                              → probe RED:
  doc: these schema keys are labelled for an action whose Args has no such field
       … ["event_create:bogus_unhonored_key (declared string)"]
```

Same key, same label, same tool — caught at one nesting depth and invisible one level down.

## Reproduction

At `0c68cdc0` on `tool-collapse`:

1. In `src/librarian/tools/artifact.rs`, add to the `event` object's `properties`:
   `"bogus_unhonored_key": {"type": "string", "description": "event_create: not honored"}`
2. `cargo test --lib every_action_labelled_schema_key_is_honored_by_that_action`
3. Observe **PASS**. Move the same key to the schema's top-level `properties`, re-run, observe FAIL.

## Environment

Branch `tool-collapse` at `0c68cdc0`, worktree
`/home/marius/work/claude/codescout/.worktrees/tool-collapse`. The non-recursion predates this
branch; the nine-key exposure is new as of `0c68cdc0`.

## Root cause

`param_probe::sweep` (`src/tools/param_probe.rs:83-130`) walks `schema["properties"]` one level and
reads each key's description for a leading `"<action>: "` label. A `"type": "object"` property's own
`properties` map is never descended into, so nested keys are neither labelled-scanned nor probed.
Measured 2026-09-02 by the paired mutation above — not inferred from reading.

The exposure this creates is `IC-15`: `event_create::Args`
(`src/librarian/tools/event_create.rs:25-45`) carries no `#[serde(deny_unknown_fields)]`, so a
nested key the `Args` lacks is silently discarded rather than refused. The probe existed to catch
exactly that, and no longer can for these keys.

**Nine keys moved out of reach** by the Task 4 fold: `kind`, `payload`, `anchor_commit`,
`head_commit`, `parent_event_id`, `author`, `also_mutates`, `resolves_intent_event_id`, `source`.
The retired `artifact_event` probe covered all nine as top-level keys (site 3 of 4, floor 11,
`accepts_any_json: &[]` — verified-empty by its own comment).

**Why this is `IC-14` and not `IC-15`.** The bug is the guard's coverage being narrower than its
name; the silent-drop exposure is the consequence. No key is *currently* dropped — `event_create`'s
`Args` has all nine. What is gone is the guard that would catch it if one were removed. Tagging the
consequence would misfile the mechanism, and `CLAUDE.md` § *Bug Tracking* asks for the mechanism.

## Evidence

### The paired mutation

Both directions run, because one result alone does not distinguish "nested keys are skipped" from
"the probe is broken". The top-level RED is the control that makes the nested GREEN mean something.

### Stale count in the probe's own comment

`src/librarian/tools/artifact.rs:445-447` reads *"37 labelled keys across the 12 actions as of
2026-08-17"*. As of `0c68cdc0` there are **14** actions and 4 more labelled keys. The floor of 30 is
unchanged, so the assertion does not false-alarm — it simply no longer describes what it measures.

### Broken site numbering across the family

The probe family's cross-references no longer agree: `artifact.rs:436` "Site 1 of 4",
`librarian.rs:248` "site 2 of 4", site 3 **deleted** with `artifact_event.rs`,
`artifact_refresh.rs:121` "site 4 of 4", `library.rs:607` "Site 5". Two "of 4"s are wrong and one
"of 5" implies a site that no longer exists.

## Hypotheses tried

1. **Hypothesis:** the probe recurses and the nine keys are still covered under a compound name.
   **Test:** injected an unhonored key inside `event.properties` and ran the probe test.
   **Verdict:** rejected — GREEN. Confirmed by the top-level control, which reds.
   **Evidence:** § Symptom.

2. **Hypothesis:** `serde` refuses the unknown nested key anyway, so the probe's blindness costs
   nothing.
   **Test:** read `event_create::Args` for `deny_unknown_fields`.
   **Verdict:** rejected — absent, so unknown nested keys are silently discarded. Read, **not
   measured at runtime** — a runtime confirmation is owed before the fix is designed.
   **Evidence:** `src/librarian/tools/event_create.rs:25-45`.

## Fix

Not yet chosen; two candidates with different blast radius, and the choice should follow a
measurement rather than taste.

- **A — make `sweep` recurse** into `"type": "object"` properties, composing the probe argument
  accordingly. Restores coverage generally, including for any future nesting. Risk: the probe must
  synthesise a nested argument shape, and every other probe site inherits the change.
- **B — `#[serde(deny_unknown_fields)]` on `event_create::Args`.** Cheap and local, and converts the
  silent drop into a loud refusal, but only for this one `Args` — it fixes the *consequence* at one
  site and leaves the probe still narrower than its name everywhere else.

**Prefer A if the recursion is tractable**, because B leaves the misnamed guard in place and the
next nesting re-opens the same hole. Whichever is chosen, the acceptance criterion is the mutation
in § Reproduction going **RED**.

Also fix, independently of A/B: the stale count comment and the family site numbering (§ Evidence).
Those are being corrected in the Task 4 fix round and may already be done — check before repeating.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED** on the nested-key mutation, with the top-level mutation
retained as the control — a single green/red pair at one depth cannot distinguish coverage from
coincidence.

## Workarounds

When adding a nested schema key to any librarian tool, do not rely on the probe. Either place the
key at top level (where it is probed), or manually verify the `Args` honors it.

## Resume

Read `param_probe::sweep` (`src/tools/param_probe.rs:83-130`) and determine whether recursion into
`"type": "object"` properties is tractable — specifically whether the probe can synthesise a valid
nested argument for the object without a per-site hand-written shape. If yes, take fix A. If the
synthesis needs per-site knowledge, take B for `event_create::Args` **and** file the residual as a
known coverage gap on the probe rather than closing this. Re-run the § Reproduction mutation pair
either way.

## References

- Found during the Opus task review of `0c68cdc0` (Task 4 of the tool-surface-collapse plan),
  2026-09-02, as review finding I4.
- `docs/trackers/issue-clusters.md` § `IC-14` (this class) and § `IC-15` (the exposure it creates —
  whose Index row records the probe as its partial mechanism, "5 of 8 sites 2026-09-02", a count
  this bug shows is measured in sites rather than in keys).
- `CLAUDE.md` § *Testing Discipline* — "Mutate once per guarded SITE, not once per feature."


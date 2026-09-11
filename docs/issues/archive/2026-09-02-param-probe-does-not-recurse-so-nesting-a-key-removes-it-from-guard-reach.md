---
id: 9b9c2a4c2c725e36
kind: bug
status: fixed
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

**A — `sweep` recurses one level.** Taken after the § Resume tractability check came back yes:
`required(action)` already hands back a type-valid parent object (`event`: `{kind, payload}`;
`augment`: `{prompt}`), so the probe clones that and ill-types one child. No per-site
hand-written shape was needed, which was this file's stated condition for preferring A over B.

**The half this file did not name, and without which A ships as a no-op.** Nested keys carry no
`<action>:` label of their own — every child of `doc.event` is described `"event author"`,
`"event kind"` — so a recursion that scanned children for labels would match nothing and stay
green on the very mutation in § Reproduction. A child inherits its **parent object's** action,
which is exact because an object like `event` exists for one action only.

Two things the widening forced:

- **`accepts_any_json` grew a dotted-path form** (`"augment.params"`). The first two keys
  recursion reached were `augment.params` and `augment.params_schema`, both `Option<Value>` on
  `augment::Args` — no value is ill-typed for them, so they came back *unhonored* when the probe
  structurally cannot speak for them. A guard that widens its reach owes its escape hatch the
  same widening, or its first use reads honest blindness as a defect.
- **`sweep` returns `Sweep { checked, unhonored, unprobeable }`.** Recursion is one level and
  needs a supplied parent, so the sweep now *reports* the nested objects it could not vary
  rather than passing over a population it quietly shrank; `assert_all_honored` reds on a
  non-empty `unprobeable` and names the call site's own `required` table as the repair.
  `unprobeable` counts **described** grandchildren, not grandchild-bearing objects: `doc`'s real
  `event.source` holds three bare `{"type": …}` entries skipped at every level, and an alarm that
  fires where no coverage is at stake is one somebody eventually silences.

**`doc`'s floor 80 → 95**, read from `sweep`'s own `checked` on 2026-09-11, not chosen. The +15
reconciles: `event`'s 9 described children plus `augment`'s 8 less the two declared blind.

**B was not also taken; the part of it A does not cover is filed rather than folded in.** B's
target — `event_create::Args` carrying no `deny_unknown_fields` — is a *runtime* contradiction
between a schema that declares `"additionalProperties": false` and code that silently drops,
where A is a *test-time* guard-coverage mechanism. It is
`docs/issues/2026-09-11-the-event-object-declares-additionalproperties-false-and-nothing-enforces-it.md`.
One thing established here that it needed: the module doc's blanket warning that
`deny_unknown_fields` "broke every `doc(update)` call" does **not** transfer to it, because
`flatten_event_args` builds `Args` from the `event` object's own keys plus `artifact_id` and
never merges sibling top-level keys.

**Both § Evidence sub-items, which § Fix flagged as possibly already done.** The stale
*"37 labelled keys across the 12 actions"* comment was already gone — not repeated. The site
numbering was still broken (three live sites reading `1 of 4`, `2 of 4`, `4 of 4`) and is fixed
by **deleting the ordinal**, not refreshing it: the `M` had already decayed twice, and the
membership is one `references(symbol="assert_all_honored")` call away, so each site now names
its tool and direction and stores no count (`CLAUDE.md` § *Observer Blindness*, ship the
derivation rather than the value). The numbering also conflated the forward and reverse
directions, which are separate call sites of separate functions.

Fix SHA: 51dd7c15
Patch-id: 764ce41546db822c69448b7d5371215fb15866f2
## Tests added

All in `src/tools/param_probe.rs`, and all feature-independent — each appears in **both** gate
lanes, which is the module's own claim about itself checked rather than assumed.

- `a_nested_key_is_probed_under_its_parents_action` — the § Reproduction shape as a guard.
  Carries `kind` beside the unhonored `bogus` on purpose: neither child is labelled, so both
  being probed is what pins the **inheritance** rule rather than the recursion alone.
- `a_nested_object_absent_from_required_is_reported_unprobeable` — the loud skip. Asserts
  `unhonored` stays empty in the same breath: *unreachable* and *unhonored* are different claims
  and collapsing them would make the alarm unreadable.
- `a_child_object_deeper_than_one_level_is_reported_unprobeable` — the declared depth limit.
- `a_child_object_with_no_described_grandchildren_is_silent` — its other half, copied
  shape-for-shape from `doc`'s real `event.source`. This is the case that made the first cut of
  the report fire where nothing was at stake.

**Acceptance was the § Reproduction mutation run on the production schema, not on a fixture.**
Adding `"bogus_unhonored_key"` to the real `event.properties` in `src/librarian/tools/artifact.rs`
was GREEN at `0c68cdc0` and now reds as
`doc: … ["event_create:event.bogus_unhonored_key (declared string)"]`. The top-level control
from § Reproduction is unchanged and still reds, so the pair still discriminates depth.
## Workarounds

When adding a nested schema key to any librarian tool, do not rely on the probe. Either place the
key at top level (where it is probed), or manually verify the `Args` honors it.

## Resume

Fixed and archived — nothing to resume here. The two threads that continue elsewhere:

- `docs/issues/2026-09-11-the-event-object-declares-additionalproperties-false-and-nothing-enforces-it.md`
  carries the runtime half (candidate fix B), with the `flatten_event_args` finding that makes it
  safe already recorded there.
- The sweep's own remaining edges are **mechanised rather than noted**: a second level of
  nesting, and a nested object no `required` table supplies, both red the build the moment they
  would cost coverage. That is the reason no "known coverage gap on the probe" note was filed as
  this file's § Resume contemplated — the fallback branch it belonged to was the one where A
  proved intractable, and a standing assertion beats a note a reader has to find.
## References

- Found during the Opus task review of `0c68cdc0` (Task 4 of the tool-surface-collapse plan),
  2026-09-02, as review finding I4.
- `docs/trackers/issue-clusters.md` § `IC-14` (this class) and § `IC-15` (the exposure it creates —
  whose Index row records the probe as its partial mechanism, "5 of 8 sites 2026-09-02", a count
  this bug shows is measured in sites rather than in keys).
- `CLAUDE.md` § *Testing Discipline* — "Mutate once per guarded SITE, not once per feature."

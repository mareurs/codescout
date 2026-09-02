---
id: '4f5c5ddf3c6f1a70'
kind: bug
status: open
title: 'BUG: param_probe reads only the first slash token of a shared label, so every later action is unswept'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`param_probe::sweep` derives the action a schema key belongs to from the **first** token of its
description label. A key shared by several actions is labelled `get/update/append_entry/gather:`,
so only `get` is ever probed and every action after the first slash is unswept. The probe's own
`checked` counter rises anyway, so the coverage loss reads as coverage.

Measured 2026-09-02: for `doc` alone this is roughly **12 action/key pairs** unswept, while
`checked = 58` reads as thorough.

## Symptom (Effect)

An action can lose its dispatch arm entirely and no probe fires. Observed by the Opus review of
`f7b7ff33`: deleting `"gather" => super::refresh::call(ctx, args).await,` from `doc`'s dispatcher —
so every `doc(action="gather")` returns `unknown action 'gather'` — left the full lib suite at
**5007 passed, 0 failed**.

## Reproduction

At `93fd8deb` on `tool-collapse`:

1. Delete the `"gather" =>` arm from `src/librarian/tools/artifact.rs`'s dispatcher.
2. `cargo test --lib` → **5007 passed, 0 failed**. Nothing detects it.
3. Now move `gather` to the front of `id`'s description label (`gather/get/update/…:`), leaving the
   arm deleted. Re-run → **RED**: `doc: these schema keys are labelled for an action whose Args has
   no such field ... ["gather:id (declared string)"]`.

Step 3 is the control that makes step 2 mean something: the probe works, it simply never looks past
the first slash.

## Environment

Branch `tool-collapse` at `93fd8deb`. The mechanism is in `src/tools/param_probe.rs`, shared with
`experiments`; the *exposure* grew on this branch because the tool-surface collapse is what created
multi-action shared labels.

## Root cause

`src/tools/param_probe.rs:107`:

```rust
let Some(action) = desc.split(':').next().and_then(|l| l.split('/').next()) else { continue };
```

`split('/').next()` takes the first token and discards the rest. A label naming N actions is treated
as naming one.

**This is the second distinct defect in the same probe**, and they narrow the population on
different axes: `4f4e1478e0a7ba2e` is *depth* (no recursion into nested object properties),
this one is *breadth* (only the first action of a shared label). Both leave `checked` looking
healthy. Filed separately because the fixes are independent — one changes traversal, the other
changes label parsing.

**How the regression entered.** At `8fed519b`, `artifact_refresh`'s own schema labelled `id` as
`"gather: artifact id"` — `gather` was the leading token, so it *was* swept. The Task 6 fold moved
that key onto `doc`, where the label became `get/update/.../gather:`. The action did not lose
coverage because anyone removed a test; it lost coverage because its **position in a string**
changed.

## Evidence

### Every indicator moved the reassuring way

`checked` rose 56→58, `PROBE_ACTIONS` went `[&str; 15]` → `[&str; 17]`, and `probe_required("gather")`
was added — and that last one is genuinely live (`assert_required_are_advertised` calls it, confirmed
by `panic!`-probing). Only the *forward sweep* never reaches gather. **Half the wiring works, which
is precisely why the dead half reads as done.**

### The asymmetry with `list_stale`

`list_stale` is covered: `threshold_hours` is labelled `list_stale:` as a leading token, and it has
a routing test. `gather` has neither. Nothing about the two actions differs except where their names
sit in a label.

## Hypotheses tried

1. **Hypothesis:** the probe skips `gather` because some other guard covers it.
   **Test:** deleted the dispatch arm and ran the full lib suite.
   **Verdict:** rejected — 5007 passed, 0 failed.
   **Evidence:** § Symptom.

2. **Hypothesis:** the probe is simply broken for `doc`.
   **Test:** moved `gather` to the leading position with the arm still deleted.
   **Verdict:** rejected — it fires immediately. The probe works; its parser is narrow.
   **Evidence:** § Reproduction step 3.

## Fix

**Iterate every slash token, not the first.** The parse already splits on `/`; the change is to
probe each token rather than `next()`. Cheap, and it converts ~12 silent gaps into coverage for
`doc` alone.

**And emit the denominator.** `checked = 58` is the number that made this invisible: it counts
key/action pairs the probe *decided to look at*, and nothing reports the pairs it declined. A
`checked N of M labelled pairs` line costs one string and makes any future narrowing visible at the
point of use. That half matters more than the parser fix — the parser bug is one line, the
missing denominator is why nobody noticed for the life of the probe.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is the § Reproduction step 2 mutation going **RED** — deleting a dispatch arm
for an action named after a slash must fail the probe. Keep step 3 as the control; a fix that makes
step 2 red while breaking step 3 has moved the blind spot rather than closed it.

## Workarounds

When adding an action to a tool with shared parameter labels, do not rely on the probe. Add an
explicit routing test, or place the new action's name first in at least one key's label — the second
is a workaround that will silently expire the next time labels are reordered.

## Resume

Change `src/tools/param_probe.rs:107` to iterate all `/`-separated tokens, then re-run the two
mutations in § Reproduction — step 2 must go RED, step 3 must stay RED. Then re-derive `checked`
across all four probe sites and add the `N of M` denominator; the delta between the old and new
`checked` is the size of the gap this closed, and is worth recording in the fix commit.

## References

- Found during the Opus task review of `f7b7ff33` (Task 6 of the tool-surface-collapse plan),
  2026-09-02, as review finding I1.
- Sibling defect in the same probe, different axis:
  `docs/issues/2026-09-02-param-probe-does-not-recurse-so-nesting-a-key-removes-it-from-guard-reach.md`
  (`4f4e1478e0a7ba2e`).
- `CLAUDE.md` § *Testing Discipline* — "A count of a defect population must arrive with its unit or
  not at all", and "Loudness is a property of a PATH, not of a failure".


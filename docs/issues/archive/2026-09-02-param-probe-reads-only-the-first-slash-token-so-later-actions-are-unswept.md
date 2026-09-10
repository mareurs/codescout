---
id: efd7c410df7bdcfb
kind: bug
status: fixed
title: 'BUG: param_probe reads only the first slash token of a shared label, so every later action is unswept'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-09
unverified: 'The prescribed second half (emit `checked N of M labelled pairs`) was judged obviated by the parser fix rather than implemented — reasoning in the Fix section. Residue: keys skipped for `accepts_any_json` or for carrying no `<action>:` label remain uncounted anywhere.'
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
different axes: `571720406a0b7f4a` is *depth* (no recursion into nested object properties),
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

Fix SHA: `80c4fd1e3a7a6507bc2c75cbfcd78b3f9f678e37` (**experiments**)
Patch-id: `3f662b6154f5c76751495977cbab12b650518e8f`

**Duplicate filing.** The same defect was filed again a week later as
`docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md`, tagged
`cluster/selector-narrower-than-its-population` where this one is tagged
`cluster/guard-narrower-than-its-name`. One commit closes both. Two authors independently
read one narrow selector as two different architectural problems, which is a datapoint
about the two classes rather than a filing error — neither tag is wrong.

**The parser half is fixed as prescribed.** Measured after the widening (2026-09-09, from
`sweep`'s own `checked`): `doc` **80** pairs against the 58 quoted above, `librarian`
**28**, `library` **1**. The ~12-pair estimate for `doc` was low; the real figure is 22.

**The denominator half was judged OBVIATED, not skipped — dispute this if you disagree.**
The argument for `checked N of M labelled pairs` was that `checked` counted pairs the
probe decided to look at while nothing reported the pairs it declined. With the loop
fixed, every labelled pair whose token names a real action *is* probed, so `M == N` by
construction — a denominator that cannot differ from its numerator is an assertion that
cannot fail (`IC-16`), and adding one here would ship that class into the guard that
exists to catch a neighbouring one. What the raised floors do instead is make a future
narrowing visible: they are set **at** the measured pair counts, so any selector change
that drops pairs reds rather than passing quietly.

What that argument does **not** cover, and is the residue worth naming: keys skipped for
`accepts_any_json` or for carrying no `<action>:` label at all are still uncounted, and a
denominator would have surfaced those too. `accepts_any_json` is at least declared
per-site as an admission; the unlabelled population is not counted anywhere.

## Tests added

Two, in `src/tools/param_probe.rs`'s own `tests` module — which did not exist before this
fix, and that absence is why a one-token selector bug survived in the shared IC-15
detector for four tools.

**Acceptance was NOT taken via this file's § Reproduction step 2.** That step prescribes
deleting `doc`'s `"gather" =>` dispatch arm and observing a red — an armed mutation in a
shared checkout, which is itself an open defect here
(`docs/issues/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`):
a peer running the gate cannot tell a deliberate red from a real one. The same
discrimination is available without arming anything, from a synthetic two-action schema:

- `a_key_labelled_for_several_actions_is_probed_for_every_one` — the fixture's dropped key
  belongs to the **second** slash token, which preserves this file's step-3 control: a
  fixture whose *first* action drops the key passes against the bug. Watched red at
  `unhonored: []` where `["beta:id (declared string)"]` was owed.
- `checked_counts_action_key_pairs_not_keys` — watched red at `1` where `3` was owed.

Both run in **both** lanes; `param_probe` is deliberately feature-independent. Verified by
reading the two test names out of the lean lane's output, not from its total.
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
  (`571720406a0b7f4a`).
- `CLAUDE.md` § *Testing Discipline* — "A count of a defect population must arrive with its unit or
  not at all", and "Loudness is a property of a PATH, not of a failure".

---
id: 190955a99d482631
kind: bug
status: fixed
title: 'BUG: param_probe checks one action per shared schema key, so 5 of 7 declared pairs on librarian are unchecked and unadmitted'
tags:
- param-probe
- schema-drift
- guards
- cluster/selector-narrower-than-its-population
closed: 2026-09-09
opened: 2026-09-09
owner: marius
related:
- d4b61746950b86b7
severity: medium
---

> **Cluster:** `cluster/selector-narrower-than-its-population` (`IC-18`, n=31). The label
> declares a population of N actions; the selector reads element 0.

## Summary

`param_probe::sweep` decides which action a schema key belongs to by taking the **first**
slash-separated token of its description label, with no loop over the rest. A key labelled
for four actions is therefore probed for one. The other three are unchecked, are **not**
listed in `accepts_any_json`, and nothing anywhere records that they are unchecked — so the
probe reports coverage it does not have, for exactly the defect class it exists to catch.

## Symptom (Effect)

`src/tools/param_probe.rs:108`:

```rust
let Some(action) = desc.split(':').next().and_then(|l| l.split('/').next()) else {
    continue;
};
```

`librarian`'s `scope` key is described
`"context/reindex/workspace_state_at/link_scan: scope. audit_doc_refs: project-scoped only in v1 — …"`.
The selector yields `"context"`. `reindex:scope`, `workspace_state_at:scope` and
`link_scan:scope` are never probed, and `audit_doc_refs:scope` — named in a *second*
`:`-delimited clause that `split(':').next()` discards entirely — is not reachable at all.

`every_action_labelled_schema_key_is_honored_by_that_action` passes throughout.

## Reproduction

```
git rev-parse HEAD    # 4d928f2d (experiments)
cargo test --workspace every_action_labelled_schema_key_is_honored_by_that_action
```

Green (3 passed) — against a tree in which `librarian(action="doctor")` provably discards a
`scope` argument (`d4b61746950b86b7`: `scope="all"` and `scope="project"` both return
`summary.total = 169` with byte-identical response buffers).

**That green is the reproduction, not a nuisance.** `sweep` flags a key when
`outcome(&base) == outcome(&probed)`. A handler that never reads `scope` cannot be
perturbed by an ill-typed one, so the two outcomes are identical by construction and the
key would be reported unhonored. The test is green, therefore the pair was never probed.

## Environment

Linux 7.2.3-zen1-3-zen · branch `experiments` @ `4d928f2d` · `src/tools/param_probe.rs`
clean · six call sites (`librarian`, `artifact`, `library`, `workspace`, `index`,
`edit_file`).

## Root cause

`sweep` was extended to understand shared keys — the comment at `:104-107` says the slash
split is *"load-bearing — without it a shared key matches no action and is skipped
SILENTLY, which is how `librarian`'s `scope` sat unprobed while an archived IC-15 member
was that exact key on that exact tool."*

**Read that sentence's grammar carefully, because it is the most interesting thing in this
file and it invites a misreading.** The *"which is how"* clause attaches to *"**without
it** a shared key matches no action"* — so the comment describes the state the slash split
**removed**, not a live defect. It is not a case of the bug being written down at the site
and ignored. (A peer session, sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`, read it the
other way and offered *"a comment is not a mechanism"* as the lesson; that lesson is true,
already in `CLAUDE.md`, and not what happened here.)

What the comment actually shows is sharper and worse. It records the failure mode as
**"matches no action"** — a zero-versus-nonzero frame. The author was fixing *"probed for
zero"* and shipped *"probed for one"*, and the module doc states the result in the singular
without flinching: *"that action is called twice."* So the residual — one of four — was
never in frame to be noticed, and the comment memorialises the danger that was removed.

**That is the promotable form: a partial fix documents the failure mode it CLOSED, and that
documentation is what makes the remainder invisible.** A reader arriving at `:104-107` finds
a comment saying "this was broken, here is why the fix is load-bearing" and correctly reads
the area as handled. The better-placed the comment, the stronger that effect — this one
names the right line, the right key, the right tool and the archived precedent, and it still
leaves 75% of the key's declared population unchecked.

The narrowing is therefore written as a **description of the mechanism** and nowhere as an
**admission of coverage**. The file has a place for the latter: `accepts_any_json`, whose
doc says *"an admission, never a pass"* (`:43-46`). Unchecked pairs do not go there and are
counted nowhere, so nothing distinguishes "`reindex:scope` was checked and is fine" from
"`reindex:scope` was never looked at".

**`floor` cannot see it either, and this is the sharp part.** `checked` increments once per
**key** (`:127`), not per action-key pair, so a key labelled for four actions contributes
`1` to a counter whose whole purpose is detecting that the sweep stopped checking things —
a count whose unit is not the thing it guards. The floor's own doc comment states its limit
exactly (`:35-36`): *"`floor` catches the convention breaking wholesale; it cannot catch one
key losing its label."* A key losing three of its four actions is a subtler instance of that
same blind spot, and the floor is satisfied by 25% coverage.

*measured 2026-09-09: `cargo test --workspace
every_action_labelled_schema_key_is_honored_by_that_action` → **green** (3 passed), while
`doctor` demonstrably discards `scope`. Had the probe reached `doctor:scope` it would have
found `base == probed` — an ill-typed `scope` changes nothing for a handler that never
reads it — and reported the key unhonored, reddening the test. Green therefore **proves**
the pair is unchecked; it is not weak evidence about it. Line citations verified against
`HEAD` (`4d928f2d`, file unmodified) after an earlier draft of this file cited `:110`/`:104`
— numbers inferred from a `read_file` window rather than read, and corrected by the same
peer.*

## Evidence

### `librarian` alone: 7 declared pairs, 2 checked

| key | label | pairs declared | probed |
|---|---|---:|---|
| `include_archived` (`librarian.rs:63`) | `context/workspace_state_at` | 2 | `context` only |
| `scope` (`librarian.rs:68`) | `context/reindex/workspace_state_at/link_scan` + a second clause naming `audit_doc_refs` | 4 (+1) | `context` only |

Six pairs from slash labels, of which 2 are probed; plus `audit_doc_refs:scope` from the
discarded second clause. **7 declared, 2 checked, 5 unchecked and unadmitted.**

### A larger population the slash form does not capture

Descriptions that enumerate several actions with `.` separators rather than `/` are narrowed
the same way and are **not** counted above. `limit` (`librarian.rs:120`) is described
`"legibility_scan: cap candidates returned/written. link_scan: cap ARTIFACTS scanned … doctor: abs_path_outside_managed_roots window size … audit_log: max rows returned …"`
— four actions, probed as `legibility_scan:limit` alone. `confirm` (`:122`) and `root`
(`:109`) have the same shape.

**This population is deliberately not counted here.** Enumerating it means classifying every
multi-action description across six schemas, and a number derived by eye beside one derived
from a regex reconciles nowhere (§ *Testing Discipline* — *count the LIST, never the
corpus*). What is established: the slash-labelled figure above, and that the `.`-separated
form is narrowed by the same line.

## Hypotheses tried

1. **Hypothesis** (raised by peer sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`):
   `scope` is type-checked, the probe should have reached `doctor:scope`, and did not — a
   probe defect beyond its declared blindness.
   **Test:** read `:110`; run the probe.
   **Verdict:** rejected as stated. There is no defect in the *comparison*; the pair simply
   never enters the loop, because `doctor` is not in `scope`'s label at all.

2. **Hypothesis** (same peer, the other branch): `scope` is read untyped like `fix`/`offset`
   and therefore **belongs in `accepts_any_json` and is missing** — silently converting an
   admission into a pass.
   **Test:** same reads.
   **Verdict:** rejected, though its *conclusion* about coverage is right. `doctor` does read
   `scope` untyped, so an admission would be honest — but adding it there would not have
   helped, because entries in that list are `continue`d at `:98`, **ten lines before** the
   label is parsed at `:108`. The pair was already outside the probed set. Right conclusion,
   wrong location, and the location was the actionable half.

3. **Hypothesis:** the narrowing is intentional and documented, so this is a note rather than
   a bug.
   **Verdict:** partially confirmed and still a bug. It is documented as *mechanism*
   (*"that action is called twice"*) and nowhere as *coverage*. A reader of
   `assert_all_honored`'s failure message — *"these schema keys are labelled for an action
   whose Args has no such field"* — has no way to learn that only one action per key was
   consulted, and `checked` counts keys rather than pairs, so the floor cannot reveal it.
   **This is the third option the peer's binary did not contain**, and both of their branches
   being wrong is itself the finding: the coverage claim is false for a reason neither
   "probe is broken" nor "list is incomplete" describes.

## Fix

Iterate every token, and count pairs rather than keys:

```rust
let Some(label) = desc.split(':').next() else { continue };
for action in label.split('/') {
    if !spec.actions.contains(&action) { continue; }
    // ... existing base/probe pair, per action ...
    checked += 1;
}
```

Then **raise every `floor` at all six call sites to the new pair counts.** A floor left at
its key-count value still passes after the widening and would tell you nothing — the same
monotonicity the floor exists to defend against.

Expect this to red real pairs. Each is an IC-15 instance in its own right and wants its own
bug file; **do not narrow the selector back to get green, and do not move a red key into
`accepts_any_json` without first reading its handler** and confirming it reads that param
through an untyped accessor.

Two follow-ups deliberately **not** bundled:

- The discarded second clause (`audit_doc_refs:` in `scope`'s description) needs either a
  multi-clause parse or a labelling convention that puts every action in the first clause.
  Fixing the slash loop does not reach it.
- The `.`-separated multi-action form needs a decision about whether it is a supported label
  shape at all. Today it silently means "the first action only".

Fixed as prescribed — the slash loop, with `checked` counting action/key pairs.

**Measured after the widening** (2026-09-09, read from `sweep`'s own `checked`): `doc`
**80** pairs (was a 58-KEY reading), `librarian` **28**, `library` **1** unchanged — its
one key is labelled for a single action, so it had no pairs to gain. Floors raised to
those readings at all three sites, not to a fraction of them: the margin between floor
and count is exactly how many labels can lose their prefix in silence.

**The predicted reds did not arrive.** 22 newly-swept pairs on `doc` alone and zero new
`unhonored` on any of the three. Recorded as a result rather than a silence, because the
instrument was watched failing first on the synthetic fixture below. What it establishes
is narrow: no shared-key action on these three tools drops a key it advertises. It says
nothing about IC-15 elsewhere.

Both follow-ups above remain unfixed and out of scope — the multi-clause `:` form and the
`.`-separated form. The slash loop reaches neither.

**SHA:** `80c4fd1e3a7a6507bc2c75cbfcd78b3f9f678e37` (**experiments**)
**patch-id:** `3f662b6154f5c76751495977cbab12b650518e8f`

## Tests added

Both in `src/tools/param_probe.rs`'s own `tests` module — which did not exist before this
fix. That is the finding underneath the bug: `param_probe` is the shared IC-15 detector
for four tools and nothing tested the detector, which is how a one-token selector bug
survived in it.

- `a_key_labelled_for_several_actions_is_probed_for_every_one` — watched red at
  `unhonored: []` where `["beta:id (declared string)"]` was owed. The fixture's dropped
  key belongs to the **second** slash token, and that placement is the whole
  discrimination; a fixture whose *first* action drops the key passes against the bug.
- `checked_counts_action_key_pairs_not_keys` — watched red at `1` where `3` was owed, so
  a floor can see a lost label.

Both run in **both** lanes; `param_probe` is deliberately feature-independent. Verified by
reading the two test names out of the lean lane's output, not from its total.
## Workarounds

None at the probe. To check one action-key pair by hand, temporarily reorder the label so
that action is first, run the probe, then revert — or assert the pair directly in the
owning tool's tests. Neither scales, and reordering a shared label to test one action
silently stops testing the one that was first.

## Resume

Write `a_key_labelled_for_several_actions_is_probed_for_every_one` first, with the dropping
action in **second** position, and run it against unmodified `sweep` to watch it pass —
confirming the fixture actually discriminates — then invert the fixture so it fails. Only
then change `:110`. After the loop lands, run the full gate and record which pairs newly
red; report them rather than fixing them, since each belongs to a different action's owner.

## References

- `src/tools/param_probe.rs:108` (the selector), `:98` (the `accepts_any_json` `continue`,
  ten lines before label parsing), `:43-46` (*"an admission, never a pass"*), `:104-107` (the
  comment recording the slash split as load-bearing — and the zero-versus-one frame),
  `:127` (`checked` incrementing per key), `:35-36` (the floor's stated limit),
  `:133-162` (`assert_all_honored`)
- `src/librarian/tools/librarian.rs:63` (`include_archived`), `:68` (`scope`), `:120`
  (`limit`), `:198` (the probe test), `:239` (`accepts_any_json`)
- Downstream instance that motivated this file:
  `docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`
  (`d4b61746950b86b7`)
- Prior instance of `scope` on this same tool:
  `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md`
- Fix sequencing: `docs/superpowers/plans/2026-09-09-doctor-per-project-isolation.md`
  Task 1 Step 4

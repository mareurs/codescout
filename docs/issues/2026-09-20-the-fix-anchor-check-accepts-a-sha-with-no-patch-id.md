---
id: '5394e9b7bdd83069'
kind: bug
status: open
title: 'BUG: terminal_status_without_fix_anchor accepts a SHA with no patch-id, discharging on the half that dies at rebase'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: fix-anchor grammar and rebase-durable provenance
---

## Summary

`terminal_status_without_fix_anchor` discharges a record the moment
`structured_fix_pointers` returns anything at all:

```rust
if !structured_fix_pointers(&content).is_empty() {
    continue;
}
```

That parser's element type is `(String, Option<String>)` — **the patch-id is optional**. A
record declaring `- **SHA:** \`abc1234\`` and no patch-id at all yields `[("abc1234", None)]`,
which is non-empty, so the check passes.

The SHA is the half that dies. `CLAUDE.md` § *Bug Tracking* and this check's own remedy text
both say so: *"The SHA is positional and dies when `experiments` is rebased (which happens after
every ship); the patch-id is a content hash of the diff and survives rebase and cherry-pick."*
So the guard named `..._without_fix_anchor` accepts precisely the anchor that will not survive
the event it exists for.

## Symptom (Effect)

No finding, no error, no warning. The record reads as anchored, and is, until the next rebase —
at which point `scan_archived_fix_sha_unresolvable` reports the SHA as unresolvable and the
patch-id that would have recovered it was never required.

## Reproduction

Measured 2026-09-20 at tree `40370bd1`, and the fixture is this campaign's own output rather
than a constructed one. Three bug files closed earlier that day wrote provenance as a single
bullet carrying both labels:

```
- **SHA:** `737a29fe` — what it did. **patch-id:** `79c64ff0427f983feb6898e438a23910adc5768f`
```

`structured_fix_pointers` binds `- **patch-id:**` at **line start** (`t.strip_prefix`, after
`trim_start`), so all six patch-ids across those three files parsed as nothing. Each file still
yielded one `(sha, None)` pair from its SHA bullet, each was therefore discharged, and
`terminal_status_without_fix_anchor` stayed silent on all three. Verified by reading the parser
at `src/librarian/tools/doctor.rs:6248-6291` and the discharge at `:6640`.

## Environment

Branch `experiments`, tree `40370bd1`. Present since the structured grammar landed; unchanged by
`496dd63e`, which made this parser load-bearing for a second check and so raised the price.

## Root cause

**The parser's permissiveness is correct and the consumer's test of it is not.**
`structured_fix_pointers` returns `Option<String>` for the patch-id deliberately — it is a
faithful reader, and reporting *"a SHA was declared, a patch-id was not"* is exactly the
distinction a caller might want. The defect is that the only caller collapses that distinction
with `is_empty()`, which asks *"did the author write anything shaped like a pointer?"* when the
question owed is *"is this record recoverable after a rebase?"*

`cluster/guard-narrower-than-its-name`: the guard's name promises a fix **anchor**; its coverage
is *a SHA bullet exists*.

## Evidence

- `src/librarian/tools/doctor.rs:6248-6291` — `fn structured_fix_pointers(...) -> Vec<(String, Option<String>)>`; the patch-id arm only fills `last.1` when a preceding SHA is still missing one, and is dropped entirely if no SHA precedes it.
- `src/librarian/tools/doctor.rs:6640` — `if !structured_fix_pointers(&content).is_empty() { continue; }`.
- Live instance, three files, six patch-ids, all invisible, all discharged. Repaired at `40370bd1` by splitting the bullets — the *records* are now well-formed, and the *check* is unchanged.
- The check's own detail message states the grammar correctly and in full: *"two labelled bullets are, outside any fence"*. **It is emitted only to a record that FAILS.** A record with a SHA bullet and no parseable patch-id passes, so the text that would teach the shape is unreachable from exactly the state that needs it — `CLAUDE.md` § *Testing Discipline*, *loudness is a property of a PATH*.

## Hypotheses tried

1. **Hypothesis:** the three files were also missed by the sibling `non_terminal_status_with_fix_anchor`, so the pair is silent in both directions. **Test:** read the sibling's population — it selects `open`/`taken`/`investigating`, and all three files are `fixed`. **Verdict:** rejected; the sibling never sees them. The gap is one-sided.
2. **Hypothesis:** a looser grep proves more records are affected. **Test:** `^- **patch-id:**` returns 11 live records against 10 carrying the well-formed pair. **Verdict:** the one difference is `fff5758f95e2fda2`, whose line reads `- **patch-id:** N/A` with no SHA bullet — the parser correctly sees nothing there. Two instruments, two units; no additional instance.

## Fix

Not implemented. Two directions, and the choice is about what a partial pointer means:

1. **Require the pair.** Discharge only when at least one element has `Some(patch_id)`, and give the partial case its own detail text — *"a SHA is declared and no patch-id; the SHA orphans on the next rebase"*. This is strictly more findings, so it needs the same migration count `496dd63e` paid for: **how many live terminal records declare a SHA with no parseable patch-id?** Measure before shipping; that number is the whole cost.
2. **Report the partial case as its own check name** rather than folding it into this one, so the existing check's population does not move and the new state is nameable in a query.

Direction 2 is likelier right on the same reasoning `496dd63e` used: a change that can red an author's existing file is a different act from one that cannot, and this repo has paid once already for a check that began demanding a shape ~150 files did not have (`src/prompts/mod.rs:2075`).

**Do not fix by tightening `structured_fix_pointers` itself.** It has two consumers since
`496dd63e`, its `Option` is honest, and narrowing the parser to serve one caller's question would
break the inverse property those two checks now hold — which is the property
`de46d402441e1e2b` was closed to establish.

## Tests added

None. A regression test is cheap and specific, and needs both halves: a fixture declaring a SHA
with no patch-id that **must** fire, paired with one declaring both that must stay silent. The
absence half alone is monotone under removal of the check.

## Workarounds

Write the two bullets. The grammar is stated in `get_guide("tracker-conventions")` § *Bug files*
and in this check's failure text; neither is reached by an author who is getting it half right.

## Resume

Open. Locus `src/librarian/tools/doctor.rs:6248` (parser) and `:6640` (the discharge). First
step is the count named in Fix direction 1 — it decides between the two directions and nothing
should be built before it exists.

## References

- `docs/issues/archive/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md` (`de46d402441e1e2b`) — the sibling check, fixed at `496dd63e`, which made this parser shared. That file also records a **second** gap found in the same run and likewise not introduced by it: `terminal_status_without_fix_anchor` has no test covering the fence escape, so a regression in the fence skip would break it silently.
- `CLAUDE.md` § *Bug Tracking* — the SHA-dies / patch-id-survives rule this check is named for.
- **Prior-instance count deliberately not stated.** Derive it: `doc(action="find", kind="bug", include_archived=true, filter={"tags": {"contains": "cluster/guard-narrower-than-its-name"}})`.

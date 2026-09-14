---
id: '9ca234d453d61233'
kind: bug
status: open
title: A reproduction-first test is red by design for hours, and unlike an armed mutation the defect behind it is real
tags:
- cluster/transient-shared-state-lies-to-readers
topic: shared-checkout reproduction-first testing
---

## Summary

CLAUDE.md mandates reproduction-first work — *"Run the reproduction before reading the fix
plan — the plan is a hypothesis about the reproduction."* Following it produces a test that
is **red by design** from the moment it is written until the fix lands: minutes to hours.

On a shared checkout every other session sees that red in their own gate run and cannot
tell it from a regression they caused. `df0c18734b20fddd` records the same observer
confusion for an **armed mutation**, but the two are not one defect, and the difference
decides the remedy:

| | armed mutation | reproduction-first |
|---|---|---|
| is there a real defect behind the red? | **no**, it is synthetic | **yes**, and it is live |
| window | seconds to minutes | minutes to **hours** |
| announcement convention | *"say so before it fires"* (`df0c18734b20fddd`) | **none** |
| mandated by CLAUDE.md? | no | **yes** |
| safe to suppress? | yes, costs no information | **no** — hides a live defect |

## Symptom (Effect)

A peer session's full default lane, run against the shared tree while a fix was in flight:

```
5514 passed, 1 failed
librarian::tools::update_entry::tests::patching_an_unrendered_field_does_not_claim_the_body_is_behind
panicked at src/librarian/tools/update_entry.rs:453:
the body renders exactly what params now say, so there is nothing to advise about —
and the message actually emitted asserts the opposite as fact
```

They asked, verbatim: *"is `patching_an_unrendered_field_does_not_claim_the_body_is_behind`
failing right now because of a mutation you injected? … I can't tell them apart from
outside."*

**They had four signals and still could not discriminate**: the hook's `wip_authors`
attribution naming the author, the author's name, a prior message from that author saying a
mutation run was pending, and the full panic text. The panic described the DEFECT precisely
and said nothing about the test's STATUS.

## Reproduction

1. On a shared checkout, write a failing test that reproduces a real defect, per CLAUDE.md's
   reproduction-first rule. Do not fix it yet.
2. Have any other session run `cargo test --workspace`.
3. They observe a red they cannot attribute, in a file whose author is known and whose
   intent is not.

Observed 2026-09-14 between 07:14 and 07:26 on `experiments`, bug `2459a3965d98170b`.

## Root cause

**The red's provenance exists nowhere in the artifact an observer actually reads.** A
failing test emits exactly one thing — its panic message — and by convention that message
describes the defect, never the test's lifecycle stage. So the one channel guaranteed to
reach every observer carries none of what they need.

`df0c18734b20fddd`'s remedy is to announce before arming. That is a **policy** someone must
remember, and it is workable for a mutation because the window is short and the author is
watching it. It does not transfer here: the window is long, the author is mid-fix rather
than mid-run, and nothing triggers the recollection.

measured 2026-09-14: one peer asked rather than assumed; they stated that with slightly more
confidence they would have gone hunting a regression that did not exist.

## Fix

**Put the provenance in the assertion message.** It rides the only artifact a red ever
produces, and writing an assertion message is already mandatory, so there is no step anyone
can forget — § *Observer Blindness* position 3 rather than position 2.

```
REPRODUCTION for <bug id> — expected RED until the fix lands.
Expected failure: <the specific wrong behaviour>.
A DIFFERENT failure here is not this; treat it as unexplained.
```

**The second line is load-bearing and the first draft omitted it.** A bare *"expected RED"*
is **monotone under wrong-reason**: it tells the reader to stand down for every red the
assertion can produce, including a genuine second regression inside the same assertion. That
converts "four signals and I could not tell" into "one signal that always says do not look"
— strictly worse, because the ambiguous state at least prompted the question. Naming the
expected failure restores the comparison at the same one-clause cost.

This is § *Testing Discipline*'s first law — *a test cannot detect a change its assertion is
MONOTONE under* — applied to **remedy text** rather than to an assertion, which is a
placement the corpus had not recorded. It generalises: any *expected* / *known-failing*
annotation on a red is monotone under wrong-reason, **including `#[ignore]`**, which is the
same hole with no message at all.

### Rejected: `#[ignore]` until the fix lands

Proposed and withdrawn in the same exchange. Two objections, the first decisive:

1. **A reproduction red is TRUE.** Suppressing an armed mutation costs zero information
   because no defect exists behind it. Suppressing a reproduction hides a live defect for
   exactly the window it is most likely to bite someone else.
2. If the fix is abandoned the ignored test rots silently as **false coverage** — the very
   failure this repo annotates inert fixtures to prevent.

## Tests added

None, and deliberately: this is an authoring convention, not a code path. A guard would have
to assert that a red test's message names its own lifecycle stage, which is unreachable —
nothing distinguishes a reproduction from an ordinary failing test at compile time, and that
indistinguishability IS the defect.

## Workarounds

Announce before writing the test, as `df0c18734b20fddd` prescribes for its own case. Works,
and is a policy rather than a mechanism — the author of this record had read that file, cited
it to a peer the same evening, and still did not announce.

## Resume

Apply the marker on the next reproduction-first test written in this repo. If it survives a
few uses, propose it for `docs/templates/` or CLAUDE.md § *Testing Discipline*; one use is
not evidence a convention holds.

## References

- `docs/issues/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`
  (`df0c18734b20fddd`) — the synthetic-red sibling. Filed separately rather than appended
  there because the remedies diverge: announce-then-revert for a red with nothing behind it,
  mark-the-message for a red with a live defect behind it. A shared title would make every
  reader work out which half applies before using either.
- `docs/issues/2026-09-13-snapshot-stale-tests-row-presence-and-reports-a-content-claim.md`
  (`2459a3965d98170b`) — the bug whose reproduction produced this incident.

**Attribution.** The class and the incident are sessionId
`8bd791df-5ff4-40fe-af30-69cc3fefc2f7`'s, who omitted the marker and caused the confusion.
The **failure-mode clause** — the second line of the marker, and the monotone argument for it
— is sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`'s, made against a first draft that
carried only the status and would have reproduced the original failure one level up.

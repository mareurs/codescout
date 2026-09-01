---
id: faca007609016a6f
kind: bug
status: open
title: The concurrent-activation guard substitutes wall-clock proximity for caller identity, so a fast linear session is a systematic false positive
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
closed: ''
opened: 2026-09-02
severity: low
---

## Summary
`Agent::concurrent_switch_warning` fires when a *different* root is activated within 5s of
the previous activation. That predicate cannot tell two different situations apart:

- a **single linear session** following the documented cross-project pattern
  (`get_guide("workspace-state")` § *Cross-project workflow pattern*: activate foreign →
  work → activate home), where the work happened to take under five seconds; and
- a **genuine two-caller race**, where a subagent activated foreign and the controller
  then activated home.

Both are "different root, <5s apart" on a slot that records no caller identity. The first
gets a warning telling it another caller shares the server — which is false. The second is
the case the guard exists for.

This is the mitigation inheriting the defect it mitigates: the guard was built because the
active-project slot has no owner, and the missing owner field is exactly what stops the
guard being accurate.

## Symptom (Effect)
A single session that activates a sibling project, runs two or three fast reads, and
returns home receives on the *return*:

```
active project switched from /work/sibling to /work/home 1.2s ago — another caller
(e.g. a concurrent subagent) shares this server's single active-project slot, so reads
may resolve against the wrong workspace.
```

No other caller exists. The recommended remedy (`workspace=<absolute path>`) is good
advice in general but is not a fix for anything that happened here.

## Reproduction
1. `workspace(action="activate", path=<sibling>)`.
2. Any two fast tool calls (well under 5s of wall clock).
3. `workspace(action="activate", path=<home>)`.
4. The response carries `concurrent_activation_warning`.

Same shape at the unit level: `concurrent_switch_warning(Some((a, 200ms)), b, 5s)` returns
`Some` regardless of whether `a` and `b` were reached by one caller or two —
`src/agent/mod.rs:872-892`.

## Environment
codescout `experiments` @ `fff4636c`. Found 2026-09-02 while verifying Phase 5(c) of
`docs/plans/2026-05-30-per-request-workspace-pinning.md`.

## Root cause
The slot has no owner field, so the guard substitutes **wall-clock proximity** for
**caller identity**. Proximity is a proxy: it correlates with contention but does not
imply it, and a fast linear session is a systematic false positive rather than noise.

There is a second, worse consequence of the same gap. The warning is attached to the
response of the call that *performed* the switch, so it reaches the **switcher** — while
the party harmed is whoever gets resolved against the wrong workspace afterwards, who
receives nothing. In `docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
the subagent did the activating and the controller paid, discovering it only via a refused
write minutes later.

## Evidence
### The guard is already unreachable from pinned flows, so tuning it is the only lever
Verified 2026-09-02, three independent facts:

- `note_activation` has exactly **one** production caller,
  `src/tools/config/mod.rs:308`, inside the activate handler.
- `workspace` and `activate_project` are in `Tool::pinnable`'s exclusion list
  (`src/tools/core/types.rs:758-767`), so no `workspace=` pin can reach that handler.
- `last_activation` is written in exactly two places: `Agent::new` (`None`) and
  `note_activation`. Pinned calls route through `with_project_at` / `ensure_resident` and
  never stamp it, so a pinned call cannot even create the precondition for a later
  warning.

### Why the obvious fix is not a fix
The handler already computes `HintScenario::ReturnToHome` at `src/tools/config/mod.rs:302`,
immediately before calling `note_activation`, and does not pass it. Suppressing the warning
on return-to-home would remove the false positive — and would equally remove the **true**
positive in which a subagent activated foreign and the controller returned home, which is
the documented harmful case. From the server's position the two are the same two calls
against the same slot. Trading one real signal for another is not an improvement, so this
is deliberately **not** proposed as the fix.


### Not observed live, and the reason bounds the severity

During the Phase 5(d) verify (2026-09-02) I ran exactly this shape against the live server
— activate mirela, work, activate home — and **no warning fired.** Four tool round-trips sat
between the two activations, putting them well outside the 5s window.

That is not evidence against the bug: the derivation is from the pure function, and
`concurrent_switch_warning_flags_rapid_foreign_switch` (`src/agent/mod.rs`) already asserts
that a different root at 200ms warns. It is evidence about **exposure**. An LLM-driven
session pays a model round-trip between calls, so its activate→work→return cycle usually
exceeds 5s and never sees the false positive. The callers who *will* see it are the fast
ones: a script, a hook, a subagent doing one pinned read, or any path where two activations
land back to back.

Recorded as a non-observation rather than dropped, because the alternative is a file that
accumulates only the runs that fired. Severity stays **low** on this basis — and the
asymmetry is worth naming: the population most likely to trip it is the least likely to
read the warning.
## Hypotheses tried
Suppress on `ReturnToHome` — rejected above on the false-negative it introduces, before
being written.

## Fix
Not independently fixable by tuning the predicate. It needs the owner field that
`IC-17` names: record *who* activated (agent/session id) alongside the active project.
With that, the guard distinguishes "same caller returning home" from "a different caller
took the slot" exactly, and can additionally address the misdirection — the warning could
be surfaced to the party whose subsequent call resolves against a root it did not choose,
rather than to the party who moved it.

That is the same substrate as Fix 2 of
`docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`.
Build once, for the class, not twice per instance.

## Tests added
None. `concurrent_switch_warning_flags_rapid_foreign_switch` (`src/agent/mod.rs:2402`)
currently pins the present behaviour, *including* the case this file calls a false
positive — it asserts a rapid different-root switch warns, with no notion of who switched.
Left as-is deliberately: it is correct about what the code does today, and changing it
before the owner field exists would assert a behaviour nothing can yet implement.

## Workarounds
Read the warning as "the slot moved recently", not as "another caller exists". If you know
you are a single linear session, it is a false positive and pinning changes nothing for
you.

## Resume
Do not tune the 5s window or add a `ReturnToHome` exemption — both trade one wrong answer
for another. This unblocks when `IC-17` gets its owner field; at that point revisit
`concurrent_switch_warning` and the delivery target together, and update the unit test,
which today encodes the proxy rather than the intent.

## References
- `docs/plans/2026-05-30-per-request-workspace-pinning.md` § Phase 5(c) — the item whose
  verification surfaced this
- `docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
  — same missing field, Fix 2
- Cluster: `IC-17` — a shared resource carries no owner

---
kind: bug
status: superseded
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-12
owner: marius
related: []
severity: low
---

# BUG: the write-time snapshot note asserts "the file did not [change]" without checking whether it did

## Summary

`append_entry` / `update_entry` return an advisory when a params write lands on a tracker that
renders a snapshot in its body. One branch of that advisory states, as fact:

> This tracker renders a snapshot in its body, and its `BL-75` row still shows the PREVIOUS field
> values — params changed, the file did not.

The note has not checked whether the file changed. Its branch condition is **id presence** —
`if in_body.contains(&num)` — so it fires whenever the row exists in the body, including when the
caller updated that row moments earlier. The clause *"the file did not"* is an inference from a
proxy, presented as an observation.

## Symptom (Effect)

A caller who follows the documented order — write the body row, then the params row — is told the
body is stale and instructed to perform the edit they have just performed. The remedy named is
already done, so the advisory is not merely noise: it invites an unnecessary second write to a
file other sessions share.

Observed live 2026-09-12 while closing `BL-75` in `docs/trackers/open-issue-work-queue.md`. The
body's table row was updated in a `doc(action="update", patch={body_edits: […]})` call; the very
next `doc(action="update_entry", …)` returned `snapshot_stale` naming that row.

## Reproduction

1. Pick a params-backed tracker whose body line-anchors a majority of its ids
   (`docs/trackers/open-issue-work-queue.md`).
2. `doc(action="update", patch={body_edits: [...]})` — edit one entry's row in the body table so it
   already carries the new values.
3. `doc(action="update_entry", entry_collection="tasks", entry_id="BL-N", fields={...})` with the
   matching values.
4. The response carries `snapshot_stale` for `BL-N`, asserting the file did not change.
5. `librarian(action="doctor")` reports `snapshot_drift: 0` for the same tracker at the same instant.

## Root cause

**The note's condition is outside its observation boundary, so it substitutes a proxy** — `IC-2`
verbatim. The question it wants to answer is *"does the body's rendered row still show the old
field values?"*, which requires re-rendering `render_template` for that row and comparing prose to
params. What it can cheaply see is the set of ids the body line-anchors. So it branches on presence
and words the result as though it had compared values.

`snapshot_stale_note` in `src/librarian/catalog/augmentation.rs` is explicit that the value case is
the one no id comparison reaches — its own comment on that branch reads *"The hard half: the row IS
in the body, showing its previous values. No id comparison can see this."* **The limitation is
documented and deliberate; the defect is the wording.** A best-effort advisory that cannot observe
its subject should hedge ("may still show") rather than assert a negative fact about the file.

## Evidence

### The branch is keyed on presence, not agreement

`snapshot_stale_note` selects between its two messages with `if in_body.contains(&num)`, where
`in_body` comes from `body_snapshot_row_indices`. No field value is read on either side.

### `doctor` reading 0 is NOT a contradiction, and saying so would be wrong

`scan_snapshot_drift` computes `ledger.claimed.difference(&in_body)` — also a **presence**
predicate. Its `0` means every params id has a body row; it makes no claim about values. The two
checks agree; they simply both stop short of the value question. This section exists because the
contradiction reading is the obvious one and it is false.

## Hypotheses tried

1. **Hypothesis:** the note contradicts `doctor`, so one of them is wrong.
   **Test:** read `scan_snapshot_drift`'s predicate.
   **Verdict:** rejected — both are presence checks over the same `ParamsBackedLedger`, differing
   only in which way round they subtract. Neither compares values, so they cannot disagree.

2. **Hypothesis:** the note is simply undocumented.
   **Test:** read `snapshot_stale_note`'s doc comment and inline comments.
   **Verdict:** rejected — the limitation is stated plainly at the branch. The gap is between what
   the code knows and what the message tells the caller.

## Fix

Not applied. The cheap form is wording: state the uncertainty the code already documents, e.g.
*"its `{entry_id}` row MAY still show the previous field values — this check sees only that the id
is present, not whether the row was re-rendered."* The expensive form is to render the row from
`render_template` and compare, which buys a true verdict at the cost of a template evaluation on
every entry write.

**Prefer the wording fix unless the value comparison is wanted for `doctor` too** — a one-sided
improvement here would leave `doctor` still unable to see value drift, which is the larger gap and
is not what this file reports.

## Tests added

None. The assertion would be over message text, which this project deliberately avoids pinning;
the testable shape is that the presence branch and the absence branch produce *different* hedging,
which is worth little on its own.

## Workarounds

Read `librarian(action="doctor")`'s `snapshot_drift` for the presence question and ignore the
write-time note's certainty. Neither surface answers the value question today.

## Resume

Decide wording-vs-comparison in `snapshot_stale_note`
(`src/librarian/catalog/augmentation.rs`). If comparison is chosen, note that `doctor`'s
`scan_snapshot_drift` and `scan_params_behind_body` share the same `ParamsBackedLedger` and would
want the same treatment, or the surfaces diverge again.

## References

- `src/librarian/catalog/augmentation.rs` — `snapshot_stale_note`, `body_snapshot_row_indices`,
  `body_keeps_snapshot`
- `src/librarian/tools/doctor.rs` — `scan_snapshot_drift`
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`
- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`
  — the bug this advisory was built to close; it closed the id-presence half.

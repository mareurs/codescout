---
id: '5394e9b7bdd83069'
kind: bug
status: fixed
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

Implemented Direction 2 of the plan (own check name) plus the Direction 1 demand at the archive transition, in `src/librarian/tools/doctor.rs`. A new predicate `declares_fix_pair` (`doctor.rs:6865`) is true when at least one `## Fix provenance` pointer carries a non-empty patch-id, or when `no_fix_commit:` is declared. `structured_fix_pointers` and `declares_fix_anchor` are unchanged, as the plan required. `terminal_status_without_fix_anchor` keeps its population; the new state, a SHA declared with no patch-id, is reported under its own check `fix_anchor_missing_patch_id`, whose detail names the consequence (the SHA orphans on the next rebase) and the remedy. `refuse_unanchored_archive` (`doctor.rs:6896`, called from `update.rs:749` and `mv.rs:243`) now demands the pair rather than any pointer, and a SHA with no patch-id is refused with its own hint rather than the "declares no fix anchor" text. Scope of "pair": at least ONE pointer carries a patch-id, so a multi-commit fix in which one SHA lacks its patch-id still passes (follow-up below). The live count of terminal bugs declaring a SHA with no patch-id was 0 at the time of the fix (sweep measurement, not re-measured here), so no existing record was newly redded.

## Tests added

In `src/librarian/tools/doctor.rs`:

- `a_sha_with_no_patch_id_is_reported_under_its_own_check_name` (:11052) pins that SHA-only, same-line-patch-id and empty-patch-id fixtures fire `fix_anchor_missing_patch_id`, while a well-formed pair and a SHA with `no_fix_commit:` stay silent and a record with no pointer keeps the old check name.
- `the_missing_patch_id_finding_names_the_consequence_and_the_remedy` (:11126) pins the finding text ("orphans", "patch-id", "git patch-id --stable", "own bullet").
- `fix_anchor_missing_patch_id_does_not_report_a_row_under_a_sibling_root` (:11158) pins root scoping.
- `terminal_status_without_fix_anchor_does_not_read_a_fenced_pair_as_a_declaration` (:11208) is the fence-escape test the sibling bug `de46d402441e1e2b` recorded as missing.

In `src/librarian/tools/update.rs`: `archiving_a_bug_with_a_sha_and_no_patch_id_is_refused_with_its_own_hint` (:3830). In `src/librarian/tools/mv.rs`: `moving_a_bug_with_a_sha_and_no_patch_id_into_archive_is_refused_and_moves_nothing` (:2814).

## Fix provenance

- **SHA:** `00fe85d6` (`experiments`)
- **patch-id:** `bc936a176841b57e16c11ed2d72933e53c5e8713`

## Workarounds

Write the two bullets. The grammar is stated in `get_guide("tracker-conventions")` § *Bug files*
and in this check's failure text; neither is reached by an author who is getting it half right.

## Resume

Closed on 2026-10-06 by `00fe85d6` (local on `experiments`, not pushed at the time of writing). Residual follow-ups, listed and not filed: (1) `declares_fix_pair` is satisfied by ONE pointer carrying a patch-id, so a multi-commit fix with several SHA bullets where one lacks its patch-id still passes; checking each pointer needs the same live-count migration measurement first. (2) Only the presence of a patch-id is checked, not that it matches the SHA above it.

## References

- `docs/issues/archive/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md` (`de46d402441e1e2b`) — the sibling check, fixed at `496dd63e`, which made this parser shared. That file also records a **second** gap found in the same run and likewise not introduced by it: `terminal_status_without_fix_anchor` has no test covering the fence escape, so a regression in the fence skip would break it silently.
- `CLAUDE.md` § *Bug Tracking* — the SHA-dies / patch-id-survives rule this check is named for.
- **Prior-instance count deliberately not stated.** Derive it: `doc(action="find", kind="bug", include_archived=true, filter={"tags": {"contains": "cluster/guard-narrower-than-its-name"}})`.

---
id: fb147f71da8e15ab
kind: bug
status: fixed
title: 'RESIDUAL: Add a write-time/CI gate refusing a terminal-status or archived bug file that lacks the fix SHA + patch-id pair'
tags:
- cluster/record-asserts-an-unchecked-completion
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a write-time/CI gate refusing a terminal-status or archived bug file that lacks the fix SHA + patch-id pair.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` (status `mitigated`):

> 10 of the 63 archived records were ALREADY unrecoverable when this was mitigated — their objects are gone from the object DB — and no patch-id can restore them. The 53 recoverable ones were back-filled, but nothing gates a FUTURE archive that omits the pair at write time; detection rests on `doctor`'s `terminal_status_without_fix_anchor`, which is run manually. `patch-id` also dies under squash, since a union diff hashes differently.

## Fix

The write-time gate this residual asked for exists for the transition that matters, and was shipped before this residual was filed (2026-09-24 filing; 2026-09-28 gate; 2026-10-05 strictness). Verified at the bytes on 2026-10-06:

- `refuse_unanchored_archive(kind, status, content, surface)` in `src/librarian/tools/doctor.rs` (line ~6896) refuses a `fixed` or `mitigated` bug that does not declare the recoverable pair (a patch-id next to the SHA, or a non-empty `no_fix_commit:`); `wontfix`, open bugs and non-bug kinds are out of scope. It runs on the content AS IT WILL BE after the call, so one `doc(update)` may add the `## Fix provenance` section and archive together. It is called from `doc(update)` when `patch.status == "archived"` (`src/librarian/tools/update.rs` ~748-749) and from `doc(move)` when the file moves into an `archive/` directory from outside one (`src/librarian/tools/mv.rs` ~241-243).
- `declares_fix_anchor` (doctor.rs ~6831, "did the author write anything shaped like a pointer") and `declares_fix_pair` (~6851, "is there a recoverable patch-id") are shared with the doctor checks `terminal_status_without_fix_anchor` and `fix_anchor_missing_patch_id`, so the guard and the check cannot disagree on what "anchored" means.
- Shipped as `04973710` (2026-09-28, "archiving a fixed or mitigated bug is refused until it declares its fix anchor"; doctor.rs, mv.rs, update.rs; eight tests added in `update.rs` and `mv.rs` (I did not scan `doctor.rs` for more), such as `archiving_a_fixed_or_mitigated_bug_with_no_fix_anchor_is_refused_and_writes_nothing` and `moving_an_unanchored_fixed_or_mitigated_bug_into_archive_is_refused_and_moves_nothing`). Documented in `docs/issues/archive/2026-09-28-archived-without-fix-provenance-is-unchecked.md`.
- The first version accepted a SHA with no patch-id, which is the half that survives a rebase, so a lone SHA bullet passed the guard. `00fe85d6` (2026-10-05, "the fix-anchor check no longer accepts a SHA with no patch-id") added `declares_fix_pair`, made both guards demand the pair with their own hint for the SHA-only case, and reports a SHA-without-patch-id record under a separate doctor check name (`fix_anchor_missing_patch_id`); test `a_sha_with_no_patch_id_is_reported_under_its_own_check_name` plus two guard tests for `update` and `move`. That strictness commit is from the sweep that includes this closure.

**Scope, stated so the green is not over-read.** The guard is at the ARCHIVE transition, not at the `status: fixed` write: the test `an_update_that_does_not_archive_is_not_guarded` pins that an update which does not archive passes. A live `fixed`/`mitigated` bug with no pair is still caught only by `doctor`'s checks, which are run manually. That matches the parent's reasoning (the check skips archive paths on purpose; the archive flip is the last moment to enforce the pair).

**Not built, and dropped:** "run doctor in CI". `.github` has no reference to `terminal_status_without_fix_anchor` or `doctor` (grep, 2026-10-06). Enforcement moved to write time, so a CI pass over the same population adds little for the cost.

No code change was made for this record in the closing sweep; the code is the two commits above.

## Tests added

None in this sweep. Added by the commits above: eight in `04973710` across `update.rs` and `mv.rs` (as counted from the diff; `doctor.rs` not scanned) (the refusal, the pass-through when the same update adds the section, `no_fix_commit:` handling, the `wontfix`/open/non-bug exemptions, the non-archive-update exemption, and the move variants), and the SHA-only tests in `00fe85d6` (`archiving_a_bug_with_a_sha_and_no_patch_id_is_refused_with_its_own_hint`, `moving_a_bug_with_a_sha_and_no_patch_id_into_archive_is_refused_and_moves_nothing`, `a_sha_with_no_patch_id_is_reported_under_its_own_check_name`). I did not run them in this sweep.

## Fix provenance

- **SHA:** `04973710` (`experiments`)
- **patch-id:** `a2ca027a70958e42e1137dd0ab4f6d6bc49c560e`
- **SHA:** `00fe85d6` (`experiments`)
- **patch-id:** `bc936a176841b57e16c11ed2d72933e53c5e8713`

## Resume

Closed on 2026-10-06; nothing to resume. Residual follow-ups, listed and not filed: (1) the guard does not fire on a `status: fixed` write with no pair, only at the archive transition; a write-time warning there would close the remaining window before the manual doctor run. (2) The parent's other two limits stand: 10 of 63 archived records were already unrecoverable, and a patch-id dies under squash.

## References

- `docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` — parent

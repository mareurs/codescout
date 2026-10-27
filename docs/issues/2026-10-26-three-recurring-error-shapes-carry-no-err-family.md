---
id: '30c21b0aa5cb249d'
kind: bug
status: fixed
title: 'three large error shapes carry no err_family: oversized-section refusals, fixed-bug-no-anchor refusals and the edit_code shrink guard'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-27
first_fix_patch_id: ccc4e87cb187b9404f075ca2ba6de4cfe7ec2a69
first_fix_sha: 03c125594 (experiments)
fix_patch_id: 9fa67bb3eddc08fa1ff915e2d0f0d909c7422d0b
fix_sha: 3ba135be5 (experiments)
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: three large error shapes carry no `err_family`: oversized-section refusals, fixed-bug-no-anchor refusals and the edit_code shrink guard

## Summary

`ERR_FAMILIES` (`src/usage/db.rs`) has no family for three recurring messages, so 103 of the 289 unclassified rows (2026-09-27..2026-10-26) sit in the `NULL` bucket and `friction-probe.py` cannot tell them apart from genuinely new errors. **Correction 2026-10-26, same day:** the first shape (oversized-section refusals, 48 rows) is already being fixed by an uncommitted change to `src/usage/db.rs` in the shared checkout, not written by the session that filed this bug. It adds the family `read_file_overflow_threshold` and the test `a_read_file_overflow_is_classified_and_stays_tool_scoped`, under `docs/issues/2026-10-24-err-family-overflow-branch-keys-on-retired-read-markdown-so-read-file-errors-are-null.md`. What this bug still owns is the other two shapes: fixed-bug-no-anchor refusals (32 rows) and the `edit_code` shrink guard (23 rows).

## Class fit

The classifier's family list is narrower than the population of error messages it labels: the members it never names stay in an anonymous bucket. That is the selector-narrower-than-its-population trait. It differs from other members because the population grew new tool messages (guards added after the taxonomy was written) rather than shifting outcome values.

## Symptom (Effect)

`err_family IS NULL` reads as "uncovered surface" and grows: 3.6, 5.1 and 8.7 per 1,000 calls over w3 to the last three days. Nothing says which messages it holds.

## Reproduction

`sqlite3 .codescout/usage.db "SELECT … WHERE err_family IS NULL AND outcome IN ('error','recoverable_error')"` grouped by message prefix (run 2026-10-26, window to 19:30 UTC).

## Environment

codescout `experiments`; `usage.db` 2026-09-27..2026-10-26.

## Root cause

The three messages were added after the last taxonomy growth and no family was added.

## Evidence

- `read_file`: `section "…" spans N lines — exceeds inline threshold — hint: use "@file_…"`: 48 rows, 30 sessions.
- `doc(action="move"|"update")` refused because a `fixed` bug declares no fix anchor: 32 rows, 17 sessions.
- `edit_code`: `shrink guard: replacing '<symbol>' would reduce N → M bytes …`: 23 rows, 10 sessions, 16 from subagents. After a shrink-guard refusal the next call was `edit_code` success 7 times (4 with `force=1`).
- Total NULL rows 289 in 74 sessions. The rest are many small shapes (`edit_code` other, `doc` shape errors, `read_file` parameter errors).
- Open question recorded, not decided: whether the oversized-section refusal is friction or the designed progressive-disclosure step. It costs a second call in 30 sessions.

## Hypotheses tried

N/A.

## Fix

Fixed on `experiments` by two commits that also close `docs/issues/2026-10-24-err-family-overflow-branch-keys-on-retired-read-markdown-so-read-file-errors-are-null.md`.

- Oversized-section refusals: `03c125594` (patch-id `ccc4e87cb187b9404f075ca2ba6de4cfe7ec2a69`), family `read_file_overflow_threshold`.
- Fixed-bug-no-anchor refusals and the `edit_code` shrink guard: `3ba135be5` (patch-id `9fa67bb3eddc08fa1ff915e2d0f0d909c7422d0b`), families `unanchored_archive_refused` and `shrink_guard_blocked`. The shrink family also covers the `body-shrink guard:` and `memory-shrink guard:` prefixes, which the source emits from four more tools.

History is re-classified by the fingerprint-versioned backfill. On the live `usage.db` after a rebuild (`codescout_sha=38ec0662e`, `user_version=2061252835`) the families hold 74, 35 and 23 rows. The bug counted 48, 32 and 23 on 2026-10-26, so the populations match and the first two grew.

Not decided here, and not part of the fix: whether the oversized-section refusal is friction or the designed progressive-disclosure step. It still costs a second call.

## Tests added

`a_read_file_overflow_is_classified_and_stays_tool_scoped` and `the_six_guard_messages_the_2026_10_24_sweep_found_unclassified_have_a_family` in `src/usage/db.rs`. Each feeds the real message shapes to `normalize_err_family` and expects its family, as this section proposed, and each carries hostile symbol names or titles that a substring rule would claim. RED was watched for both. The full `./scripts/gate.sh` was green for each commit.

## Workarounds

Classify by message prefix in ad-hoc queries.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G8). A related open bug covers a different NULL head: `docs/issues/2026-10-24-err-family-overflow-branch-keys-on-retired-read-markdown…`. Fix not started.

## References

- `src/usage/db.rs` (`ERR_FAMILIES`, `normalize_err_family`)
- `docs/trackers/friction-deep-dive-2026-10.md`

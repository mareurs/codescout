---
id: '3f0dfd4524d7164f'
kind: bug
status: open
title: Refusals and hints in the librarian write path still carry absolute paths and an artifact id
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-13
severity: low
unverified: Line numbers come from the Plan C1 branch at about 7846817b0 and were read by reviewers, not re-checked on experiments. The race-only sites were not triggered by a test.
---

# BUG: Refusals and hints in the librarian write path still carry absolute paths and an artifact id

## Summary

Several agent-facing refusal texts in `src/librarian` name an absolute path, a link target, the repository root or an artifact id, against the rule that a refusal names a document by its grammar form or file name only. Plan C1 scrubs the paths it adds (`without_path` in `catalog/dual_write.rs`) but left the older texts, which are reachable only in races or rare states, or are unchanged old code.

## Class fit

cluster/unclassified: one rule (no path, hex id or row id in agent-facing text, spec G6) broken at several independent sites.

## Symptom (Effect)

A caller can read an absolute path, a link target or a 16-hex artifact id in a refusal. Sites seen by the Plan C1 reviews (line numbers are from the Plan C1 branch and may have moved):

- the symlink refusal names the link target (`util.rs` `refuse_write_through_symlink`, ~50-73); it reaches a caller through `write_entry_into_ledger` and the restore path;
- the outside-root refusal names the resolved path and the repository root (`util.rs` ~337-364);
- the read-door `verdict` refusal on a restore names `root_real` (`read_door.rs` ~283);
- `allocate_entry_id` texts use `{abs_path}` (`catalog/augmentation.rs` ~1784, ~1802, ~1815) and one params text uses `{path}` (~920);
- the "has no augmentation" refusal puts `artifact_id`, a 16-hex id in production, in its message (`augmentation.rs` ~850).

## Reproduction

The first three need a link or directory swap between a read and the following write, so they are race-only. The `augmentation.rs` texts are reachable by an `append_entry` against a ledger without a declared augmentation, or against an unreadable file. No test asserts their absence.

## Environment

Any session. Found by reading in the Plan C1 task reviews (Tasks 1, 2, 5).

## Root cause

The texts predate the "no path or id in refusals" rule, or embed a lower layer's error text unchanged. Plan C1 added `without_path` for the new code only.

## Evidence

Reviewer reports of Plan C1 Tasks 1, 2 and 5 (session scratch, not committed); the research doc `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md` lists them under "Bugs".

## Hypotheses tried

None.

## Fix

Not made. Direction: map write-door and read-door refusals to fixed wording that names the document by `document_label`, and replace the remaining `{abs_path}` / `{path}` / `{artifact_id}` uses. The read door already refuses to name link targets for this reason (`read_door.rs` ~177-179, R2-9).

## Tests added

None.

## Workarounds

None needed; the leak is information only.

## Resume

Do the sweep as one task in Plan D or E, with a grep gate over `src/librarian` refusal texts.

## References

- docs/superpowers/plans/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars.md

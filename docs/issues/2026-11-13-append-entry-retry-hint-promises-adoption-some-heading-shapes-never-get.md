---
id: '6b48ebbadbf3753d'
kind: bug
status: open
title: The append_entry retry hint promises adoption that some heading spellings never get
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-13
severity: low
unverified: The duplicate for the three shapes is inferred from reading entry_sections, def_re and heading_number; no test writes one.
---

# BUG: The append_entry retry hint promises adoption that some heading spellings never get

## Summary

After a partial write, the `RetryRule::Adopts` remedy tells the caller to retry with the same title, body and cites, and says the retry finds the section in the file and records it. `find_unrecorded_section` adopts only the server's own heading spelling, so three shapes of orphan section are not adopted and the retry writes a second section.

## Class fit

cluster/unclassified: a remedy text broader than the code it describes.

## Symptom (Effect)

A retry writes a duplicate entry instead of adopting the orphan when:

- the heading uses a hand-written `-` or `–` where the server writes the em dash (`link_scan::extract::def_re` accepts all three; `heading_number` in `catalog/dual_write.rs` accepts only the em dash);
- the caller's body contains a heading at or above the entry's level, so `entry_sections` bounds the section differently from what the splice wrote;
- the `anchor_heading` is deeper than the entry level, so it falls inside the section text.

## Reproduction

Not run. Stated by the Plan C1 Task 2 and its re-review from reading `entry_sections`, `def_re` and `heading_number`; the doc of `find_unrecorded_section` lists the three cases.

## Environment

The params path (`RetryRule::Adopts`); the prose path uses delete-first wording.

## Root cause

Adoption compares the section text the server would have written with the file's section; any shape the splice does not produce the same way is not matched. The remedy text does not name these exceptions.

## Evidence

Plan C1 Task 2 fix round 1 re-review (commit b950b627c); the doc comment of `find_unrecorded_section` in `src/librarian/catalog/dual_write.rs`.

## Hypotheses tried

None.

## Fix

Not made. Options: widen `heading_number` to the three dashes, or narrow the remedy text to say adoption works for a section the server wrote with its own heading and a body without top-level headings. No test shows a duplicate actually written for the three shapes.

## Tests added

None for the three shapes.

## Workarounds

Check the ledger for an orphan section before retrying after a failed append.

## Resume

Settle with Plan D, which reuses the dual-write library.

## References

- docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md

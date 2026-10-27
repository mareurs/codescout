---
id: 09102de754efd8ae
kind: bug
status: fixed
title: the edit_file batch path answers "old_string not found" and "found N times" with no path, nearest content or line numbers
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-27
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: the edit_file batch path answers "old_string not found" and "found N times" with no path, no nearest content and no line numbers

## Summary

In batch mode `edit_file` (`src/tools/edit_file/mod.rs:651-661`, read 2026-10-26) answers a miss with `edit[i]: old_string not found` and the hint `Batch aborted — no changes written.`, and an ambiguous match with `edit[i]: old_string found N times` and `Add more context or set replace_all: true. Batch aborted.` The single-edit path (`not_found_msg`, `mod.rs:276`) answers the same miss with `old_string not found in <path>. Nearest content at lines s-e:` and the matching text.

## Class fit

The batch error is composed without the request: the failing edit's path and the nearest text are known and omitted, while the single path carries them. That is the hint-composed-without-the-request trait. It differs from the class's other members because the richer message already exists in the sibling code path.

## Symptom (Effect)

An agent whose batch fails cannot see why, so it resends the whole batch, usually with changed text.

## Reproduction

Source read; not executed. A batch whose `edits[1].old_string` differs from the file by indentation returns the bare message.

## Environment

codescout `experiments` at `8df90f1b9`.

## Root cause

The batch loop reports only the edit index.

## Evidence

- `usage.db` `edit_stale_match` 128 errors in 37 sessions; subagent count: 63 are `edit_file` batch failures, 45 of those 63 failing `old_string`s start with whitespace, and 27 retries succeeded after a whitespace-only change (subagent figures, not rechecked).
- A same-path write by another principal in the prior 30 minutes preceded only 2 of 155 edit failures, so staleness from concurrent editing is not the cause (subagent check, not rechecked).

## Hypotheses tried

N/A.

## Fix

Fixed in `src/tools/edit_file/mod.rs`. The batch loop now answers a miss with `edit[i]: ` plus `not_found_msg` (the file path and the nearest content), and an ambiguous edit with its match lines. A new helper `match_lines_csv` computes the lines for both the single-edit and the batch path, so they cannot disagree. A new helper `batch_lines_note` adds, for `i > 0`, the clause that line numbers count the content after the earlier edits, because the batch edits an in-memory copy. The substrings `old_string not found` and `old_string found ` that `src/usage/db.rs` classifies on are unchanged, so error families do not move.

## Fix provenance

- **SHA:** `6441894a1` (`experiments`)
- **patch-id:** `0d8c6f8fef392e71aeb455592d0c0ff552f39a1c`

## Tests added

In `src/tools/edit_file/tests.rs`, all seen failing before the fix and passing after:

- `a_batch_miss_names_the_file_and_the_nearest_content`
- `a_batch_ambiguity_lists_the_match_lines`
- `a_later_batch_miss_says_its_lines_follow_the_earlier_edits`

Not tested: the batch path still does not apply a whitespace-normalized match the way the single-edit path does. This fix only improves the error text.

## Workarounds

Send the failing edit alone, to get the single-edit message.

## Resume

N/A — fixed.

## References

- `src/tools/edit_file/mod.rs`

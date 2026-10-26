---
id: c04d2f540b83ea9a
kind: bug
status: open
title: the edit_file batch path answers "old_string not found" and "found N times" with no path, nearest content or line numbers
tags:
- cluster/hint-composed-without-the-request
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

Open. Reuse `not_found_msg` in the batch path with the edit index, and list match lines in the ambiguous case.

## Tests added

None. A test would assert a batch miss contains `Nearest content at lines`, and a batch ambiguity contains the line numbers.

## Workarounds

Send the failing edit alone, to get the single-edit message.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G2). Fix not started.

## References

- `src/tools/edit_file/mod.rs`

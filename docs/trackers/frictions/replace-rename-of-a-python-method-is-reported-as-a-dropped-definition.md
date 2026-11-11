---
kind: tracker
status: active
title: "Evidence — a rename of a Python method inside replace is reported as a dropped definition"
tags:
  - frictions
  - edit_code
  - replace
---

# Evidence — a rename of a Python method inside `replace` is reported as a dropped definition

Ledger entry: FR-13 in [index.md](index.md). Window: 2026-10-17 to 2026-11-09 UTC. Corpus: `.codescout/usage.db`. Source: the `replace` miner; the verdict function was read by the lead on 2026-11-11.

## Scale

`edit_code`, action `replace`, `err_family='replace_dropped_sibling'`: 30 rows, 12 sessions. The `replace` family overall: 1,779 calls, 108 rows in the four refusal families (6.1%).

## The two confirmed rows

- Row 223327 (2026-10-28): symbol `IntegrityTest/test_errored_run_is_invalid`. The body declares `def test_errored_run_with_no_model_work_is_invalid`, a complete method under a new name. The message says the edit "dropped the symbol definition … not just body statements".
- Row 232089 (2026-10-29): the same shape (`test_zero_codescout_calls_is_a_result_not_an_invalid_arm`).

The same cause on Rust gets the correct message. Row 248940: "given a complete declaration, but it declares … instead of … `replace` cannot rename". That message points at `edit_code(action="rename")` and agents follow it (FR-ledger "Healthy": 21 of 41 renamed, then replaced, within 3 calls).

## Recovery (3 sampled rows: 223327, 232089, 273661)

0 recovered by a `replace` on the same symbol. 2 went to `edit_code` remove then insert (row 223328 removed the test), 1 went to `edit_file`.

## The code (verified)

`corruption_verdict` in `src/symbol/edit.rs`, lines 547 to 616. When the target name path has vanished, it returns `TargetRenamed(name)` only if `pre_set` is present, nothing else was lost, and exactly one new name appeared. In every other case it returns `TargetDropped`: `pre_set` is `None`, or two or more names appeared, or a sibling is also gone. The comment says a wider shape "must keep falling through to the blunter verdicts".

## The mechanism (hypothesis, not reproduced)

For the two Python rows one of those conditions held. The candidates are a `pre_set` that is `None` for Python, or a replacement body that makes more than one new name (a nested function or an attribute that the symbol extraction lists). Nothing in this pass distinguishes them.

## Known

`docs/issues/archive/2026-09-13-edit-code-replace-with-a-renamed-body-misattributes-the-error.md` (archived, fixed in `cacbae14` for the rename message at `src/tools/symbol/edit_code.rs:1495`). The Python rows are after that date.

## Fix sketch and test

Reproduce first: a Python class with two methods, `replace` one with a body that declares a new name. If it returns "dropped", find which of the three conditions held. A test that goes red: the same call must return the rename message.

## Not verified

- Why the Python case falls through.
- Recovery beyond 3 sampled rows.

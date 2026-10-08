---
id: 53bac996d683f94a
kind: bug
status: fixed
title: Shrink report panics when one size dimension grows while the other shrinks
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-08
opened: 2026-10-08
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-edit-code-and-create-file-shrink-refusal.md
severity: medium
---

# BUG: Shrink report panics when the other dimension grows

## Summary

The shared shrink predicate correctly accepts either byte or line reduction as grounds for refusal, but its report subtracts both new-size percentages from unsigned 100. Growth in the other dimension panics in debug builds.

## Symptom (Effect)

```
thread 'tools::edit_file::tests::create_file_refuses_overwrite_shrink_without_force' panicked at src/util/shrink_guard.rs:150:19:
attempt to subtract with overflow
```

## Reproduction

Baseline: experiments `21237e7f2734d8757c6b9666e2f79a9cfd6a318c`. While adding the create-file overwrite guard, run `check(&"abcdefghijk\n".repeat(20), &"y".repeat(250))`: 240 bytes / 20 lines become 250 bytes / 1 line. The line arm triggers and `byte_pct` underflows. The opposite case can underflow `line_pct`.

The new `create_file_refuses_overwrite_shrink_without_force` test observed the panic before the helper fix.

## Environment

Linux, debug Cargo tests, isolated worktree `/tmp/codescout-write-safety-oct08`, 2026-10-08.

## Root Cause

`byte_pct = 100 - new_bytes * 100 / old_bytes` and the corresponding line expression assume both dimensions decrease, whereas the predicate requires only one.

## Fix

The byte and line loss calculations now use saturating subtraction, so a growing dimension reports zero loss while the genuinely shrinking dimension still triggers refusal. Thresholds and size operands are unchanged. Verified on experiments with the full gate: `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

**SHA (experiments):** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34`.

**patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`.

## Tests added

`reports_zero_loss_when_the_other_dimension_grows` exercises both byte-only shrink with growing line count and line-only shrink with growing byte count. It asserts the operands, selected dimension, and exact loss percentages, including zero for the growing dimension. The create-file refusal regression additionally proves that line-only shrink preserves the original disk bytes.

Review first applied unsafe line subtraction and observed it survive all 15 then-existing helper tests. After adding this regression, the same mutation was killed: 15 passed and one failed. This closes the observed hole rather than inferring coverage from the sibling case.

## Resume

N/A — verified with a regression that kills the previously surviving unsafe-subtraction mutation.

## Fix provenance

- **SHA:** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34` (`experiments`)
- **patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`

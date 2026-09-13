---
id: '83b1a79f605582b3'
kind: bug
status: investigating
title: Architecture probe test-field masking corrupts production structure
owners:
- marius
tags:
- architecture
- measurement
- cluster/selector-narrower-than-its-population
opened: 2026-09-13
owner: marius
severity: medium
---

# BUG: Architecture probe test-field masking corrupts production structure

## Summary

The architecture probe assumes a test attribute is followed by an item ending at a semicolon or matched brace. A cfg(test) struct field or initializer field ends at a comma instead; masking can consume neighboring production syntax.

## Symptom (Effect)

Full probe run on 2026-09-13 stopped with `error: src/agent/mod.rs (function index): function 'new' at line 471: unmatched '{' at byte 21822`.

## Reproduction

Run `python3 tests/test_architecture_boundary_probe.py ContextControls.test_test_only_field_does_not_erase_constructor_delimiters` against the pre-fix probe. The fixture includes test-only struct and initializer fields followed by a live field and a live context-reading function. The new constructor disappeared from the function index (IndexError on its empty body list).

## Environment

Python 3; codescout experiments shared working tree. Measurement was from git-archived source; the instrument itself was an untracked script being validated.

## Root cause

`scripts/architecture-boundary-probe.py:production_mask` uses next-brace/semicolon termination for every test attribute. Test-only comma-delimited fields are not covered. Measured by the full-run failure and isolated production-function regression before the fix.

## Evidence

Background run `@bg_00000018` stopped before publishing a baseline. The named regression also failed before implementation.

## Hypotheses tried

Ordinary constructor parsing is broken / inspect Agent::new / actual source is balanced. Test-field removal consumes neighboring syntax / isolated fixture / confirmed.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: production_mask now masks test-only fields through the outer comma, preserving production struct/constructor delimiters.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

No fix commit has been made. SHA and patch-id are therefore not available; this record is not archived. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: ContextControls.test_test_only_field_does_not_erase_constructor_delimiters. Disabling the field branch in an isolated copy produced the expected regression error.
## Workarounds

Do not skip the failing source file to obtain a nominally complete context baseline.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.
## References

- `scripts/architecture-boundary-probe.py`
- `src/agent/mod.rs`
- `docs/trackers/architecture-boundary-measurement.md`

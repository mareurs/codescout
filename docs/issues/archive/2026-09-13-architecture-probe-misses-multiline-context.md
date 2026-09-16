---
id: 360ba099bdb3c1e5
kind: bug
status: fixed
title: Architecture probe misses multiline context access
owners:
- marius
tags:
- architecture
- measurement
- cluster/selector-narrower-than-its-population
closed: 2026-09-16
opened: 2026-09-13
owner: marius
severity: medium
---

# BUG: Architecture probe misses multiline context access

## Summary

reachable_context uses regexes requiring ctx.agent and alias.method without whitespace around the dot. Rust formatting permits line breaks at those positions.

## Symptom (Effect)

Measured 2026-09-13 before changing production code:

```text
test_multiline_access_and_alias_method_are_observed failed: context_fields was [] instead of ['agent', 'lsp'].
```

## Reproduction

Run `python3 tests/test_architecture_boundary_probe.py` against the pre-fix probe. The named test invokes the production measurement function with a controlled fixture.

## Environment

Python 3; shared codescout experiments working tree. The probe and regression suite were untracked during reproduction; a HEAD SHA alone does not identify these bytes.

## Root cause

reachable_context uses regexes requiring ctx.agent and alias.method without whitespace around the dot. Rust formatting permits line breaks at those positions. Reproduced by the named test, not inferred only from reading.

## Evidence

The test output above is from the initial nine-test run: four assertion failures and one missing-field error. Only the named assertion establishes this defect; the other outcomes concern separate changes.

## Hypotheses tried

The predicate handles the fixture correctly / run the production function / rejected by the observed assertion failure.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: Context field and Agent alias matching now accepts whitespace and newlines around member access; direct fields are reported separately from helper-expanded reads.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

**Fixed in `d3a2c24f`** — patch-id `f7ee24322b07702e7e87e5f4e17d78422089f2d1`. That commit introduced the corrected probe and its control suite in one change, which is why the fix and its regression tests share a SHA. **Re-verified 2026-09-16 at current HEAD: 14/14 regression tests pass, `self-test: ok`** — re-run rather than cited, because `40fb2843` later touched the probe script; inspected, and it only de-hardcodes the `--runtime-binary` default, touching none of this defect's code. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: ContextControls.test_multiline_access_and_alias_method_are_observed; test_direct_and_delegated_reads_are_separate.
## Workarounds

Do not use this output as a complete architectural measurement until this case passes.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.
## References

- `scripts/architecture-boundary-probe.py`
- `docs/trackers/architecture-boundary-measurement.md`

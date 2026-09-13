---
id: dd4a2785969615c9
kind: bug
status: investigating
title: Architecture probe registration control accepts unrelated Arc construction
owners:
- marius
tags:
- architecture
- measurement
- cluster/assertion-satisfiable-by-accident
opened: 2026-09-13
owner: marius
severity: medium
---

# BUG: Architecture probe registration control accepts unrelated Arc construction

## Summary

static_measurement treats any capitalized Arc::new argument in src/server.rs as a registered tool, without reconciling it to a tool import.

## Symptom (Effect)

Measured 2026-09-13 before changing production code:

```text
test_unrelated_arc_is_not_a_registration_control failed: ProbeError was not raised for a source containing only Arc::new(UnrelatedState).
```

## Reproduction

Run `python3 tests/test_architecture_boundary_probe.py` against the pre-fix probe. The named test invokes the production measurement function with a controlled fixture.

## Environment

Python 3; shared codescout experiments working tree. The probe and regression suite were untracked during reproduction; a HEAD SHA alone does not identify these bytes.

## Root cause

static_measurement treats any capitalized Arc::new argument in src/server.rs as a registered tool, without reconciling it to a tool import. Reproduced by the named test, not inferred only from reading.

## Evidence

The test output above is from the initial nine-test run: four assertion failures and one missing-field error. Only the named assertion establishes this defect; the other outcomes concern separate changes.

## Hypotheses tried

The predicate handles the fixture correctly / run the production function / rejected by the observed assertion failure.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: The source control now requires constructed types imported from crate::tools, rejecting unrelated Arc constructions. It remains a positive control, not a complete live-registration counter.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

No fix commit has been made. SHA and patch-id are therefore not available; this record is not archived. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: StaticControls.test_unrelated_arc_is_not_a_registration_control; StaticControls.measure supplies an actual tools import/construction as the positive control.
## Workarounds

Do not use this output as a complete architectural measurement until this case passes.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.
## References

- `scripts/architecture-boundary-probe.py`
- `docs/trackers/architecture-boundary-measurement.md`

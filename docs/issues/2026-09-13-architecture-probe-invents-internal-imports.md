---
id: '0094594dea9a52b7'
kind: bug
status: investigating
title: Architecture probe invents internal paths for external imports
owners:
- marius
tags:
- architecture
- measurement
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-13
severity: medium
---

# BUG: Architecture probe invents internal paths for external imports

## Summary

The uncommitted architecture probe can produce incorrect raw dependency references. Do not accept static totals from baseline-initial.json or baseline-actions.json until corrected.

## Symptom (Effect)

Observed production-function output: `crate::ast::anyhow::Result`.

## Reproduction

At source HEAD 23adef79023ebc43ea30d8d8e50a2175bacab5aa on experiments, with the untracked worktree probe:

```python
import runpy
p = runpy.run_path("scripts/architecture-boundary-probe.py")
print(p["resolve_relative"]("anyhow::Result", "src/ast/mod.rs"))
```

## Environment

Linux, Python 3; codescout shared checkout. The Python probe is not committed at that HEAD.

## Root cause

resolve_relative qualifies any bare import except std/tokio against the current source module. It treats external crates and unresolved bare roots as proved local modules.

Measured 2026-09-13 by calling the production function via runpy; source read through symbols in scripts/architecture-boundary-probe.py. Classification left unclassified rather than forcing a defect-class match.

## Evidence

The reproduction above was executed before planning a fix. Raw architecture baseline artifacts are in .codescout/measurements/architecture-boundary/2026-09-13/; see docs/trackers/architecture-boundary-measurement.md.

## Hypotheses tried

Normalization changes reference identity: confirmed by the production-function reproduction.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: resolve_relative only resolves explicitly self/super-qualified paths; bare roots remain unproven rather than being fabricated as local dependencies.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

No fix commit has been made. SHA and patch-id are therefore not available; this record is not archived. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: StaticControls.test_external_imports_do_not_invent_local_edges observed a wrong edge before the fix and passes afterward, with explicit self/super controls.
## Workarounds

Withhold affected static totals; inspect raw evidence rather than accepting the aggregate.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.

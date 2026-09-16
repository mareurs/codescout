---
id: 5b4a43dea0277aa5
kind: bug
status: fixed
title: Architecture probe strips alias-like suffixes inside identifiers
owners:
- marius
tags:
- architecture
- measurement
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-16
opened: 2026-09-13
severity: medium
---

# BUG: Architecture probe strips alias-like suffixes inside identifiers

## Summary

The uncommitted architecture probe can produce incorrect raw dependency references. Do not accept static totals from baseline-initial.json or baseline-actions.json until corrected.

## Symptom (Effect)

Observed production-function output: `['crate::lsp::b']`.

## Reproduction

At source HEAD 23adef79023ebc43ea30d8d8e50a2175bacab5aa on experiments, with the untracked worktree probe:

```python
import runpy
p = runpy.run_path("scripts/architecture-boundary-probe.py")
print(p["expand_use_tree"]("crate::lsp::base"))
```

## Environment

Linux, Python 3; codescout shared checkout. The Python probe is not committed at that HEAD.

## Root cause

expand_use_tree removes whitespace before applying as[A-Za-z_]\\w*$, so the letters as inside base are interpreted as alias syntax.

Measured 2026-09-13 by calling the production function via runpy; source read through symbols in scripts/architecture-boundary-probe.py. Classification left unclassified rather than forcing a defect-class match.

## Evidence

The reproduction above was executed before planning a fix. Raw architecture baseline artifacts are in .codescout/measurements/architecture-boundary/2026-09-13/; see docs/trackers/architecture-boundary-measurement.md.

## Hypotheses tried

Normalization changes reference identity: confirmed by the production-function reproduction.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: expand_use_tree removes whitespace-delimited alias declarations before removing whitespace, preserving identifiers containing as.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

**Fixed in `d3a2c24f`** — patch-id `f7ee24322b07702e7e87e5f4e17d78422089f2d1`. That commit introduced the corrected probe and its control suite in one change, which is why the fix and its regression tests share a SHA. **Re-verified 2026-09-16 at current HEAD: 14/14 regression tests pass, `self-test: ok`** — re-run rather than cited, because `40fb2843` later touched the probe script; inspected, and it only de-hardcodes the `--runtime-binary` default, touching none of this defect's code. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: StaticControls.test_alias_removal_preserves_identifier_bytes observed base/Task/Alias mangling before the fix and passes afterward.
## Workarounds

Withhold affected static totals; inspect raw evidence rather than accepting the aggregate.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.

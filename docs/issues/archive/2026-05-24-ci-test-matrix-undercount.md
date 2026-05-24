---
status: fixed
opened: 2026-05-24
closed: 2026-05-24
severity: medium
owner: marius
related: [docs/issues/2026-05-24-tool-docs-manual-drift.md, docs/issues/2026-05-24-symbols-auto-inline-test-contract-drift.md]
tags: [ci, matrix, github-actions, dormant-since-2026-04-13]
kind: bug
---

# BUG: CI Test matrix only runs 3 of 9 expected job combinations

## Summary

`.github/workflows/ci.yml` Test matrix is defined with `os: [3 values]`
and `include: [3 entries with name+flags]`. Expected: 3×3=9 jobs. Actual:
3 jobs, all with `name=no-features` (the LAST include entry). Default
and local-embed feature configurations have therefore not been
test-verified in CI — ever. Surfaced when CI started firing again on
2026-05-24 after 6 weeks dormant. Pre-existing.

## Symptom (Effect)

`gh api repos/.../actions/runs/<run_id>/jobs` returns 8 jobs total when
~14 expected (5 non-Test jobs + 9 Test matrix entries). The 3 Test jobs
returned are all `name=no-features` across `[ubuntu-latest,
macos-latest, windows-latest]`.

## Reproduction

```bash
git rev-parse HEAD
# any commit on experiments since 2026-05-24

# Push to experiments or open a PR — observe only 3 Test jobs ever run.
gh api repos/mareurs/codescout/actions/runs/<latest_id>/jobs \
  --jq '.jobs[].name' | sort | uniq -c
# Returns 3 Test entries, all (... / no-features)
```

## Environment

- GitHub Actions
- Workflow: `.github/workflows/ci.yml`
- Affects: every CI run since the workflow was authored on 2026-02-26.

## Root cause

The YAML matrix definition:

```yaml
strategy:
  fail-fast: false
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    include:
      - name: default
        flags: ""
      - name: local-embed
        flags: "--features local-embed --no-default-features"
      - name: no-features
        flags: "--no-default-features"
```

GitHub Actions matrix expansion semantics when `include` entries share
NO keys with the base matrix is empirically producing 3 jobs (one per
os, each picking up the *last* include entry's values). The intended
3×3=9 expansion does not happen with this YAML shape.

## Evidence

CI run 26355932027 (2026-05-24): 8 jobs total, including 3 Test
combinations all with `name=no-features`. Every prior run shows the
same shape.

## Hypotheses tried

N/A — root cause established from empirical run data.

## Fix

Restructure the matrix using explicit cross-product. Option A:

```yaml
strategy:
  fail-fast: false
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    config:
      - name: default
        flags: ""
      - name: local-embed
        flags: "--features local-embed --no-default-features"
      - name: no-features
        flags: "--no-default-features"
```

Then reference `matrix.config.name` and `matrix.config.flags`. The
matrix expansion 3 os × 3 config = 9 jobs is unambiguous when both
dimensions are top-level matrix keys (not `include`).

## Tests added

The matrix expansion itself is the test. After the fix, push to
experiments and verify 9 Test jobs appear in CI.

## Workarounds

None — silently testing only 1 of 3 feature configurations is the
current behavior. Manual verification via `cargo test --features X`
locally fills the gap until the fix lands.

## Resume

1. Edit `.github/workflows/ci.yml` Test job matrix per Option A above.
2. Update `runs-on` / `with.key` references to use `matrix.config.name`.
3. Push, observe 9 Test jobs.

## References

- `.github/workflows/ci.yml` Test job.
- Sibling rot exposed by the same CI restart:
  - `docs/issues/2026-05-24-tool-docs-manual-drift.md`
  - `docs/issues/2026-05-24-symbols-auto-inline-test-contract-drift.md`
- F-11 in `docs/trackers/bug-fix-session-log.md` (mold linker rot).

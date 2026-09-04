---
id: '633ded4a1dfa2cf8'
kind: bug
status: open
title: 'BUG: scripts/probe-chunk-drift-by-root.py hardcodes a personal home path, recurrence of the sweep-scripts pattern'
owners:
- marius
tags:
- scripts
- machine-specific
- cluster/config-propagation-is-additive
closed: ''
opened: 2026-09-04
owner: marius
severity: low
---

## Summary

`scripts/probe-chunk-drift-by-root.py:20` hardcodes `CODESCOUT = "/home/marius/work/claude/codescout"` as a committed default. Same defect class already fixed once in this repo (`docs/issues/archive/2026-08-14-sweep-scripts-hardcode-dead-machine-specific-paths.md`), which is exactly why `tests/committed_paths.rs::no_committed_script_hardcodes_a_personal_home_path` exists — this script landed (in the 813-commit pull reconciled this session) after that gate was written, and trips it.

## Symptom (Effect)

`cargo test --workspace` fails:

```
committed scripts hardcode machine-specific home paths:
  scripts/probe-chunk-drift-by-root.py:20 — /home/marius
```

## Reproduction

`cargo test --test committed_paths no_committed_script_hardcodes_a_personal_home_path`

## Environment

codescout repo, `experiments` branch at `087cabfb` and after. Discovered while running the project gate during an unrelated `git pull` reconciliation (`docs/trackers/observer-blindness.md`/`reconnaissance-patterns.md`, commit `331ca5a1`) — this bug is unrelated to that work.

## Root cause

Same as the archived sibling bug: a machine-specific convenience default committed directly rather than derived from the script's own location or an environment variable.

## Fix

Not yet applied — same shape as the archived fix: derive the repo root, e.g. `CODESCOUT = str(Path(__file__).resolve().parent.parent)`.

## Tests added

None yet — `tests/committed_paths.rs` already covers this class; no new test needed once the line is fixed, the existing gate will hold it.

## Resume

Open. One-line fix, same pattern as `86d0794657b1ab62`.

## References

- `scripts/probe-chunk-drift-by-root.py`
- `docs/issues/archive/2026-08-14-sweep-scripts-hardcode-dead-machine-specific-paths.md` — the sibling instance and the gate's origin
- `tests/committed_paths.rs`


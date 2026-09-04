---
id: 0f957214c8ad9164
kind: bug
status: fixed
title: 'BUG: scripts/probe-chunk-drift-by-root.py hardcodes a personal home path, recurrence of the sweep-scripts pattern'
owners:
- marius
tags:
- scripts
- machine-specific
- cluster/config-propagation-is-additive
closed: 2026-09-04
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

Fixed at `13679cd8` (patch-id `ba68dcd2ffcd5ff034710231eb51f96ceb23f868`, experiments). `CODESCOUT` now derives from the script's own location: `os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))`, matching how the script is used elsewhere (as a plain string, not a `Path`).

## Tests added

None yet — `tests/committed_paths.rs` already covers this class; no new test needed once the line is fixed, the existing gate will hold it.

## Resume

Closed. `cargo test --test committed_paths` green; verified against the full workspace gate (fmt/clippy/both test lanes) at `13679cd8`.

## References

- `scripts/probe-chunk-drift-by-root.py`
- `docs/issues/archive/2026-08-14-sweep-scripts-hardcode-dead-machine-specific-paths.md` — the sibling instance and the gate's origin
- `tests/committed_paths.rs`

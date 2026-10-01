---
id: 8c067bb1dd5853ac
kind: bug
status: archived
title: 'Codex: System1 judge gate relaunch overwrites vote evidence'
owners:
- codex
tags:
- system1
- judge
- cluster/guard-narrower-than-its-name
closed: 2026-09-29
opened: 2026-09-29
severity: medium
---

## Summary

`run_gate` validates the location of `log_dir`, but does not reserve it or reject earlier run evidence. `CodexChannel.complete` writes each deterministic vote filename with `write_text`. Relaunching the same gate spends again and replaces its prior logs.

## Reproduction

Codex review, 2026-09-29: execute the real `run_gate` twice on `_fixture_gate_repo`, with a fake `complete` that writes its generation into the supplied log path. First call: 15 fake calls. Second call: 15 additional fake calls, 15 existing vote logs overwritten. No model calls. Evidence: `docs/evals/data/2026-09-27-system1-base-rates/codex-review-2026-09-29/relaunch-before.json`.

## Impact

Accidental relaunch loses first-run evidence and repeats model spend. A failed run also leaves no durable reservation to prevent its partial evidence being overwritten. No evidence was found that the reported 243-call campaign was relaunched; its stored first-try records remain intact.

## Fix and verification

Fixed in commit `fa034d42` on `experiments`, patch-id `e4359491931dc7b0608ae1895a78d4d1686463f0`. It was integrated 2026-09-29 by the SDD controller, session 3c5b02df-b6ce-45f5-9d03-1194e38465c0, after all nine measure suites passed with a codex stub, including the new reservation and run-output tests. `run_gate` now reserves a fresh log directory before constructing the channel; an exclusive marker handles racing contenders and survives failed attempts. `run.py::_gate` also exclusively reserves distinct text/JSON destinations before any model calls, preventing overwrite and late discovery of unusable destinations. Dry runs do not reserve live log directories, but their report destinations must also be fresh.

Verification: 66 judge tests and 5 CLI output tests pass; six applied in-memory mutations produce six assertion failures, zero errors and zero survivors. See `docs/research/2026-09-29-codex-system1-judge-review.md` and its counts-only evidence. Full Rust gate not rerun for these Python-only changes.

Scope: per-directory/per-destination protection, not a global ban on deliberately naming a fresh attempt. Mid-write filesystem failures and durable archive policy remain separate concerns.

## Fix provenance

- **SHA:** `fa034d42a97a782f77ae1ec1b46de66c15c7efb9` (`experiments`)
- **patch-id:** `e4359491931dc7b0608ae1895a78d4d1686463f0`

## References

- `scripts/measure/judge.py`
- `tests/test_measure_judge.py`
- Earlier related runner defect: `docs/issues/2026-09-26-codex-labeller-run-overwrite.md`.

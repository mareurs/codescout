---
id: ec8adf891fb84d8c
kind: bug
status: taken
title: Offline Python test collection constructs a live subscription judge
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- albert-follow-up
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
opened: 2026-10-14
owner: marius
severity: medium
---

# BUG: Offline Python test collection constructs a live subscription judge

## Summary

Importing the scorer creates SubscriptionJudge and checks credentials immediately. A test's global dummy profile/environment masks the failure when files collect in a favorable order, while an empty-home isolated collection exits.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

Importing the scorer creates SubscriptionJudge and checks credentials immediately. A test's global dummy profile/environment masks the failure when files collect in a favorable order, while an empty-home isolated collection exits.

## Evidence

Clean-home collection @cmd_3a38d4fb exited3 from SystemExit in SubscriptionJudge.__init__. New tests @bg_0000003f observed three failing test functions and two failing subcases; after the fix the combined targeted run @bg_00000040 passed15 tests and3 subtests. No real credentials or model calls were supplied.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Keep inert constructor metadata and authenticate at complete() before launching any client. Scope the test profile to Scorer setup/teardown; remove module-global JUDGE_CONFIG_DIR mutation.

## Tests added

tests/test_offline_python_collection.py: pure guarded imports, independent clean-home collection, live missing-auth refusal, and authenticated mocked channel isolation.

## Workarounds

Use a controlled coherent trial and the follow-up recovery guide; keep all writers stopped.

## Resume

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

---
id: 049c568841156ce1
kind: bug
status: fixed
title: Offline Python test collection constructs a live subscription judge
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- albert-follow-up
closed: 2026-10-14
opened: 2026-10-14
owner: marius
severity: medium
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
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

Fixed and archived after source `c773a2ca5b5225010c06b5784a13470319bc4617` landed on experiments and the final ordered gate passed. See [the implementation evidence and rulings](../../superpowers/plans/2026-10-14-albert-recovery-hardening.md). Operator native/corpus acceptance is tracked separately.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

## Fix provenance

- **SHA:** `c773a2ca5b5225010c06b5784a13470319bc4617` (`experiments`)
- **patch-id:** `1e6a43618eb28337de242fcfb31271132a9b0354`

Verified final ordered gate: FMT0 CLIPPY0 LEAN0 PYTHON_LIGHT0 DEFAULT0, with normal temporary paths and eight test threads. SQLite27pass, ordinary CLI4pass and genuine two-record MiniLM CLI1pass; credential-free Python light1223pass/424subtests. Both review coverage gaps have observed mutation RED and restored GREEN.

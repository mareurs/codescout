---
id: fd3062033f44aa98
kind: bug
status: fixed
title: Synthetic evaluation imports scorer code from a hardcoded main checkout
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

# BUG: Synthetic evaluation imports scorer code from a hardcoded main checkout

## Summary

stage2 mine_pairs uses a literal machine checkout as ROOT, so a worktree's generator imports the main selector/scorer rather than the code being tested. This retained the credential-dependent old implementation during worktree probes.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

stage2 mine_pairs uses a literal machine checkout as ROOT, so a worktree's generator imports the main selector/scorer rather than the code being tested. This retained the credential-dependent old implementation during worktree probes.

## Evidence

Read-only audit observed mine_pairs.ROOT = pathlib.Path('/home/marius/work/claude/codescout') and audited imported filenames outside the worktree. A new subprocess regression verifies both actual selector/scorer __file__ paths inside the current checkout.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Derive ROOT from HERE.parents[4], matching the neighboring generator's live layout.

## Tests added

test_pure_imports_and_model_pins_need_no_credentials_or_client_calls asserts imported worktree paths.

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

---
id: 2e6481c470fee2bf
kind: bug
status: taken
title: Synthetic evaluation imports scorer code from a hardcoded main checkout
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

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

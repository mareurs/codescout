---
id: '239252057ac4af21'
kind: bug
status: taken
title: SQLite memory rebuild dry-run accepts WAL despite its strict no-file-change contract
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
opened: 2026-10-14
owner: marius
severity: medium
---

# BUG: SQLite memory rebuild dry-run accepts WAL despite its strict no-file-change contract

## Summary

SQLITE_OPEN_READ_ONLY constrains database SQL writes but does not establish that WAL shared-memory files remain untouched. The existing preview opens WAL databases while the recovery guide promises no source write.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

SQLITE_OPEN_READ_ONLY constrains database SQL writes but does not establish that WAL shared-memory files remain untouched. The existing preview opens WAL databases while the recovery guide promises no source write.

## Evidence

Initial @bg_0000003e strict WAL preview test observed Ok rather than refusal. That initial keeper-open test did not demonstrate a sidecar byte change. The later closed-process/missing-SHM characterization (@bg_0000004b) passed: strict preview left all files unchanged, while a subsequent raw SQLITE_OPEN_READ_ONLY connection recreated the missing SHM file. A separate stdlib copy probe also measured WAL header bytes 2/2 preserved in a coherent backup and changed to 1/1 only after PRAGMA journal_mode=DELETE on the fresh trial.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Inspect the SQLite header before opening SQLite and refuse a WAL preview with a coherent non-WAL-copy instruction. Keep real WAL rebuild supported.

## Tests added

a_strict_wal_preview_refuses_before_sqlite_or_sidecar_changes, plus closed WAL/SHM characterization.

## Workarounds

Use a controlled coherent trial and the follow-up recovery guide; keep all writers stopped.

## Resume

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

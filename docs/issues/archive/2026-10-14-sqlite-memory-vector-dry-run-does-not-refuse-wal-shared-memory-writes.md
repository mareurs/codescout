---
id: 473fb34c3e2eceba
kind: bug
status: fixed
title: SQLite memory rebuild dry-run accepts WAL despite its strict no-file-change contract
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
closed: 2026-10-14
opened: 2026-10-14
owner: marius
severity: medium
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
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

Fixed and archived after source `c773a2ca5b5225010c06b5784a13470319bc4617` landed on experiments and the final ordered gate passed. See [the implementation evidence and rulings](../../superpowers/plans/2026-10-14-albert-recovery-hardening.md). Operator native/corpus acceptance is tracked separately.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

## Fix provenance

- **SHA:** `c773a2ca5b5225010c06b5784a13470319bc4617` (`experiments`)
- **patch-id:** `1e6a43618eb28337de242fcfb31271132a9b0354`

Verified final ordered gate: FMT0 CLIPPY0 LEAN0 PYTHON_LIGHT0 DEFAULT0, with normal temporary paths and eight test threads. SQLite27pass, ordinary CLI4pass and genuine two-record MiniLM CLI1pass; credential-free Python light1223pass/424subtests. Both review coverage gaps have observed mutation RED and restored GREEN.

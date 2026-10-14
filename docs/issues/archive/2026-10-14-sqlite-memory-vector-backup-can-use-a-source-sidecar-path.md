---
id: 9be9db5a55a54f8f
kind: bug
status: fixed
title: SQLite memory rebuild accepts a source sidecar as its rollback backup
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
closed: 2026-10-14
opened: 2026-10-14
owner: marius
severity: high
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
---

# BUG: SQLite memory rebuild accepts a source sidecar as its rollback backup

## Summary

A fresh backup filename can name the source's SQLite sidecar, including through a parent-directory alias. The original code validates nonexistence but not the destination's relationship to the source.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

A fresh backup filename can name the source's SQLite sidecar, including through a parent-directory alias. The original code validates nonexistence but not the destination's relationship to the source.

## Evidence

Initial new tests @bg_0000003e: direct -shm backup returned Ok with one vector published; an aliased source journal was also accepted. Tests refused those success results. Exact file deletion effects for the journal case were not claimed from that assertion.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Canonicalize the backup parent and refuse source/WAL/SHM/journal/super-journal names before model work. Pin source/reserved-backup identities and recheck publication boundaries.

## Tests added

sqlite_sidecar_backup_destinations_are_refused_without_touching_any_file; a_parent_directory_alias_cannot_hide_a_sqlite_sidecar_destination; a_byte_identical_source_replacement_is_refused_before_creating_a_backup.

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

---
id: '7b75f3b0ef1dd3c4'
kind: bug
status: taken
title: SQLite memory rebuild accepts a source sidecar as its rollback backup
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
opened: 2026-10-14
owner: marius
severity: high
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

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

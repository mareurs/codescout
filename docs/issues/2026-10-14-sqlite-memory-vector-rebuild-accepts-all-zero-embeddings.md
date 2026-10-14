---
id: d8b943fe6a44b6b9
kind: bug
status: taken
title: SQLite memory rebuild publishes all-zero embeddings as usable vectors
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

# BUG: SQLite memory rebuild publishes all-zero embeddings as usable vectors

## Summary

The guard checks width and finite values but accepts a vector with no nonzero component. The operator acceptance requires usable replacement embeddings.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

The guard checks width and finite values but accepts a vector with no nonzero component. The operator acceptance requires usable replacement embeddings.

## Evidence

Initial @bg_0000003e all_zero_embeddings_are_refused_before_backup_or_publication failed because the rebuild returned success for a 384-dimensional zero vector.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Require at least one nonzero component before reserving a backup or changing the index.

## Tests added

all_zero_embeddings_are_refused_before_backup_or_publication.

## Workarounds

Use a controlled coherent trial and the follow-up recovery guide; keep all writers stopped.

## Resume

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

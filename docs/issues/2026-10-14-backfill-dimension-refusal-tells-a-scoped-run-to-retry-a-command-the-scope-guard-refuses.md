---
id: '8b7a1374ac8e9a1e'
kind: bug
status: open
title: 'BUG: backfill''s dimension refusal tells a project-scoped run to retry a command that the scope guard then refuses'
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
topic: backfill-chunks dimension migration
closed: ''
opened: 2026-10-14
related: []
severity: low
---

# BUG: backfill's dimension refusal tells a project-scoped run to retry a command that the scope guard then refuses

## Summary

A project-scoped `backfill-chunks` with an embedder dimension that differs from `artifact_vec_v2` fails in `ensure_artifact_vec_v2_dim` with "set `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1` and retry". Retrying exactly that fails again, in `backfill_chunk_vectors_with`'s scope guard, with "dimension migration requires backfill-chunks --all". The first message is composed without the request's scope.

## Symptom (Effect)

Two round trips, the first on advice that cannot work. Read from the code; not run.

## Reproduction

Project-scoped backfill with a 384-dimensional embedder against a `FLOAT[768]` table: the first error names the env var; set it and rerun the same command; the second error names `--all`.

## Environment

sqlite-vec scope; any embedder whose dimension differs from the table.

## Root cause

`ensure_artifact_vec_v2_dim` is shared by the write path and the backfill preflight, and its refusal text does not know whether the caller is scoped. The scope guard runs only when the opt-in is already set.

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), finding 10.

## Hypotheses tried

N/A.

## Fix

Unknown — under investigation. Option: have the scoped preflight name `--all` together with the env var in its first refusal.

## Tests added

N/A — not fixed.

## Workarounds

Run `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1 codescout backfill-chunks --all`.

## Resume

Open. Not in the current fix set.

## References

- `src/librarian/indexer.rs` (`ensure_artifact_vec_v2_dim`, `backfill_chunk_vectors_with`)

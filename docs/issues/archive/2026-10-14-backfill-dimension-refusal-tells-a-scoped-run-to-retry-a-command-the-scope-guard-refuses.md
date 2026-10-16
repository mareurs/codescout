---
id: e33228bb4787d18b
kind: bug
status: fixed
title: 'BUG: backfill''s dimension refusal tells a project-scoped run to retry a command that the scope guard then refuses'
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
topic: backfill-chunks dimension migration
closed: 2026-10-15
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

Verified and committed on **experiments**.

- **SHA:** `934946e95e5f81c5091c80ae95d21c3624133612`
- **patch-id:** `361950b91e00766a6b4fbf03f5e19a63f0f2f120`

The scope guard in `backfill_chunk_vectors_with` now runs for every project-scoped sqlite-vec run, whether or not the opt-in is set, and it runs ahead of `ensure_artifact_vec_v2_dim`. A scoped run with a dimension change is therefore refused once, in the first refusal, with `--all` and `LIBRARIAN_ARTIFACT_VEC_MIGRATE` both named. The shared preflight's own message is unchanged: it still names only the opt-in, because the write path (`write_embeddings_v2_with`) has no scope to name.

The same commit fixes `docs/issues/archive/2026-10-14-on-a-qdrant-host-backfill-writes-its-vectors-to-sqlite-vec-and-clears-vectorless-without-a-searchable-vector.md` and `docs/issues/archive/2026-10-14-backfill-repair-docs-contradict-the-code-62177b8c-shipped.md`.

## Tests added

- `a_scoped_dimension_change_names_the_catalog_wide_command_in_its_first_refusal` (`src/librarian/indexer.rs`): observed RED first. The first refusal read "set LIBRARIAN_ARTIFACT_VEC_MIGRATE=1 and retry" and did not contain `--all`. It also asserts that the refusal changed no vectors (`artifact_vec_v2` stays at 768).

Mutation with `scripts/mutation-probe.sh` in an isolated worktree: the scope guard gated on the opt-in again (the old behaviour) was killed by this test. The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4509 passed, default library 6825 passed, 0 failed.

## Workarounds

Run `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1 codescout backfill-chunks --all`.

## Resume

Complete. Fixed in `934946e9`, patch-id above.

## References

- `src/librarian/indexer.rs` (`ensure_artifact_vec_v2_dim`, `backfill_chunk_vectors_with`)

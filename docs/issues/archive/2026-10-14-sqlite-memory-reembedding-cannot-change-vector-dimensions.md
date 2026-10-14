---
id: '2b65eb20fe8c0fb7'
kind: bug
status: fixed
title: 'BUG: SQLite memory reembedding cannot change the stored vector dimension'
owners:
- marius
tags:
- cluster/declared-not-wired
topic: Albert R-02 SQLite memory recovery
closed: 2026-10-14
fix_patch_id: 892b29741757566939c72750ff52112f20467b89
fix_sha: 36080b79daed5ba712f14dd75a935b3fd68188f6
opened: 2026-10-14
severity: high
unverified: TRACKED abf1db6dec85edfe — Albert's own snapshot/corpus recall, index inventory and Claude/Codex integrations remain operator acceptance work. Root's genuine MiniLM CLI/model/reopened-recall test passes, along with synthetic reconstruction, failure/retry/rollback and the full ordered gate.
---

# BUG: SQLite memory reembedding cannot change the stored vector dimension

## Summary

Albert's V2 verification reproduces memory(recall) failing for a 768-dimensional SQLite index with the local 384-dimensional model. Existing in-place reembedding uses ordinary upsert, which refuses the dimension change, and its skipped-row report is not evidence of recovery.

## Symptom (Effect)

Recall is unavailable after the model switch; list/read and the stored metadata remain readable. Albert compared 192 records and 146 topic files unchanged.

## Reproduction

Seed a real synthetic SQLite semantic store at width 768. The new actual-CLI dry-run regression initially failed with unrecognized subcommand rebuild-memory-vectors (@bg_00000011, exited 101). Core rebuild regressions then failed at the explicit unimplemented behavior (@bg_00000012, two failures). After implementation, the 192-record rebuild and failure/retry tests pass; ordinary store upsert retains its dimension refusal.

## Environment

Source baseline: experiments 223b83aa712a175a7905a56420877fc77abd57ac. SQLite memory backend; model dimension 768 to 384. Synthetic databases only for this work.

## Root cause

SqliteVecSemanticMemoryStore::ensure_vec_table refuses the fixed vec0 dimension. The advertised in-place dimension repair never replaces that table. Ordinary upsert also deletes and serializes memory_item, so it cannot guarantee literal row/JSON/ID preservation for a vector-only repair.

## Evidence

Albert's V2 report sections 5–6 and the source trace identify ordinary upsert's fixed-width refusal and record serialization. The new command's actual CLI regression initially failed as unknown subcommand, and two real database regressions initially failed at unimplemented behavior. No production database was migrated.

Fresh Astra review applied seven mutations: five killed, two survived, zero inconclusive. The survivors were test gaps: UUID normalization and deletion of final count/ID guards. Evidence: /tmp/codescout-r02-review-result.json.

After adding tests, alternate-spelling normalization reaches publication and fails exact uppercase-vector-ID comparison (@bg_00000026: 14 passed, 1 failed). Removing both publication guards fails after a one-vector incomplete commit (@bg_00000024: 14/1); removing only the ID guard fails after a two-vector unique-orphan commit (@bg_00000025: 14/1). Both guards are restored. The scratch is byte-equal to root at SHA256 8ce07123a119365e551122d0aecd931c16ee80db369132245b5d33c3d9cb5e17. The restored targeted run passes all 15 tests.

## Hypotheses tried

N/A — source trace and controlled reproduction are the initial investigation.

## Fix

On experiments: 36080b79daed5ba712f14dd75a935b3fd68188f6; stable patch-id 892b29741757566939c72750ff52112f20467b89.

The explicit SQLite-only rebuild validates and embeds every stored record, takes an exclusive WAL-aware verified backup, and publishes only memory_vec atomically after source consistency checks. Original memory_item SQL values and literal IDs are preserved, including accepted alternate UUID spellings. Any failed/missing vector refuses publication. Ordinary store dimension refusal remains with supported recovery guidance.

## Fix provenance

- **SHA:** `36080b79daed5ba712f14dd75a935b3fd68188f6` (`experiments`)
- **patch-id:** `892b29741757566939c72750ff52112f20467b89`

## Tests added

The previously ignored genuine-model test was explicitly run and passed (@bg_0000002f: one passed, zero failed). It executes the actual CLI against a two-record synthetic 768-dimensional database, loads the isolated AllMiniLML6V2Q snapshot, publishes 384-dimensional vectors, preserves raw records and queries a reopened store with the same model. Snapshot: Xenova/all-MiniLM-L6-v2 revision 751bff37182d3f1213fa05d7196b954e230abad9, quantized ONNX SHA256 afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1. Model manifest: /tmp/codescout-r02-model-snapshot.json. The 15 SQLite and four ordinary CLI tests also pass on the merged checkout (@bg_0000002c/@bg_0000002d).

Fifteen real SQLite/sqlite-vec tests pass on restored source (@bg_00000028, 15 passed, zero failed). They cover 192 literal IDs with raw JSON/unknown SQL columns preserved, reopened KNN recall, verified backup and separate-copy old-index rollback, halfway failure/retry, empty/same-width rebuild, missing/orphan repair, malformed rows/schema, wrong/non-finite vectors, invalid inputs, existing/failed backups, uncheckpointed WAL, record/vector-only external writers, actual insertion-error rollback, alternate accepted UUID spellings, incomplete/unique-orphan publication refusal, and characterization of legacy in-place dimension refusal. Four actual CLI tests pass with one real-model test explicitly ignored. Seven initial review mutations produced five killed and two surviving gaps; both gaps now have independently observed failing regressions under the corresponding mutations. Full ordered gate exited zero: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, FMT_STATE=passed. Gate limits include heavy Python, server-stack, and two skipped ONNX weight tests; the new real-model CLI test is explicitly ignored.

## Workarounds

Keep real records and backups; use the new 384 profile only on fresh isolated storage until recovery is verified. A same-width alternative model does not establish compatibility.

## Resume

Source fix verified and merged into experiments. Full gate: PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0 FMT_STATE=passed. Source SHA and patch-id are recorded above. Remaining real-machine acceptance is tracked separately in docs/superpowers/plans/2026-10-14-albert-r02-copy-acceptance.md. No daily database was migrated.

## References

- src/memory/sqlite_semantic_store.rs
- src/migrate/memories.rs
- docs/superpowers/specs/2026-10-14-sqlite-memory-vector-rebuild-design.md

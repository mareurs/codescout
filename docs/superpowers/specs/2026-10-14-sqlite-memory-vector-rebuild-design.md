---
id: 9fe44b12aeca0a0c
kind: spec
status: done
title: SQLite memory vector rebuild for Albert R-02
owners:
- marius
tags:
- albert-r02
- memory
- sqlite-vec
source_commit: 36080b79daed5ba712f14dd75a935b3fd68188f6
---

# SQLite memory vector rebuild for Albert R-02

## Intent and authorized scope

The user asked to continue after Albert's V2 report and the proposed next work: rebuild the existing SQLite memory vectors while preserving every memory record, with failure recovery, recall after restart, and rollback verified on copies. This implementation runs only against synthetic fixtures here. Applying it to Albert's daily databases is a separate operator step.

Albert's representative copy has 192 memory_item records and 768-dimensional memory_vec vectors. The verified local snapshot produces 384-dimensional vectors. Existing migrate-memories --in-place calls ordinary SemanticMemoryStore::upsert, which refuses the new dimension and can report every row skipped. That upsert also deletes and serializes memory_item, so it cannot be used for an exact record-preserving repair.

## Interface

Add codescout rebuild-memory-vectors with required --db, --model, --dimensions and --backup, plus --dry-run. The database is one explicitly selected existing per-project .memories.db; the operation covers all memory_item rows in that file, without project or bucket filtering.

The initial CLI supports an explicit local-dir: snapshot, loaded through codescout_embed::create_embedder_with_config(model, None, None) and CodeEmbedderAdapter with the requested dimension. It never creates an Agent or resolves a vector store from the current server profile. Environment endpoint/model overrides therefore cannot select Qdrant or silently replace the explicitly chosen snapshot. No downloader or remote model migration is added.

Use the existing MigrationEmbedder document-side seam and HttpMigrationEmbedder's segmentation. Dry-run opens read-only, validates and counts the existing records/schema, and neither constructs an embedder nor creates a backup. Requested dimensions are reported separately from published dimensions.

Library interface: RebuildOptions { dimensions: usize, backup: PathBuf, dry_run: bool }; async rebuild_sqlite_memory_vectors(db: &Path, embedder: Option<&dyn MigrationEmbedder>, options: &RebuildOptions) -> Result<SqliteVectorRebuildReport>. Report counts the actual enumerated records and actual inserted vectors; success is returned only after publication. No skipped-row success mode.

## Preservation and publication

Register sqlite-vec, open an existing database without CREATE, and validate memory_item plus the supported memory_vec schema. Reject a foreign/legacy schema, invalid IDs/payloads, zero dimensions and unrecognised vector columns. Read every row in stable point_id order. Keep the raw SQL values for every memory_item column, including future columns and original JSON spelling; parse SemanticMemory only to obtain document content. Never regenerate point IDs or write memory_item.

Read PRAGMA data_version on the same connection before processing. Embed every row and validate exact requested length and finite floats. Hold replacement vectors in memory. There is no persistent partial staging or resume cursor: after an embedding failure/process interruption the old vector table remains and retry starts afresh. This deliberately avoids mixing models or conventions across partial runs. This is a bounded one-database maintenance operation; large databases need enough memory for their complete replacement vectors.

After all embeddings validate, reserve a fresh backup destination with exclusive creation and owner permissions on Unix. VACUUM INTO produces the consistent SQLite snapshot, including WAL content; verify integrity and the source record snapshot in the backup. A failed or existing backup refuses publication. Never fall back to copying an active main .db file. The original is unchanged by VACUUM INTO. See [SQLite VACUUM](https://www.sqlite.org/lang_vacuum.html).

Acquire BEGIN IMMEDIATE, reject a changed data_version or changed record snapshot, then drop/recreate only memory_vec and insert all replacement vectors under their original literal IDs. Validate full ID coverage, vector dimensions and unchanged raw memory_item values inside the same transaction before commit. Any publication error rolls back the old vector table and leaves the backup available. Stop external writers in the operator procedure; concurrency checks protect against accidental overlap. See [transactions](https://www.sqlite.org/lang_transaction.html) and [data_version](https://www.sqlite.org/pragma.html#pragma_data_version).

Keep ordinary upsert's dimension refusal. Replace its misleading clear-the-index advice with the explicit rebuild command and copy/backup requirements. Clarify that legacy in-place reembedding does not perform this SQLite dimension rebuild.

## Verification

Tests use real bundled SQLite and sqlite-vec. Only the model is supplied through the existing deterministic embedder seam.

- First reproduce the current 768-to-384 upsert/reembedding refusal on a real synthetic store.
- Rebuild 192 rows, preserving literal IDs, raw JSON including unknown fields, every SQL column, anchors, buckets and timestamps; verify every vector dimension and no orphan/missing IDs.
- Reopen a fresh semantic store and verify a real KNN query.
- Fail midway during embedding, assert all old vector bytes and records intact, then retry to complete.
- Exercise wrong/empty/non-finite dimensions, malformed rows, missing/wrong schemas, multiple project namespaces and empty indexes.
- Force an insertion error after the vector table has been dropped/recreated, and observe transaction rollback restoring the old table and vector bytes.
- Change the source from another connection during embedding, and refuse publication.
- Seed an uncheckpointed WAL row and prove the verified backup includes it.
- Restore the backup to a separate closed fixture and verify original rows, vector bytes and old-dimension query.
- A same-dimension rebuild still recomputes every vector; identical dimensions are not model identity.
- Exercise the actual CLI dry-run, invalid options/model/schema, machine-readable report and refusal exit codes.
- Apply review mutations at the real preservation, dimension, backup, source-change and publication call sites; observe test outcomes and restore hashes.
- Run the ordered complete gate in a leased target and make the ONNX/server-stack coverage limits explicit.

## Delivery and limits

Add a supported operator procedure: inventory every active memory backend/database, stop writers, work on coherent copies, rebuild each selected SQLite file with the verified model snapshot, reconcile records and vector coverage, recall after restart, and restore the compatible backup/profile on a separate copy. No real project data, credentials, plugins, drivers or services are changed by this development work.

This fixes SQLite memory vector reconstruction. It does not identify the historical model, migrate Qdrant, recover records absent from memory_item, guarantee whole-corpus relevance, or validate Albert's interactive Claude/Codex integrations.

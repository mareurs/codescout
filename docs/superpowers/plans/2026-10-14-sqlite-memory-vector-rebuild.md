---
id: '59958a4ffc39bd90'
kind: plan
status: done
title: Implement SQLite memory vector rebuild
owners:
- marius
tags:
- albert-r02
- memory
- sqlite-vec
source_commit: 36080b79daed5ba712f14dd75a935b3fd68188f6
verification: full ordered gate zero; 15 SQLite, 4 ordinary CLI and 1 genuine MiniLM CLI/reopened-recall tests pass; initial mutation gaps closed
---

# SQLite Memory Vector Rebuild Implementation Plan

> For agentic workers: use superpowers:executing-plans to implement this plan inline, with one fresh whole-branch review.

**Goal:** Rebuild every vector in one explicitly selected SQLite memory database while preserving its original records and retaining a coherent rollback backup.

**Spec:** [SQLite memory vector rebuild design](../specs/2026-10-14-sqlite-memory-vector-rebuild-design.md).

**Architecture:** Add a migration module independent of Agent/profile selection. Read raw SQL rows, embed all content through MigrationEmbedder, create a verified VACUUM INTO backup, and publish only memory_vec in one transaction after source consistency checks. CLI uses an explicit local-dir snapshot; dry-run loads no model.

**Tech stack:** Rust, rusqlite, sqlite-vec, existing codescout-embed local ONNX adapter, Clap.

**Global constraints:** Synthetic fixtures only during development. No memory_item writes, no skipped-row success, no automatic backend/model inference, no overwriting existing backups, no partially resumed embeddings. Preserve peer checkout changes. Keep ordinary store dimension refusal.

### Task 1: Atomic SQLite rebuild and real database tests

**Files:** create src/migrate/sqlite_memory_vectors.rs; modify src/migrate/mod.rs; tests in new module.

**Interfaces:** RebuildOptions { dimensions, backup, dry_run }; SqliteVectorRebuildReport; async rebuild_sqlite_memory_vectors(db, Option<&dyn MigrationEmbedder>, &options).

1. Add stubs and real SQLite fixtures/tests for 192-row preservation and failure/retry, then run scripts/with-slot.sh cargo test --lib migrate::sqlite_memory_vectors::tests. Expected: assertion failures at unimplemented rebuild behavior, not missing imports.
2. Implement source/schema validation, raw column preservation snapshots, all-vector preparation, exclusive coherent backup, concurrency check, atomic vector-only publication.
3. Add cases for empty/same-dimension rebuilds, malformed/foreign schemas and rows, wrong/nonfinite vectors, concurrent external write, WAL backup, restored old-index recall, and actual insert-error rollback.
4. Run the targeted suite. Expected: every added test passes on real SQLite/vec0. Capture measured record/vector counts only.

### Task 2: Explicit CLI and recovery hints

**Files:** src/main.rs; src/memory/sqlite_semantic_store.rs; src/migrate/memories.rs; tests/rebuild_memory_vectors.rs.

**Interfaces:** consumes Task 1 APIs; new rebuild-memory-vectors required --db/--model/--dimensions/--backup and --dry-run.

1. Existing integration regression already observed RED: unknown subcommand. Add explicit validation cases and refusal exit checks.
2. Wire explicit local-dir model to create_embedder_with_config with no URL/key, CodeEmbedderAdapter expected dimension, and HttpMigrationEmbedder document segmentation. Dry-run calls library without constructing model or Agent.
3. Correct old clear-index hint and legacy in-place help to point SQLite dimension changes at this command.
4. Run scripts/with-slot.sh cargo test --test rebuild_memory_vectors and Task 1 suite. Expected: passing actual CLI dry-run with byte-unchanged fixture, no backup/model access.

### Task 3: Operator procedure, fresh review, delivery

**Files:** add docs/conventions/sqlite-memory-vector-rebuild.md; update R-02 issue and its issue-cluster membership through librarian.

1. Document closed-writer coherent copies, inventory of other indexes, explicit verified snapshot, JSON reconciliation, fresh-process recall, backup restoration to a separate copy, and pending actual-machine/plugin acceptance.
2. Fresh whole-branch reviewer must apply candidate mutations, run tests, report observed killed/surviving/inconclusive counts, and restore original source hashes. Add survivor tests only after observing survival.
3. Run ./scripts/gate.sh --check in leased target. Expected: all printed gate flags zero. Record skipped ONNX/server-stack coverage honestly.
4. Commit only owned work after shared-checkout coordination; integration follows existing user authorization and repository policy.

**Review Focus:** Exact raw records/unknown SQL columns, literal IDs, empty baked-dimension schema, all-row coverage across namespaces, concurrent writer ordering, coherent WAL backup, transaction rollback after virtual-table DDL, read-only dry-run, no Agent/config backend selection, no publication on any failed embedding. Real local model inference and actual operator laptop integrations require explicit measured evidence and remain stated limits when unavailable.

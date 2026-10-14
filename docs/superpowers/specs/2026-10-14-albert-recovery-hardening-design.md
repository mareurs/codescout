---
id: '7584256bee7a6dbe'
kind: spec
status: done
title: Albert SQLite recovery safeguards and offline acceptance
owners:
- marius
tags:
- albert-r02
- sqlite
- offline-tests
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
---

# Albert follow-up: SQLite recovery safeguards and offline acceptance

## Intent and constraints

Albert supplied a separate implementation and copy-only acceptance in pachet-fix-codescout-2026-10-14.zip (SHA256 0eed24e0314118536cd2d9c659958996aa4fd3d98e81406ba56b9b14df5d2f92). Reconcile useful protections into the existing supported command, make the light Python lane collect without developer credentials, and supply one tested client acceptance procedure. The user authorized this work and prior integration into experiments. Daily databases, installed plugins, credentials, drivers and profile configs remain operator-owned.

Retain the standalone rebuild-memory-vectors CLI and explicit local-dir snapshot/target dimensions. Preserve every raw memory_item SQL value, literal UUID spelling and multiple namespaces; do not adopt the candidate's derived-ID or single-project restrictions. Keep the verified five-file MiniLM snapshot and ordinary dimension guard.

## SQLite recovery

Keep an open source file identity across the connection and embedding work, reject source replacement before backup/publication, and protect the reserved backup identity. Use the already-cached cross-platform same-file handle crate as a direct dependency; unsupported identity implementations must fail rather than silently skip protection. Refuse backup destinations that name the source or its SQLite WAL/SHM/journal/super-journal paths, including parent aliases. Hold a consistent read transaction while recording original rows, vector schema/bytes and data_version.

Verify coherent backup integrity, raw records, width and original vector bytes; keep its identity and recheck those contents before commit. Publication must run identity checks after DDL and inserts so errors/cancellation retain the old index. Preserve full-ID/width/count guards. Keep zero-record rebuild support. Reject zero vectors as well as wrong widths and non-finite values. Do not expose backend errors that can echo private memory text.

Strict dry-run must refuse a WAL database before opening SQLite, explaining that a coherent non-WAL copy is required; SQL read-only flags alone do not promise unchanged WAL/SHM files. Actual rebuild continues to support WAL. Add genuine subprocess crash, WAL rollback and cancellation tests on synthetic databases; distinguish already-working behavior from new RED→GREEN fixes.

## Offline Python

The scorer currently constructs SubscriptionJudge during import; an empty-home collection reproduces SystemExit. Defer credentials/live-client creation until actual live scoring. Import, collection, help and pure validators must not access credentials or launch a client. The first live request must still refuse absent authentication and preserve existing clean-channel checks. No fake production credentials, auto-login, network calls or skipped test inventory.

## Client acceptance and delivery

Update a portable guide/config packet for the exact new source patch and verified 384 CPU snapshot. Verify supported STDIO/MCP settings against local client code/help and official sources as needed. Keep read-only tools available, explicitly explain that memory multiplexes read and write actions, and retain shell/write/index protections. Validate Claude companion manifests and Pi source contract separately; do not infer actual client hook/approval behavior from a synthetic MCP client.

Tests here use isolated synthetic data/private config. Albert's own current client loading, corpus relevance, remaining index inventory and daily promotion remain acceptance work; the new report covers his own candidate rather than our implementation.

## Verification

Run observed failing regressions before product fixes, then existing and new targeted Rust/Python tests, the genuine offline MiniLM integration, a fresh strong review with actually applied mutations, and the full ordered gate. Package source-only patch, guides and hashes; verify clean application to public base223 and exact resulting source bytes. Commit only owned paths and fast-forward experiments after checks, without pushing or including peer edits.

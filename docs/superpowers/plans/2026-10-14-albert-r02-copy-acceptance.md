---
id: abf1db6dec85edfe
kind: plan
status: active
title: Albert R-02 acceptance on isolated copies
owners:
- marius
- albert
tags:
- albert-r02
- operator-acceptance
- memory
source_commit: 36080b79daed5ba712f14dd75a935b3fd68188f6
---

# Albert R-02 acceptance on isolated copies

Root verification is complete: full ordered gate passed; 15 real SQLite tests, four ordinary CLI checks and one actual MiniLM CLI/reopened-recall test passed. Actual-model revision: Xenova/all-MiniLM-L6-v2 751bff37182d3f1213fa05d7196b954e230abad9. The operator checks below remain required for Albert's own snapshot, data and clients.

This tracks the remaining actual-machine work after the SQLite rebuild implementation and synthetic tests pass. It is an operator procedure, not authorization to modify daily databases.

Use [the recovery guide](../../conventions/sqlite-memory-vector-rebuild.md). Keep the V2 report and accepted setup ZIP unchanged as historical evidence.

- [ ] Record the tested source SHA/binary SHA and the verified AllMiniLML6V2Q snapshot hashes.
- [ ] Inventory every active memory database/backend and its original namespace, filename and directory; inventory code/catalog/Qdrant stores separately.
- [ ] Stop all writers and make a coherent fresh trial copy, including WAL content. Retain original profile and topic files.
- [ ] Run the optional genuine CLI/model/reopened-recall regression with the existing offline snapshot and record the actual result.
- [ ] Dry-run the selected trial and record actual record count/baked dimensions. Albert's V2 copy had 192 records; use the actual copied population as the acceptance denominator.
- [ ] Rebuild with the verified 384-dimensional snapshot and fresh backup. Require vectors_written == records, published_dimensions == 384 and zero failed/skipped records.
- [ ] Compare every original memory_item SQL value, literal ID and raw JSON against the rebuilt trial, and the separately retained topic files against their source hashes.
- [ ] Start a fresh process with the same snapshot, SQLite trial storage, original stored project namespace and correct database basename. Confirm list/read sees the original population before evaluating meaningful recall.
- [ ] Restart and repeat known-topic/anchored/multiple-bucket recall; record actual returned topics and relevance.
- [ ] Force a failed run on a disposable extra copy, verify original index retained and complete retry; verify backup restoration into another closed fresh directory.
- [ ] Record historical-model status. Original physical rows/vectors can be restored, but another 768-dimensional model does not prove compatible old recall.
- [ ] Reconcile remaining active SQLite files and other stores. Verify interactive Claude Code plugin loading and Codex integration separately before any global switch.

No daily-data promotion is requested by this plan. Report synthetic test results, actual local inference, actual-corpus recall and integration acceptance separately.

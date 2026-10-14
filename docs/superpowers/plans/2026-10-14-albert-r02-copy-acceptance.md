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

Historical verification of source36080b79 is complete: full ordered gate passed; 15 real SQLite tests, four ordinary CLI checks and one actual MiniLM CLI/reopened-recall test passed. Actual-model revision: Xenova/all-MiniLM-L6-v2 751bff37182d3f1213fa05d7196b954e230abad9. The operator checks below remain required for Albert's own snapshot, data and clients.

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

## Albert's parallel candidate evidence, received 2026-10-14

ZIP SHA256 0eed24e0314118536cd2d9c659958996aa4fd3d98e81406ba56b9b14df5d2f92; candidate patch SHA256 7e5a3f8d80b85e2ae34a61e8697245ea66eff4a9f85ca371454385dc509124fc. The four members passed CRC checks. Its patch reconstructs all five reported source files exactly on base223b83aa and its five model hashes match our verified snapshot.

Albert reports copy-only reconstruction of192 records from768 to384 dimensions, unchanged raw rows and146 topic files, failure/retry/restoration, and restarted recall through a synthetic MCP client. These private-corpus results are reported evidence, not independently repeated here. His candidate has a different migrate-memories flag; do not mix its CLI recipe with our standalone rebuild-memory-vectors command.

His original gate was not all green: overall1, three Python credential-collection errors and a lean packaging failure attributed to borrowed Git objects. A separate packaging rerun passed four checks; that does not establish a complete green lean rerun.

The follow-up implementation incorporates the additional recovery checks and repairs our credential-free collection. Use [native client acceptance](../../conventions/albert-client-copy-acceptance.md) with the next source-specific packet. Actual native Claude/Codex/Pi acceptance, inventory of every store and daily promotion remain pending. Keep the checkboxes above open for the supported implementation on Albert's laptop.

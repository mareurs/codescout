---
id: 41cb03b1ce1ef258
kind: bug
status: fixed
title: 'BUG: a failed backfill leaves chunks without vectors that its retry skips'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: Albert R-01 backfill failure recovery
closed: 2026-10-14
opened: 2026-10-13
severity: high
unverified: CLEARED — final source passed the full ordered development gate, restored 65-test indexer suite, and all 14 applied-mutation probes; committed on experiments.
---

# BUG: a failed backfill leaves chunks without vectors that its retry skips

## Summary

Albert's R-01 reported a failed 768-to-384 backfill followed by a successful-looking retry that embedded nothing. The pre-fix selector checked absent chunk metadata, so existing chunks without vectors were outside its repair population. Retry now selects durable vector gaps and preserves stored chunks and healthy vectors.

## Symptom (Effect)

Reported by Albert on 2026-10-13 at b336e43ff0687e115ef589928c012f4ce9d51af0, using independent catalog copies:

```
first backfill: dimension mismatch; 92 chunks, zero vectors, FLOAT[768]
retry with LIBRARIAN_ARTIFACT_VEC_MIGRATE=1: exit 0, artifacts=0, embedded=0
after retry: 92 chunks, zero vectors, FLOAT[768]
```

These are external measurements, not a local reproduction. Ordinary document/history rows survived the reported probes.

## Reproduction

Albert's report: migrate a private v10 catalog copy to v13, run backfill with a 384-dimensional embedder without migration opt-in, then retry with opt-in. Only three source documents were exposed, so this does not measure whole-corpus recovery.

Local SQLite tests from experiments 388416f2242ff09de86ee7cc967854ca2c281686 reproduced successful zero-work retry after two chunks persisted and embedding failed. Further RED fixtures reproduced a legacy cursor concealing missing vectors, a checkpoint ahead of the final flush, and a populated old-dimension index bypassing validation. The correction was developed in `.worktrees/albert-r01-backfill-retry-20261013`; its verified source commit is now on experiments.

## Environment

External: Zorin OS 18.1; dash; local AllMiniLM-L6-v2 384-dimensional model; copied catalog. Local: deterministic 768- and 384-dimensional embedders, real in-memory and file-backed SQLite catalogs, private project roots, and sqlite-vec queries. Albert's installation was not changed.

## Root cause

Verified by local RED tests and source inspection on 2026-10-13:

- Backfill selected absent chunk metadata, while `embed_queue_items` wrote chunks before vector persistence.
- Historical cursors could conceal missing vectors, and a page checkpoint could precede a below-threshold final flush.
- Dimension validation followed selection, allowing populated old-dimension catalogs to return successful zero work.

Shared dimension rebuilds remove vectors across projects while retaining chunks, so recovery must cover all affected projects.

The first patch re-chunked current disk content before filtering missing vectors. An observed review regression deleted a healthy vector and replaced stored rows. Repair must use persisted chunks and stored entry-title context; ordinary indexing applies content and grain changes.

A final pre-commit compatibility scout found that remote embedders report dimension zero until their first response, while the factory/bootstrap do not warm them. The first preflight rejected these configured embedders. Three additional tests were observed RED before a short dimension probe was added ahead of all catalog writes; its response length establishes compatibility.

## Evidence

External: `/home/marius/Downloads/raport-validare-codescout-pentru-marius-2026-10-13.md`, R-01. Corpus counts remain Albert's measurements.

Local evidence under `.codescout/resident-private/albert-r01-backfill-retry-20261013/`: `selection-red.log`, `recovery-red.log`, `recovery-green.log`, `reviewer-changed-file-regression.log`, `stored-repair-red.log`, and `stored-repair-green-corrected.log`. The latter reports 62 passing indexer tests. First review: five mutations killed, one checkpoint-before-write mutation survived 58 tests. Re-probing the persisted-row patch killed all nine applied mutations with zero survivors or inconclusive runs; `review2-report.json`, `review2-mutation-results.json`, `review2-M1.log` through `review2-M9.log`, and the restored 62-test `review2-restored.log` retain that proof. The checkpoint-before-write mutation now fails the intended persistence-failure assertion. The SQL escaping error in `stored-repair-green.log` was corrected; that log is not GREEN evidence.

Remote compatibility: `deferred-dimensions-red.log` reports nine existing backfill tests passing and three new tests failing; `deferred-dimensions-green.log` reports all 65 indexer tests passing after the probe correction. F-194 records the source contract and observed failure. Final review of this revised source is complete: `review3-report.json` and the fourteen `review3-P*.log` / `review3-M*.log` records show fourteen mutations killed, zero survivors, and zero inconclusive runs. The restored `review3-restored.log` reports 65 passed, zero failed. Root checked every mutation log's test-result record against the report and verified both source copies against `review3-hashes.json`. The final ordered gate (`gate-final.log`, `gate-final.exit`) completed with exit 0 on source matching `review3-hashes.json`. `source-commit.json` records the native commit, stable patch-id, and committed source hashes.

## Hypotheses tried

1. Chunk metadata hides missing vectors: confirmed by retry RED and restored-vector KNN GREEN.
2. Cursor progress hides holes: confirmed by separate RED fixtures. A checkpoint-before-write mutation survived the first suite, prompting a write-failure regression.
3. Re-chunking current files preserves recovery state: disproved by the review's changed-file RED. Permanent content/grain/absent-source tests also failed before persisted-row repair and passed afterward.

## Fix

Verified and committed on **experiments**.

- **SHA:** `62177b8cc6ebce0d667cc7c48c37d755356bc388`
- **patch-id:** `a917aff453617da7e34630e24b6ab144b9bcaf8f`

The committed source hashes match the independently reviewed and fully gated revision.

Select absent chunks or missing sqlite-vec vectors. Restart from durable coverage rather than legacy cursors. Flush each page before checkpointing. Validate model dimensions before selection. Existing explicit migration opt-in retains a file-backed backup; shared dimension changes require `backfill-chunks --all`.

Repair persisted chunks using stored content and heading context, without re-chunking changed files or grain. Preserve healthy vectors byte-for-byte. Create chunks from source only when none exist. Ordinary indexing still embeds every item using current titles.

Probe unknown remote dimensions outside the catalog mutex and before any catalog writes. Use the response length, reject failed or empty probes, and exclude the probe from the persisted-vector report. Known dimensions require no probe.

## Tests added

Twelve behavioral regressions cover embedding failure/retry with a working query; partial coverage and legacy cursor; final-flush and write-failure durability; 768-to-384 recovery across two projects with and without old vectors, unchanged metadata, and verified backup; changed content, changed grain, and absent source preserving full stored rows, healthy vectors, and the literal stored entry-title prefix; compatible deferred dimensions with honest vector counts; failed/empty probes preserving catalog state; and deferred dimensions through populated cross-project migration.

The revised focused indexer run reports 65 passed, zero failed. Final review applied fourteen production mutations to this revision; every mutation was caught, with zero survivors or inconclusive runs. The full ordered `./scripts/gate.sh --check` completed with exit 0: Python light 1,219 passed plus 421 subtests; formatting and Clippy passed; lean Rust library 4,476 passed, zero failed, seven ignored; default Rust library 6,769 passed, zero failed, eight ignored. The workspace, integration, and documentation-test groups also completed successfully.

Coverage limits printed by the gate remain: heavy Python suites, server-stack, librarian in the lean lane, and two local ONNX weight tests are not covered. Private deterministic embedder fixtures verify recovery and real SQLite queries; no live-provider, production-corpus, or arbitrary-concurrency claim is made. The first gate attempt found retired call forms only in a completed private pilot staging snapshot; that snapshot was retained byte-for-byte with a `.md.snapshot` suffix. No gate behavior was changed.

## Workarounds

Build the recorded source fix, then rerun backfill against the affected copied catalog. Compatible dimensions allow normal project scope. A shared-table dimension change requires explicit catalog-wide scope and the existing migration opt-in:

```sh
LIBRARIAN_ARTIFACT_VEC_MIGRATE=1 codescout backfill-chunks --all
```

The opted-in dimension change backs up the file-backed catalog and regenerates missing vectors from persisted chunks across its projects. Albert's real installation and catalog were not modified in this work.

## Resume

No implementation or verification work remains for this defect. The source fix is recorded by its experiments SHA and stable patch-id above.

## References

- `src/librarian/indexer.rs`
- `src/librarian/catalog/chunk.rs`
- `src/cli/backfill_chunks.rs`

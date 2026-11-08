---
id: ee471f5a1b5aa3a8
kind: bug
status: fixed
title: Native index verification labels zero-byte Rust fixtures as missing after a successful complete build
tags:
- cluster/unclassified
- indexing
- verification
closed: 2026-11-07
opened: 2026-11-05
unverified: 'STANDING Limits that no action settles here: verify now reads every walked file once (extra I/O on large trees, same order as a build); indexer_can_represent mirrors the skip rules of stream_index by duplication, not through one shared predicate, so a change to those rules can drift verify again; a file that was indexed earlier and later emptied shows as an orphan.'
---

# BUG: Index coverage treats two empty Rust files as missing code

## Class fit

This is a mismatch between a verification population and the property being verified. The filesystem walk includes zero-byte source fixtures that cannot emit code chunks, then treats their absence from a chunk store as incomplete coverage. It differs from a genuinely missing member collection because source content is empty and root semantic queries already return relevant indexed code.

## Symptom (Effect)

Selected rust-analyzer source-only CLI indexing exits0 after adding61595 chunks. A fresh physical clone's native index(action="verify") reports verdict incomplete, expected_files1613/stored_files1611, and lists exactly crates/parser/test_data/parser/ok/0000_empty.rs and crates/parser/test_data/parser/ok/0024_const_item.rs as missing. A separate network-disabled read-only Docker probe measured stat().st_size == 0 for each of those exact paths in the same immutable source image. No other missing code path, orphan or vectorless chunk was reported by this verify call.

## Reproduction

Image sha256:0d74e4f14c68a438dce6cf9e4a02c626432ce12b866f55ac69c6cc687f7fff7a; native binary clean e6f07e9a6. Evidence BASE=/home/marius/.cache/codescout-resident/codescout: external-default-index-20261105/rust-lang__rust-analyzer-22992/{index.stdout,index-result.json}; external-semantic-verify-20261105/rust-lang__rust-analyzer-22992/coverage-result.json. Metadata probe: fresh docker run --rm --network none --user1000:1000 --workdir /testbed --entrypoint python3 on the pinned image, pathlib.Path(each_exact_path).stat().st_size. Both measured zero; no model/provider/credential use.

## Cause boundary

Observed native verification compares an eligible-file population with files represented by code chunks; these empty files have no source content to represent. Correction 2026-11-07: the source cause is now known and the earlier 'unverified' statement no longer holds. In src/retrieval/sync.rs, verify_index_coverage built its expected set from indexable_files, which lists every walked file, while stream_index skips a file it cannot read as UTF-8 and a file whose chunks are all blank. Such a file can never be stored, so verify reported it as missing. The fix is commit a34c4e2fe, which is an ancestor of experiments (checked 2026-11-08 with git merge-base --is-ancestor). Do not erase the original incomplete verdict or repeatedly rebuild unchanged empty files.

## Containment

Preserve the exact two empty-file exceptions and independently measured sizes as a source-only admission qualification. Continue requiring no missing nonempty eligible code, no orphan and no chunks_without_vectors. Keep memory-vector readiness separate: generated scaffold memory points remain pending migration after author completion. No repair or hidden expected-test set is changed by this qualification.

## Fix

The fix was made on branch fix/open-issues-batch-20261107-r2 and is now on experiments. Both commits listed below are ancestors of experiments at 143c1941f, which is the same SHA because experiments reached that tip by fast-forward from the branch.

Where the change lives: src/retrieval/sync.rs, function verify_index_coverage. It now filters the expected set through indexer_can_represent. A file is not expected when it cannot be read as UTF-8, or when its content is empty or whitespace-only (zero-length metadata is a fast path). stream_index skips both kinds of file, so verify no longer counts them as missing. indexable_files is unchanged, so stream_index, sync_worktree and verify still share one walk.

Plan deviation: Unknown (none was given).

Commits (made on branch fix/open-issues-batch-20261107-r2; all are now on experiments at 143c1941f, the same SHAs):

- a34c4e2fec888c1b80108c8bcae93bce4c541798 (branch fix/open-issues-batch-20261107-r2) / patch-id 7ce287733f6b11803f27ff78d402ebc8b68e8b68 - the fix.
- 6d75578828765941385d79ea3a97a41695d887ef (branch fix/open-issues-batch-20261107-r2) / patch-id d7874f2a0f154fde3bd3994cea4024684055dc61 - test for the non-UTF-8 exit.

Verification evidence: ./scripts/gate.sh --check was run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07. That branch is deleted; af3e2a9ec is an ancestor of experiments. Result: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests. An Opus reviewer examined the fix adversarially. The reviewer ran the new test red against the reverted production change (or used mutants) and approved.

Limits of the fix:

- verify now reads every walked file once. This is extra I/O on large trees, of the same order as a build.
- A file that was indexed earlier and later emptied now shows as an orphan.
- indexer_can_represent mirrors the two skip rules of stream_index by duplication, not through one shared predicate. A change to the skip rules of stream_index can make verify drift again.

## Fix provenance

- **SHA:** `a34c4e2fec888c1b80108c8bcae93bce4c541798` (`experiments`)
- **patch-id:** `7ce287733f6b11803f27ff78d402ebc8b68e8b68`

## Tests added

retrieval::sync::tests::verify_does_not_count_files_the_indexer_cannot_represent_as_missing

The test builds three files: empty.rs (empty), ws.rs (whitespace-only) and bad.rs (non-UTF-8). Before the fix it was red with missing_count 2 for the first two. A mutant that checks only zero-length files went red on ws.rs. The reviewer ran the test red against the reverted production change (or by mutants).

The earlier evidence (native verify on a rust-analyzer clone, and same-image size witnesses) is kept in Reproduction and Symptom (Effect).

## Resume

Nothing outstanding. The fix landed on experiments (commits a34c4e2fec888c1b80108c8bcae93bce4c541798 and 6d75578828765941385d79ea3a97a41695d887ef, tip 143c1941f, by fast-forward), ./scripts/gate.sh --check on that tree gave FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0 PYTHON_LIGHT=0, and this file was archived to docs/issues/archive/. See Fix provenance.

---
id: f4419d3fd1546572
kind: bug
status: fixed
title: Native index verification counts zero-byte files as missing chunks
tags:
- cluster/selector-narrower-than-its-population
- effort-steering
topic: effort-steering
closed: 2026-11-12
opened: 2026-11-09
unverified: 'Fixed on main in a34c4e2fe (2026-11-07). Checked on a rebuilt server (target/release/codescout built 2026-11-12 13:35 EET): a scratch tree with a zero-byte and a newline-only .rs file verifies complete, expected 2 stored 2. Not run: the frozen binary e6f07e9a6 on that tree (no paired run), or the rebuilt binary on CodeRank, Arrow or Dioxus.'
---

## Observed

The CodeRank full native index exited0 after441.289s, added61603 chunks, then six memories were read/upserted. Native verify still reports incomplete:1614 expected files,1612 stored, exactly two missing,zero orphan files,zero vectorless chunks,all6 memories present,git up_to_date. Both named missing source files are zero-byte parser fixtures: crates/parser/test_data/parser/ok/0000_empty.rs and 0024_const_item.rs. Raw result is preserved; no completeness override applied.

## Class fit

The verifier compares eligible paths with chunk-bearing paths without distinguishing zero-byte inputs that cannot supply a content chunk. Its missing-file population includes empty files, so successful indexing cannot satisfy the literal verdict on this source.

## Evidence

/home/marius/.cache/codescout-resident/codescout/common-preparation-20261109/coderank-readiness-raw.json contains exact native result; source byte sizes must be retained with hashes before any host-side coverage interpretation. Native/proxy/network cleanup confirmed complete. References/symbol_at/call_graph/semantic actual primary results are present, but paid admission remains closed.

## Arrow frozen-binary recurrence — 2026-11-12

Known recurrence in common-preparation-v3/all-configured-members-v1: native verify reports619eligible files,617stored, exactly testing/README.md and testing/data/orc/README.md missing. Both were independently reopened and measured0bytes, each SHAe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855. No orphan/nonvector chunks, all5root memories present, Git up-to-date. Keep literal incomplete verdict; no empty source file was deleted and no full reindex was run to chase this cosmetic gap. Actual configured member retrieval is separately checked, not inferred from these counts.

## Dioxus frozen-binary recurrence — 2026-11-12

One reused source-only onboarding and one root CodeRank index finish with source unchanged and owned cleanup. Verify literally reports incomplete:1060eligible files,1053chunk-bearing files,7missing. All7reported names were inspected:4files are0bytes;3are exactly the byte0a(newline), so all7are whitespace-only. All5memories are present,0orphans/nonvector chunks,Git current8a5e0f9c. No rebuild/re-onboarding is justified from this same known verifier/content predicate mismatch. This evidence extends the observed file class to whitespace-only sources; it does not establish actual member-scope retrieval, LSP/context delivery or repair efficacy. Evidence under Dioxus common-preparation-v1/index-coderank-v1, plus the exact seven-path size/hex probe in this thread's run log.

## Root cause and verdict — 2026-11-12

The verifier has been fixed on main since 2026-11-07, two days before this bug was opened. `verify_index_coverage` (`src/retrieval/sync.rs`) now drops from the expected set every file that `indexer_can_represent` rejects: zero bytes, whitespace-only, or not UTF-8. These are the files `stream_index` can never store a chunk for. This bug is a recurrence of the same defect as `f199e23bcd3e7eb6`, which `a34c4e2fe` named in its subject.

All three reports above ran the frozen native binary. Its own build record says `e6f07e9a6` (2026-11-04), with executable SHA-256 `cdc53728d28a659b5a697048e6e5b719149693df2cf57cb8554ee0751ebf51bb` (see `docs/trackers/effort-steering/reports/2026-11-09-onboarding-readiness-audit.md`). I read that tree: `git show e6f07e9a6:src/retrieval/sync.rs` holds zero occurrences of `indexer_can_represent`, and `a34c4e2fe` is not an ancestor of `e6f07e9a6`. So the frozen binary lacks the fix, and its report was correct for its own code.

The missing populations match the fixed classes: CodeRank two zero-byte files, Arrow two zero-byte README files, Dioxus four zero-byte and three newline-only files. No non-blank, readable file appeared in any list.

### What I ran

`scripts/with-slot.sh cargo test --lib -- retrieval::sync::tests::verify_does_not_count_files_the_indexer_cannot_represent_as_missing` passes on `experiments` at 1ca1495de. The test indexes a tree with `a.rs`, a zero-byte `empty.rs`, a newline-only `ws.rs` and a non-UTF-8 `bad.rs`, then asserts `missing_count == 0` and `expected_files == 1`. It covers every class in the three recurrences.

### Live check on a rebuilt server — 2026-11-12

The release binary `target/release/codescout` was rebuilt at 13:35:32 EET, after `1ca1495de` and the fix commits, and the server restarted at 13:35:38. On that server I built a scratch project with `src/a.rs` (content), `src/empty.rs` (zero bytes) and `src/newline.rs` (one `\n`), plus a `Cargo.toml`, then ran `index(action="build")` and `index(action="verify")`. The verdict was `complete`: `expected_files` 2, `stored_files` 2, `missing_count` 0, `orphan_count` 0. A binary without the fix would have counted `empty.rs` and `newline.rs` as expected and reported 2 missing, as the frozen binary did on CodeRank, Arrow and Dioxus. The scratch project is under the session scratchpad `verify-live/`.

I also ran `index(action="verify")` on this repo. It read `stale` with 5 missing files, all new files from the 3 commits the index was behind, and no empty file in the list. That result says nothing about the fix, because this repo has no indexable empty source file.

### What this does not show

I did not run the frozen binary `e6f07e9a6` on the same scratch tree, so the contrast rests on the three recorded reports, not on a paired run. I did not run the rebuilt binary on CodeRank, Arrow or Dioxus. The frozen binary is unchanged by decision, so the readiness gate keeps reading `incomplete` for it.

## Fix provenance

- **SHA:** `a34c4e2fe` (`experiments`)
- **patch-id:** `7ce287733f6b11803f27ff78d402ebc8b68e8b68`
- **Test:** `6d7557882` adds the non-UTF-8 exit of `indexer_can_represent`

## Resume

The tempdir test and the live scratch check both pass. What remains is optional: run a binary built at or after `a34c4e2fe` on the Dioxus tree and expect `verdict` complete with `expected_files` equal to `stored_files`. Do not delete the empty files to get that result.

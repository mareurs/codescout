---
id: '461849009868a998'
kind: report
status: complete
title: ECC navigation truth independent review — observed survivors closed
---

# Independent review — initial pass and survivor rerun

## Scope and verdict

Reviewed the uncommitted change to `src/tools/symbol/call_graph/mod.rs` in `.worktrees/ecc-navigation-truth`, baseline HEAD `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`. Reviewed file SHA256: `fc6db29690a4046265088239404bc641222148f122abc479d3220e8e22526c75`.

No production correctness defect was identified. The initial pass found two regression-coverage gaps; both are now CLOSED by the reviewed follow-up test and observed kills of the exact formerly surviving mutations. Review is approved within the targeted scope. The final reviewed file SHA256 is `d5b158f5e38d67119e0abb4ffa077e65b083f9dc8d2c439b965c04af713693c8`; the initial file hash above anchors the preserved before evidence.

The parent owns the full development gate and MCP smoke test. This review ran the targeted module suite, not the full gate. No production or test source was edited by the reviewer, and no commit, merge, activation change, or rollout was performed.

## Initial findings — closed by survivor rerun below

1. **Important — preserving a previous completeness warning is unguarded.** In `attach_analysis_failures`, replacing the existing-warning merge arm with `Some(_existing) => warning` actually survived: **35 passed, 0 failed**. This would erase the scan coverage warning whenever an analysis failure also exists. Existing tests cover each warning independently and did not reject their destructive combination. Add a case with both sources, asserting the scan gap and symbol plus analysis symbol, direction and reason remain in raw and compact output. The survivor was observed and reported to the parent before a new test was written.

2. **Minor — the new displayed-detail limit is unguarded.** Removing only the new helper's `.take(SHOWN)` actually survived: **35 passed, 0 failed**. The one-failure fixture cannot observe that list limit. Add more than five distinct failure scopes and assert the first five are present, omitted names are absent, the omitted suffix is accurate, and the reported total reflects the whole input set. This is a helper-level boundary guard; do not claim it proves an outer-tool multi-failure execution path. The survivor was observed and reported before new test work.

These initial findings were addressed after their survivors were observed. The follow-up review and measured reruns are recorded below.

## Baseline and mutation evidence

Baseline command: `env TMPDIR=/home/marius/.cache/codescout-ecc-tmp scripts/with-slot.sh --slot 2 cargo test --lib tools::symbol::call_graph::tests`.

Observed baseline: **35 passed; 0 failed; 0 ignored; 0 measured; 8594 filtered out**, exit 0. See [baseline-v2.json](baseline-v2.json).

Every applied candidate ran sequentially through `scripts/mutation-probe.sh --strict --file src/tools/symbol/call_graph/mod.rs --find <literal> --replace <literal> -- cargo test --lib tools::symbol::call_graph::tests`. The script carried the uncommitted source into its isolated worktree, verified a unique literal, applied the mutation, ran the actual suite, and restored the file. Builds used leased slot 2 and external TMPDIR. Exact literals, arguments, source hashes, return codes and logs are retained in [mutation-review-v2-results.json](mutation-review-v2-results.json) and [mutation-bound-v2-results.json](mutation-bound-v2-results.json).

| Candidate | Observed test result | Verdict | Evidence |
|---|---|---|---|
| m1: suppress recoverable-hop failure recording | 34 passed, 1 failed | KILLED | [log](m1-suppress-failure-record.log) |
| m2: remove outer tool warning attachment | 34 passed, 1 failed | KILLED | [log](m2-remove-outer-attachment.log) |
| m3: discard an existing scan warning when merging | 35 passed, 0 failed | SURVIVED | [log](m3-discard-existing-warning.log) |
| m4: emit analysis warning for an empty failure set | 33 passed, 2 failed | KILLED | [log](m4-warn-on-trustworthy-leaf.log) |
| m6: remove only the new helper's detail-list bound | 35 passed, 0 failed | SURVIVED | [log](m6-remove-detail-bound-unique.log) |

**Measured population: five successfully applied mutations, three killed and two survived.** Each ran 35 tests. m1 and m2 failed the new integration regression at the missing warning. m4 failed that regression's true-leaf assertion and the existing completed-walk silence case.

A separate m5 attempt matched `const SHOWN: usize = 5;` twice and was refused before arming, exit 2; no tests ran and no mutation verdict exists. It is excluded from the five applied mutations. The uniquely scoped m6 replacement corrected the instrument input. See [refusal log](m5-remove-detail-bound.log).

## Inspection and limits

Read the full diff, CLAUDE.md, testing discipline, error-handling / progressive-disclosure / workspace-state guides, and ADR-2026-08-27. Inspected resolver, formatter and relevant tests through semantic symbols; checked the new attachment's references. The actual resolver is in `src/tools/symbol/call_edges/resolver.rs`. The warning records the failed symbol, direction and reason while the error arm leaves other graph edges usable.

The added test uses a real Python parse and edge cache with a no-call-hierarchy mock, asserts the concrete `build → Marker` incoming edge, checks a genuine leaf stays warning-free, then asserts the class's failed callee analysis produces a warning and the compact formatter includes it.

The existing large-result test exercises scan-warning overflow. This review does not claim a new outer-tool scenario with simultaneous scan and analysis warnings has been run. The added paired helper test is described at its actual scope below.

## Initial-pass restoration

After all probes, the reviewed source and isolated source both hashed to `fc6db29690a4046265088239404bc641222148f122abc479d3220e8e22526c75`, and direct byte comparison was equal. The session's mutation marker was absent. Final git status matched the pre-state: only the existing source modification and two native-provenance untracked files. Slot 2 was released before the parent was told it could edit the tests or build.

## Survivor rerun — final verdict

The parent added `analysis_warnings_preserve_scan_scope_and_bound_details_without_losing_total` after the two survivals. Direct comparison against the restored initial source showed only this new test and rustfmt layout of an existing assertion; production code was unchanged.

The test calls the actual scan-warning helper with one unplaced symbol and a cut-short-at-three coverage record, verifies empty analysis failures leave the entire result unchanged, then adds seven distinct failure descriptions. It asserts preserved scan-warning prefix and metadata, actual total seven, all first-five symbol/direction/reason descriptions, absent sixth/seventh names, accurate `(+2 more)`, and propagation of the entire combined warning through compact formatting. This is helper-level coverage; the existing outer-tool regression independently exercises actual parsing/cache, aggregation and attachment.

The reviewer independently reran the clean suite: **36 passed, 0 failed**, exit 0. See [baseline-v3.log](baseline-v3.log). The exact m3/m6 commands from the before records were then reused sequentially through the strict isolated mutation script, with no source edits by the reviewer:

| Former survivor | Before new test | After new test | Failing observer |
|---|---|---|---|
| m3: discard existing scan warning | SURVIVED, 35 passed / 0 failed | KILLED, 35 passed / 1 failed | New helper test rejects missing scan-warning prefix |
| m6: remove detail-list bound | SURVIVED, 35 passed / 0 failed | KILLED, 35 passed / 1 failed | New helper test rejects displayed hop_5 / hop_6 |

Both reruns executed 36 tests and failed only the new helper regression. See [m3 rerun log](m3-discard-existing-warning-v3.log), [m6 rerun log](m6-remove-detail-bound-unique-v3.log), and [exact rerun records](mutation-review-v3-results.json).

**Rerun population: two applied mutations, two killed, zero survived.** The initial five-candidate population remains separately recorded above; the three initial kills were not rerun after the test-only addition. No claim of five mutations applied to the final hash is made.

Final source and isolated source both hashed to `d5b158f5e38d67119e0abb4ffa077e65b083f9dc8d2c439b965c04af713693c8`; direct byte comparison was equal. The session mutation marker was absent and git status matched the pre-state. Slot 2 was released before notifying the parent to run the full gate. Both review findings are closed; full-gate and MCP validation remain the parent's responsibility.

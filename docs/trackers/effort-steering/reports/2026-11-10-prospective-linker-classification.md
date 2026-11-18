---
id: '04a93e5c27efe31d'
kind: report
status: done
title: 'Prospective ordinary-linker classification: native controls and bounded review correction'
tags:
- effort-steering
- evaluation
- prospective-policy
topic: effort-steering
---

# Prospective ordinary-linker classification

## Current result — 2026-11-10

**F6 is locally qualified for the prospective evaluator. Parent research goal remains open; paid admission remains closed.** New complete-stream policy identity is semantic-stream-contract-v2. A failed command with an ordinary, canonical top-level Rust linker diagnostic becomes compile_failure. Captured payload, ambiguous provenance, successful commands and malformed headers cannot supply that affirmative evidence. Infrastructure diagnostics retain precedence.

Only grade_policy.py and test_stream_policy.py changed against this slice's preserved 99-file WIP baseline. compiler_diagnostic, adjudicate_private and scoped_lines are independently checked source-byte equal. Historical tail replay and every old grade remain unchanged. No source commit, merge or push.

## Actual native controls

Generated dependency-free valid and undefined-symbol crates ran on both exact frozen images. These are compiler/runner instrument controls, not benchmark repairs.

| Image / runner | Valid crate | Undefined symbol |
|---|---|---|
| Obscura image b22988cd6022…, cargo-nextest 0.9.128 | return 0, one selected pass, policy pass | return 101, no selected status, compile_failure |
| Lean image b72f902d24c0…, rustc 1.97.1 / cargo test | return 0, one selected pass, policy pass | return 101, no selected status, compile_failure |

Each command has two actual memory.events probes, oom_kill delta zero, registered resource inspection and fresh owned-container absence. v2 adjudication occurs after closure. Earlier v1 runtime flags were synthetic replay inputs and are retained with that limitation. After correction, all four original complete captures replay to byte-equivalent verdicts; no second compiler/container run was needed.

## Independent review and correction

One fresh strongest reviewer verified the sealed full source plus exact two-file delta, ran 21 prospective and 49 historical tests, then actually applied 12 guard mutations. **Initial result: 10 killed, 2 survived, zero invalid or timeout.** The original result is preserved.

- F6-R1, Important: non-LF separators inside a physical source-excerpt line could expose a fake linker header. Parent independently reproduced the consuming result and immediately filed issue bf0d019daf7cc5ef. New consuming tests observed RED. The prospective linker helper now preserves CRLF and prevents other splitlines separators from creating headers; the named historical helpers remain unchanged.
- F6-R2, Important test gap: removing the whitespace-only-command guard survived. Parent read the exact mutant diff, successful test log and differential probe before writing regressions. Exact replay with the new tests is killed by 10 targeted failures in 73 tests.
- F6-R3, Minor endpoint gap: excluding accepted exit127 survived. The explicit 1..127 contract is now pinned at 127/128. Exact replay is killed by two targeted failures in 73 tests. Production range did not change.

The initial new-test RED reported 24 tests and 66 failures. Two assertions incorrectly demanded pass for bare CR, which the preexisting complete-stream contract already refuses; that test-instrument mistake and the failed first green are retained. The corrected expectation preserves the stricter runtime rule. Final policy suite: **73 tests, OK**. No second review was requested.

## Final verification

| Verification | Observed result |
|---|---|
| Direct tools/setup-ab-eval suite | 1,392 tests, OK, 256.711 seconds |
| Project gate, final source | exit 0, 125 seconds; FMT/CLIPPY/LEAN/DEFAULT/PYTHON_LIGHT all 0 |
| Gate Python light | 1,765 passed, one skipped, 554 subtests passed |
| Gate Rust lean / default | 6,318 passed / 48 skipped; 9,215 passed / 62 skipped |
| Exact prior survivor replays | two killed, zero survived/invalid/timeout |
| Retained native capture replay | four prior verdicts equal |

The gate excludes three heavy Python suites, server-stack tests, librarian in the lean build and local ONNX runtime tests. The direct evaluator suite is separate and is not inferred from the gate.

## Custody

[Checked closure](../evidence/prospective-linker-20261110/closure.json) and [independent readback](../evidence/prospective-linker-20261110/independent-readback.json) bind final-evidence.tar.gz: **11,022,876 bytes**, SHA-256 **06c45de573b8b249b23c3d3585eb8fdf9ccaf281e161c1e4d8bb90eaabe62f3d**.

The archive reopened all **1,949 regular payloads plus its manifest**, including 99 source files, baseline and final diffs, original native inputs/full streams/runtime/closure, initial review and all mutants, bounded correction, final tests/gate and 11 terminal producer records. Independent readback found no duplicate or nonregular member.

The archive is a snapshot: its progress ledger precedes task-done, and its own redirected sealing stdout was open at sealing. The adjacent closure and independent-readback receipts contain the final packaging result; no claim is made that this self-produced output was already inside its own archive. Current source was checked byte-equal at sealing. Task 2 subsequently completed with the independent readback checker.

## Implementation rulings and limits

1. Reuse the isolated grader worktree and preserve all prior WIP. If wrong, local qualification refuses; no payment follows.
2. Standing autonomy settles routine design choices without repeated approval. Changes remain unpublished and reversible.
3. Apply new semantics only through prospective v2; keep historical helpers and results unchanged. A wrong binding must fail admission.
4. Retain the publication hold and scratch instead of the skill's default commit/delete steps. Integration stays outstanding.
5. Accept canonical ordinary header statuses 1..127; exclude encoded137/139 from this new recognition. F7 remains a separate policy question.
6. Distinguish v1 replay flags from v2 measured cgroups and actual closure. Only v2 supports the native runtime claim.
7. Review the sealed WIP and exact baseline delta, because HEAD..HEAD is empty under the hold. An empty commit range is not a review.
8. Accept R1/R2 and close them in one bounded pass. Include the measured endpoint gap as completion of the explicit boundary contract; cost was one small extra mutant replay, with no production change.
9. Preserve F3 missing-selected-status semantics and generic F7 encoded-signal attribution. They are not implicitly approved or repaired by this slice.
10. Prior complete-stream architecture, outer admission, exhaustive terminal encodings, independent native reruns, repair efficacy and publication were outside this review. Parent admission must validate the actual integrated path separately.

No new provider/model/author/onboarding call. Historical known client subtotal remains 64.401093499999998654 USD, not freshly recomputed here; the unresolved 4 USD hold stays carried under the 200 USD series ceiling.

## Next

Integrate the exact prospective evaluator with the actual fixed eight-attempt controller. Qualify full P/R phases, accounting, resource floors and immutable registration in the common runtime, then perform the already required whole-admission review and no-cost preflight. Only that can admit the L/R/P/X repair pilot. These compiler controls establish no low-versus-xhigh efficacy result.

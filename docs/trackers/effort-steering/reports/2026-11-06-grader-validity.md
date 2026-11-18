---
id: 9574b5ab98a5ea84
kind: report
status: done
title: 'Grader validity: committed classifier and retrospective Rust-status audit'
tags:
- research
- effort-steering
- grader
---

# Grader validity closure — 2026-11-06

The parent took over after the user relayed that the peer had run205 baseline tests but made no edits or commit. The bounded classifier and retrospective audit are now committed and verified; a prospective live oracle is still unadmitted.

## Source and behavior

Commit `a2ffc143266c5e828afd7d8ef2ceb665318df49f` on `fix/grader-validity-20261106`; stable patch-id `6d3afca0f9ca24e2ccc9032c5aea79f418b64b7c`. Worktree /home/marius/work/claude/codescout.worktrees/grader-validity is clean; eight committed source/test/fixture bytes match the verified hashes. No merge/push.

live_grader now marks anchored process-fatal diagnostics plus zero parsed statuses as infra=toolchain_crash with a truthy error understood by the old wrapper. It preserves raw counts. Normal candidate compiler errors stay functional failures; logs with some parsed statuses retain the existing ordinary-grade path. Error wording reports parser observations, not an unmeasured assertion that no test executed. Signal text does not establish disk/RAM/LLVM cause.

audit_reparse.py is retrospective only. It uses the record parser unchanged, recovers expected binary/test identities attributable to the current physical split, and preserves original failures. It neither modifies a frozen record nor becomes the primary live oracle.

## Tests and independent review

Baseline205tests/69subtests passed. Classifier helper first failed as missing; initial live boundary fixture exposed its own simplistic-parser assumption. The wrong-reason red is preserved; wiring was removed, fixture corrected, intended missing-infra red observed, then wiring reapplied (F-213). Audit module missing-import red and subsequent no-op4assertionfailures/4passes are preserved.

Original changed suite220tests/79subtests passed. One fresh independent review froze and actually applied12 mutations:8killed/4survived. It also reproduced ordinary compiler source text falsely becoming a crash and cumulative-prefix line misattribution. MAIN b1e1f89b7d587cb8,73efe0aa746d30f9,77828ecdbae3b8c2 were filed immediately.

One offline fix pass: new counterexample tests first observed6 assertion/subtest failures; final changed suite227tests/85subtests passed. The four original audit survivors M06/P2P,M07/standalone failure guard,M09/terminal suffix,M10/ANSI prefix were all reapplied and killed.

Final mutation accounting:11 original literals applied; oldM02 literal absent because observation wording changed, explicitly resited as M02-v2.12 applied cases total:11killed/1survived. M04 removes a test-name exclusion already implied by the new anchored regex. Four scratch counterfactual cells actually observe fixedregex guard/no-guard PASS/PASS, oldregex guard/no-guard PASS/FAIL on the same targeted test. This is observed redundancy on that overlap, not a12-kill claim or independent coverage claim. No second review/fix loop.

Full final gate12a CLOSEDexit0: Pythonlight1642tests/529subtests; FMT/CLIPPY/LEAN/DEFAULT0. The targeted227 and the gate1642 are separate populations, not a grand total. Gate scope exclusions remain heavyPython,server-stack,localONNXweighttests and librarian in lean. D1–D5 explicit review rulings are archived in analysis/rulings.md.

## Historical audit calibration

[First audit](../../../research/data/2026-11-06-grader-validity-audit.json) SHA2221fee48b54ee56e0040be7bbd4676f802494747c6ad41b4b72af6bf70657b4 is retained. [Final-source audit](../../../research/data/2026-11-06-grader-validity-audit-v2.json) reproduces the same four rows and17protectedunique input hashes.

- NUlow:0parsedstatuskeys plus linkerSIGBUS; original grade is infrastructure-invalid. The already-closed unchanged-patch diagnostic separately rejects bothF2P while8620P2Ppass/missing0; it was not repeated.
- NUxhigh:8724parsedkeys; one expected P2P recovered at physical line3725; missing1to0/notpassing2to1. The genuine mixed-string regression remains.
- RAxhigh/low:8194/8193parsedkeys respectively,8182selectedexpectednames each, both expected-set verdicts pass.

These are different measured populations. Raw grades, logs, candidates, bundles, records and original results remain unchanged. No new model/provider call or native regrade in this correction.

## Durable evidence and remaining work

[Evidence archive](../../../research/data/2026-11-06-grader-validity-evidence.tar.gz) SHA9b992b4d7841f54a42a88323ef9cdbe169a87e9db839c71613720a6018fcf422,1694700bytes,189payloads plus manifest, all reopened and hash-verified.144reviewmanifest payloads rehashed at closure. [Receipt](../../../research/data/2026-11-06-grader-validity-evidence.tar.json). ROOT /home/marius/.cache/codescout-resident/codescout/grader-validity-20261106. Plan144495d0b8e0f23e bounded closure done.

54fe4b112aa4e423 and the three review issues are fixed. Dataset/live glued-parser issue0a2929ec2a4f688f remains mitigated: the historical interpretation is corrected, but a future normalization/adjudication policy still needs explicit registration and fresh two-task empty/gold/repeat controls. Then bind a concrete paid adapter and finite resource-stop behavior before the fixed six-arm pilot. No new mechanism efficacy/low-xhigh parity conclusion follows.

MAIN research documents and artifacts are separate uncommitted work; the eight-file source commit does not include them. Foreign MAIN edits/index were left alone. Source commit and archive preserve this bounded correction without treating unrelated MAIN gate109 failure as green.

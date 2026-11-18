---
id: d1af57772d54fc0e
kind: report
status: active
title: Prospective policy review corrections and applied guard verification — 2026-11-10
tags:
- effort-steering
- evaluation
- review-corrections
topic: effort-steering
---

# Prospective policy review corrections

## Current state

**Superseded for readiness by the second review on2026-11-10: changes required.** The green suites below remain historical test evidence, not admission. Independent replay on unchanged f586d35f4 confirms cut-stream, legacy-section, ANSI-boundary and multiple-print counterexamples. New investigation is isolated on fix/grader-stream-provenance-20261110; no source correction or new efficacy result yet. See the [native stream-provenance investigation](2026-11-10-grader-stream-provenance.md), including actual task-image nextest 0.9.128 and complete-versus-cut stderr adjudication. F3 remains unchanged.

F1/F2/F4/F5 corrections are committed locally as f586d35f4b0d41768931648dd1a9a33f1cdd450f on branch fix/grader-policy-review-20261110, based on reviewed commit 23bd63dca. Stable patch-id: 53c4b6e6027c6135823b4fc1b0994c1a719b7301. Source blobs and every archived evidence member were verified again after the commit. The final five related suites pass 293 tests / 180 subtests in 33.26 seconds. Full product gate exited 0 in 549 seconds (script TOTAL=548): Python light 1765 passed, 1 skipped, 554 subtests; FMT/CLIPPY/LEAN/DEFAULT all exit 0. These Python populations are separate.

No main merge, push, paid/model call, private candidate regrade or historical receipt rewrite. This is a measurement-infrastructure correction, not a new efficacy result for low. The existing frozen studies retain their source and outcomes. Repository imports alone do not prove how every external historical wrapper used the old module; this pass makes no affected/unaffected claim about those old outcomes.

## Confirmed review cases

| Finding | Observed before | Correction and observed check |
|---|---|---|
| F1: separate nextest stream | Reviewer's legacy captured STDERR tail made a selected pass compile_failure. | Shared capture scope recognizes the legacy header and the actual nextest 0.9.150 indented output format. Actual JSON stdout plus separate stderr now preserves either selected pass or selected test failure. |
| F1: UTF-8 tail boundary | The real control decoder cuts the last 4096 bytes inside a two-byte character; U+FFFD defeated suffix context and promoted captured text to compile_failure. | Associate the decoded suffix with the enclosing printed-log scope when possible, including the replacement prefix. The first accidentally single-line fixture is retained as an uninformative control; the corrected real-decoder reproduction was RED then GREEN. |
| F2: runtime-looking test data | Captured storage, permission and rustc-fatal strings erased a genuine selected failure. | Both compiler and infrastructure consumers retain test-output scope; framed JSON test payloads are data. Independent streams are scanned independently. Genuine top-level storage/fatal diagnostics still invalidate. |
| F4: malformed mappings | Direct raw=None/string/list and record=list calls raised AttributeError. Nested bundle.expected None/string/list also raised after the root-only fix. | Explicit root and expected-mapping validation returns invalid; no broad catch hides arbitrary AttributeError. Direct tests avoid the helper that silently substitutes raw=None. |
| F5: optional rebuild | None was rejected although the native executor treats it as no rebuild. | Missing/null/empty/blank rebuild metadata matches executor normalization; a genuinely required rebuild remains required. |

## Native characterization

The dependency-free generated crate was actually run with installed cargo-nextest 0.9.150 and libtest-json-plus. One test passes and one deliberately panics while printing compiler/storage/permission/fatal-looking strings. Returncode 100, independent stdout/stderr and hashes are retained. This characterizes one installed runtime, not all nextest versions. The current format is an indented output block; the review's legacy header case remains a separately labelled compatibility fixture.

The actual Rustc 1.99.1 ordinary-link failure control confirms F6: its linker diagnostic has no classified source span and a synthetic rebuild receipt remains invalid. This is a real host diagnostic plus a synthetic policy input, not a native private grade.

## Applied mutation results

Every application uses a fresh independent source copy and actual pytest execution. Syntax/collection failures and nonmatching patterns are not kills.

- Initial original catalog on immutable 23bd63dca: 48 applied, 31 killed / 17 survived, 0 invalid / 0 not applied. This differs from the peer's reported 33/15. Keep both claims; our full logs are evidence for our run.
- The extended runner was calibrated on three unchanged base-source overlap cases; all three outcomes matched the original runner.
- After the first guard regressions: 59 applied (48 original plus 11 new helper/call-site mutations), 54 killed / 5 survived. N10, removing the rebuild context boundary in the new helper, actually survived before its regression was written.
- Final replay on corrected source: 59 applied, 55 killed / 4 survived, 0 invalid / 0 not applied. The original 48 contribute 44 killed / 4 survived; all 11 new mutations are killed. Three moved original guards have explicit semantic mappings, and N6 has a source-version mapping after nested validation. No unmatched edit is counted as applied.
- The new N10 regression then killed that exact mutation by showing why a failed rebuild must retain its own compiler context even when identical diagnostic text appears in captured test output.

Regressions pin cgroup=1 and floating 2.0, storage/permission source gutters, coherent duplicate/empty expected sets, full verdict contradiction, returned recovered statuses, unknown parser strings, a real nondeterministic parser, successful-step compiler text, the storage suffix anchor, negative/boolean test-step returncodes and blank rebuilds. Returned statuses and valid/invalid outcome assertions reach the intended consumer boundary.

## Remaining survivor interpretation

The four survivors are retained, not falsely called killed. Verdict equivalence is narrower than byte-identical API results; error wording may change.

| Mutation | Why it repeats another admitted-JSON boundary |
|---|---|
| M36: initial raw-pass bool check | The later typed raw-versus-parser verdict comparison still rejects non-bool pass values. |
| M42: tail type check | Every step now reaches the context helper; missing tails raise KeyError and non-string tails raise TypeError, both classified invalid by the existing boundary. This redundancy appeared after the scoping refactor; the old source killed this mutant. |
| M43: empty-tail context branch | For an empty suffix, using the full log repeats the pure full-log fallback and infrastructure scan without changing the verdict. |
| G4: force pass=false in no-status crash path | The immediately preceding verdict of an empty status mapping is already false for every expected-set size. |

Code reasoning was checked against 24 finite JSON input pairs, with zero mismatch in valid/pass/kind/statuses/error-presence, plus four actual empty-status verdict controls for G4. This finite matrix is evidence for those cases, not a universal equivalence measurement. Guards remain in place; no assertion on error wording was added merely to inflate a mutation score.

## Deferred decisions and limits

- F3 is unchanged: missing expected statuses without compiler evidence remain invalid, including the candidate-deletion/abort concern. This correction authorizes neither dropping nor selectively rerunning those trials. The operator must settle reporting/admission treatment before it can support efficacy claims.
- F6 is recorded as a low-severity coverage limitation. The actual host linker control is now retained; no recognition widening is included in this requested bounded pass. It fails closed and awards no pass.
- F7 is unchanged: negative process codes invalidate; shell-encoded 137/139 remain functional failures unless other evidence invalidates them. A code alone does not establish process origin. No policy decision is inferred.
- Framed libtest, characterized nextest and JSON test payloads are covered. Arbitrary unframed/nocapture output, missing stream boundaries and all runtime versions are not qualified by these fixtures. Source-bound classification remains bounded evidence, not proof of executable provenance.
- The root product gate does not collect tools/setup-ab-eval tests; report its result separately and retain its printed heavy/feature/ONNX exclusions.
- This is the single correction pass following the supplied independent Opus review. No new independent approval of the resulting commit is claimed.

## Evidence custody

Committed [member manifest](../evidence/policy-review-corrections-20261110/manifest.json) and [verification capsule](../evidence/policy-review-corrections-20261110/verification.tar.gz): 767380 bytes, SHA-256 8c2803c67a60168bca2f99d29bc1ed1d126b6136e0ad83589ff290656159dd22. All 232 members were reopened and checked against recorded sizes/hashes, then rechecked after commit. The receipt names actual commands, exits, raw red/green logs, native fixtures, source hashes, initial/calibration/final mutation populations and finite equivalence controls. Peer scripts are preserved as review data; their co-author trailer does not claim approval of this correction.

All 5823 tracked entries matched before/after the final gate; exact tested patch SHA-256 is 9a1e188ac774ef394f8ffb49792341e39375e27dbf05e667bebf511ed58d28b4. The capsule and this final documentation were added after that source gate. Fresh scratch remains /home/marius/.cache/codescout-resident/codescout/grader-policy-review-20261110. Original review source, preservation/integration commits, add2 and old capsule remain intact. The [correction plan](../plans/2026-11-10-policy-review-corrections.md) records execution and constraints. Main integration and the deferred policy decisions are separate.

---
id: '97aace06d095c0d9'
kind: report
status: done
title: 'ECC pilot on real doc(create): same finding, integration exercised'
tags:
- ecc
- evaluation
- doc-create
topic: ecc-integration
---

# Real doc(create) comparison — 2026-11-09

## Outcome

The two installed ECC adapter skills executed successfully alongside the existing default-profile plugins on real codescout source. Both native runs used successful codescout symbols, references and call_graph calls. The explicit ECC run read both installed skill bodies in a completed command with exit 0; the baseline's completed requests/commands contained no adapter-body reads.

Both reports independently identified the **same conditional cleanup-reporting defect**. No additional confirmed finding was established by ECC in this pair. The result supports bounded coexistence/application, not a quality or cost improvement.

## Scope and primary records

Source: /home/marius/work/claude/codescout, recorded starting HEAD 6043fb27aeb21eb03c52238f2af3c189a97c16e9 in a dirty shared checkout. Scope: create::call, temporary-workspace guard, nearest dispatcher/error conversion and selected tests. Native CLI: 0.167.0; default profile model settings: gpt-6.1-sol, medium, default service tier.

- [Baseline final response](2026-11-09-doc-create-baseline.md)
- [ECC final response](2026-11-09-doc-create-ecc.md)
- [Observed-behavior draft](2026-11-09-doc-create-observed.md)
- [Machine report](../evidence/2026-11-09-doc-create/report.json)
- [Runtime settings](../evidence/2026-11-09-doc-create/runtime.json)
- [Evidence hashes](../evidence/2026-11-09-doc-create/manifest.json)
- Raw native JSONL/final-message/stderr gzip files, prompts, runner text, before/after source hashes and native preflight results are retained in that same evidence directory. Compressed raw Markdown preserves exact native responses; catalogued copies add capture labels.

## Adjudication

| Property | Existing workflow without adapter-body use | Explicit ECC invocation |
|---|---|---|
| Temporary-root refusal/admission | Correct bounded predicate and admission table | Correct WHEN/THEN requirement with admission qualification |
| Recoverable protocol | Distinguishes transport success from ok:false | Same distinction, anchored to bridge and host routing |
| Cleanup | Describes best-effort attempts, not guaranteed rollback | Same distinction with per-requirement evidence state |
| Confirmed finding | Conditional false rollback assurance | Same conditional false rollback assurance |
| Partial state/atomicity | Contract ambiguity, not forced into a defect | Same classification |
| Tests | Inspected assertions; no execution claimed | Inspected assertions; no execution claimed |
| Scope/provenance | Explicit inspected boundary and navigation limits | Explicit boundary, observed HEAD/dirty state and per-requirement evidence |

The controller read create::call, undo_failed_create, artifact::delete, guard predicates, nearest dispatcher/adapter, select_error_render and relevant cleanup/guard tests. These bodies support one diagnostic defect: a write failure invokes cleanup that may only warn on delete failure, yet the returned error always asserts the row was taken back. Create still fails; the finding is not false successful creation. [Code bug recorded separately](../../../issues/archive/2026-11-09-doc-create-claims-rollback-after-best-effort-cleanup.md).

Canonicalization fallback, unknown catalog-location admission, audit/GC enrichment and partial completion remain explicitly bounded implementation behavior or open contract/reachability questions. No cleanup-delete failure was injected. Neither an atomic-create contract nor an accuracy denominator was invented.

## Native measurements

Each row below is the single terminal turn.completed usage record for one subprocess. Input counters aggregate native input processing across the turn's requests; they are not one prompt/context-window size. Cached input is reported separately as a subset and is not added again.

| Run | Elapsed subprocess seconds | input_tokens | cached_input_tokens | output_tokens | reasoning_output_tokens |
|---|---:|---:|---:|---:|---:|
| Baseline | 245.551 | 700012 | 625664 | 5934 | 486 |
| Explicit ECC | 329.080 | 1071452 | 989184 | 7871 | 1213 |

The ECC invocation used larger native counters in this observed pair. No monetary price was calculated. Fixed ordering, warm caches/LSP, shared tool services, stochastic output and concurrent verification prevent attributing elapsed differences to ECC. The parent gate overlapped the pilot phase; these times are descriptive, not a speed benchmark.

## Control and source limits

The first attempted per-command plugin enabled=false control failed before model evaluation: skills/list still returned both skills enabled. That cause was not established. The completed comparison instead leaves installed metadata enabled in both arms and compares explicit body use with requested non-use. It is not installation versus absence.

Before/after snapshots detected changes in src/tools/core/cap_probe.rs and src/tools/native_provenance_fence/holders.rs. The create, guard, librarian adapter, artifact dispatcher and server enforcement files were byte-identical across the pair. Neither changed path was explicitly named in completed requests, which does not prove that a broad tool request could never include it. **The strict whole-checkout unchanged control did not hold.** Scoped source stability supports inspection of this boundary; this is not a fully frozen causal comparison. HEAD also advanced in the shared checkout by the controller's later inspection, so use retained file hashes rather than HEAD alone.

Profile config hashes matched before and after; default-profile settings were not changed by these evaluations.

## Repository verification

The controller separately ran ./scripts/gate.sh --check. The background job exited 0: FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, FMT_STATE=passed; Python reported 1764 passed, 1 skipped and 554 subtests. [Gate record](../evidence/2026-11-09-doc-create/gate-record.json) and gate.log.gz retain the run.

LEAN excludes librarian; DEFAULT is the relevant Rust lane. The gate itself excludes heavy Python, server-stack and execution of the local ONNX path. This green shared-checkout gate is not reproduction of the conditional cleanup defect. New tracker documents were authored after the gate and receive separate link/frontmatter checks; no Rust implementation changed for this evaluation.

## Decision and next evidence

Keep the user-enabled two-skill adapter available for explicit optional use. The pilot supplies a useful structured spec/report shape, but this case showed no additional confirmed defect relative to the existing workflow. General effectiveness, false-positive rate, cost savings and reliability remain unmeasured. A future discriminating cohort needs a stable source snapshot, a qualified control and several cases with independently adjudicated outcomes.

No code fix, source mutation campaign, full ECC installation, commit, merge or push is part of this result.

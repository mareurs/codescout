---
id: d433f009630e6de8
kind: research-note
status: archived
title: 'Historical: Bounded topology simplification of recorded low tests'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-02-case-simplification.md
source_body_sha256: e5fc0d2313a112f2e82a76535ea9a6d9cbb0f86b4d810f31abe78e4987bf1658
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-02-case-simplification.md
  sha256: '5f3fb23d1ca0362474413f95e4eefafc4007f1e553c021482d844e51038fc634'
  body_sha256: e5fc0d2313a112f2e82a76535ea9a6d9cbb0f86b4d810f31abe78e4987bf1658
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-02-case-simplification.md
  sha256: '5f3fb23d1ca0362474413f95e4eefafc4007f1e553c021482d844e51038fc634'
  body_sha256: e5fc0d2313a112f2e82a76535ea9a6d9cbb0f86b4d810f31abe78e4987bf1658
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-02-case-simplification.md

---


# Bounded topology simplification of recorded low tests

## Question and fixed procedure

**Valid:** dated 2026-11-02

The preceding [execution-witness diagnostic](2026-11-02-case-witness.md) showed a wrong MatchedPath state cleared before two generated probes observed it. This development spike tests an executable variant-generation operation: remove one composition layer while retaining the test's request and assertions.

Exactly two recorded low-generated tests are inputs: the generic challenger and the structured contract-to-assertion challenger. For each, replace the unique Router::new().nest_service("/", SERVICE) expression with the original SERVICE. All bytes outside that expression remain identical. No expected extension value, new assertion, defect location or fallback implementation name is inserted.

This is a bounded Axum-specific exact-byte adapter. It is not a general Rust parser or a claim that removing a wrapper preserves semantics. The test's unchanged oracle must pass on both fixed xhigh and gold before a candidate failure counts as detected. Reference/gold failures and compile errors remain outcomes. There is one frozen variant per input, no outcome-driven revision.

## Evidence and execution

Private root /home/marius/.cache/codescout-research/codescout/case-simplification-20261102 holds the registration, frozen script/fixture hashes, copied inputs, generated variants and evaluator outputs. The registration and generated identities were frozen before evaluation.

The existing unchanged evaluator lists and executes each variant on candidate, fixed xhigh reference and gold. All six registered grades completed; each listed and executed exactly one test with none ignored. The prior sealed quiet baseline grades are comparators, not newly generated model samples. Binding failure stops the spike.

## Results

| Recorded seed and observation | Original candidate / xhigh / gold | Simplified candidate / xhigh / gold | Detected |
|---|---|---|---|
| Generic challenger, status-only assertion on fallback | Pass / Pass / Pass | Pass / Pass / Pass | No |
| Structured challenger, response-body assertion | Pass / Pass / Pass | Fail / Pass / Pass | Yes |

The structured variant fails its unchanged body assertion: observed served:/src/main.rs:true versus expected served:/src/main.rs:false. It passes the status assertion before that failure. This is a behavioral failure in the relevant observation, not a compile failure or timeout. Reference and gold pass the exact same variant.

Thus one generated low test becomes a detecting test after a mechanical change confined to its API composition. The generic seed remains a miss: its direct fallback case only asserts status200 and leaves the MatchedPath body unasserted. No new assertion, expected value or request was supplied by the operator.

Frozen input → variant SHA256:

- Generic: c83f879da6ae91014d6b75b04b572d5c4f715748038f8eeff43d2dddcd17b592 → 528fd262491578fff067f441bda45a6324418152a73af6a5916c12b6bfa07350.
- Structured: 7b819a124698739678ba38a7c21eeddeb93e2ca2883873843203df40be0e715c → 2024b9460a9f3912e12c22d7dbaa558094fb7cc360104b0d7f1a956438905617.

## Interpretation and next gate

This supplies an executable detector for the exposed development case: the existing structured assertion plus one composition variant. Topology variation alone did not make the generic status-only oracle useful. The two seeds differ in other ways too, so this is not a randomized isolation of the prompt's causal effect.

The adapter is selected after the previous trace diagnosis and is deliberately specific to these Axum expressions. It must not be presented as an automatically learned or outcome-unseen selector. Reference/gold are independent evaluation labels; they are not inputs to generation, and a future deployed low workflow cannot assume access to their solutions.

Next, freeze the portable rule and bound: from the task's documented invariant, test both the immediate component and the composed public-API path; preserve the invariant's terminal observation; generate at most one independently justified simplification per selected composition before observing outcomes. On another task, choose the adapter and oracle from its public contract before inspecting candidate/reference implementations or outcomes. Record unsupported/invalid cases and construction/validation/repair costs.

A prospective matched workflow must additionally settle false-positive handling and bounded repair/escalation without consulting gold at runtime. Only that evaluation can support transfer, blind discovery, low/xhigh parity or whole-workflow cost superiority. No further paid wording retry or repair was made on this exposed case.

## Validation and cost

Three focused transformation guard tests passed. They verify the exact changed expression, reverse reconstruction of the real inputs, and refusal of absent/duplicate/unregistered inputs. Exact outside-span equality, rather than assertion-token counts, establishes preservation of requests and assertions.

A separate post-run comparison verifies all six source inventories against their quiet baselines: the review test is the only changed file in every pair. The frozen generator/registration hashes and exact private fixture patches revalidate; the literal failed assertion bytes decode to the reported response body. The prior witness seal and nested contract/assertion closures revalidate unchanged.

No model call, credential read, paid retry or new paid phase. Campaign model-client estimate remains24.269845199999999810 of40; CPU/native orchestration dollars are unmeasured.

Project runtime/test source is unchanged. The preceding full gate retained its documented known lean timeout-test failure, passing in exact isolated rerun; see the preceding report for its raw evidence and the separately fixed bug. It is not reported green here.

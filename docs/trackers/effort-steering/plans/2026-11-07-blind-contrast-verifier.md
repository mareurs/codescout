---
id: 8d91311c0a8a4ff3
kind: plan
status: done
title: 'Blind contrast verifier: discriminate saved wrong fixes from a correct repair'
tags:
- effort-steering
- evaluation-integrity
---

# Blind contrast verifier

**Goal:** Determine whether a fresh low-effort verifier can discover an executable counterexample to a plausible repair from public information alone.

**Architecture:** Reuse the clean adapter worktree at `fe8bb4b34f5e7912f9b16c4c0bf1ff73a3a10a23` without modifying it. Clone the existing source-only onboarding snapshot once into new resident scratch. A fresh Sonnet 5.5/low session receives the public task, public source and the saved low-control proposal. It may add tests, but must leave production and existing tests unchanged. Freeze its output before evaluating those tests against baseline, the three saved wrong proposals and the reference repair. Evaluation inputs never enter the verifier container or prompt.

**Tech stack:** Existing native Codescout/Claude CLI adapter, isolated Docker filesystem, cached Rust toolchain, Python one-shot orchestration. No product behavior changes.

**Design authority:** Operator's continue instruction after diagnosis bfb2a49230efdf94; the controller chooses this bounded development probe. This exposed Nushell task is a development control, not a held-out efficacy sample.

## Global constraints

- One fresh low verifier attempt, no retry based on private results. Reserve $4, CLI cap $3 within the standing $200 series; prior known subtotal $61.746769099999999054. Unknown cost stops further calls.
- Public source-only onboarding and global/project instructions remain active. No repeated onboarding, private test patch, reference solution, grading log, diagnosis, prior actor transcript or outcome label in actor context.
- Do not modify old runs, frozen registrations, the six staged grader files, foreign checkout/index changes, or peer branches. No merge/push.
- A green workflow hygiene command is not detector evidence. Judge only generated assertions executed on all evaluation variants after freezing.
- New confirmed issues go immediately into MAIN docs/issues. New scratch lives outside the repository via resident-scratch convention.

## Review focus

1. **Input boundary:** Enumerate actual image additions/mounts and cloned guidance; absence of private paths in prompt alone is insufficient.
2. **Binding:** Hash proposal, prompt, source, generated patch and evaluation variants; no changing the test after seeing private outcomes.
3. **Assertions:** Require real named test statuses; compilation errors, zero selected tests, resource failures and missing outcomes are invalid, not rejection.
4. **Controls:** A useful detector rejects wrong repairs while retaining the reference repair; baseline outcome provides interpretation, not an automatic success criterion.
5. **Claims/accounting:** Three wrong patches share one exposed task/error family. Report cell outcomes and marginal verifier cost; do not claim population improvement or complete-workflow savings.

## Tasks

- [x] 1. Prepare the public-only verifier packet using the unchanged adapter and a new disposable image/state clone. Inspect actual packet/mounts, instruction fidelity and resource headroom; freeze inputs before spending.
- [x] 2. Run one fresh low verifier. Preserve/cost-reconcile output and cleanup evidence. Freeze generated tests and verify only permitted test additions occurred.
- [x] 3. Execute frozen generated assertions on baseline, each saved proposal and the reference. Report discrimination, limitations, cost, and the next research decision; update current memory.

## Stop and decision rules

Do not launch if the input boundary or runtime cannot be established. Do not silently retry an invalid attempt. A mechanism signal requires an assertion-level wrong-versus-correct contrast, with no false rejection of the reference. If it fails, record the failure rather than increasing exhortation or repeating the same cohort. Even success here only admits a later outcome-blind transfer probe; it does not establish generic low/xhigh parity.

## Technical follow-up registered before further spending

The first verifier was infrastructure-invalid: zero continuations prevented delivery of its native background jobs. It is preserved and unscored, with reconciled client cost $0.1780582. This exposed no private outcomes. Authorize exactly one mechanical follow-up under the operator's standing autonomous scope, with identical public prompt/proposal/image and max_continuations=4 to permit normal job delivery. Use a new state clone/session/output directory; retain the first attempt. The second call has the same $3 CLI cap/$4 reserve. No further retry is admitted automatically by this note. Any output remains a development-control result, and total probe cost must include the invalid call.

## Verifier closure

The technical follow-up closed with lifecycle settled, three turns/two background-delivery continuations, candidate capture complete and 14 healthy native cgroup samples, observed OOM kills zero. It generated two new test functions and reported no justified counterexample. Cost $0.2291159; including the invalid call, probe generation cost is $0.4071741 and tracked series subtotal $62.153943199999999054. The generated candidate was frozen as SHA256 e1f8465d91523380d74e7f2ae8a31ec851888913cff9d3a5b3d9cb7cced79c94 before any evaluation. Scientific result remains pending actual assertion execution.

## Offline classification and resource recovery

The original offline evaluator stopped before applying any patch because Docker cp into its tmpfs was invisible to the running process. A separate stdin revision then executed the baseline but wrongly assumed test failures always exit 101; its raw two-assertion result was reclassified using the observed normal harness exit 1, without a baseline rerun. Both evaluator failures are retained and filed in MAIN issues. The next revision produced valid low-control and low-contract cells, each passing both generated tests.

The xhigh cell was then invalidated by the unchanged resource guard: host memory availability dropped from 57.77 to 45.41 GiB, below 48 GiB. Its container OOM counters were zero; no named assertion status was produced. The guard killed that owned container and prevented reference launch. A separately registered offline continuation preserves this invalid cell, reuses all three valid cells, and completes only xhigh and reference with the same frozen test patch, profile, guard and commands. No further model call or test edit occurred. The final comparison remains pending closure.

## Final closure — negative mechanism result

The offline continuation closed 0 with all five final cells valid. Both generated tests pass all three known wrong proposals; the reference and baseline each pass the first and fail the second. Therefore wrong proposals rejected = 0/3 variants of one defect, reference retained = false. The valid verifier found a useful partial-success input but imposed an incorrect no-error oracle. This protocol is NO-GO for extension; the overall effort-steering goal is not achieved.

The final audit re-parsed all named statuses, checked source/test/log/resource bindings and fresh owned-container absence without a model/native rerun. Report 323832c9f7e4015f and issue e4ea36b8419fda10 record the result. Evidence archive contains 110 payloads plus manifest, all reopened/hash-verified, SHA256 a12dd34af4b43fa0b3a6f3843c381405eec0e1b9291da19c56ff7a76ac434d8a. Total new model spending including the invalid attempt is $0.4071741; tracked subtotal $62.153943199999999054. No product source, old frozen cohort, merge or push was changed. The next distinct hypothesis is public baseline/proposal differential observation before accepting a generated oracle; it is not yet implemented or validated.

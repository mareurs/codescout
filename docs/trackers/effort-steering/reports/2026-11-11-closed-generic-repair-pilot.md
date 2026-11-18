---
id: '23e9a7272ee3d941'
kind: research
status: done
title: Closed eight-attempt generic repair pilot — primary results
tags:
- effort-steering
- primary-results
- negative-screen
topic: effort-steering
---

# Closed generic repair pilot — 2026-11-11

The frozen eight-attempt pilot completed all actors and all private grades. **The registered protocol version is NO-GO.** P repairs both tasks under the independent tests, but gains no acceptance over ordinary low or matched-continuation low, costs more than xhigh across the pair, and closes neither registered P workflow. The broader goal of a useful generic low-effort mechanism remains OPEN.

## Primary results

L is ordinary low; R is low with the same three phase opportunities as P without evidence gates; P is evidence-gated low; X is ordinary xhigh. All share frozen source-only onboarding, Codescout, Recon/global/project rules, model/CLI/plugin, issue and common public checks. Private grading started after all actors closed. No same-task resampling or prompt retuning.

| Task | Arm | Private patch acceptance | Workflow state | Client USD |
|---|---|---|---|---:|
| Lean1168 | L | pass | closed | 0.237782 |
| Lean1168 | R | pass | closed | 0.608742 |
| Lean1168 | P | pass | incomplete | 1.104507 |
| Lean1168 | X | fail | incomplete | 1.047643 |
| Obscura421 | X | pass | closed | 0.546750 |
| Obscura421 | P | pass | incomplete | 0.546073 |
| Obscura421 | R | pass | closed | 0.395165 |
| Obscura421 | L | pass | closed | 0.119801 |

Displayed USD are rounded to six decimal places; exact charged strings remain in the data. All eight grades are valid with zero missing expected statuses. Lean's expected population is8591tests per candidate; Obscura331. Test statuses are not independent experimental tasks. Every private runtime proof has complete=true, resource_ok=true, two cgroup probes, oom_kill=0, no recorded errors and fresh container absence. The controller exited normally.

| Arm | Hidden-pass candidates | Closed AND hidden-pass workflows | Pair client USD |
|---|---:|---:|---:|
| L | 2/2 | 2/2 | 0.357582 |
| R | 2/2 | 2/2 | 1.003907 |
| P | 2/2 | 0/2 | 1.650581 |
| X | 1/2 | 1/2 | 1.594393 |

P-L and P-R each have0wins/0losses/2ties. P-X has1win/0losses/1tie in primary captured-patch acceptance. The xhigh operational caveat below prevents turning that win into an inference that low reasoning is stronger. Two tasks establish neither general equivalence nor a validated generic mechanism.

## Registered decision

| Necessary condition | Observed |
|---|---|
| All eight attempts interpretable | pass |
| Both P protocols closed | fail |
| P acceptance gain over both L and R, no loss | fail |
| P accepted-task count at least X | pass |
| P total client cost below X | fail |

Decision: NO-GO, under the original criteria. Private acceptance does not retroactively close an incomplete protocol. The runner's advance calculation correctly exits at protocol incomplete with an empty paired field. A separate read-only analysis publishes all descriptive paired results and costs, reopens every source/input closure and receipt binding, consumes all actual verdicts, preserves the original runner analysis and checks the exact batch cost against the earlier actor audit.

## What the traces explain

Both P final submissions retain uncertainties; the controller returns final uncertainty cannot be marked complete. Some concerns are broader than the repaired behavior. In Obscura, P infers no JS engine from standalone node/deno/qjs lookup despite running the existing Rust suite successfully, then relies on static probes. Its repair probe raises ValueError after the old source spelling is removed; its submission acknowledges that. The controller proves command execution/source identity/declared roles, not exhaustive behavioral coverage. Private acceptance still passes both P patches. This exposes a completion-policy and evidence-quality mismatch; it does not prove that relaxing gates improves unseen repairs.

Obscura X extends the existing Rust/CDP harness with two behavioral regressions. Actual tool results show both fail on a whole-change revert, the restored six-test suite passes, and the final broader suite passes169tests. This is one whole-change revert exercised twice after test diagnostics improved, not independent per-guard mutation coverage. L/R patches implement the same core requested fix without new regression tests; P adds static Python probes. The public issue includes reproduction snippets, root causes and fix directions, leaving little observed acceptance headroom above low on this pair. The process difference is real; a private-quality benefit is absent here.

Lean X reaches its three-native-turn continuation bound while an in-place mutation is still running. Its last turn says the replay fix is temporarily reverted and restoration is pending. The captured primary patch retains that buggy line. Private cached400/429/500status-replay tests fail; other expected statuses pass. Earlier public tests passed with the intended fix. This incomplete failure stays counted. No repaired artifact, extra paid turn or earlier candidate is substituted. A separately labelled offline counterfactual could diagnose restoration, without changing the primary score.

Lean P has a separate unresolved public-suite discrepancy: its actual model-visible result shows223pass, while isolated public replay shows222pass/1ordinary ledger assertion failure. It is not a fabricated result. Its private grade is valid/pass. The later [fixed baseline/candidate matrix](2026-11-11-public-ledger-diagnosis.md) reproduces the exact assertion in1of3candidate-default processes, with the other2default and2serial processes passing. Baseline3default/2serial all pass. Candidate nondeterminism is established; a baseline failure and exact writer attribution are not. No RAM/disk cause, permanent fix or primary-score change is inferred.

## Accounting, controls and custody

Batch client charges:4.60646289999999987USD, including incomplete attempts. New shared preparation charge is explicitly0because frozen preparation is reused. Known series client charges:69.375358899999998524USD; historical unknown4USDhold remains unreleased. Total commitment:73.375358899999998524USD against200USD; no new reservation remains. Proxy charges are not invoices. The earlier opening balance is carried, not freshly reconstructed; root/review/embedding/compute costs remain unmeasured/excluded.

The complete actor audit covers16native invocations and187actual request payloads. Every payload has the expected low/xhigh effort and claude-sonnet-5-5 model. Each request-file census matches proxy requests/forwarding; normalized cost/context/capture/cleanup are valid. All192bound inputs and captured-source closures verify before and after. Raw Lean X lifecycle cost_is_final=false remains retained; its final known charge is independently supported by complete terminal proxy forwarding coverage. No missing cost is zero-filled.

- [Exact paired analysis](../evidence/paired-pilot-results-20261111/analysis.json), SHA2567caf0b9720a890203062ffa931ee5721bc5934beb6f5d3ce38abbd38b533ff08. Exact producer.py retained alongside(SHA256f48cd221fb8f8ceabfc8b8e9642bf0977847c64909aad4f401c388c3e714b532).
- [Actor/cost/control audit](../evidence/closed-campaign-audit-20261111/audit.json), SHA256838b384d8b0c871f32d98324da4686063cdedc3cab5509f5a3a6e620492309af.
- [Public-process audit](2026-11-11-prospective-public-process-audit.md) retains before-outcome Lean observations and Obscura process evidence captured before its private outcomes, including applied-revert outputs; scope/timing are explicit.
- Frozen originals: /var/tmp/ep11-01a15/paid-campaign-headroom-v3. Result, actor-batch-closed, ledger-final, eight attempts, native receipts/request captures and private verdicts stay unchanged. Closed paid-output custody is now verified: ../evidence/closed-paid-pilot-20261111/evidence.tar.gz is2584835393bytes, SHA25642d8f9a0ca4d95eccd177761d9aeb7a3db1d289644e06787b471aa9f977b4fb8. All151829selected payload entries, including132183regular files, were reopened and checked against their manifest; selected originals matched at closure. Independent GNU SHA-256 and size agree. The19omitted paths are exact writable compiler/scratch build mounts verified from actual container inspections; none was deleted. All source snapshots, native requests/receipts, patches, public logs and private grades are retained. Symlink targets are retained literally, external referents/images remain references; selected files were scanned and contained no account-credential bytes. Archive report snapshots retain their pre-closure pending-custody text; this live report is updated after closure. The separate earlier admission archive still excludes paid outputs.

## Follow-up justified by this diagnostic

Close this registered version; retain all negatives and charges. Finish paid-artifact custody and diagnose the public assertion discrepancy. Generic follow-up hypotheses are: discover and reuse an executable project test harness; distinguish source checks from behavioral evidence; scope unresolved uncertainty; protect the last good candidate during negative tests. These are observations to test, not validated improvements or a new frozen design.

A revised mechanism must use fresh outcome-blind tasks, preserve common infrastructure and measure complete-workflow client cost plus private acceptance. Do not tune exposed Lean/Obscura prompts, hand-pick tasks from known low failures, broaden success criteria retrospectively or claim the goal achieved. Held-out validation follows only an actually beneficial development screen.

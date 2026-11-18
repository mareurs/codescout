---
id: a3ef3292620ffcb9
kind: research
status: done
title: Effort steering — situation, evidence and next decision
tags:
- effort-steering
- evaluation
---

# Effort steering — situation, evidence and next decision

## Goal and current verdict

Find a generic mechanism that lets low effort deliver xhigh-quality repairs at lower total workflow cost. Candidate mechanisms are reused onboarding and complete instructions, Codescout navigation, executable public checks, bounded feedback and repair. Measure correctness, completed verification and total cost separately.

As of 2026-11-05, this goal is NOT demonstrated. The latest actual model pair produced two independently accepted patches, but neither observed every prescribed public check finish. Subsequent scripted controls qualify the runtime/verification loop; they are not model-quality results.

## Last actual model evidence

One exposed diagnostic task, hce-07-build_chunks, required a line_offset argument, shifted line coordinates, preserved metadata and token lookup, and correct offsets in initial/forced indexing callers. Both candidates passed the private acceptance and selected regressions.

| Arm | Known client cost USD | CLI wall seconds | Patch accepted | Entire public workflow observed complete |
|---|---:|---:|---|---|
| low, declaration absent | 0.40236020000000006 | 75.419 | yes | no |
| xhigh, declaration absent | 0.5673488000000001 | 449.635 | yes | no |

Low ended awaiting background clippy. Xhigh observed clippy success and ended awaiting background test lanes. The one-shot worker then closed the proxy and worker state. Historical workflow_completed=true means successful CLI turn, not full verification; raw receipts remain intact. Therefore the lower low cost/time does not establish equivalent completed-workflow efficiency. One exposed task per arm cannot support a generic transfer claim or an authority-clause effect.

Earlier four-cell authority results were budget-censored, with no normal CLI completion. They do not establish that low is superior or that reconnaissance/authority causes worse repairs. See the [pilot report](2026-11-04-hint-authority-functional-pilot.md) and data/2026-11-04-hint-completion-results.json.

## Runtime integration checkpoint — 2026-11-05

The opt-in storage/resource integration is complete and committed locally as `7e57bf8f91a2fd369411a2d89db51cf97c070d9f` (stable patch-id `528637cc65548ba4cb3f84424d2c8ebf725ec148`). The isolated V worktree is clean. Full development gate all five exits0,42 lifecycle tests and4 actual CLI/native scripted controls passed with independent raw-data readback. Two functional admission bugs and one guard-coverage gap were fixed with observed RED/GREEN and focused mutation verification. No new model cost, merge or push.

The [runtime checkpoint](2026-11-05-verified-worker-runtime.md) records review counts, exact artifact links and limits. Earlier component/runtime statuses below are historical. Preparation inventory covers36composed files and unchanged frozen onboarding/runtime inputs; it is explicitly not paid admission. Next: freeze/review a fresh matched complete-workflow driver with full cost ledger/reserve and private acceptance controls, then measure low/xhigh. Generic quality parity remains unproven.

## What changed since the paid pair

The opt-in verified worker now retains the same CLI/MCP session and provider budget, delivers background outcomes, and runs a public command manifest in the same identity-pinned native container. Public failures return bounded feedback to that session; private grading stays outside the repair loop. Unknown outcomes and uncovered provider costs prevent a success claim.

Real CLI/native controls with SCRIPTED provider responses checked successful settlement, public-failure repair, and timeout/incomplete behavior. The final controls used short probe commands, not the complete Rust gate. The component has 34 passing tests; independent review actually applied 14 mutations after fixes, killing 13 with one invariant-equivalent survivor. The full research-worktree development gate passed. These observations support the instrument, not model quality.

Source commit 0998c3b64f81c13331db26e0091ff25404272c66 is local on research/fixed-acceptance-population-20261101 at /home/marius/.codex/worktrees/615e/codescout. The worktree was freshly observed clean. No merge or push. [Composition report](2026-11-05-verified-worker-composition.md) holds exact controls, review, gate and limitations.

MAIN documents and evidence remain local. Their commit preflight has unresolved provenance coverage, tracked as issue421ed73df25f795d; no guard bypass or foreign staged-file changes are authorized by this report.

## Why progress has looked circular

Several prerequisites had to be corrected: actual instruction/tool delivery, native capability, conservative budget censoring, and the distinction between a successful turn and a finished workflow. Fixing each removes a concrete confound, but none counts as demonstrating better low-effort reasoning. The next decision must be based on actual completed workflows rather than another infrastructure-green result.

Do not rebuild the already-qualified lifecycle/composition component or rerun its unchanged tests. Only new seams and changed launch composition need qualification.

## Immediate continuation

A zero-provider positive control is running the full Rust public profile on the known reference patch under the worker's actual native environment. It uses the existing PublicVerifier and native_check implementations, with exact container name/ID cleanup and no credential/provider access.

Root: /tmp/verified-public-profile-01a15ca9-20261105.
Script: /tmp/verified-public-profile-01a15ca9-20261105.py.
Job: @bg_00000047, record /tmp/codescout-bg/455879-a720241d-bg_00000047.json.
Status at this document's creation: outcome not yet observed.

The preparation manifest includes the legacy frozen-runtime/input fingerprint plus all lifecycle, completion/resume source and the Rust public profile. It is an inventory for preparation, NOT paid admission or a technical-review marker. A future paid driver and its final launch inputs must be included before review/admission.

The legacy pair driver admits a next run only with a USD1 reserve. The remaining original pilot balance USD1.42373509999999987 cannot admit two such runs up front. Do not silently reset the pilot ledger, lower the reserve to force a pair, or buy a partial block. A fresh matched calibration needs an explicit bounded allocation within existing overall authorization and a reviewed driver.



Preparation inventory has been independently read back: all 17 explicitly listed additional files match their recorded SHA256, and the canonical inventory digest recomputes to b541c9985e029599ff1ce988eede755f5615d1612566a6d067c5dee8cdb9ef9d. A copy is preserved at data/2026-11-05-workflow-calibration-inventory.json. The legacy input fingerprint is intentionally different from the old paid-pair fingerprint because worker code changed. No existing review marker is transferred to it. The canonical foreign staged bug-triage blob remains f4ad12b9cf7b8b830ac35cc341afcde702d0a56c.



**Observed follow-up:** the control ended with in-memory job status exited120. Clippy log records completion; lean compilation failed writing its incremental query cache with Disk quota exceeded (os error122). The terminal receipt is empty and result.json absent, so default-check completion is unknown. The generated native container is independently absent. This is an infrastructure-control failure, not model evidence; no provider cost. Recorded immediately in docs/issues/2026-11-05-public-profile-reference-temporary-build-quota.md. The earlier running status above is historical. Next fix is disk-backed, capacity-checked task-owned build storage with reliable evidence retention, before paid use.



**Operator storage update and resumed control:** /tmp is now32GiB with24GiBavailable; persistent /home has134GiBavailable at preflight. A new exclusive disk-backed reference-control attempt is running as @bg_00000048 in /home/marius/.cache/codescout-resident/codescout/public-profile-reference-20261105-v2, via /tmp/retry-public-profile-01a15ca9-20261105.py. Original control script hash3369ee8d629ff14155deccc96a5021d2b3a1d3a641bc483c8cf29f830d65b7f4 is checked before launch; only host candidate/build/evidence location changes. Old failed evidence is preserved at data/2026-11-05-public-profile-reference-v1.tar.gz (SHA49cec5bcbc782b62315c4e6839c7e1a4e111e6c74c9d0c308a173e4f2b6a8e0d,15payload files independently read/hash-verified).

Independently, @bg_0000004a is qualifying actual stream-CLI/native xhigh effort compatibility on the unchanged component using scripted provider responses and short public probes, at /home/marius/.cache/codescout-resident/codescout/workflow-xhigh-20261105. The earlier corrected composition controls used low only; this checks the other intended effort cell without buying model generations. Both outcomes are pending at this checkpoint. No new onboarding, provider spend, source changes or paid admission.



**Xhigh transport outcome now observed:** @bg_0000004a exited0. Independent readback found19actual request bodies, all output_config.effort=xhigh; two raw terminal events in sessiondd267c18-4616-4602-979d-418607c8edf8; one background continuation with observed exit0; public probe log contents FORMAT/COMPILE/LEAN/DEFAULT with exits[0,0,0,0]; the same native containerID for all four checks; both exact owned container names independently absent. No provider forwarding and scripted terminal cost0. Durable readback: data/2026-11-05-workflow-xhigh-verification.json. This supersedes xhigh-pending wording above. Full Rust reference control @bg_00000048 remains separate and has no outcome yet.



**Final public-profile checkpoint (2026-11-05):** the known-reference control now completes the entire frozen Rust public profile: fmt, long clippy, lean tests and default tests all exit 0. Final root: /home/marius/.cache/codescout-resident/codescout/public-profile-reference-20261105-v6; job @bg_00000051 exited 0. Public verification took 203.20 seconds with the owned warm build cache; this is not an unscripted model timing result.

Qualification established concrete runtime prerequisites: disk-backed build/evidence storage; two-CPU quota with PID ceiling 512; an init process to reap orphaned descendants; and the same Git baseline initialization already performed by the actual worker. Earlier attempts and failures remain visible. The source, test assertions and four public commands were not changed. No new filters or skips were added.

Independent readback verified all command exits and raw test-summary lines, the same native container ID, exact container absence, wrapper hash and CPU probe. Source and shared knowledge stayed unchanged; three newly created Codescout database/lock paths were enumerated separately rather than excluding the knowledge subtree. Evidence: data/2026-11-05-public-profile-final-verification.json and data/2026-11-05-public-profile-final.tar.gz (SHA256 6e197b71805837f9e5cda1c0d6ed296811c761f00c4ca201bfeb5bd29a6ece49; 61 payload files independently hash-verified). Earlier v1/v2/v3 archives and xhigh compatibility evidence remain separate.

This closes the full public-profile reference-control prerequisite. It does not admit a paid run: the production opt-in worker still needs these verified storage/resource settings, a complete launch fingerprint, technical review and qualification of the resulting composition. The Git omission was in the disposable control; the actual worker already initializes Git. Source remains 0998c3b64 in the clean research worktree. No product source change, push or merge occurred.

All these controls cost $0 in new provider/model calls. The last actual low/xhigh quality pair remains the previous exposed-task pair with accepted patches but unfinished public validation. The generic low/xhigh goal is still open.

## Next decision and stop rules

1. Observe whether the known reference can finish all four actual public commands in the frozen native worker. If it cannot, diagnose that recorded failure before buying model attempts.
2. Bind the new driver, workflow modules, public profile, runtime, frozen task/context and oracle to one reviewed composition. Qualify the intended low/xhigh request cells; preserve closed historical cohort markers.
3. Run only a fully budget-admitted matched diagnostic pair. Charge every failed/incomplete attempt; stop on unknown cost or incompatible effort/context delivery. Apply the same complete public manifest and independent private grade to both arms.
4. Count accepted patch AND completed public verification as the quality endpoint; report whole-workflow cost and time. If low needs public-feedback repair, show that trace and cost.
5. Even a successful pair is only calibration on a known task. Generic promotion requires an admitted external task population, held-out acceptance and matched xhigh controls. Current budget may support a small diagnostic, not a powered validation.

## Budget

No new provider/model cost in the two infrastructure stages or this situation/preparation stage.

Original pilot: USD2.57626490000000013 spent / USD4; USD1.42373509999999987 left.
Thread: USD31.797419199999999964 spent / USD40; USD8.202580800000000036 left.
All historical aborted, failed and incomplete attempts remain charged. Separate peer-campaign spending is not silently pooled. This report neither increases a cap nor launches a paid run.

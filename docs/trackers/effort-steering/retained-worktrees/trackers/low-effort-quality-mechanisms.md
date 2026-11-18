---
id: '48e290e38effad4e'
kind: research-note
status: archived
title: 'Historical: Low-effort quality mechanisms — evidence ledger'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/trackers/low-effort-quality-mechanisms.md
source_body_sha256: c6c26dc9d2a0935e3e4fcd1d15447510bfb4cf36aad87f83b081da0b90bdf3d4
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/trackers/low-effort-quality-mechanisms.md
  sha256: e093194428d8df60bb625418de6f4854a66cb39ff9dfad57f15638d6a650d07f
  body_sha256: c6c26dc9d2a0935e3e4fcd1d15447510bfb4cf36aad87f83b081da0b90bdf3d4
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/trackers/low-effort-quality-mechanisms.md
  sha256: e093194428d8df60bb625418de6f4854a66cb39ff9dfad57f15638d6a650d07f
  body_sha256: c6c26dc9d2a0935e3e4fcd1d15447510bfb4cf36aad87f83b081da0b90bdf3d4
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/trackers/low-effort-quality-mechanisms.md

---


# Low-effort quality mechanisms

## Decision scope — 2026-10-29

Living evidence ledger for scripts, checks, prompts and context scaffolds that might let a low-effort producer reach an independently accepted result at lower cost. The first twelve entries aggregate prior local work without new model calls. LEM-13 records the separately registered four-arm pilot executed afterward; prior registrations, negatives and charges remain intact.

Concrete compiler feedback at a forced completion boundary is an observed correction mechanism. A precise check instruction can also expose a compiler failure before completion. Parent-side acceptance detects/selects a final output; intervention and instruction adherence require their own actual traces. Each entry separates prevention, detection, correction and selection.

## Evidence policy

Accepted means passing the declared contract, not universal correctness. Count actual run records/outcomes, not search hits, calls or scenarios as independent tasks. State model, effort, task, sample, client cost and latency when known; missing fields remain unknown. Preserve negatives, capped attempts and later counterexamples. Effort, model tier, prompt, oracle and retry budget are distinct interventions.

LEM headings are citable definitions; augmented entries provide the filterable index. Status describes evidence rather than deployment readiness. Entries declare dated validity and supporting sources.

## Search scope and limitations

Read the inherited effort-steering conversation; project memory/catalog; targeted codescout research/session logs; claude-plugins effort trackers; original cs-p20 gate/paragraph run records, gate events and author transcripts; prompt-engineering conclude-last results and arm definitions; critic/router summaries. This is a targeted aggregation, not an exhaustive scan of every Claude/Codex profile. A broad filesystem lookup was stopped and replaced with catalog/known-path reads. No paid model run or internet survey.

Related histories retain their scopes: claude-plugins:docs/trackers/effort-steering-second-phase.md (plugin execution), prompt-hamsa-audit-log (individual prompt audits), prompt-surface-measurement-session-log (measurement craft). This new ledger owns the cross-experiment mechanism comparison and supersedes none of them.

## Implications for the current experiment

- The proposed ordinary-low / precise-instruction-low / enforced-feedback-low / fixed-xhigh diagnostic is complete in LEM-13. Its frozen parent accepted L1/2,I2/2,G2/2,X2/2. G intervened once; both I traces repaired compiler errors before Stop. These are two samples per arm on one known task.
- Keep earlier transfer-controller rules and all older negative outcomes unchanged. This separate registration allowed one low-effort compiler-feedback round and retained a valid control compilation failure. A passing parent oracle is a finite task-contract result; both xhigh authors also encountered broader test failures.
- The next useful evidence is prospective transfer beyond cs-p20, with checks/compatibility cases frozen before outcomes and costs for the whole workflow measured. Further repetitions of the known task cannot establish general low/xhigh equivalence.

## Refresh protocol

On a relevant experiment closure, gather this tracker, then read its registration, complete outcome population and raw evidence. Check fixture/source dates and actual model/effort. Allocate entries through append_entry; update one row with update_entry and its narrative with body_edits. Keep the original verdict when later controls change interpretation. Compute money and wall time separately, retaining unknown native cost. Refresh the dated review marker only after catalog read-back and reference audit. Revisit on new repair outcomes, new oracle classes, changed model/CLI or a contradicting source; no scheduled paid runs.

## Evidence entries

## LEM-1 — Compiler Stop gate: enforced feedback repairs missing all-target compilation

**Valid:** dated 2026-10-29

**Status:** measured; outcome supported.

**Role / evidence:** detection, correction / trace_plus_cross_batch.

**Population:** claude-sonnet-5-5 low, CLI 2.1.296. 5 gated cs-p20 runs; cross-batch ungated references.

**Observed:** 5/5 accepted; 4/5 received one Stop-hook block with E0063 then check_ok; 1 already compiled. Raw five-row cost $1.2584474. Source report historical references: ungated low0/5, paragraph low0/5, fixed-xhigh5/5; reported means gated low$0.252 versus xhigh$0.478 (latter three rerun calls). These are different batches/populations.

**Causal interpretation:** Actual compiler failure → feedback → source correction → successful check is observed within four traces. Population uplift versus historical references is not a randomized matched effect.

**Limits:** One retrospectively chosen compiler-class task; references from other batches. Hook fails open if check cannot run or after one block. Adds check wall time; no general behavioral-correctness guarantee.

**Rests on:** [docs/research/2026-10-14-effort-steering-when-to-switch.md](../research/2026-10-14-effort-steering-when-to-switch.md); `/home/marius/work/claude/es-stage0/es-gate/effort-steering/gate/stop-gate.mjs`; `/home/marius/work/claude/s5-work-v2g/rows-gate.jsonl`; `/home/marius/work/claude/s5-work-v2g/configs/gate.json`

## LEM-2 — Verification paragraph: no all-target checking gain on cs-p20

**Valid:** dated 2026-10-29

**Status:** measured; outcome negative.

**Role / evidence:** prevention / trace_plus_cross_batch.

**Population:** claude-sonnet-5-5 low, CLI 2.1.296. 5 paragraph runs on cs-p20.

**Observed:** 0/5 passed; all five build_failed; raw client estimates $1.1074432. All selected narrow lib/feature checks rather than all-target compilation.

**Causal interpretation:** This wording did not close the known failure in this batch; no randomized matched prompt comparison.

**Limits:** One task and user-prompt placement; the existing no-default-features hint may interact. A precise compile-all-targets instruction was not tested here.

**Rests on:** [docs/research/2026-10-14-effort-steering-when-to-switch.md](../research/2026-10-14-effort-steering-when-to-switch.md); `/home/marius/work/claude/s5-work-v2p/rows-para.jsonl`

## LEM-3 — Unconditional source verification: controlled prompt lift on a proxy task

**Valid:** dated 2026-10-29

**Status:** measured; outcome supported.

**Role / evidence:** prevention, scaffolding / controlled_proxy.

**Population:** Sonnet alias pinned in prompt-tdd; effort not independently varied; resolved model version unknown here. 35 records per arm; 15 plausibility-class records per arm in the historical grid.

**Observed:** Ground-truth-file verification: conclude-last 2/15 (13.3%) versus grounded clause 11/15 (73.3%); imperative-only 15/15; verify prose 14/15. This measures artifact consultation, not low/xhigh coding equivalence.

**Causal interpretation:** Controlled arm wording changes moved verification in the seeded two-turn belief-planting fixture. The proposed active ingredient is an interpretation of the grid, not an independently proven general law.

**Limits:** Later real-profile test prompt-hamsa-audit-log:A-33: verified 2/10 before prose deletion and 4/10 after; small sample and fixture-rot caveats. Full context/placement can dilute isolated prompt effects. No effort-level comparison.

**Rests on:** `/home/marius/work/claude/prompt-engineering/scenarios/conclude-last/README.md`; `/home/marius/work/claude/prompt-engineering/results/conclude-last/v3-a3.json`; `/home/marius/work/claude/prompt-engineering/results/conclude-last/v3-a3v2-conclude-last-grounded.json`; [docs/trackers/prompt-hamsa-audit-log.md](../trackers/prompt-hamsa-audit-log.md)

## LEM-4 — Server-computed provenance and concrete tool guidance: scaffolding evidence only

**Valid:** dated 2026-10-29

**Status:** measured; outcome mixed.

**Role / evidence:** scaffolding, prevention / indirect.

**Population:** Initial uncontrolled model later identified as Fable; pinned-Sonnet correction; no low/xhigh effort contrast. prompt-hamsa-audit-log:A-7 single-turn cells and subsequent calibration reruns.

**Observed:** prompt-hamsa-audit-log:A-7 reports key-priority held across both model conditions. Plain guidance footer already hit the adoption ceiling; adding an authority/delegation sentence showed no incremental gain. Initial 1/3 calibration was revised after larger samples.

**Causal interpretation:** Server-computed freshness/conflicting-body structure can steer source priority in this fixture. It does not establish improved low-effort implementation quality.

**Limits:** Model-pinning defect and judge misfires changed interpretations. Single-turn ceiling does not establish long-session persistence; use corrected prompt-hamsa-audit-log:A-7 rather than its initial headline.

**Rests on:** [docs/trackers/prompt-hamsa-audit-log.md](../trackers/prompt-hamsa-audit-log.md); [docs/evals/2026-07-03-delegation-envelope.md](../evals/2026-07-03-delegation-envelope.md); [docs/evals/2026-07-03-provenance-keys.md](../evals/2026-07-03-provenance-keys.md)

## LEM-5 — Cheap LLM critic: planted-defect recall did not transfer to natural plans

**Valid:** dated 2026-10-29

**Status:** measured; outcome negative.

**Role / evidence:** detection, correction / controlled_proxy.

**Population:** Sonnet 5.5 low/high critics; Stage C low/low+gate/xhigh authors; Opus 5.5 high audit. Stage A 20 documents × 2 variants × 2 efforts; Stage C six plans × three arms.

**Observed:** Stage A blocking planted-defect recall low18/20, high17/20. Stage C verified blocking defect totals L5, LG4, X0 across six plans each; only one LG run blocked. Raw Stage C18 rows independently recounted.

**Causal interpretation:** A usable planted-defect detector did not show substitution for xhigh plan production under the registered Stage C criteria.

**Limits:** One sample per cell; model and effort of auditor confounded; longer xhigh plans may receive less audit depth per line. Audit finding totals are not task success counts.

**Rests on:** [docs/research/2026-10-14-effort-steering-when-to-switch.md](../research/2026-10-14-effort-steering-when-to-switch.md); `/home/marius/work/claude/es-stage0/critic-a/prereg.md`; `/home/marius/work/claude/es-stage0/critic-c/grading/results-c.json`

## LEM-6 — Fresh stronger repair improves feedback tests while missing compatibility

**Valid:** dated 2026-10-29

**Status:** measured; outcome negative.

**Role / evidence:** correction, detection / retrospective.

**Population:** Original low candidate; two separate claude-opus-5-5 high repairs, CLI2.1.309. One selected saved write-guard candidate, two sequential repairs.

**Observed:** First repair passes nine supplied cases but fails two compatibility cases. Second passes all eleven known validator cases but fails one of four new pre-frozen probes. Later operation matrix: original96/120, first102/120, second114/120; fixed reference120/120.

**Causal interpretation:** Repairs caused real targeted improvements while retaining or introducing wrong operation semantics; executable source reversals/ancestry localize the regression. No accepted complete repair.

**Limits:** Known failed candidate chosen retrospectively; later120-case matrix is diagnostic, not held-out evaluation. Repeated feedback on one case cannot estimate general repair reliability. New calls cost $0.2571432 and $0.3842718, excluding native overhead.

**Rests on:** [docs/research/2026-10-28-confirmed-defect-repair.md](../research/2026-10-28-confirmed-defect-repair.md); [docs/research/2026-10-28-validator-feedback-repair.md](../research/2026-10-28-validator-feedback-repair.md); [docs/research/2026-10-28-operation-contract-validator.md](../research/2026-10-28-operation-contract-validator.md)

## LEM-7 — Parent acceptance selects low successes without showing an intervention effect

**Valid:** dated 2026-10-29

**Status:** measured; outcome supported.

**Role / evidence:** detection, selection / descriptive_acceptance.

**Population:** claude-sonnet-5-5 low versus fixed-xhigh, CLI2.1.309. Producer-status retained low plus fresh xhigh; two further prospectively selected historical tasks, one call per arm.

**Observed:** Producer pair passes38 cases+compile; low model cost78.2% lower. Lock/registry pairs each pass16 cases+compile; low costs66.9%/72.7% lower. No low repair eligible in these cases.

**Causal interpretation:** Declared acceptance and observed client-cost comparisons hold for these artifacts. A validator run after completion did not cause low to solve them.

**Limits:** Selected bounded historical tasks, one sample per arm, fixed low-first order and unknown native overhead. Original capped producer baseline remains invalid and charged. Full research-worktree gate had disclosed unrelated failures.

**Rests on:** [docs/research/2026-10-28-producer-status-xhigh-extension.md](../research/2026-10-28-producer-status-xhigh-extension.md); [docs/research/2026-10-28-independent-effort-transfer.md](../research/2026-10-28-independent-effort-transfer.md)

## LEM-8 — Matched Stop-hook comparison: inactive gate and incomplete oracle

**Valid:** dated 2026-10-29

**Status:** measured; outcome mixed.

**Role / evidence:** detection, selection / matched_no_intervention.

**Population:** Sonnet5.5 fixed low, CLI2.1.309. Four real build-metadata attempts: two ordinary, two checked.

**Observed:** All four pass frozen17 checks; neither checked run requested a Stop repair. Later stronger24-method suite rejects every untouched raw candidate (five failures in three, six in selected checked-0). Selected checked-0 also failed standalone Clippy before supervisor correction.

**Causal interpretation:** Matched initial prompts/bases isolate hook availability, but the gate never intervened; these outcomes establish no correction benefit. Wider counterexamples demonstrate the original oracle's omissions.

**Limits:** Retrospective wider oracle is diagnosis, not replacement for frozen scores. Final supervisor-fixed implementation must not be attributed to low alone.

**Rests on:** [docs/research/2026-10-24-matched-checked-completion.md](../research/2026-10-24-matched-checked-completion.md)

## LEM-9 — Voluntary compiler-feedback repair is not a Stop-hook effect

**Valid:** dated 2026-10-29

**Status:** measured; outcome supported.

**Role / evidence:** correction, detection / matched_no_intervention.

**Population:** Sonnet5.5 fixed low, CLI2.1.309. Four attempts, two ordinary/two checked; nine upstream regressions.

**Observed:** All four pass the independent nine; zero Stop repairs. checked-1 voluntarily fixes a missing TextRange import after compiler output, before the hook. One selected checked patch later passes2220 upstream library tests, four ignored.

**Causal interpretation:** Concrete feedback-to-edit-to-pass correction is directly observed; it cannot be assigned to the inactive hook or generalized to all four broad-suite checks.

**Limits:** Wrapper cwd refusal and piped shell status masked failures: eight tool results had is_error=false despite initial refusals/compiler error. Classify returned output and real process status, not the tool success bit.

**Rests on:** [docs/research/2026-10-24-upstream-gated-repair.md](../research/2026-10-24-upstream-gated-repair.md); [docs/research/2026-10-24-upstream-gated-repair.json](../research/2026-10-24-upstream-gated-repair.json)

## LEM-10 — Late effort routing after edits/checks did not recover fixed-xhigh behavior

**Valid:** dated 2026-10-29

**Status:** measured; outcome negative.

**Role / evidence:** correction / trace_plus_cross_batch.

**Population:** Sonnet5.5, CLI2.1.296; fixed medium/xhigh, rule-v3, random and later rule-v4. G16 five per arm; G17 rule-v4 ten runs, historical G16 references.

**Observed:** G16 medium0/5, xhigh5/5, v3 0/5, random1/5; G17 v4 3/10. v4 xhigh request share0.436 and broad checks2/10. Seven v4 failures still lacked the needed all-target/default-feature verification despite late xhigh requests.

**Causal interpretation:** Tool-step routing delivered more xhigh-labelled requests but did not meet preregistered success or reproduce fixed-xhigh outcomes. New user-turn delivery/history effects must be separated from labels.

**Limits:** One repeated task and cross-batch v4 controls; association between broad checks and passing does not identify every cause. Offline summary/source inspection here, no new request replay.

**Rests on:** `/home/marius/.cache/g17/final-table.json`; `/home/marius/.cache/g17/analyze17.out`; [docs/research/2026-10-14-effort-steering-when-to-switch.md](../research/2026-10-14-effort-steering-when-to-switch.md)

## LEM-11 — Applied mutations and calibrated instruments protect acceptance, not reasoning parity

**Valid:** dated 2026-10-29

**Status:** measured; outcome supported.

**Role / evidence:** detection, selection / retrospective.

**Population:** Independent reviewers/controllers; no isolated low/xhigh comparison. Matched build-metadata review; prospective transfer contracts; prompt-surface-measurement-session-log:W-19, prompt-surface-measurement-session-log:W-24 and prompt-surface-measurement-session-log:W-25.

**Observed:** Matched review observed one surviving short-SHA fallback mutation under the17-check suite before adding its guard. Transfer review applied six Rust plus three Python mutations; all nine killed. Measurement controls exposed pairing/deny-list and calibration faults.

**Causal interpretation:** These are direct tests of whether an instrument can express a known failure. They improve confidence in acceptance/accounting and may stop bad promotion; they do not establish more capable low generation.

**Limits:** Finite chosen mutations and non-independent craft observations. A green suite can coexist with an untested compatibility branch; no survivor percentage generalizes beyond the applied set.

**Rests on:** [docs/research/2026-10-24-matched-checked-completion.md](../research/2026-10-24-matched-checked-completion.md); [docs/research/2026-10-28-independent-effort-transfer.md](../research/2026-10-28-independent-effort-transfer.md); [docs/trackers/prompt-surface-measurement-session-log.md](../trackers/prompt-surface-measurement-session-log.md); [docs/trackers/prompt-surface-compaction-session-log.md](../trackers/prompt-surface-compaction-session-log.md)

## LEM-12 — Detailed context, plans and checklists: useful scaffold with unisolated effect

**Valid:** dated 2026-10-29

**Status:** hypothesis; outcome unproven.

**Role / evidence:** scaffolding, prevention / hypothesis.

**Population:** Current paired authors share full base/prompt; no isolated scaffold arm. Current historical-task pairs and prompt/measurement audits.

**Observed:** The recent low successes received precise requirements and ordinary check commands, but the experiment varies effort rather than context completeness. There is no measured standalone lift attributable to the scaffold.

**Causal interpretation:** Hypothesis: concrete commands/contracts can reduce uncertainty and missed scope. Existing results cannot separate this from task simplicity, model priors or normal prompt adherence.

**Limits:** Do not retrofit a cause to low successes. A new comparison must vary one scaffold and keep acceptance, task, base and model/effort fixed.

**Rests on:** [docs/superpowers/specs/2026-10-28-independent-effort-transfer-design.md](../superpowers/specs/2026-10-28-independent-effort-transfer-design.md); [docs/trackers/prompt-hamsa-audit-log.md](../trackers/prompt-hamsa-audit-log.md)

## LEM-13 — Matched compiler pilot: precise checks and one enforced low repair

**Valid:** dated 2026-10-29

**Status:** measured; outcome mixed.

**Role / evidence:** prevention, detection, correction / controlled proxy.

**Population:** eight revised calls, two per arm, one known cs-p20 task; claude-sonnet-5-5 low/fixed-xhigh, CLI2.1.309.

**Observed:** parent acceptance L1/2,I2/2,G2/2,X2/2. In G2, two narrow checks passed, first Stop exposed E0063, one delivered block led low to fix exactly five integration constructors, then compiler and parent19/19 passed. G1 needed no intervention. Both I runs repaired E0063 before first Stop; ordinary L1 voluntarily repaired E0609, while L2 failed compilation and executed zero behavioral tests. Mean client USD I0.3801,G0.3685,X1.2503. Revised calls4.6732616; cumulative15.2172051 of20.

**Causal claim:** the low compiler-feedback→edit→pass chain is observed in G2. Precise instruction also exposed the target failure in both I traces. Two samples per arm do not establish population reliability or general superiority.

**Limits:** known task, revised behavior prompt and CLI; one invalid earlier protocol charged0.0451035 and one disclosed restart, no revised paid retries. An unpaid grade preflight required exact-byte CLI recovery. A frozen literal command counter omitted manifest-path arguments, so its original output is retained with a calibrated, explicitly post-hoc scope supplement. Both xhigh broader checks failed; nineteen-case acceptance is finite. Native cost unknown.

**Rests on:** [complete report](../research/2026-10-29-compile-mechanism.md), [protocol revision](../superpowers/specs/2026-10-29-compile-mechanism-revision.md), [instrument](../../tools/reliable-agent-eval/compile_mechanism/README.md). Raw registration, receipts, source inventories, delivered feedback, parent logs, analysis.json, repair-delta-2-G.json and command-scope-supplement.json are retained in .codescout/resident-private/compile-mechanism-r2-20261029.

## LEM-14 — Behavioral transfer: accepted low candidates with inactive feedback

**Valid:** dated 2026-10-29

**Status:** measured; outcome unproven.

**Role / evidence:** detection, correction, scaffolding / matched no intervention.

**Population:** three prospectively registered calls, one per L/G/X, on one new historical runtime-config task within Codescout's already represented bounded-filesystem area; claude-sonnet-5-5, CLI2.1.309.

**Observed:** L/G/X each independently accepted31/31, first Stop2/2 passed, zero delivered feedback and no first-Stop-to-final source changes. Client USD L0.0837787,G0.0804468,X0.3600069; new total0.5242324, cumulative15.7414375 of20. Both low candidates reused the available bounded reader and added no tests. X added eight tests, passed its targeted16-function check, and retained a failing wider server check. Its separate post-hoc baseline qualification is in the report.

**Causal claim:** matching finite acceptance is observed; the gate's correction effect is unproven because it never activated. The contribution of detailed prompt/context and shared-reader availability was not isolated. This repeats the inactive-gate limitation already present in LEM-7, LEM-8 and LEM-9.

**Limits:** one call per arm, one same-repository task, finite31 functions; no external-repository or independent-family validation. The wider xhigh failure is retained rather than declared unrelated. Client estimates omit unknown native orchestration/review dollars, and build timing prevents a latency comparison. No general low/xhigh equivalence or superiority.

**Rests on:** [complete report](../research/2026-10-29-behavior-transfer.md), [registered design](../superpowers/specs/2026-10-29-behavior-transfer-design.md), [instrument](../../tools/reliable-agent-eval/behavior_transfer/README.md). Raw registration, receipts, exact patches/inventories, Stop/Cargo traces,31-function grades, independent applied-mutation evidence and analysis.json are retained in .codescout/resident-private/behavior-transfer-20261029.

## LEM-15 — External calibration: upstream acceptance without feedback, shared gold input hole

**Valid:** dated 2026-10-29

**Status:** measured; outcome unproven.

**Role / evidence:** detection, correction, scaffolding / descriptive acceptance.

**Population:** two registered low calibration calls on public ripgrep-2576 and bat-1892 fixtures, claude-sonnet-5-5, CLI2.1.309. The disjoint axum-691 holdout and fresh xhigh arm were not admitted.

**Observed:** C1 first/final F2P1/1 and compatibility86/86, client USD0.1600911; C2 first/final F2P1/1 and compatibility80/80, USD0.20421690000000003. Both first Stops pass,0feedback blocks, identical first/final patch hashes. Phase0.36430800000000003; cumulative16.105745499999999865 of20. A post-hoc empty-suffix probe aborts both C2 and upstream gold; nonempty controls pass.

**Causal claim:** these two low calls meet their finite upstream contracts without intervention. No feedback-repair effect or low/xhigh equivalence is established. A passing gold oracle can miss a concrete CLI crash; independent behavioral validation remains necessary.

**Limits:** calibration only,n2, public tasks with unknown training exposure; no holdout or xhigh comparison, general quality/cost/latency superiority. Bat PTY fixture amended with assertions preserved; image Cargo.lock frozen for axum. C1 wider optional cargo test --all is unavailable offline, independently reproduced on unchanged base. Native workflow dollars unknown. Original primary verdicts and post-hoc qualifications remain separate.

**Rests on:** [report](../research/2026-10-29-external-feedback.md), [fixed design](../superpowers/specs/2026-10-29-external-feedback-design.md), [instrument](../../tools/reliable-agent-eval/external_feedback/README.md). Raw registration, complete two-call population, first/final grades, source reviews, applied-mutation evidence and boundary probes are retained in .codescout/resident-private/external-feedback-20261029.

## LEM-16 — Fresh low review repairs a known crash; broader behavior still varies

**Valid:** dated 2026-10-30

**Status:** measured; feedback effect and general low/xhigh parity unproven.

**Role / evidence:** detection, correction, scaffolding / descriptive_acceptance.

**Population:** one selected natural bat defect, three low review trajectories and one xhigh review, identical initial C2 source and prompt. This is development evidence, not a disjoint holdout.

**Observed:** all first/final snapshots pass6CLIcases+80upstreamfunctions, with one Stop and zero feedback blocks per arm. Costs U0.10819439999999998,V0.1211422,F0.0955674,X0.34754440000000003. Phase0.67244840000000001; cumulative16.778193899999999875 of newly authorized40; native workflow dollars unknown. Generic/precise messages were never delivered, so their assigned treatments cannot explain outcomes.

**Independent qualification:** a separate corrected overlap probe finds that the original low draft, U and V lose expected JSON highlighting for a custom suffix overlapping a built-in suffix; upstream gold, F and X pass. All six nonoverlap controls pass and all corrected processes exit0. The initial malformed probe was rejected before behavior and is retained separately. Primary acceptance verdicts and paid patches are unchanged.

**Claim and limits:** fresh low review repairs the known crash in all three observed trajectories, but its broader coverage varies. No general equivalence, latency or cost-superiority claim follows. X supplied additional tests and had denied build/format requests under the same restricted tool policy; parent independently grades its exact sources. A future diagnostic should deliver an observed failure before the repair call, then validate the frozen mechanism on disjoint tasks.

**Rests on:** [natural-failure report](../research/2026-10-30-natural-failure-feedback.md), [fixed design](../superpowers/specs/2026-10-29-natural-failure-feedback-design.md), retained raw phase analysis/source-bound grades/receipts and post-hoc probes. The report records the source, workflow and measurement limits.

## LEM-17 — Prechecked diagnostic repairs both low attempts; generic low also succeeds once

**Valid:** dated 2026-10-30

**Status:** measured; supported on this selected case, external transfer and general parity unproven.

**Role / evidence:** detection, correction, selection, scaffolding / controlled_proxy.

**Population:** one known bat overlap defect, identical saved U source; fixed G1,D1,X,D2,G2, two low generic, two low diagnostic and one xhigh generic trajectory. No paid retries or task substitutions.

**Observed:** diagnostic low2/2, generic low1/2, xhigh1/1 accepted on7CLIcases+80upstreamfunctions. Both diagnostics are observed verbatim in actual initial primary-session prompts. G1 leaves the source unchanged and fails overlap despite80passing tests; G2 repairs it without diagnostics. Costs G1.0717037,D1.0688737,X.31701159999999995,D2.0573732,G2.1047363; phase.61969849999999995, cumulative17.397892399999999825 of40; native workflow USD unknown.

**Mechanism:** build/check the exact draft, deliver the actual failing command with complete actual/expected output before low repair, then independently validate exact final source and compatibility. No source fix is supplied. This produces actual delivery where the previous Stop intervention stayed inactive.

**Claim and limits:** this component works in both diagnostic attempts, but generic low also succeeds. One selected development defect, small trajectory counts, restricted tools and repair-only costs cannot establish general effect size, low/xhigh parity, latency advantage or broad savings. Freeze before disjoint validation; retain all failed drafts and inactive checks.

**Rests on:** [report](../research/2026-10-30-prechecked-repair.md), [design](../superpowers/specs/2026-10-30-prechecked-repair-design.md), [plan](../superpowers/plans/2026-10-30-prechecked-repair.md). Private phase prechecked-repair-20261030 retains raw5calls, frozen7case/80function populations, actual prompt proofs, source reviews, applied mutations and closure.

## LEM-18 — Source-bound low repair transfers to two public tasks; single xhigh references miss declared cases

**Valid:** dated 2026-10-30

**Status:** measured; mixed. Both complete low endpoints accepted, but the prespecified GO rule is false because both xhigh references missed a declared case.

**Role / evidence:** detection, correction, selection / controlled proxy.

**Population:** two prospectively fixed public Rust tasks in different repositories: tokio-rs__axum-691 and uutils__coreutils-6690; six valid calls, both single low repairs activated, no paid retry.

**Observed:** Axum low124/125→125/125 versus xhigh124/125; Coreutils low7/8→8/8 versus xhigh7/8. All three Coreutils candidates pass the prospectively added source-bound generated-completion property. Both actual diagnostics were observed verbatim in the primary repair sessions, followed by changed accepted patches. Complete lowL+R costs0.3587821 versus single-xhigh0.4321283 aggregate clientUSD; Coreutils low0.1939923 costs more than xhigh0.1595897. Phase0.7909104; cumulative18.1888028 of40.

**Causal interpretation:** the failed-draft→source-bound diagnostic→correction→acceptance chain transfers beyond the bat development defect. Only low receives evaluator feedback and a second opportunity, so this does not isolate feedback necessity or a reasoning-level effect. Both xhigh drafts fix the main reported behavior but miss an additional declared case.

**Limits:** two selected public tasks, prior Axum controls seen, scoped structured-history census only, pretraining unknown. Coreutils eight-case official target is not the full repository; generated script structure is not interactive zsh. No generic-repair or matched xhigh-repair control, no equal-quality accepted-baseline cost estimate, native/reviewer/billed dollars unknown. Retain both xhigh failures and the unchanged false GO result; test equal checker/repair access on new tasks next.

**Rests on:** [complete report](../research/2026-10-30-workflow-transfer.md), [prospective design and completion amendment](../superpowers/specs/2026-10-30-workflow-transfer-design.md). Raw registration351de5cae422f735992ea7016029eece77512eadd4f52298d3dfbbe482b02529, exact source inventories, primary transcripts, all six charges, independent grades, source reviews, applied mutations and analysis.json are retained in .codescout/resident-private/workflow-transfer-20261030.

## Verification — 2026-10-29

The initial aggregation held twelve indexed entries with matching citable headings and a committed recovery sidecar. After the separately registered pilot, the catalog holds thirteen entries. Final scoped verification compares all twelve original structured rows and definition sections unchanged, including the four prior negative classifications; LEM-13 adds the pilot result. Raw source recounts cover all five compiler-gate records, all five paragraph records, all 35 records in each of four conclude-last arms, and all eighteen Stage C plan rows; the derived metrics and source hashes are retained in the main repository at .codescout/resident-private/low-effort-mechanisms-20261029/source-audit.json. Gate logs and four author transcripts show the actual Stop feedback followed by successful compilation. Other results are attributed to the linked reports/retained summaries rather than falsely described as a fresh raw reanalysis.

For the initial aggregation, the scoped reference audit found 22 resolved references, zero broken references and eleven outside-project paths classified unknown by the checker; those eleven were checked separately for local existence. These local paths are retained evidence, not portable links on another machine. Full Rust/build suites were not rerun for that initial prose-only aggregation, as requested. The later pilot's completed full gate and scoped closure checks are recorded in the LEM-13 report. No frozen experiment evidence was edited.

The behavioral transfer adds LEM-14: the catalog now holds fourteen structured entries and matching citable sections. A complete comparison preserved all thirteen preceding rows and definition sections, including their original classifications. The new scoped reference audit scanned fifteen owned Markdown files and reported44 references:33 resolved,0 broken and11 outside-project paths unknown to the checker. All eleven exist locally; this checks existence, not freshness or portability. The behavioral report records the completed full gate, ten actual code-mutation candidates with zero remaining survivors, three valid31-function parent grades, inactive feedback, raw cost reconciliation and the post-hoc historical baseline failure. The augmentation sidecar shape is unchanged; live params remain catalog-only by repository convention.

External calibration adds LEM-15: read-back holds fifteen structured entries and matching citable definitions. Exact comparison with the preceding sealed tracker-final-v3.json preserves all fourteen prior rows and definition sections. The final scoped audit scans seventeen owned Markdown files and reports59references:48resolved,0broken and11outside-project unknown paths from preserved entries; the display includes50of59findings, so those totals come from the full audit summary. The expanded scope includes eight metadata-only repairs to preceding issue reports and their new issue record. The report distinguishes two inactive accepted calibrations, unstarted holdout/xhigh, the shared candidate/gold empty-suffix crash, and a wider offline dependency limitation. Eight selected guard mutations were re-applied after regression additions:8killed,0survived,0invalid; this is not a coverage claim beyond those eight. Raw primary verdicts and post-hoc qualifications are retained separately.

The natural-failure diagnostic adds LEM-16, for sixteen structured entries and citable definitions. All fifteen preceding rows and definition bodies remain exactly unchanged against the sealed external tracker snapshot. A scoped audit covers four owned Markdown files:46references,35resolved,0broken and11historical outside-project unknown paths. Four valid paid review calls and all first/final parent grades are reconciled; the separate overlap probe preserves primary denominators. Full project gate passes, frozen controller verification passes14tests, and three later closure tests give17passing tests. Two applied closure controls are killed separately from the six selected protocol mutants. Native orchestration cost remains unknown.

The prechecked-repair diagnostic adds a seventeenth structured entry and citable definition; all sixteen preceding rows and exact definition bodies are preserved against the prior sealed natural tracker snapshot. Final scoped audit covers five owned Markdown files:52references,41resolved,0broken and11historical outside-project unknown paths;50of52findings are displayed, so totals come from the complete summary. Five valid calls and actual initial prompt delivery are reconciled. Diagnostic low2/2, generic low1/2 and xhigh1/1 are one known-case development result, not external validation. The full project gate passes. Eleven prepayment tests plus three later closure tests pass14; six selected review mutants and two separate launch controls are all killed after regressions, and two separate closure controls are killed. Native workflow dollars remain unknown.

## Append entries here

The catalog allocates identifiers. Headings are the canonical narrative; there is no hand-maintained parallel table.

## LEM-19 — Matched workflow stops at capped xhigh repair; saved-source checks expose artifact gap

**Valid:** dated 2026-10-31

**Status:** measured; unproven. The matched comparison is incomplete, with Ruff low unstarted after a required invalid-call stop.

**Role / evidence:** detection, correction, scaffolding / controlled proxy.

**Population:** two fixed public tasks, tokio-rs__tokio-4384 and astral-sh__ruff-15394; four paid calls, one activated xhigh repair, two own-parent zero-cost skips, no paid retry.

**Observed:** Tokio low118/118 at0.1023558 and xhigh118/118 at0.3521277. Ruff xhigh8/9 at0.3604388; its repair ended error_max_budget_usd at0.7009894 with observed xhigh. Saved final and an exact intermediate source each pass9/9 in separate unpaid diagnosis, explicitly ineligible for support. Phase1.5159117; cumulative19.7047145 of40 (rounded). Ruff low is unmeasured, not a failure or zero-cost endpoint.

**Causal interpretation:** checker delivery followed by changed passing saved source is observed for xhigh. Valid workflow completion failed. No low/xhigh repair contrast or feedback-necessity effect is identified. The three captured author Cargo checks used identical patch bytes; passing source was available during checking, without proving a cheaper stopping policy.

**Tool contract:** raw CLI records11permission-denial entries. An independent real-shim probe confirms generated container snapshot files are absent from the author workspace and caller INSTA_UPDATE is not forwarded. Their contribution to budget exhaustion was not isolated. Restore constrained author artifact visibility before buying another comparison; preserve independent oracle separation.

**Limits:** incomplete selected public two-task pilot, adapted lock/cache environment, scoped history census and unknown pretraining. Native/reviewer/billed dollars unknown. Previous false GO and invalid completion remain unchanged.

**Rests on:** [complete report](../research/2026-10-31-matched-workflow.md), [prospective design](../superpowers/specs/2026-10-31-matched-workflow-design.md), [open artifact issue](../issues/2026-10-31-cargo-shim-hides-generated-snapshots.md). Registration f59bf46ea77467bfda1ad2ad5a7a9de4a470efda5f4b9a27b1e097b2ba6d6cba and all actual charges, source-bound controls/reviews, mutation logs and diagnosis remain in .codescout/resident-private/matched-workflow-20261031.

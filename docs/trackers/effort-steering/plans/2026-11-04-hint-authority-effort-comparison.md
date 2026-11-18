---
id: 6f6c445bbf50937e
kind: plan
status: active
title: Compare hook authority with low/xhigh effort using matched functional outcomes
tags:
- effort-steering
- harness
- evaluation
---

# Hook authority and effort comparison plan

> **For agentic workers:** Use superpowers:executing-plans for native execution. Preserve the operator's standing autonomy and budget; no duplicate approval request for ordinary reversible preparation.

**Goal:** Determine whether the user-endorsed cs-hint clause improves low-effort functional outcomes and reduces the quality gap to xhigh at lower whole-workflow cost.

**Architecture:** Reuse the qualified real-client/native Codescout environment and cached onboarding. Hold the current common runtime fixed and vary only effort and the authority clause. Use the existing hook-authority experiment as prior process evidence, not as functional acceptance data or a historical control.

**Tech stack:** Frozen Claude Code2.1.309, claude-sonnet-5-5, native Codescout MCP, existing cached onboarding and executable acceptance checks.

**Spec:** [Audit and design](../reports/2026-11-04-claude-instructions-comparison.md), artifact d2b9a7be7899af27. Operator confirmed the cs-hint authority treatment.

## Global constraints

- No permanent global CLAUDE.md/profile or production hook changes for this comparison.
- Clause bytes: use the exact `DECLARATION` string in `tools/hint-channel-eval/hook.py`, including literal backticks around [cs-hint]. Read, hash and freeze the actual constant; do not paraphrase or copy a Markdown-rendered version.
- Four paired cells: low/absent, low/present, xhigh/absent, xhigh/present. All other instructions, hooks, onboarding, tools, tests and task bytes identical.
- Model output_config.effort must actually equal the registered level; never infer it from a launch argument alone. [Anthropic effort documentation](https://platform.claude.com/docs/en/build-with-claude/effort) lists Sonnet5.5 support; actual client/provider compatibility remains a runtime check, including thinking mode at xhigh.
- Historical registration used a40USD thread ceiling and a4USD original pilot. Operator's2026-11-05 larger-budget instruction supersedes the thread ceiling: the current finite registration is60USD, with known prior33.571720199999999964 and a fresh8USD pair allocation (4perarm). The original pilot remains closed at2.57626490000000013/4; never reset it. These are CLI-reported model costs, not complete research/setup expenditure. Reserve both arms, stop on unknown/unpriced consumption, and charge failed attempts.
- No repeat onboarding; new tasks require explicit cached-context suitability and task/oracle admission before payment.
- The exposed RA22849/bindgen3390 cases are development controls. No claim of fresh validation from rerunning them.
- Report workflow cost and functional acceptance separately from hook compliance. A request to run references and a successful references call are not correctness evidence.

## Review focus

- Missing actual instruction text or hook event means the treatment was not delivered.
- A provider accepting the request without honoring effort must not silently qualify a low/xhigh comparison.
- An arbitrary or wrong hint may raise irrelevant tool use; delivery/obedience must not be scored as helpful behavior.
- A hook that stops the run before the first edit cannot measure accepted repairs.
- Partial-block stopping, errors and unknown cost must remain in accounting and cannot become selected successful cases.

## Task1: Freeze and qualify the isolated treatment, without provider spend

**Existing seam:** tools/hint-channel-eval/run.py stage_run creates A3/W3 CLAUDE variants; hook.py carries DECLARATION; score.py score_run stops at the first edit. worker-context-verification-20261104/client_context_capture_v3.py proves actual real-client instruction delivery with a scripted loopback endpoint. Reuse these mechanisms after reading their current shapes; do not invoke stage3.main or run_one by accident.

- [x] Freeze canonical DECLARATION, prior-context absent/present CLAUDE instruction templates and manifests in `/home/marius/.cache/codescout-resident/codescout/hint-authority-effort-20261104`. Complete global rules unchanged (3014B); project template2274B, present variant2405B. Preserve source selection user,project,local in subsequent launches. No fresh functional task or actual hook delivery is qualified by this template freeze.
- [ ] Replay all four cells through the existing actual-client capture. Require full visible global/project instructions in all cells, declaration absent in exactly two and present in exactly two, native Codescout tools actually offered, same hook implementation/hash, and a delivered cs-hint in every applicable diagnostic.
- [ ] Apply a deletion mutation to the declaration and a hook-disable mutation; observe delivery qualification fail separately. Observe a request-effort mismatch fail its check. Restore originals and observe qualification pass. Scripted responses prove transport only.
- [ ] Freeze a single shared Codescout/companion build for all cells. Current live server changed to clean aebb0e76f after restart; do not transfer exact-artifact qualification from the older8f567f17 binary without checking the intended worker package.

## Task2: Qualify a functional task pair and run the bounded pilot

**Existing evidence:** the ten signature-change tasks in docs/evals/data/2026-11-04-harness-channel-hints/tasks.jsonl have stopped-before-edit process scores, not functional outcome scores.

**Execution ruling (2026-11-04, before paid outcomes):** Run one qualified task as the first four-cell block, then assess the budget before admitting a second. The task has a new explicit functional contract and private executable oracle, but belongs to the population exposed in the separate HCE process study; call this a diagnostic, never external validation. All current cells freeze companion1.20.29 (the older cache was removed), the exact controlled native package and a research-only PostToolUse hook. Runtime/input hash binding supersedes assumptions that named cache paths stayed immutable. See current execution report f789fec3ea4737ba.

- [ ] Select two fresh tasks from already-onboarded projects using a fixed documented eligibility rule, before inspecting new arm outcomes. Require an actual baseline failure and an independent executable acceptance/regression check. If no eligible pair exists, record why and do not buy another compliance-only replication.
- [ ] Verify task source/knowledge populations and held-out oracle isolation against actual worker mounts and tools. Demonstrate that the worker can run relevant public checks without seeing held-out expected answers. Verify source/config inventories before/after and exact owned-resource cleanup.
- [ ] Use a controller that permits implementation through completion. Reuse an already reviewed paid controller if one satisfies the interface; the HCE first-edit stopper does not. Any new load-bearing controller must have observed failing/positive controls and applied-mutation review before paid use.
- [ ] Register run order low/absent, xhigh/present, low/present, xhigh/absent on the first task, reverse on the second. Treat eight runs as a diagnostic, not a powered validation. Keep invalid attempts and incomplete blocks visible.
- [ ] Within the4USD pilot ceiling and total balance, perform the first compatibility check with an actual request capture. Stop if the requested effort/treatment does not reach the provider or if cost is unknown; do not silently fall back to a different effort.
- [ ] Finish eligible paid blocks only within the reserved budget; assess independent task acceptance, preserved regressions, complete cost, completion and timing. Report hint delivery, relevant evidence use and harmless-decoy behavior as secondary diagnostics.
- [ ] Publish a local report with raw receipts and explicit counts/denominators. Do not promote the mechanism from a couple of process improvements or spend the rest automatically for more precision.

## Completion rule

**Checked outcome (2026-11-04):** First four-cell functional diagnostic is complete; report f789fec3ea4737ba. Terminal-verified cost1.60655589999999997USD including prior aborted compatibility attempt. Only low/absent produced an accepted patch; three candidates failed compilation before functional tests. All four were reserve-censored and none completed normally. Therefore do not infer effort superiority or an authority-clause effect, and do not auto-spend on the second task. Next prerequisite is a budget/control policy that allows finite normal completion and distinguishes budget censoring from transport failure; existing complete context/runtime/onboarding qualification must be preserved. Second full development gate exited0. No further provider calls during the status check.

A useful result is either a correctly qualified diagnostic comparison with functional outcomes and honest limits, or a concrete failed prerequisite that prevents another uninformative paid experiment. The current audit itself spends0new provider dollars and does not claim this plan has been executed.


Controller-correction checkpoint (2026-11-04): issue 4b4e9c353cea58c5 now has an implemented, independently reviewed correction. Paid proxies receive the terminal-verified campaign balance rather than a separate $1 run ceiling; actual campaign exhaustion is recorded separately. Offline replay admits all four historical cutoff requests under their actual remaining balances. Pilot28/28 and resume3/3 tests pass; three observed launcher-test survivors were reapplied and killed after closing coverage. Full isolated-worktree development gate @bg_00000020 exited0 with all five lanes0; its stated coverage exclusions remain. This does not demonstrate real model completion and spends no additional model budget. Historical qualifiers are invalid for changed controller bytes; next work is a fresh isolated zero-cost qualification, then assess the remaining authorized budget before a matched paid completion check. Do not reset the closed pilot ledger or count old censored outcomes as completed workflows.


Zero-cost requalification checkpoint (2026-11-04): @bg_00000027 exited0; all4 corrected-controller actual-client scripted cells passed and private controls behaved correctly. Baseline compiled/ran and failed both intended assertions; positive passed2 private,9 chunk and39 indexer tests. Independent readback verified terminal costs0, on-wire hints, current fingerprint0ef13885b2c5021225fd6551df6b7990480c4110808e2f41fd8c96ff0c715355, native mounts and eight worker containers absent. New issue36f004f53aae1e7c records that the generic boundary predicate lacks a host-canary-presence witness; this run has an independent presence/digest witness, so it does not rely on the faulty predicate alone. Source remains clean at979fffc58. Durable summary docs/research/data/2026-11-04-hint-authority-requalification.json. No additional provider spend or real-model completion measured. Ledger rechecked: original4USD pilot balance2.39344410000000003USD; thread40USD balance9.172289800000000196USD. Next bounded step is a matched low/xhigh completion check under one fixed declaration state with all prior spend carried, before attempting another factorial task block. No cap reset or automatic second-task run.


Operator go after requalification authorizes one fixed low/absent then xhigh/absent matched completion-calibration pair on the same task, within original4USD pilot cap including all historical1.60655589999999997USD. No new task or authority factorial; CLI target0.65/32turns/8192output unchanged. Q/completion-plan.json is registered before payment. New isolated wrapper revalidates historical terminals, hashes/review/canary, saves known cost before grade and disallows retries via exclusive marker. Five new wrapper tests RED→GREEN; independent applied-mutation review pending before first paid call. Report f789fec3ea4737ba and memory research/effort-steering/hint-completion-pair-20261104 carry exact launch/recovery paths.


Matched-pair checked outcome: paid job @bg_0000002a exited0, low0.40236020000000006USD and xhigh0.5673488000000001USD; both independent private grades passed. Neither hit host/CLI budget or transport failure. New modelcost0.96970900000000016USD; cumulative pilot2.57626490000000013USD of4, thread31.797419199999999964USD of40. No unknown cost or automatic further spend. Source commit083d95a8b28e55fc3ad5a11caf7cc9b40969bd60, V clean;9completion+28pilot+3resume tests and full Vgate@bg29 allpass.

The complete-workflow prerequisite is still NOT met: both models explicitly yielded awaiting background completion wakes, and the frozen one-shot driver cleans up after a single successful CLI turn. Low left clippy unobserved; xhigh observed clippy0 then left lean/default results unobserved. Actual outcomes are2successfulCLIturns+2acceptedpatches+0workflows with all mandated public checks observed, not2fullyverifiedcompletedworkflows. New issuebf167b83c0166a24 and durable completion-validation-protocol.json preserve this lifecycle finding. Next work is a generic bounded background-job settlement/continuation mechanism (or truthful one-shot tool contract), verified with offline lifecycle controls before another paid replication. Do not spend remaining budget to remeasure effort before this prerequisite is resolved. Exact evidence/report f789fec3ea4737ba and memory research/effort-steering/hint-completion-pair-20261104.


2026-11-05 offline checkpoint: standalone background-continuation component completed under plan6b6f3546c3761800, source f89af31e371268944b4e5e920f27a591bfe7bf75 on research/fixed-acceptance-population-20261101; patch-id b6635eb237a411a4bc62a3184794683a694a15a2. Native negative/success/exit7 controls passed and independently checked, zero provider calls. Lifecycle18tests, prior28/9/3 regressions and fullgateall5lanes passed; 12non-equivalentmutationprobesallkilled after fixing3reviewbugs. Report7447904928197ebf. Legacy paid launcher unchanged. The parent prerequisite is still open: compose lifecycle into a new worker with continuous cost accounting, qualify it, and separately observe required-public-check completion. Do not use lifecycle settlement as a replacement for that measurement or resume the closed paid pair. Thread budget remains31.797419199999999964/40USD.


2026-11-05 composition checkpoint: plan22ed6d6c4e0c2288/report0185b660ea701471 complete, source 0998c3b64f81c13331db26e0091ff25404272c66 (patch-id 971ae969a0c1417a98d387e1c314a1fec4dbf3b2), clean isolated research worktree. The infrastructure prerequisite is now satisfied by the explicit opt-in workflow mode: true native public-command results, same-session failure feedback and fresh complete verification round, one continuous budget, truthful unknown-cost and timeout outcomes. Corrected actual-client controls3/3 at$0,34tooltests, prior28/9/3, fullgateall5lanes0,13substantivemutationsallkilled. Paid repair comparison is still pending: fresh fingerprint must cover new modules, manifest, runtime and controls; do not reuse legacy qualifications or reopen the closed pair. Budget unchanged31.797419199999999964/40USD.



**Situation checkpoint (2026-11-05):** Report c5dc7388ef84e4b5, docs/research/2026-11-05-effort-steering-situation.md, distinguishes accepted patches from unfinished public workflows and scripted infrastructure controls from model-quality evidence. Current source0998c3b64 is clean/local. New zero-provider full Rust public-profile positive control @bg_00000047 is running on the known reference in /tmp/verified-public-profile-01a15ca9-20261105; no outcome claimed yet. Preparation composition inventory b541c9985e029599ff1ce988eede755f5615d1612566a6d067c5dee8cdb9ef9d is NOT paid admission. The old USD1-per-run admission rule cannot reserve both cells from the original pilot remainder USD1.42373509999999987. Do not launch a partial pair, reset historical costs, lower the reserve to force admission or reuse the legacy fingerprint/review for new modules. Thread remainder USD8.202580800000000036 remains distinct; any fresh calibration needs explicit bounded allocation and full reviewed composition. No new model spend.



**Full reference gate qualified (2026-11-05):** situation report c5dc7388ef84e4b5 now records final v6, all four public commands exit 0, independent readback and archived failed attempts. New memory research/effort-steering/public-profile-20261105 is authoritative for the control sequence. Required runtime settings are disk-backed actor/build/evidence, CPU quota 2, PID ceiling 512 and init reaping; actual worker Git setup was reproduced. No source/test/acceptance relaxation and no provider cost. Next is minimal integration of the verified storage/resource settings into opt-in worker mode, tests/review/gate and a fresh full composition fingerprint before paid calibration. Neither the original pilot cap nor closed cohort markers changed; new allocation must be explicit and within authorized total. Do not redo the already completed reference controls on unchanged source.

## Current paid calibration checkpoint — 2026-11-05

Read reportd7d2a509aa58c121 and recovery planea0f4cf74fd4ba8f before acting on the older task checkboxes below. Source0f7fb1dd69f6807e4a5564c25ce379bf267c7fc5 is locally committed/reviewed/full-gated; actual cold reference full public profile and corrected native four-cell transport qualification passed. Budget cumulative60USD, prior32.222358599999999964 including the aborted LOW, freshpair8/perarm4. Full onboarding and global/project instructions are reused. No merge/push.

Fresh low/xhigh declaration-absent baseline calibration is ACTIVE@bg00000073, resident scratch complete-workflow-cold-recovery-20261105. Source frozen; do not reset/relaunch. Older complete-workflow-calibration-20261105 cohort was intentionally stopped after LOW cold deadline, xhigh unstarted, and remains archived/charged. New current outcomes and costs await observation. This baseline measurement does not yet isolate the authority declaration or demonstrate generic transfer.

When this pair is fully read back: decide whether actual complete-workflow evidence is valid, then return to the registered authority comparison and untouched external task population. Avoid another broad infrastructure pass without an observed blocker.

## Latest runtime checkpoint — 2026-11-05

Opt-in runtime integration is committed locally as `7e57bf8f91a2fd369411a2d89db51cf97c070d9f`, patch-id `528637cc65548ba4cb3f84424d2c8ebf725ec148`; V clean. Plan ef4f575ec0760ac0 is complete for this infrastructure stage. Report328a44811943317e records42lifecycle tests, all5Vgate exits0, four final-byte actual CLI/native scripted cells with independent readback, initial12appliedmutations11killed1survived and focused8variants5killed3redundant survivors plus3greencontrols. Actual full Rust reference gate was separately qualified in v6. No new provider cost or paid admission.

The preparation composition digest isbb3ee8c6c57e09c6b848ae7b543d62049104d9a9c10aca6dadec73a5319fb1ed (36measured composed files plus frozen runtime/input fingerprint). It intentionally cannot authorize paid use. Next: a fresh exclusive matched full-workflow calibration driver, final launch/budget plan, full cost/reserve ledger and acceptance/context controls, all included in review/fingerprint. Do not reopen prior cohorts, reset old pilot spending or interpret scripted success as low/xhigh quality parity. Existing authority-factorial checkboxes remain historical/pending their own valid experiment; this infrastructure checkpoint does not mark them satisfied.

## Latest measured checkpoint — closed cold pair and action policy

Supersedes the older runtime next-action paragraphs above. Current reportd7d2a509aa58c121 records closed pair cost1.3493616: LOW completes public+private; XHIGH's patch passes private and separately all four public commands, but its original workflow is censored by inherited32turn cap. Neither runtime nor censored arm establishes generic quality parity. All costs preserved, cumulative33.571720199999999964; finiteceiling60USD under operator's larger-budget instruction.

Next narrow step is planc336f2784639da9c: omit32turn flag only for opt-in workflow, retain4USD/3600s/4continuations andlegacy32. Exact5path review accepted; real provider-disabled client/native control observed51assistanttool_use calls before first result and completed background/public settlement. Full gate@bg_00000079 stillrunning at this checkpoint. FreshROOTcomplete-workflow-action-budget-20261105 prepared with8USD/4perarm but no paid admission until final gate/commit/sourceproofbinding. Reuse onboarding and unchanged cold/private controls. After a valid complete pair, proceed to untouched task population/ablation; do not reopen old implementation checkboxes or rerun closed cohorts.

## Current outcome and research direction — 2026-11-05

Action-policyplanc336f2784639da9c iscomplete. Freshuncensoredlow/xhighworkflowspasspublicchecks, with privatefunctionalacceptanceofbothafterseparatelydocumentedgradercorrection241d6eb64c605b37. LOW0.48744760000000004USDvsXHIGH1.1965462999999996USD; phase1.68399389999999964, cumulative35.255714099999999604. Rawregisteredacceptedfalseforxhighispreserved; supplementaladjudicationcorrects'0 passed;'suffixbugandverifiesall866rawpayloadhashes/requiredtestnames/controls. Sourcefix774d81703ec14efc9fa50c72d4a3eb6380258c13/fullgategreen; LMC-4trackerrecordd3c1d104708df2d21a6cf6920d72e3af8ff9ddc6. No newproviderchargesduringcorrection.

Thisissingleexposedtask/fixeddeclarationstatefeasibility, notauthorityfactorialoragenericmechanismresult. No publicfailure/repairfeedbackwastriggered. Do not reopenruntimecalibrationorcounttheoldpendingfactorialcheckboxesassatisfied. Activefollowupae0b584f43fc96e8 admitsoutcome-blindNushell18863/RA22992withreusedprojectknowledge/fullinstructionsandprivateoracleisolationbeforeanotherpair. Currentdrivercannotadmitanotherrootwithitshistorical33.57172prior; carryallclosedcostsforward.

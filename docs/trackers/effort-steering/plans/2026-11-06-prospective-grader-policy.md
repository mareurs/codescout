---
id: eb3326eb00009864
kind: plan
status: done
title: Prospective grader policy and fixed native controls
tags:
- effort-steering
- evaluation
- grader
---

# Prospective grader policy implementation plan

> **For agentic workers:** Execute inline using superpowers:executing-plans under the user's standing autonomous instruction. One fresh whole-change review with applied mutations; one offline fix pass. No additional review cycle or repeated closed experiment.

**Goal:** Qualify an explicitly declared live adjudication policy with six fresh native controls, preserving the raw grader and frozen historical evidence.

**Architecture:** New generic grade_policy.py classifies public boundary results and adjudicates private raw grades using the unchanged record parser and reviewed retrospective helper. A private source-bound runner uses the existing live_grader and exact images/inputs, observes runtime/cgroup health, then checks negative/gold/repeat discrimination. It does not launch models or modify public workflow integration.

**Tech Stack:** Python stdlib/unittest/pytest, Decimal-free zero-provider controls, Docker, SHA256/JSON.

**Spec:** docs/superpowers/specs/2026-11-06-semantic-contract-pilot-design.md, especially Prospective policy v1 and Grading and admission.

## Global Constraints

- Existing isolated worktree codescout.worktrees/grader-validity, clean base a2ffc143266c5e828afd7d8ef2ceb665318df49f. New source only; keep prior source and cohorts unchanged. No merge/push.
- Keep existing private resources4CPU/8GiB, step timeout3600; actor public32GiB is a separate qualified profile. Six new controls in fixed order NUnegative,RAnegative,NUgold,RAgold,NUrepeat,RArepeat. Each fresh container, no retries/resampling.
- Bind all source, exact image IDs, bundles/records/gold/expected sets and protected old hashes before run. None of these oracle bytes reach repair actors.
- No model/provider/token/onboarding. Host floors80GiBroot,4GiBtmp,48GiBMemAvailable at starts/boundaries and10second samples. Kill only the exact container created by this runner on runtime failure; retain attempt/cleanup evidence. Independently query daemon and container absence.
- Raw log/grade/parser/results immutable; new policy results separate. Capture complete full-step evidence, not raw return's stripped step list. Missing/truncated/unknown evidence fails closed.
- Paid and public-workflow integration stay unadmitted here. No repeated closed NUlow diagnostic or six CLI delivery fixtures. Archive all attempted controls, including invalid runs.

## Review Focus

- Some parsed tests must not conceal a fatal toolchain/storage/OOM event.
- Ordinary candidate compiler failure must remain semantic even when no statuses exist; a compiler source excerpt must not imitate infrastructure.
- Recovery must observe expected P2P too, keep original failure and exact physical provenance, never invent a missing result.
- Nonzero print status/truncation/missing or permuted full steps must not produce a valid pass from partial/stale output.
- Missing cgroup probe, host floor failure or cleanup uncertainty cannot be silently zeroed; no second control after invalid measurement.

### Task 1: Generic policy and behavioral tests

**Files:** tools/setup-ab-eval/grade_policy.py and test_grade_policy.py in isolatedworktree, committed867c680924cb75575bec7ccd7068b8dfd7fd5889.

- [x] Safe no-op tests observedRED before implementation; initial8failed6passed/23subtests.
- [x] Fail-closed policy preserves rawrecords, exactexpected-key recovery and physicaldiagnostic evidence; final review tailguard regression observedRED beforefix.
- [x] Four-log logical calibration matches all4knownkinds,17oldfilesunchanged; explicitly syntheticruntime. Both earlyfailedcalibrationsretained.

Historical pre-review targeted254tests123subtests; finalexpanded393tests138subtests, distinct scopes. Nativequalification below is actualexecution, not thosefixtures.

### Task 2: Fixed control runner and runtime observation

**Files:** Resident qualify.py,test_qualify.py,register.py,progress.md.

- [x] Deterministic tests firstRED thenGREEN; final native wiring cases now observe actual native_attempt/monitor boundary with fake Docker/grader, not whole-attempt substitution. Offline only, not native qualification.
- [x] Existing live_grader unchanged, fullgrade.json tails required, exact owned container sampled beforedeletion and independentdaemonabsence.
- [x] One fresh review10actualmutants5killed5survived,2Importantissuesfiled. Singleofflinefixpass: tailguardRED→GREEN, five survivor gaps covered. Exact10reapplied/10killed0survivors, complete pytest summaries verified and restoration hashes equal. No resite/secondreview. Final expandedtargeted393tests138subtestsPASS; fullgate133CLOSED0/all5lanes0/122s. Pythonlight1642tests529subtests separate population/exclusions retained.

### Task 3: Native qualification and durable handback

- [x] Commit867c680924cb75575bec7ccd7068b8dfd7fd5889 contains exactly2newproductfiles; cleanisolatedworktree, nomerge/push. Registrationddadac4045041a8344077f2d78a61e49dae12998d86ff7360fff09952a4a3fc5 binds80paths, frozenpayloadmappingsverified beforestart/afterclosure.
- [x] Sixfixednativecontrols137CLOSED0. BothnegativesvalidandallF2Pfail; gold/repeat NU8622/RA8182selectedexpectedpass/statusmapsidentical. NUrepeatrealgluedstatusrecoveredfromphysical3725/rawunchanged. Allresourceprobes/freshdaemonabsence/80boundhashesverified inclfullclosedpolicyrecomputation; no retry/native/modelrerun.
- [x] Archive138CLOSED0: docs/research/data/2026-11-06-prospective-grader-policy-evidence.tar.gz, SHA2562bc2fc8706b4b4d7c3d302e6b6cdd0b832704903db3bd4fe4ba4b23e7cf17fbc,14758129bytes,2570payloads+manifest allreopened/hashverified. Reporta812bb15c2d91b80 andissues/currentmemory recordclosedqualification; paid/publicadapterremainsseparatenextscope.

**Next work, explicitly outside this plan:** concretepaid/publicworkflowadapter withpolicybeforecontinuation andmidstage-resourcestops, thenfixedsixpilot. Privatequalificationdoesnotadmitpaid/publicworkflow, mechanismstillunmeasured.

## Rulings

User delegated the implementation method and asked go on this exact next step. Continue inline without another design/permission handoff; source-specific choices are declared in the spec before native controls. Cost if wrong: reject the fixed controls/admission and revise a separately versioned registration, never silently reinterpret or resample.

Use resident scratch instead of in-repo .superpowers workspace to avoid the previously observed watcher/disk expansion; archive its ledger. Batch the small coupled product policy/tests into one final validated source commit. Preserve the entire old correction branch/worktree and MAIN foreign index.

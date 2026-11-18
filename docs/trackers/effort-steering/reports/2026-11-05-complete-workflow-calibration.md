---
id: fee7802dbe77506b
kind: research
status: done
title: Fresh low/xhigh complete-workflow calibration
tags:
- effort-steering
- evaluation
---

# Fresh low/xhigh complete-workflow calibration

## Goal and registration

Test whether the corrected workflow can finish an actual low/xhigh repair pair, with all public checks and private acceptance observed. This uses the exposed hce-07-build_chunks diagnostic task and cannot establish generic transfer or an authority-clause effect.

Operator allowed a larger allocation to avoid censoring the final comparison. New allocation: up to USD8, USD4 per arm, within existing thread cap USD40. Prior thread spending USD31.797419199999999964 and original pilot USD2.57626490000000013 remain charged. No cap reset. Both arms are reserved before the first paid call; an unknown cost stops the campaign.

Fixed cells: low then xhigh, declaration absent. Same frozen source, prompt, full global/project CLAUDE.md, Codescout, recon skill and reused onboarding. CLI max32turns/output8192; public profile unchanged, 1800s workflow deadline, 900s per check, at most4continuations. Private oracle stays outside feedback and runs after the loop on a separate copy.

## Implementation and validation

Isolated source worktree /home/marius/.codex/worktrees/615e/codescout, base7e57bf8f9. New tools/hint_authority_workflow/{driver.py,test_driver.py,README.md}; optional validated client_budget_usd in worker.one; through-worker regression tests in agent_lifecycle/test_runtime.py. Default legacy CLI0.65 is retained. Custom budget requires workflow, positive<=4, and cannot exceed paid proxy allowance.

Driver ledger writes cost before grading and marks numeric totals nonfinal while an attempt has unknown cost. An exclusive start marker prevents retries/reopening. Success requires known cost, valid full context, settled lifecycle, every registered public check passing, safe transport/cleanup and private acceptance.

Observed tests: driver17, lifecycle44, pilot28, completion9, resume3 passed. Initial review applied12mutations:6killed/6survived; one survivor is redundant under the fixed historical-total invariant. Five real guard-coverage gaps were immediately filed and targeted regressions added. Focused recheck is pending. Parent also reproduced/fixed ambiguous ledger finality with an observed RED14tests1failure followed by GREEN. Issues:b0214bf90135b9b1,7ecbe51427e022c5,d468588cea173d29,edf04832a5809d11,c29e0d85ccbd0d27,c51237303dcfa962.

## No-cost actual controls

Job@bg_00000062 exited0: four actual CLI/native controls at explicit CLI budget4 (low success, low failure/repair, low unknown timeout, xhigh success). Independent raw readback checked efforts, session continuity, public rounds, native resources and12exact absence inspections. No provider forwarding or cost. Full context, hint delivery and denial of the named host canary were independently checked on all four wire histories.

Private controls were reused only after reconstructing identical candidates with the current unchanged oracle and comparing all2060files in each source population to the original graded copies. The baseline uses the old API and has two observed assertion failures; the reference passes both acceptance tests and both selected regression filters. Actual Docker argv/image, logs and hashes are retained. An initial byte check caught the wrong baseline API in disposable staging; failed staging remains preserved, corrected v2 matches exactly. No repeated LLM onboarding or Rust compilation for identical controls.

## Current state and next action

Source committed locally as `c278effb9b36259480e079051d16238d66314e8e`, stable patch-id `3daaab6a1223c754beb4366f6425de4bd6e26d45`, canonical Session-Id; V clean. Full gate@bg_00000063 exited0, all five lane exits0. Final independently restored offline suites pass17/44/28/9/3 tests (101 test methods across five suites). Normal gate exclusions and the commit hook's no-runnable-binary artifact lookup skip remain explicit.

Review accepted. Initial12applied mutations:6killed/6survived; focused6applied after fixes:6killed/0survived. This is not a re-run of the complete original matrix. Final source/proof hashes and composition `faf053c592896d9bdabb98ec554258a4584abdfdf89e4c04bee0b87a9bfa739a` were independently checked. review.json installed from the actual review binding after clean commit/gate; actual --check passed with zero provider calls.

Fresh paid pair has now been launched with `driver.py --paid` against the registered root. Outcomes and final costs are not yet known. Do not infer success from launch or a CLI terminal. Preserve all in-flight state and never rerun this root. MAIN documentation/evidence remain local under provenance issue421ed73df25f795d; foreign staged work is untouched.

## Aborted cold workflow — observed result

LOW spent USD0.4249394 in19forwarded model requests. Its1801.1547937230207s workflow expired during an agent-started cold Rust background check: lifecycle incomplete, controlled public verification never started, no budget stop or transport error. The isolated private grader passed2acceptance,9chunk-filter and38indexer-filter named tests. These populations overlap; do not add them as independent tests. This is private patch acceptance, not complete workflow success.

Remaining xhigh admission was deliberately withdrawn before it started by preserving the accepted binding as review-before-cold-deadline-revocation.json. Driver exited1 on the next missing review.json check. No provider request for xhigh. One paid arm, zero completed/accepted workflows; no low/xhigh comparison is valid.

Independent aborted readback checked all19raw request contexts, latest terminal/receipt/forwarded cost coverage, unchanged composition and review/proof hashes, private test populations and three absence inspections (two native names plus the observed LOW immutable ID). The full native background log was not copied before teardown; bounded earlier observations are partial. Open issues:f458f9840337469e cold-deadline admission,39785443931155d4 background owner identity,7aa44115b3456514 log retention. Readback false-positive9fcdf784e795e02a concerned reference-added tests, not candidate acceptance.

Known thread spend is USD32.222358599999999964, including this failed attempt; original pilot USD2.57626490000000013 remains unchanged. User permits additional budget; no unlimited spending is inferred. No new paid call until a fresh registered resource/deadline profile is qualified.

Preserved archive: data/2026-11-05-complete-workflow-evidence.tar.gz, SHA25635e6b5523aaf29f3ef896c3bcc31d9e3623bb1b581782c4b272f6301cee835c8;485payload files, all hashes checked on readback. Results and independent verification JSON are adjacent. Vsource c278effb9b remains unchanged, no merge/push.

## Prospective cold control

Zero-provider known-reference full public profile is now running at resident scratch public-profile-cold-20261105-v1, job@bg_00000069. Controller chose CPU4,memory12GiB,init/PID512,total3600s/percheck1800s; unchanged four public commands and assertions, no filters, no shared build-cache mount. Actual Docker witness confirms resources and target absent before the first command. This is a combined infrastructure bundle, not CPU-versus-memory causal evidence. Success is not yet known.

## Cold recovery preparation

Full cold known-reference control@bg69 EXITED0: all four unchanged public commands passed634.6421497459523s. Actual CPU4/memory12GiB/init/PID512, no target cache mount and target absent before first command. Source bytes unchanged; native runtime created three .codescout files, which are listed separately in independent readback. Raw proof archived in data/2026-11-05-cold-public-profile-*. No model/provider calls.

Controller sets finite cumulative thread cap60USD under operator's permission for more. Prior32.222358599999999964 remains charged; next pair allocation8USD/4perarm. Driver reconstructs the aborted LOW cost from raw terminals/receipt and binds the exact cold command/resource/deadline proof and log hashes.

Recovery source in V now eleven exact paths, not yet committed. Private staging had19driver/55lifecycle tests; later callback/order/review regressions yield21driver/56lifecycle tests. Existing pilot28/completion9/resume3 also pass. Independent reviewer initially13applied mutations:11killed/2survived; both survivor gaps now have focused tests (command cohort and aborted raw-cost callsite). Additional teardown-order mutations exposed a scheduling-sensitive test; replaced with deterministic delegating stdin.close/killpg recorders around a real subprocess. Final focused verdict pending.

Actual first native qualification@bg6e failed on missing log capture after CLI auto-removed its container. Preserved, zero provider cost. Source now snapshots before client stdinclose/error-groupkill. Fresh native qualification@bg6f in qualification-budget4-v2 EXITED0, low success/repair/unknown + xhigh success all read back, captured logs complete,12absence inspections (two names plus immutable ID percell), provider0. Full context/hint/host-boundary wire audit and runtime source hashes bound. No campaign start marker or paid run yet.

Full Vgate@bg6d remains active. Final reviewed source hash manifest /tmp/workflow-cold-recovery-final-source-hashes-01a15ca9.json. Fresh root complete-workflow-cold-recovery-20261105; old aborted cohort stays untouched. After final review/gate/commit and no-cost preflight, launch only this fresh root. Generic mechanism is still unproved.

## Fresh recovery pair launched

Committed exact eleven recovery paths locally as `0f7fb1dd69f6807e4a5564c25ce379bf267c7fc5`, stable patch-id `31f764c7402839b87c20a0afb29d34ee5ca8ca46`, canonical Session-Id. V clean; MAIN foreign staged blob unchanged. No merge/push. Full gate@bg6d exited0 (Python/fmt/clippy/lean/default), with normal heavy Python/server-stack/ONNX exclusions. Final explicit five suites21/56/28/9/3 passed (117 test methods).

Review accepted exact composition `55e1053bc4b8592424c398a3deb73ae951a5b84a7e99aaaa739cc31927470711`. There were23mutation executions,19killed/4historical survivors; all4 survivors were later exactly re-applied and killed,0unresolved observed survivors. This is an execution ledger, not a count of unique mutations or a coverage percentage. Both actual native qualification and cold full Rust control passed independently of the reviewer.

Actual no-provider --check passed after binding source commit/review/three proof hashes. Preflight archive data/2026-11-05-cold-recovery-preflight-evidence.tar.gz, SHA256765ae0f4bbe356fdaa821e3aeff69e6824a42e5bd39ca5dd0e0702428f965b9a,471payload files, every payload hash verified. The external post-run verifier had a one-character syntax typo corrected and py_compile checked after that archive; the original archive stays immutable and the revision is recorded in campaign/post-run-verifier-revision.json. Frozen campaign runtime is unchanged.

Paid driver job@bg_00000073 has now started against complete-workflow-cold-recovery-20261105 (record /tmp/codescout-bg/455879-a720241d-bg_00000073.json). LOW is active,13request artifacts at the first observation, no terminal or final receipt yet. Independent live Docker sample confirms CPU4/memory12GiB/init/PID512 and writable exclusive actor. Final costs/outcomes remain unknown; ledger cost_is_final=false. Prior known32.222358599999999964 remains charged, ceiling60, newallocation8/perarm4. No restart/resume/reset.

Next: observe both full workflows, reconcile terminal costs, read all public/isolated private results and cleanup evidence, run independent raw readback and archive final outcomes. No low/xhigh or generic-quality inference yet.

## First complete LOW observation; xhigh active

LOW receipt: workflow_completed=true, public round0 completed/passed, all four exact registered public commands exit0, settled lifecycle/two turns/one background continuation, final cost0.4205932 with19/19terminal-forwarding coverage, complete752912-byte native log snapshot and cleanup_error=null. Workflow wall750.7413746700622s; outer driver wall1087.402699128259s includes setup/private grading.

Private grader subsequently returned0 with execution witnesses for all three registered filters; driver accepted=true. Independent full raw readback waits for both arms and will verify exact named test populations. This is the first observed actual full public+private success in the recovered workflow; no generic parity or mechanism attribution follows from one exposed task.

Xhigh has now started. Independent live resource/ID sample recorded; same CPU4/memory12GiB/init/PID512 policy. Current aggregate cost_is_final=false because xhigh is awaiting reconciliation. LOW's reported CLI cost is known; total pair cost is not. Public workflow latency excludes private grading; outer timings include a shared grading-cache asymmetry and must not be compared as symmetric model latencies. Research setup/review cost is not measured by the per-run CLI cost instrument.

## Closed pair — one complete workflow, one turn-censored arm

Paid job@bg73 EXITED0 after recording/grading both arms. Independent raw readback passed all51request contexts, latest cumulative terminal/receipt/stdout cost corroboration, public manifest/captured-log hashes, unchanged private baseline test populations and six Docker absence inspections. Two recorded arms does not mean two completed workflows.

| Arm | Reported CLI USD | Public workflow | Private grade | Workflow wall |
| --- | ---: | --- | --- | ---: |
| low | 0.4205932 | All four commands exit0; settled | Passed2acceptance/9chunk/38indexer named tests | 750.7413746700622s |
| xhigh | 0.9287684 | Absent; terminal error_max_turns at32requests | Passed2acceptance/9chunk/42indexer named tests | 241.86607002606615s, censored |

Filters overlap, so their passing-name counts must not be summed as independent tests. LOW has19model requests/two terminals/one continuation; XHIGH32requests/one error terminal/no continuation. XHIGH did not exhaust USD4 or3600s. Its private patch passes, while complete workflow acceptance remains false. Do not infer unconstrained low/xhigh quality or cost superiority from this censored comparison.

Final reported pair cost1.3493616, cumulative tracked experiment CLI cost33.571720199999999964, remaining26.428279800000000036 within60. CLI-reported cost covers model repair requests/continuations; research setup/review cost and monetary CPU cost are not measured here. Outer wall includes private grading with shared-cache asymmetry, so it is not a symmetric model-latency comparison.

Final evidence archive data/2026-11-05-cold-recovery-final-evidence.tar.gz, SHA2567ee89423633505746820ea6e5e9fe35efd13a02a8e78fac20560823831cd05d7,811payload files, every hash read back. V remains clean at0f7fb1dd69f6807e4a5564c25ce379bf267c7fc5. No merge/push.

New issue3a69dda510501d5a records the legacy32turn ceiling. Before any further paid run, replace this prospective action limit while keeping money/time/continuation controls and all earlier costs. Zero-provider public validation of the already closed xhigh candidate has started on a separate copy (xhigh-patch-public-diagnostic-20261105,@bg77); its result will not relabel the original model workflow as completed.

## XHIGH ex-post public diagnostic and next policy

The unchanged closed xhigh patch passed all four exact public commands when checked separately without a model: format, clippy, lean and default, each exit0. Public wall574.4910728689283s. Before/after complete file-hash inventories differed only at .git/index; source bytes unchanged. Provider calls0 and model cost0. Cleanup exactID and name independently absent. This establishes public validity of that patch; it does not change the recorded error_max_turns or make the original model workflow complete.

Preserved19payloadfiles with every hash checked after extraction in docs/research/data/2026-11-05-xhigh-expost-public-evidence.tar.gz, SHA256f7c7ba9d4f207d203b9806b2f31ef3cd41bf3d4b4ddfd1809bcdc81179eb74eb. Resource argv requested4CPU/12GiB/init/PID512; no separate live HostConfig export was captured for this ex-post diagnostic.

Next policy is spec02ff0a616cd02962 / planc336f2784639da9c: remove inherited32action option only for verified workflows; retain CLI/proxy4USD,3600s and4continuations, legacy32 untouched. New history reconstruction independently corroborates both closed arm raw costs, preserving cumulative33.571720199999999964 within finite60USD. Fresh root complete-workflow-action-budget-20261105 allocated8USD (4each), prepared but not paid-admitted. Independent five-path review accepted after8distinct applied mutations/9executions: sole initial survivor re-applied and killed by4identity subtests;0unresolved. Actual >32action native control and full gate pending. No generic parity conclusion.

## Actual longer-action control — provider disabled

The fresh qualification job@bg_0000007b exited0. Four retained actual CLI/native controls passed independent full-population readback (low success/repair/unknown, xhigh success), plus a separate long-action cell completed with51observed assistant tool_use blocks before the first result event. The count comes from the raw stream, not the35added script-plan entries. All five cells retained the cost flag4USD and omitted max-turns, full context/hint visibility/host-boundary checks passed on every actual request, and background completion/public probes settled. Zero forwarded provider calls and model cost0. qualification.json binds actual runtime source, rawlongstream/receipt/readback hashes and these controls.

Exact source/plugin/reference inventories equal the previous closed cohort,2060source entries, and acceptance/cold proof bytes were copied unchanged. No onboarding or private oracle LLM work repeated. Gate@bg_00000079 stillrunning; no paid admission yet.

## Fresh cost and time bounded paid pair launched

The final gate@bg_00000079 exited0 (Python light1311passed/442subtests, FMT/CLIPPY/LEAN/DEFAULT0; normal heavyPython/server-stack/ONNX exclusions remain). Exact5ownedpaths committed as2fbbce1cbabff4ba743ab1d41512a284fe8b550d, stablepatch-ide277a6484e01baf0b8c6f3c5cc2b06de406d630b; Vclean. Commit hook passed open for dead-artifact lookup because no runnable binary was found; Python source behavior is covered by recorded explicit/gate/actual-native checks, not by that skipped hook.

Independent finalbindingce5428f158aa2ba5adaa7023059caaa2a59839823b158f134d382cea2b20f6a9 matches reviewed5bytes,16executedruntimehashes and actual controlproofs. Parentadmission enforced clean exactcommit and completedgate beforeinstallingreview.json; actual CLI--checkpassedwithoutprovider. Preflightarchive2026-11-05-action-budget-preflight-evidence.tar.gz SHA256a4999bc43558388acf9a8af00857b3b9502866baa22d2263eef25ab651f3218f,463payloadfilesallhashverified.

Paid job@bg_0000007f launched once in fresh complete-workflow-action-budget-20261105 at2026-11-05~15:03UTC. Prior33.571720199999999964/cap60/allocation8/arm4; oldpilotclosedunchanged. LOWfirst, XHIGHsecond, declarationabsentboth. Native actualLOWsample verifiesCPU4/12GiB/initPID512/exclusivewritableactor; sample is not continuousattestation. Bothresults/public/private/costs remain pending; provisional ledger newcost0 is not a measured bill. No sourcechanges untilfinalevidence.

## Closed action-budget pair and supplementary adjudication

Both model workflows completed, settled and passed all four exact public commands with no max-turn censoring. LOW:0.48744760000000004USD,755.6565021220595s workflow,22requests,3terminalevents/2continuations. XHIGH:1.1965462999999996USD,908.8578626508825s workflow,38requests,1terminal/no continuation. Paircost1.68399389999999964; cumulativeknownCLIcost35.255714099999999604/60, remaining24.744285900000000396. Outerwithprivategrading1105.50s/1077.06s has shared-cache asymmetry and is not a model-speed comparison.

Originalregisteredacceptedflags areLOWtrue/XHIGHfalse. Actualprivateprocesses and namedlogs pass2acceptance/9chunk/38indexer (LOW) and2/9/40 (XHIGH), all required baseline names preserved; filters overlap, do not sum as unique tests. Xhigh's private grade98 is an instrument defect: literal'0 passed;' matches'40 passed;' (issue241d6eb64c605b37, measuredonactualsource+log+predicate beforefix). Do not interpret that as a repair failure.

Originalrawreadback/finalizer@bg81 exited0 and finalarchive2026-11-05-action-budget-final-evidence.tar.gz SHA2565a9c571c6d1d926a645d0409a3910f7766fa246debb59d49198eafa7176d14b8 preserves866payloadfiles. Supplemental no-provider replay independently reverified all866hashes, correctedsummary+unique namedtestcorroboration, retainednegativebaseline rejection/positivecontrol acceptance and found bothfunctionallyaccepted undercorrectedgrading. Originalledger/archiveflags stayunchanged. SupplementalROOTaction-budget-adjudication-20261105/adjudication.json; no newmodel/testexecution, only existinglogs reread. Source2pathfix30pilotmethodsGREEN and independent5appliedmutations/5killed/0survivors; fullgate@bg83running beforecommit/sealing.

ObservedcostratioXHIGH/LOW2.4547177994; LOW59.2621percentlower onthissingleexposedtask. This supports the feasibility of a complete low workflow under the shared full-context/native/public-verification bundle. It neither isolates the useful component nor establishes generic parity, authority treatment effect or external transfer. Currentdriver's registeredprior is now historical: no new paid root may reuse it without carrying thisclosedpaircost into35.255714099999999604 or above.

## Final preservation and next external admission

Graderfixcommitted774d81703ec14efc9fa50c72d4a3eb6380258c13, stablepatch-id2c1eeffab6fe5f6800e53fe9687630071ad686a6; source2pathsreviewed/30pilotmethodsGREEN/5actualmutationsallkilled/fullgate@bg83all5lanes0. Supplementaryarchive2026-11-05-action-budget-adjudication-evidence.tar.gz SHA2565bb489a63843be810bb5ded207258fe4ab6ff8c279706cbd2494548244927497,40payloadfilesallhashverified. Original866payloadarchiveandregisteredflagsunchanged. No additionalmodelcostinthiscontinuation. Mechanismtracker7728cf1b9c13a59b LMC-4updatedandcommittedd3c1d104708df2d21a6cf6920d72e3af8ff9ddc6; Vclean/no mergeorpush. Mainreport/dataremainlocalunderknownprovenancehold421ed73df25f795d.

Externaladmissionplanae0b584f43fc96e8 isnowactive. Outcome-blindrecordselectionfrozeNushell18863andRA22992beforemodeloutcomevalues; RA23586excludedforfresh-screenrun-directorypresence. Bothbase/assembledimagespresent, storedprescanspositive-controlpass/0hits/no reasons. Actualread-onlycontainerprobesobservedprojectCLAUDE/AGENTSwithhashes butnoembeddedCodescoutprompt/config/memoryMarkdown. ThisdoesnotproveabsenceofprojectKBelsewhereoractualinstructiondelivery. Bothprobecontainersindependentlyabsent; provider/onboardingcalls0. Selectionandpreflightevidence in docs/research/data/2026-11-05-transfer-{candidate-selection,context-preflight}.json.

Next: inventoryreusableprojectKBs, bindtaskpubliccontracts/existingF2P-P2Pproofsandnewnativecontext/oracleadmission, thenregisterproperlybudgetedfourrepairrunswithall35.255714099999999604costscarried. Existing setup-ab sourceslocal/user,local still omitprojectinstructionloading (knownb8c35dc28bd3e8dd), so do not launchits oldcomparisonunchanged. The sharedbundle'sgenerictransfer andcomponentattributionremainunmeasured; bothcurrentfirstpublicroundspass, so correctivefeedbackeffectwasnotexercised.

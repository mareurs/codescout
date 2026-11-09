---
id: e7f6ae632852c7d2
kind: research-note
status: archived
title: Agent lifecycle — offline component complete, 2026-11-05
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/agent-lifecycle-20261105
original_sha256: 97f425b0b75b69b678333a2428f8cf128ae05d18cfe0f8d9948f16738ff23695
---

# Agent lifecycle — offline component complete, 2026-11-05

User said ok, go to implementing bounded background-job continuation. No new paid calls this stage. Generic tools/agent_lifecycle/ now locally committed f89af31e371268944b4e5e920f27a591bfe7bf75, stablepatch-id b6635eb237a411a4bc62a3184794683a694a15a2, canonical Session-Id codex:01a15ca9-f939-7603-acb5-01c6b02f68e2. V=/home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101. Seven new files only, clean V, no push/integration. Prior core979fffc... and completion083d95a8... unchanged.

Mechanism: session.run_session uses pinned CLI stream-json stdin/stdout in one process/session, stdin kept open. PostToolUse observer.py writes atomic run_command receipts. Controller decodes actual native MCP text JSON, inventories @bg handles/records, validates record schema/identity/state, waits terminal outcomes via bounded read_record callback, sends same-session completion messages, then allows a successful subsequent turn before shutdown. Limit4continuations/default900s; nonblocking os.read/os.write+select bounds local pipes, isolated process-group cleanup handles deadleader/livechildren. Failed/signalled jobs can settle but all_jobs_succeeded false. Cost receipts keep latest cumulative CLI total, cost_is_final=false on incomplete work; caller owns provider proxy/budget/cleanup. No paid launcher entry point. No-job settled=true does NOT prove any tests ran. Lifecycle settlement does NOT prove task correctness, agent read all output, or required-check coverage.

Offline actual-client driver qualify.py reuses frozen source/onboarding/global+projectCLAUDE; CLI2.1.309 nativeDockerCodescout, scripted proxy with upstream and workerTOKEN raisingguards, dummykeyonly. Qualified root /home/marius/.cache/codescout-resident/codescout/agent-lifecycle-20261105-v5. @bg_00000039 exited0. Three controls: negative maxcontinuations0 ends firstturn incomplete with jobmarkerabsent afterworkerremoved; success nativejobrunning→exited0 yields1continuation and2successCLIterminals, originalhandle read and latercreate_file; failed nativejobrunning→exited7 samecontinuation/read, all_jobs_succeeded=false. Native CLI creates2connections (probe + jobserver), not1; job owner resolved by exact record existence among identity-checked ownedlivecontainers and pinned. Independent verify.py rawreadback checked sourcehashes, matchingSIDonrawterminals, originaltaskhistory oncontinuationHTTPrequests (NOT unscriptedmodelrecall), exactownercontinuity, bothrealjoboutcomes, zerocost/forwards, all6ownedcontainers independentlyabsent. Durable MAIN docs/research/data/2026-11-05-agent-lifecycle-{results,verification}.json. Verifier /tmp/lifecycle-verify-01a15ca9.py, copied v5/verify.py.

SourceSHA256 session.py c4b8994f51d27c6e2a8123e0155bc0f403460a325383b5edc5c4c411e189cbeb; observer.py8b46ef41f3122cd180d3ae42443dbd7197d715607c30ded62ca8c1112f4a0b37; qualify.py77cee388c56f049641559181d014d571bd04c105290c941a3f7c357a213efb16.

Failures preserved: v1rejected2nativeconnections (newbugec430b3a1b23cdcb nowfixedresearch); v2nativecontinuationsucceeded thenlegacycleanuprace2db5e6fbda0834c2raised; v3/v4boundedcleanupreportedremovalpending, exactcontainerslaterabsent. Newqualifiercleanupvalidatesimage+label, removesexactId, pollsactualabsenceupto30s, logsallprobes; v5allcontrols pass. Do not mutate/overwrite oldroots. Legacycleanupbugstillopen; newpathmitigatesit.

Initial6behavior tests observedREDmissingcontroller→GREEN. Fresh independent reviewer lifecycle_review gpt-6-astra/high found3Importantbugs: JSONtextdecoder swallowedsemanticrecorderror(a926056cc47be553); inheritedstdoutafterleaderdeath defeateddeadline(7e757ef79f426f29); synchronousstdinwrite defeateddeadline(2a758a6f17667628). All loggedimmediatelyMAINdocs/issues byreviewer, eachregression observedRED thenGREEN. Newowner+cleanuptests alsoRED→GREEN. Current18lifecycle tests green; prior28pilot+9completion+3resume green. Reviewer ACTUALLYapplied13mutations initial:4killed,8substantivesurvivors,1equivalentsurvivor. Exactrecheckwithall18tests:12non-equivalentkilled,0substantivesurvivors,1sameequivalentsurvivor (failedstate inclusion still excludedbyexitcode). Raw /tmp/lifecycle-review-20261105{,-recheck}/mutations.json+logs; durableMAIN2026-11-05-agent-lifecycle-mutations-{initial,recheck}.json. Currentcontrollerhash matchesrecheck.

FullVgate @bg_00000035 exited0 all5lanes0, archivedMAIN docs/research/data/2026-11-05-agent-lifecycle-gate.log.gz. NormalheavyPython/serverstack/ONNX exclusions;18newtooltestsrunseparatelysincePythoninventorytests/test_*.py only. Sourcecommitjob@bg_0000003b exited0. Precommitownership+format checks passed; deadartifactid checkskippedbecausehookcouldnotfindbinary. MAINnewdocs/data/memory/issuesremainlocal underknowndocprovenancebug421ed73df25f795d, do not bypass/ackforeignauthor. Foreignstagedbug-triageblobf4ad12b9cf7b8b830ac35cc341afcde702d0a56c verifiedunchanged.

Plan6b6f3546c3761800 and report7447904928197ebf DONE forstandalonecomponent; previouspilotreportf789fec3ea4737ba and parentplanc3836cf55c7b4b87 updated. New3reviewbugs+qualifierownerbugfixedwithstructuredSHA/patchidanchors, leftunarchiveduntilintegratedexperiments. Originalbf167b83c0166a24 remainsINVESTIGATING withcaveat: legacypaidone() unchanged, workflow_completed flagstillturnsuccessonly. Do NOT rerunclosedcompletionpair/exclusiveattemptmarker. Nextwork: composequalifiedlifecycle into fresh pairedworkerpath with onecontinuouscampaignbudget and separate OBSERVED required-public-check completion; qualifycompositionbeforeanotherpaidrepaircomparison. Transportproofdoesnotproveeffortmechanismimprovesquality.

No new modelspend. Pilot2.57626490000000013/4USD, remaining1.42373509999999987; thread31.797419199999999964/40USD, remaining8.202580800000000036. Do not resetbudget or waive historicalfailedattemptcosts. Credentialpathauthorizedbutnotreadthisstage /home/marius/.config/s5/token-claude-stef.

## Latest composition checkpoint — 2026-11-05

The previously pending worker integration is now complete in0998c3b64f81c13331db26e0091ff25404272c66 (patch-id971ae969a0c1417a98d387e1c314a1fec4dbf3b2), same V clean, no push/merge. See research/effort-steering/verified-worker-20261105 and report0185b660ea701471. Explicit workflow=config enables one-budget same-session public verification/failure feedback/full rerun, exact container ID pinning, and truthful cost coverage. Corrected actualCLI/native3controls passed at$0,34tooltests+prior28/9/3+fullgateall5lanes0;13substantivemutations killed,1invariant-equivalent remains. Parentissuebf167b83c0166a24 mitigated by opt-in, legacy default/history kept. NEXT is fresh matched calibration with fullcomposition/manifest/runtime fingerprint and acceptance controls before anypaidcalls; do not redo this integration. Budgetunchanged31.797419199999999964/40USD.

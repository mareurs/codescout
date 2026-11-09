---
id: '02497b40701faf22'
kind: research-note
status: archived
title: Verified worker composition — 2026-11-05
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/verified-worker-20261105
original_sha256: 54629d2e696de276380036cb07783d7166fc8f6575177416e24376581a73092d
---

# Verified worker composition — 2026-11-05

User said ok, go to integrating the offline-qualified lifecycle controller into the repair worker and measuring public verification. No new paid calls authorized in this stage.

V=/home/marius/.codex/worktrees/615e/codescout, research/fixed-acceptance-population-20261101. Base f89af31e371268944b4e5e920f27a591bfe7bf75. FINAL CHECKPOINT: source committed0998c3b64f81c13331db26e0091ff25404272c66, stablepatch-id971ae969a0c1417a98d387e1c314a1fec4dbf3b2, canonicalSession-Idcodex:01a15ca9-f939-7603-acb5-01c6b02f68e2. Ten exact paths; V clean. Full gate @bg_00000040 exited0 all5lanes0; raw log archived MAIN docs/research/data/2026-11-05-verified-worker-gate.log.gz. Do not redo implementation/controls after restart. All source is ours; reviewer did not edit V. Main foreignstagedbug-triageblob f4ad12b9cf7b8b830ac35cc341afcde702d0a56c unchanged.

New opt-in run.one(...,workflow=config), defaults preserve legacy. Adapter workflow.py keeps same proxy+Budget across CLI turns, validates nonempty public manifest before credentials, installs observer alongside frozen hint, invokes session.run_session with completion_check callback after bgsettlement. PublicVerifier executes every configured command sequentially against idle current native candidate, records per-round/per-command exit/logs, does not shortcircuit ordinary failure, returns public feedback for bounded same-session repair; unknown/timeout aborts. No private oracle feedback. Same name AND nativecontainerId pinned. workflow_completed requires lifecycle settled +completepassingfinalround +knowncostwithinbalance +cleanprovider/budget +successfulcleanup. CLI cost latestcumulative, notsum. terminal_observer records server.forwarded atvalidCLIterminal; furthergenerationwithoutnewterminal => unknown_provider_cost true, staleknownlowerboundpreserved. Profile tools/hint_authority_workflow/public-checks.json matches frozen task4Rustcommands; timeout1800/percheck900/continuations4, output8192/max32CLIturns/$0.65CLI unchanged.

Actual corrected composition qualification /home/marius/.cache/codescout-resident/codescout/verified-worker-20261105-v2, job@bg_00000041 exited0. v1pre-review preserved separately, do not overwrite. Real pinnedCLI2.1.309+nativeCodescout+full frozen onboarding/global/projectCLAUDE+recon+deliveredhintthenedit, scripted HTTPprovider (credential/upstreamguardsraise). Threecontrols: success2CLIturns/1bgcontinuation/public[0,0,0,0]; repair3turns/2continuations(bg+publicfail)/rounds[0,1,0,0]then[0,0,0,0]; unknown2turns/1bgcontinuation/format0thencompile sleep10timeout.2s =>incomplete/nosuccess. Shortpublicprobecommands, NOTactualRustrepair/modelquality. Independent /tmp/verified-worker-readback-01a15ca9.py <root> readrawreceipts/terminals/requesthistory, exactworkername+ID+jobowner, sourcehashes, all6ownedcontainersindependentlyabsent, zerocost/forwards. Durable MAIN docs/research/data/2026-11-05-verified-worker-v2-{results,verification}.json; earlier v1 names withoutv2 retained. Sourcehashes: session.py3c1905da29bd1de39ac47f8905bea6869df4e5a66a9c30c41263ea0b1d9af44f; workflow.py966dd0438aa8c991de32d2d3c58eb10ca0ffcf8176d60edb659bf6ffbfc6162e; verification.pyf66b367e018a7380a3ac6129029671337a8c9f8113e7cc1897d953ee822bc277; pilot/run.pyb9e5d3e15b654a229de6ee8f38619d45c18bbe1bbeeb3c7f2196ee158fcc8a08.

Current lifecycle/composition34tests GREEN; priorpilot28+completion9+resume3GREEN. Freshreviewer workflow_review gpt-6-astra/high reproduced2Importantfunctionalbugs immediatelyfiledMAIN:0b471a97357e4b0f stale terminal undercounts further paid response; f0e56c3c59f66b42 containernameonlypin. Both regressionREDobserved, thenGREEN afterfixes. Review actuallyapplied14mutations initial5killed9survived. Directguardtests closedsubstantivegaps; exactrecheck14applied13killed1invariant-equivalent(M13publicpredicate redundantwithgenuinecallbacksettlement), all34testseachrun. Coverageissue30fca68660938262. Raw /tmp/workflow-review-20261105 and /tmp/workflow-review-recheck-20261105; durableMAIN -mutations-{initial,recheck}.json. IndependentJSONreadbackconfirmedcounts34/everymutation,14total13nonzero.

Minor aa79e4013575950a: missingvalidationmutation M11 let original test create oldlaunch-v1/unused and /tmp/hfa-5853d1d6461a415b8168b94eca805f81/project before TOKEN Forbidden stopped; no realcredentialread/CLI/provider. FixedtestrootsTemporaryDirectory. Parent moved positivelyattributed unused/ directory to /tmp/workflow-review-20261105/escaped-unused-attempt preservingcontents; didnotrewriteoldreceipts. Actual root testsnowisolated, exactM11killed safely.

Plan22ed6d6c4e0c2288 and report0185b660ea701471 MAIN DONE for this stage; four new review/isolation issues fixed with structured SHA/patch-id anchors, unarchived until integrated experiments. Parentbf167b83c0166a24 MITIGATED by explicit workflow opt-in, not fixed legacy defaults. Source10paths staged: README/session/test_session, newqualify_workflow/test_verification/test_workflow/verification/workflow, pilot/run.py, newhint_authority_workflow/public-checks.json. Gate rawlog /tmp/codescout-bg-AH2vbB.log, record /tmp/codescout-bg/455879-a720241d-bg_00000040.json; actual exited0/all5laneexits0 verified. Commitjob@bg_00000043 exited0; ownership/format hooks passed, dead-artifact-id lookup skipped (no runnable binary). Normal heavyPython/serverstack/ONNX exclusions, newtools34runseparately. No push/integration. MAINdocs/memoriesremainlocal; knownprovenancebug421ed73df25f795d stillopen whencheckedthisturn, don'tbypassguard/ackforeignauthor.

Stage complete: source commit, clean worktree, reports/plan/newissues/parentplan/previouspilotreport updated. MAIN documentation remains local and unstaged: fresh read-only file-provenance preflight on new report returned UNKNOWN with coverage gaps; known421ed73df25f795d updated. No guard bypass. Source commit only; no merge/push. Then nextresearchwork: fresh matched calibration driver MUSTfreeze/reviewwholecomposition+manifest+runtime, qualifyintendedlow/xhighcells+acceptancecontrols BEFOREspend. Legacy audit.fingerprint onlyincludesoldpilotfolder, sooldqualification is NOTcompositionproof. Closedpaidpairmarkersremainclosed. No inference lowequalsxhighfromscriptedcontrol.

No newcost:pilot2.57626490000000013/4USD,remaining1.42373509999999987; thread31.797419199999999964/40USD,remaining8.202580800000000036. Historicalfailedcostsremaincharged. Realcredentialpathauthorizedbutnotreadthisstage ~/.config/s5/token-claude-stef.

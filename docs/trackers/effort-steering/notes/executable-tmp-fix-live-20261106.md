---
id: a037a2d056d7709a
kind: research-note
status: archived
title: Executable temporary storage fix LIVE — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/executable-tmp-fix-live-20261106
original_sha256: 3cbc20a4080667ad348a68d01ce403b383af7879131b0e52684d43e17d6c6e93
---

# Executable temporary storage fix LIVE — 2026-11-06

User authorized correcting noexec and adding RAM. Current implementation worktree /tmp/codescout-tmp-exec-01a15ca9, branchfix/external-tmp-exec-20261106 basedonc1d51c446. Original V /home/marius/.codex/worktrees/615e/codescout unchanged to protect registered old cohort. MAIN spec8965d0be731c902c and plan9dfa837b81e7baaa; taskledger /tmp/codescout-tmp-exec-01a15ca9/.superpowers/sdd/2026-11-06-executable-native-tmp/progress.md. No source commit/merge/push yet; wait checks/provenance. Own publishholdref absent at check, no release performed.

## Implemented correction

workflow.verified_native_argv supplies exactly1 /tmp:rw,exec,size=4g,uid=1000,gid=1000 tmpfs, normalizing POSIX aliases incl /tmp/,/tmp/.,/other/../tmp,//tmp before replacement/duplicate checks; preserves unrelated tmpfs and payload. Internal memory default12GiB unchanged. external.validate_prepared now requires32GiB external memory, optional swap32GiB,4CPU/init/PID4096 and exacttmpfs before credentialaccess. README corrected retired2CPU prose.32GiB is headroom, not a demonstrated OOM fix. No hostmount/configprofile changes and no liveactor mutation.

## Observed verification

Baseline lifecycle71pass. Initial RED12externaltests9failures/2errors (expected newfeatureabsence), GREEN76lifecycle+30pilot+24workflow. Independentreview4appliedmutations initially3killed/1swapguardsurvived75. New swaptest accepts absent/split32g/equals32g, rejects24g/64g/unlimited; exactsurvivorreappliedandkilled3subtests among76, scratchrestored76pass. Reviewerfound actualtmpfsaliasoverlaynoexec bug; parentnewtestsRED7subtestfailuresbeforePOSIXfix, GREEN76after. ActualfinalgeneratedDockerprobes onbothimmutabletaskimages verify32GiBcgroup/4GiBtmpfs/executableexit0/doctestexit0. FouradditionalactualDockeraliascontrols show exactly1exec4GiB/tmp andbinaryexit0, allcontainers independentlyabsent. No providercalls.

MAIN issues4d66b80cca2e6390 (primary),501a1f3b1384ceb5 (alias),e6b99f20243e4fd0 (swapcoverage),d2adc4dbdbd67505 (README). Allremainopenpendingfinalverification/integration; fixnotclaimedonexperiments.

## ACTIVE jobs — use these, not older handles

Compactfullgate @bg_000000ed record /tmp/codescout-bg/455879-a720241d-bg_000000ed.json. Command CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4 ./scripts/gate.sh --check, workspacecurrentfix. Sameallgatelanes/tests, debugmetadatareduced. Lastobservedcompiling/clippy, notcomplete.

Resumedzero-providerqualification @bg_000000ee record /tmp/codescout-bg/455879-a720241d-bg_000000ee.json. Helper /tmp/resume-tmp-exec-qualification-01a15ca9-20261106.py imports preserved /tmp/qualify-executable-tmp-01a15ca9-20261106.py. ROOT /home/marius/.cache/codescout-resident/codescout/external-exec-baseline-20261106-v2. Finalsourcehashesand2runtimeprobesalreadysaved. Script waits successfulcompactgate, oldrepairs-closed.json,>=50GiBrootfree (max10800s), then sequentialunchanged-sourcepublicmanifests innewcontainers/newwritablemountcopies, originalexactcommands/noaddedskips. Check waiting/blocked/progress/results before restartinganything. Noqualifyingfullbaselinehadstartedatlastcheck. OriginalimageHEADandcleantrackedsourceverifiedbeforetests. Mayexposeunrelatedfailures; do notexclude them resultdependently.

## Interrupted attempts and disk pressure

Oldinitialgate@bg_e8 andqueuedgate@bg_ea were stopped, NOT passed. Qualificationwaiters@bg_e9 then@bg_eb stoppedbeforefullbaselines; allinitialprobesretained. Rootfreefellto13GiBamidconcurrentwork; DockerreportedownNUlowprivategrader44300652544writablebytes andforeignNU19054control68922834944 (notexclusivephysicalusage). Noactivecontainers/images/volumesdeleted orpaused. StoppedonlyownprocessgroupsverifiedbyPID/cmdline. Removedonlyidle slot-0/debug afterexistingexclusive slot-0.lock inode268acquired andnoprocessreferencedtarget. duallocated20685115392; dfavailableobserved22605533184before/34503839744after,notexclusivephysicalreclaimclaim. Laterrootfree66.18GiB; externalconditioncanchange. Cleanupreport MAIN docs/research/data/2026-11-06-stopped-gate-cache-cleanup.json. Sharedlockinoderetained. No source/evidence/oldgraders lost.

## Old cohort and preservation

OldexternalcampaignROOTexternal-matched-repairs-20261105 nowhasall4candidate/costclosures; privategradingrunning,results.jsonabsentatlastcheck. Do notduplicate/reset/modify itsdriver/registration. No newpaidcohortornewonboarding.

Preflightarchive MAIN docs/research/data/2026-11-06-executable-tmp-preflight-evidence.tar.gz SHA12faf888791d504237117f455876f4962af9b4ad94700cdd2ad74f835c2a4796,24payloadfilesreopened/hashverified. Finalpatch/source,probes,aliases,helpers,reviewerTRANSCRIPT-DERIVEDexcerptslabelled(notoriginaldisklogs). Excludesunfinishedgates/fullbaselines and laterdisk-pressure/resumehelpers; preservethoseafterclosure. Aliasdata SHA0838697d9d4e5b755e1fe3e98d6a66eb7648b40dce6f4b1b3d3abd6a7965b079; initialdiagnosisSHA18243378597a8e1c186ff2ee0d83034f5621112c804cd7be38b6b26c6b7ee578. Do notcleancurrent/tmp helpers/worktree/state.

Next readcompactgate/qualifieroutcomes, diagnoseanyactualfailureswithoutLLMspend, archiveclosure, updateMAINissues/plan, commitexactsourcepathsifgate/provenanceallow. No merge/push or paidrerun beforequalifiedprofile.

---
id: '3e3b4ce598318fa1'
kind: research-note
status: archived
title: Private grade disk-pressure audit COMPLETE — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/private-grade-audit-20261106
original_sha256: 97f59701d328e6c9e7355458acc1c3121dea6bd1ad0057c688f5bd70391b014d
---

# Private grade disk-pressure audit COMPLETE — 2026-11-06

MAIN report fce51ffc4baf7906 docs/research/2026-11-06-private-grade-disk-audit.md. User asked to check evaluation validity after disk exhaustion. Original four raw grades/candidate/cost receipts remain immutable. Their booleans/counts reproduce exactly, but original Nushell/low false had no functional observation: linker SIGBUS, zero statuses. Both rust-analyzer candidates pass all 8,182 selected expected names. Nushell/xhigh passes both acceptance/F2P tests but genuinely fails mixed_str_length_result_does_replace_last (String ab vs Int2); one extra parser-missing result is actually a concatenated raw pass. Copied-whole-log newline control removes missing1→0/notpassing2→1; false remains. All four original public workflows remain incomplete due environment failures; no generic low-to-xhigh mechanism proven.

## Same-candidate diagnostic replay CLOSED

@bg_000000f7 EXITED0,704.5s. Exact original NUlow candidate SHA789cb6fa483752b1472f61d03e07e83d4ec03bb2686b8d1b86a46cd74080fd97, image/record/parser/all53grader sources, original4CPU/8GiB profile. Both2F2P fail, all8620P2Ppass,missing0,8725totalparsednamesincludingoutsideexpected. Raw assertions last_result.rs839/860 observe Error list replacing expected Record list. So original crash-invalid receipt is not a valid rationale, but a separate preregistered replay genuinely rejects this same patch. No model/provider call, candidate sampling or onboarding. Original sources/candidate/receipts unchanged. Container removed/independently absent.

All70 resource observations/68successfulcontainerprobes examined: minimumsampledroot322369671168bytes (~300GiB), sampledOOM/oom_kill/group_kill0, memorylimitmaxcounter857457 pressureevents (NOTOOMs), peak8589938688bytes at8GiBlimit. 10s sampling is not continuous telemetry. Complete replaylog has no declared disk/linkercrash signatures. This does not isolate historicalSIGBUScause.

## Historical ENOSPC evidence

Hostjournal confirms ENOSPC+multiplelinkerSIGBUS09:51:20local. Rust1.95.0 crashedPID434016 cgroup docker0cdda52eccee77301892550ddae51c6df0b6e794f8fdeff8e4930d414de2b0a7, originalruntime09:38–09:52. Adjacentdockerdnetworkjoin names original sab-grade-nushell__nushell-18863-low-e9d452bc; rawgradeswritten09:52:44/47. Scope8GiBpeak. Samecoredumphandler recordsENOSPCcompression thenPID434016SIGBUS. Hostexhaustion is directlyestablished; scope→gradername association is labelledinferencefromadjacentevents,notdirectsamefieldmap. ENOSPCsavingcoredoesnotitselfprovewhyLinkercrashed. Onlyallowlistedjournalfieldsretained,nocredentialenvironmentmetadata.

## Independent controls and durable evidence

Reviewer /root/tmp_exec_review CLOSED originalfour-arm audit,14actualparserperturbations+wholelognewlinecontrol, journal/replay supplements, andsourceguard review. Applied4guardmutationsinitial3killed1prefixsurvivor; addedninthnonemptyprefixwrapper test killsidenticalmutation,restored9PASS. Reviewer's initialreplayintegrityinstrument chosewrongnestedgradepath; originalmismatchandexplicitcorrectionretained; finalall53source/originalreceiptchecks pass.

Archive docs/research/data/2026-11-06-private-grade-disk-audit-evidence.tar.gz SHA89dfc97b0d7a901d4da3f37a9beb5576e7fe7d36f319cfdce8ddbf986367f518,33232147bytes,1600payloadfilesplusmanifest, ALLreopened/hashverified. Includesoriginalfourarmterminal/candidate/modelstream/public/background/privateevidence, audits, replay,controls53sources/helpers. No writablehomes,semantic/modelcaches,imagelayers,unfinishedv3. OriginalresultsSHAe28e9eac4c326d6bebe4109336cd1594a712cd3374b041d41ad299ff602059da unchanged. ROOT /home/marius/.cache/codescout-resident/codescout/external-grade-audit-20261106. All originals retained; no cleanup ofthisnewarchive/state.

Opengraderbugs MAIN54fe4b112aa4e423 and0a2929ec2a4f688f: classification/parserproductionfixes stillrequired beforetrustingnewpaidgrades. Derivedauditcorrectsinterpretation,notproduction. These two privateoutcomesnowvisible; furthertuningonthemisdevelopmentnotfreshheldoutvalidation.

## Next active work

Nativefix5ownfilesremainuncommitted in/tmp/codescout-tmp-exec-01a15ca9; compactfullgate@bg_ed0/sourceunchanged. Newdisposableguardallowscleanorexact.gitignoreappendonly, requiresHEAD/index/modes/allothertrackedpathsunchanged; issue1720ba9339aa9ad6, testcoverage5d69759479e75de7.

Zero-provider sequential fullpublicbaselinev3 ACTIVE @bg_000000fd record/tmp/codescout-bg/455879-a720241d-bg_000000fd.json job2748923 commandpython3-B/tmp/qualify-executable-tmp-v3-01a15ca9-20261106.py. ROOTexternal-exec-baseline-20261106-v3.32GiB/executable4GiBtmpfs,sameimages/commands,noaddedskip,isolatedwritablestate,noLLMre-onboarding. Bothgeneratedruntimeprobes0/removed; NUactualpre-sourceguardexactappendaccepted,publicbuild0; fulltestsongoing. Checkrecord/PIDandresults, doNOTduplicatereset. No newpaidcohortadmitted. Fullbaseline+strictgradercontrols mustclose beforepaidrerun. No commit/merge/push performed.

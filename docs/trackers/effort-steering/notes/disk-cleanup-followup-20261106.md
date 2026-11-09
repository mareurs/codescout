---
id: '227fa9312702f0fd'
kind: research-note
status: archived
title: Own cache cleanup follow-up COMPLETE — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/disk-cleanup-followup-20261106
original_sha256: b304dffebb22e8d3bfc4cb46ec743f941664aba7eb497c41673a6b6f1c07b39a
---

# Own cache cleanup follow-up COMPLETE — 2026-11-06

User explicitly requested reclaiming disk from this work only, preserving important information. MAIN report4dcc14505cbc396a docs/research/2026-11-06-own-cache-cleanup-followup.md. ROOT /home/marius/.cache/codescout-resident/codescout/disk-cleanup-followup-20261106. No source, result, log, cost, patch, model, image, volume, existingcontainer or snapshot deleted.

Removed20Cargo cache directories (.fingerprint,deps,build,incremental) across5CLOSED targets: public-profile-reference-20261105-v2/project/target, public-profile-cold-20261105-v1/project/target, hint-authority-effort-20261104/{launch-v1,launch-budget-requalification-20261104}/grader-target, originalV615e/target. No process/container references beforeapply, existingCargo locks held wherepresent, no newmarkers fabricated. All52683non-target regular-file/link records identical before/after, zero preservationdeltas. Top-level productbinaries/cachetags preserved.118active/currentinputhashes and4exactactor/graderimageIDs verified unchanged. Cleanup@bg_f2 exited0. Helpers /tmp/cleanup-own-closed-caches-followup-01a15ca9-20261106.py.

Compactgate@bg_ed finishedALL5lanes0. Savedwholegateoutput then removed idle slot-0/debug5649510400duallocatedbytes underEXISTINGexclusive slot-0.lock inode268, no processmaps/commandrefs; lockinodepreserved. Helper /tmp/cleanup-closed-compact-gate-01a15ca9-20261106.py. Totalallocatedremoved27135889408bytes (~25.27GiB), NOT exclusivephysicalreclaimedmeasurement. Btrfs/backups/concurrentwriters apply. Finalrootdfavailable53717123072bytes (~50.03GiB), fluctuating. Do not rebuilddiscardedcaches just to validatecleanup.

Evidence: docs/research/data/2026-11-06-own-cache-cleanup-followup-evidence.tar.gz SHAb442d400cd366c81f17edbb767bf5985d952c27964c5b0f757fead5f21f7d17a,3210924bytes,10payloadfilesreopened/hashverified (completepreservationmanifest/inventories/scripts/protectionrecords). Final supplement2026-11-06-own-cache-cleanup-followup-closure.tar.gz SHA9cefd8fd24f34e86e8d611c33a4d5c11d0762ab8ef4465897466d090e45193a2,252856bytes,4payloadfilesreopened/hashverified (completegateoutput/cacheleasecleanup/finalclosure). No tokens or providerheaders.

## Research/fix state superseding earlier LIVE notes

Oldcohortcontroller @bg_cc nowEXITED0, external-matched-repairs-20261105/results.json EXISTS. ALL4candidate/costclosures and privategrades closed; detailedfunctionalresults not read duringcleanup. No namedownlivegradercontainer at finalDockercheck. Do notrerun/regrade automatically. Preservecohortresults and read/archivethem next.

Newfix worktree /tmp/codescout-tmp-exec-01a15ca9 remainsdirty5ownsource/test/docfiles, originalV615e sourceunchanged. All118boundinputsrematch aftercleanup. Compactfullgate@bg_ed exit0 withPythonlight/FMT/CLIPPY/LEAN/DEFAULT0 underdebug0/incrementaloff/buildjobs4/testthreads4. Nativeexecutabletmp4GiB/32GiBRAM+aliascontrols/76lifecycle30pilot24workflow alreadypassed. ReviewfindingsclosedbyobservedRED/GREEN; no merge/push/commit performed.

Zero-providerfullbaseline qualifier@bg_ee EXITED1, no remainingloop. ROOT external-exec-baseline-20261106-v2/nushell__nushell-18863 exists; do NOT blindlyrestart exclusivecase. imagebaselineHEADactual=expected5ec462b99bb0773843538cb545a741db84d0da92. Beforepublicchecks, gitdiffguard rejects sole .gitignore addition `.codescout/private-memories/` after native startup. No public-results.json; cleanup exactcontainer29099faa8c3e21c5ccbd6929d91a23785c5de10aab6ed7c3cc42b42231a866d3 remove0/independentlyabsent. MAIN NEWBUG1720ba9339aa9ad6 docs/issues/2026-11-06-baseline-qualifier-rejects-native-gitignore-initialization.md. Need exactexpectedsetupdelta handling, never blanketignore/resettrackedchanges. Fullnewpublicqualification remainsincomplete, no paidrerunadmitted. Futureheavyqualification should wait sufficientdisk and avoidoverlappingoldprivategraders (nowclosed).

Remaining ownnative-env directory includes repeatedJinaweightcopies; canonicalglobalcache retained, no modeldeletion/dedup attempted. Registeredactor/graderimages/SABtemplates/backups stillprotected. User-relayedpeer scope in docker-peer-dependencies-20261106 remainsapplicable; excludedtaskimagecandidates were not found by currentimage-name search, so no speculativeimageprune.

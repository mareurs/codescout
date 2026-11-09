---
id: c2a0439d5bff8f0d
kind: research-note
status: archived
title: Corrected native environment baseline COMPLETE — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/executable-native-baseline-20261106
original_sha256: ecff8b204abc9f04698bbe0bce0a74c1181353ac755116ccdad922a58100f2c9
---

# Corrected native environment baseline COMPLETE — 2026-11-06

This supersedes prior active v3/baseline-pending notes. @bg_000000fd EXITED0, exactcommand python3 -B /tmp/qualify-executable-tmp-v3-01a15ca9-20261106.py, record /tmp/codescout-bg/455879-a720241d-bg_000000fd.json. ROOT /home/marius/.cache/codescout-resident/codescout/external-exec-baseline-20261106-v3. Both unchanged actor image baselines pass exact original registered public commands, no new skips/model/provider calls/onboarding. Runtimeprofile32GiB RAM/executable4GiB/tmp, CPU4. Both generated executable/doctest probes0 andremoved.

Nushell18863 build0/8.298s; tests0/518.5867s: cargo test --offline --workspace --no-fail-fast -- --skip plugin_persistence::plugin_process_exits_when_nushell_exits (only preregistered skip). HEAD5ec462b99bb0773843538cb545a741db84d0da92 unchanged. Rust-analyzer22992 build0/5.9933s; tests0/175.9964s: RUN_SLOW_TESTS=1 cargo +stable test --offline --no-fail-fast -- --nocapture. HEAD3678da48aebe17797d2f792bde07a46f5bf4a827 unchanged.

Both actual pre/post source guards identical: no staged/mode/other trackedchanges, onlyexact native-startup .gitignore append .codescout/private-memories/. Disposable guard9realGitcontrols pass; independent4actualmutations initially3killed1prefixsurvivor, new9thprefixwrappertestkillsidenticalmutation. Guardneverresets or blanketignores. Sourceguardissue1720ba9339aa9ad6 mitigated/controlreplacement archived; coverage5d69759479e75de7 fixedforad hochelper.

Both containers removed, independently absent again at finalclosure: NU553498c6de28d9a92cf0d7086ed3686fcb614befb35a7521d43822cb9f1fa571; RA4d87bee8169d36787551e634dac45689cbea6fa6bcb3402bfe8d85c9dfe36341. Complete publiclog hashes, resourceinspect/terminalstates, pre/postguards/cleanup, source/helperhashes and exactcommand/casepopulations verified. Helper/tmp/close-native-baseline-01a15ca9-20261106.py, verified-closure.json retained.

Durablearchive docs/research/data/2026-11-06-executable-native-baseline-evidence.tar.gz SHAa6f224468b952c814988dc784f1bf036d02ad71aea1ba22ab3408b0bfc405f38,211918bytes,44payloadfilesplusmanifest,ALLreopened/hashverified. Includes5proposedlifecycle source/test/docfiles, logs/registration/resources/guards/helpers. Excludes mountedstatecopies/SQLite/modelcaches, allofwhichremainonscratch. Originalfailedv1/v2controls preserved. Fullprojectcompactgateed0 andPython76+30+24 previouslypassed; fixworktree /tmp/codescout-tmp-exec-01a15ca9 remainsdirty5ownfiles, no sourcecommit/merge/push. OriginalV615e sourceunchanged.

Mainreportfce51ffc4baf7906 docs/research/2026-11-06-private-grade-disk-audit.md updatedwithfullclosure. Auditseparatearchive89dfc97b0d7a901d4da3f37a9beb5576e7fe7d36f319cfdce8ddbf986367f518 (1600payloadfiles) preservesoriginalfourmodelarms, privategrades,independentcontrols,journal,and exactNUlow diagnosticreplay. Read research/effort-steering/private-grade-audit-20261106: NUlow originalcrashinvalid butsamepatchreplay2F2Pfail/8620P2Ppass; NUxhighrealP2Pregression+parserartifact; bothRAselectedsuitespass. Alloriginalwholeworkflowsremainincomplete; correctedprofilepassdoesnotretrofitoldmodeloutcomes or provegenericmechanism.

NEXT beforepaidruns: implement strict grader classification forobservedtoolchain/OOM/storagefault vssemanticfailure and treatparsermissing as distinct measurementissue (open54fe4b112aa4e423,0a2929ec2a4f688f), requalifypositive/negativecontrols; then newprospective matchedworkflow withonce-per-projectonboarding reused. These2now-openedtaskoraclesaredevelopmentcases, notfreshheldout. No activeownheavybaseline/replayjobs remain atclosure. IncidentalMAINissue7b9c9e4a040ec3cf recordslongcurrentmemorytopic semantic-anchorHTTP500; durabletextwritesucceeded, exacttopicreadworks, smallerdedicatedtopics/reportavailable.

## Subsequent peer commit — current fix SHA and metadata

Current sourcecommit36fec1201b0f809d6f21cc31082def4d651b6cf8 onfix/external-tmp-exec-20261106 replacesHISTORICAL30e4d928561410d55d390cc87323110eb698ae6f. CommittingClaudesessionb3485581-2540-4da8-a440-3bed2e9c517f correctedthetrailer toCo-Authored-Session-Id: codex:01a15ca9-f939-7603-acb5-01c6b02f68e2. FreshGitchecks: old→newtreediffempty, worktreeCLEAN, stablepatchide97db759fc5d8684ce5dd7faed27309d59a106f1 unchanged. Earlierall5source/archivebyteidentityandfullqualificationapplyunchanged. ThisCodexsessiondidnotamend/merge/push.

Archive+receipt separatelycommittedonexperiments as867546ab6c54851177004eb11076c0658f3755b2; actualGitmetadata stillbarecoauthoredUUID. Issuef4926e0a61d4cd69 remainsforarchiveinstance/guardvalidationgap, NOTalreadycorrectedfixbranch. Donotamendsharedhistory. Actualpublish_hold_refmapsbaretoken toClaude refs/holds/<sid>, codextoken torefs/holds/_codex/<sid>; noactualpush/activeholdbypassclaimed. Primarybug4d66b80cca2e6390 andplan9dfa837b81e7baaa updatedtocurrent36fec1201 withhistorical30e4d9285 labelledsuperseded. Peerreportthatbranchunpublishedwasnotindependentlyremoteverified; no pushperformedhere.

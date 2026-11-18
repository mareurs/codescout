---
id: '9dfa837b81e7baaa'
kind: plan
status: active
title: Correct native temporary execution and qualify external resources
---

# Correct native temporary execution implementation plan

> For agentic workers: use superpowers:executing-plans inline. Autonomous implementation already authorized.

**Goal:** Fix reproduced /tmp execution failures and qualify32GiB external resources without changing the live cohort.

**Architecture:** Normalize verified native argv, validate exact new external resources before credential access, qualify Docker behavior and complete public manifests.

**Tech Stack:** Python unittest, Docker, Rust.

**Spec:** docs/superpowers/specs/2026-11-06-executable-native-tmp-design.md

## Global Constraints

Worktree /tmp/codescout-tmp-exec-01a15ca9 based on Vc1d51c446. Original V/live inputs immutable. Four CPUs/init,32GiB external RAM,4GiB executable tmpfs UID/GID1000. Internal memory default12GiB. No paid reruns, host mount changes, merge or push.

## Review Focus

Equals-form/duplicate temporary flags; preservation of other mounts and payload; missing/noexec/wrong-size/memory rejection before credentials; actual Docker execution versus string-only checks; qualification mount isolation.

### Task 1: Temporary policy and qualification

**Files:** tools/agent_lifecycle/{workflow.py,external.py,test_external_runtime.py,test_external_entry.py,README.md}.

**Interfaces:** verified_native_argv retains memory_gib and returns corrected Docker argv; validate_prepared admits32GiB/exec4GiB profile. Original cohort continues on archived source.

- [x] Add tests for normalization/preservation/duplicate rejection and strict external policy; observe RED.
- [x] Implement minimal correction and run lifecycle/pilot/workflow Python suites; expected exit0.
- [x] Run same-image generated-argv executable/doctest checks and applied guard mutations; preserve observed outcomes.
- [ ] Run project gate and independent review; commit exact source/test paths with Session-Id, no push. Update MAIN issue with evidence.
- [x] Run sequential unchanged-source public qualification using isolated writable state and new resources. Preserve failures; no new paid cohort before qualification.

## Execution checkpoint

Fresh worktree clean; baseline lifecycle71tests passed. MAIN owns these managed artifacts because persistent catalog refuses /tmp-root artifacts by design. Implementation remains isolated in new worktree; frozen Vunchanged.

## Implementation checkpoint — 2026-11-06

Production correction implemented in isolated worktree, original Vclean/unchanged. RED external suite12methods observed9failures/2errors before production correction (new32GiB fixture rejected by old validator; temporary normalization/duplicate checks failed); postchange lifecycle75, pilot30 and workflow24tests pass. Both actual immutable-image generated-argv probes pass with memory.max32GiB, tmpfs4GiB, no noexec, executable exit0 and doctest exit0. No model calls. Runtime evidence /home/marius/.cache/codescout-resident/codescout/external-exec-baseline-20261106/runtime-probes.json.

Gate @bg_000000e8 record /tmp/codescout-bg/455879-a720241d-bg_000000e8.json running. Zero-provider qualification @bg_000000e9 waits for successful gate and old repairs-closed.json before sequential complete public checks with fresh writable mount copies. Reviewer tmp_exec_review applies mutations only to its own copy. No paid launch, commit, integration or push yet.

## Latest execution state — compact gate and resumed qualification

Reviewfixpass complete: swap-survivor regression killed on actual reapplication; POSIX/tmp aliases fixed after7observed REDsubtests and76GREEN. FouractualDockeraliascontrols eachsingleexec4GiB/tmp mount/executableexit0. Source5files remainuncommitted in isolatedworktree. Preflightarchive docs/research/data/2026-11-06-executable-tmp-preflight-evidence.tar.gz SHA12faf888791d504237117f455876f4962af9b4ad94700cdd2ad74f835c2a4796 includes24payloadfilesreopened/hashverified, excludesunfinishedfullchecks.

Diskpressure interrupted owninitial/queuedgates and qualificationwaiters before fullbaselines. Nonearepassed. Onlyidleleasedgate debugcache removed; originalcohort/activeDockerimages/containers/volumes/archives preserved. Compactfullgate@bg_000000ed nowusesdebugmetadata0/incrementaloff/buildjobs4/testthreads4, samegatelanes. Resumedqualifier@bg_000000ee waitscompactgate/old4closures/>=50GiBfree thensequentialunchangedpublicbaselines. ROOT external-exec-baseline-20261106-v2, details memoryresearch/effort-steering/executable-tmp-fix-live-20261106. Old4candidate/costclosures done/privategradingrunning. Laterrootfree66.18GiB, point-in-time only. No providercalls/newpaidprofile/commit/merge/push.

## Evaluation audit and source guard — 2026-11-06

User requested checking disk-pressure validity before continuing. Reportfce51ffc4baf7906 docs/research/2026-11-06-private-grade-disk-audit.md supersedes any unqualified reading of original raw2pass/2fail. NUlow is infrastructure-invalid (linker SIGBUS/zero test statuses in confirmed ENOSPC window); NUxhigh retains one genuine selected regression plus a parser-missed pass; bothRA selected suites pass. All4 public workflows incomplete. ExactsameNUlowpatch original4CPU8GiB diagnostic replay @bg_f7 is active with disk/cgroup telemetry and zero model/provider calls; original data immutable.

Prospective qualifier v2 failed before publiccommands on sole expected native.gitignore append; issue1720ba9339aa9ad6. New disposable guard accepts exactly clean or baseline+one.codescout/private-memories/line, requires sameHEAD, no staged/mode/othertracked changes. Realgitcontrols9/9 pass; reviewer actual prefix-omission survivor from initial8tests motivated ninthforwardingwrapper case. No production runtime source change for this guard. Independent guardreview underway; newhelper /tmp/qualify-executable-tmp-v3-01a15ca9-20261106.py is prepared but NOT launched. It requires compactgateed0, replayf7closed/absent and80GiBhostfree. Heavypublicbaseline runs must be sequential; no paidrerun beforequalification. First3 source/probe planstepschecked; commit and fullpublicqualification remainopen.

## Replay closed and corrected qualification launched

NUlow exactsamepatch original-profile diagnostic@bg_f7 CLOSEDexit0 with all8622selectedstatuses:2F2Pfail,8620P2Ppass,missing0.70resourcesamples/68successfulcontainerprobes,minimumroot322369671168bytes, no sampledOOMkills. Originalreceiptinvalid; this newdiagnostic genuinelyrejectspatch. No providercalls/candidatesampling; originalsources/receiptsunchanged. Independentrawassertionrecheckpending.

V3zero-providerpublicqualifier LAUNCHED@bg_000000fd, record/tmp/codescout-bg/455879-a720241d-bg_000000fd.json job2748923. Bothruntimeprobes0/removed; actualNushellpre-sourceguard accepts onlyexact.gitignoreappend withmatchingbaselineHEAD; publicbuild0. Publicworkspacecompilation/testsrunning in32GiB/executable4GiBtmpfs, no newskip. No paidrepairadmitted. Botholdbaselineattempts retained. Closurearchivepreparing; no sourcecommit/merge/push yet.

## Full qualification closure — 2026-11-06

@bg_fd EXITED0: bothNushell18863 andrust-analyzer22992 unchangedimagebaselines pass exactpublicbuild+testcommands, no addedskip. Testwalls518.59sand176.00s. Bothnativeexec/doctestprobes0; bothpre/postsourceguardsidentical withoriginalHEADandexactstartup.gitignoreappendonly. Bothcontainersremovedandindependentlyabsentagainatclosure. Registrationboundsource/helperhashesandoriginalcommandpopulationmatch.

Evidence docs/research/data/2026-11-06-executable-native-baseline-evidence.tar.gz SHAa6f224468b952c814988dc784f1bf036d02ad71aea1ba22ab3408b0bfc405f38,211918bytes,44payloadfilesplusmanifest;ALLreopened/hashverified. Includes5proposedsource/test/docfilesandcompletepubliclogs/resources/guards/cleanup. Auditclosurearchive89dfc97b... separatelypreservesfourmodelarmsanddiagnosticreplay. No paidcalls/commit/merge/push. Four of fiveplanstepscomplete; remaining step includeslocalcommitwhichhasnotbeenperformed,despitegate/reviewpartsbeinggreen. OriginalVfrozenunchanged. Beforepaidrerun alsoaddressgraderclassification54fe4b112aa4e423 andparser0a2929ec2a4f688f; environmentqualificationdoesnotclosethosebugs.

## Subsequent peer commit

Current operator-relayed Claude Code commit: `36fec1201b0f809d6f21cc31082def4d651b6cf8`, stable patch-id `e97db759fc5d8684ce5dd7faed27309d59a106f1`, on `fix/external-tmp-exec-20261106`. It replaces historical `30e4d928561410d55d390cc87323110eb698ae6f`; the committer corrected the Codex co-author trailer. Fresh tree comparison is empty and worktree clean. The namespaced trailer now matches the authoring Codex root; prior source/archive verification still applies.

This supersedes all earlier uncommitted-source and incorrect-current-trailer notes for the fix branch. This Codex session did not amend, merge or push. Separate archive commit `867546ab6c54851177004eb11076c0658f3755b2` on experiments retains the bare trailer; issuef4926e0a61d4cd69 remains open for that instance and any validation gap. Do not rewrite shared history to repair it.

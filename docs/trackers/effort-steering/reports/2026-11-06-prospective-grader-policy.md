---
id: b921f535e0b10832
kind: report
status: done
title: Prospective grader policy — native controls qualified
tags:
- effort-steering
- grader
- evaluation
---

# Prospective grader policy — native controls qualified

## What this establishes

A declared prospective wrapper preserves the raw dataset parser and grader evidence while explicitly separating measurement invalidity, semantic compiler failure and expected-test outcomes. It may recover only exact expected Rust statuses attributed to a glued physical line by the previously reviewed audit helper. This is measurement infrastructure for the low-versus-xhigh mechanism experiment; it does not measure the mechanism's repair effect.

## Source and scope

Plan93c543c96e2d7766, spec28aab59cb5842630. Isolated worktree `/home/marius/work/claude/codescout.worktrees/grader-validity`, branch `fix/grader-validity-20261106`, new sourcecommit867c680924cb75575bec7ccd7068b8dfd7fd5889, patch-id8aaef7e82bf4106b5be13beff64238d89b3d2a6c. Exactly2newproductfiles committed; clean worktree, no merge/push. Private tooling/evidence ROOT `/home/marius/.cache/codescout-resident/codescout/prospective-grader-policy-20261106`.

RegistrationSHA256ddadac4045041a8344077f2d78a61e49dae12998d86ff7360fff09952a4a3fc5 binds80unique file paths, with every input/source copy reopened and verified under frozen-inputs/. No provider/token/onboarding call.

## Offline evidence

Policy safe no-op first observed8failed6passed with23subtests. Four-log overlap calibration then exposed a false compiler classification: stripping indentation promoted an indented `GenericError` debug field into apparent compiler output. Excluding Cargo test-summary footers alone was insufficient. Three debug variants observed RED before the fix; physical indentation plus Rust diagnostic evidence now discriminate them. Before final review, policy17tests29subtests passed. Initial failed calibrations are retained; calibration-v3 matches all4expected kinds. Runtime proof is explicitly a SYNTHETIC fixture, so this is logical overlap only, not native qualification. All17historical protected files remain unchanged.

The NUlow original linker SIGBUS remains invalid; NUxhigh retains its real P2P regression after exact recovery of another P2P at physical line3725; both rust-analyzer outcomes retain selected expected pass. No old cohort verdict or byte was rewritten, and the already closed NUlow diagnostic was not repeated.

Deterministic private runner tests first observed15failed3passed including subtest outcomes. Before final review, runner and real-manifest builder tests10passed9subtests. Builder schema errorccebd6842eda67fd was reproduced before changing it. Combined new policy/private runner plus prior grader modules254passed123subtests (`targeted-before-review-v2.log`); this count does not include the separate project gate population.

## Admission criteria

Keep the existing private4CPU/8GiB profile. Six fixed fresh controls in order NUnegative,RAnegative,NUgold,RAgold,NUrepeat,RArepeat, without retry or resampling. Bind source/input/image identities; require complete expected statuses, valid discriminating negative, all selected expected gold/repeat pass, identical gold/repeat status maps. Sample host floors80GiBroot/4GiBtmp/48GiBMemAvailable every10seconds plus initial/final cgroup observations before deletion, zero OOM counters and independent daemon-confirmed absence. Samples do not reserve host resources or prove continuous health between observations. Any invalid attempt stops the sequence and remains recorded.

## Verification and native results

The fixed native cohort closed with exit0 at2026-11-06T19:00:57.811455543UTC (start18:22:12.187953454UTC). check_closed.py reopened and rehashed all80boundpaths and their frozen payload mappings, recomputed all6policies from full raw logs/grade.json, checked every recorded resource sample and freshly queried the daemon for absence of all6CIDs. Both control_verdicts are ok with zero gold/repeat mismatch. No run was retried or resampled.

| Task | Empty patch: F2P passes | Gold: selected expected passes | Gold repeat: selected expected passes |
|---|---:|---:|---:|
| Nushell18863 | 0/2 | 8622/8622 | 8622/8622 |
| rust-analyzer22992 | 0/1 | 8182/8182 | 8182/8182 |

All selected P2P also passed at the empty baseline (NU8620/8620,RA8181/8181). Nushell gold-repeat reproduced the real glued-status defect: rawmissing1, exactexpectedP2Ppass recovered at physical line3725, adjustedmissing0. Original raw/log bytes remain unchanged. All6adjustedmissing0. This qualification covers the registered two tasks, exact images/source and4CPU8GiB profile.

Resource sample counts were NU63/60/60 andRA16/18/18, including initial/final samples before deletion. Every observed OOM/group-kill counter was0. Lowest sampled host values across the cohort: root297.907GiB,tmp25.2527GiB,MemAvailable76.224GiB, above registered80/4/48floors. Largest observed sampling gap11.0552seconds; samples are not continuous assurance or a resource reservation. Saved cleanup receipts and a fresh daemon query confirm all6containers absent.

Source867c680924cb75575bec7ccd7068b8dfd7fd5889 and clean worktree verified after controls. Prior fresh final review:2Important findings fixed in one offline pass. Final expandedtargeted393tests138subtestsPASS; gate133CLOSED0/all5lanes0. Pythonlight1642tests529subtests is separate scope; heavy3/server-stack/localONNX/leanlibrarian exclusions retained. Exact10mutations reapplied/noresiting:10observedkills0survivors against290tests108subtests baseline, allsourcebytesrestored. Original review5kills5survivors and1340payloads preserved.

Evidence archive `docs/research/data/2026-11-06-prospective-grader-policy-evidence.tar.gz` is closed and verified: SHA256 `2bc2fc8706b4b4d7c3d302e6b6cdd0b832704903db3bd4fe4ba4b23e7cf17fbc`,14758129bytes,2570payloads plus manifest. Every payload was reopened and hashchecked; archive job138CLOSED0. The archive preserves initial failed calibrations, review/mutation evidence, frozen inputs, all6native logs/receipts/resource traces and closed-audit.json. Public classification is still not integrated into the live public workflow; paid admission remains false. Next is the concrete paid adapter with resource/continuation stops, then the fixed six-arm pilot. No new repair model/provider/token/onboarding call occurred. Mechanism effect remains unmeasured.

## Rulings I made

- Retain private4CPU/8GiB and treat public32GiB as a separate profile. Cost if wrong: this fixed cohort fails qualification and needs a separately declared registration; no silent retry.
- The required review-package helper refused the empty commit range because new product files are deliberately uncommitted before verification. Freeze their actual bytes and private tooling in `review/manifest.json` and an explicit package supplement. Cost if wrong: a delta could be omitted from review; the manifest and named new-file scope make that auditable.

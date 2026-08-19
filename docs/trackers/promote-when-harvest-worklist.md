---
id: '0a18b891df4e1bbf'
kind: tracker
status: draft
title: Cross-repo Promote-when harvest — measured worklist (2026-08-20)
owners:
- marius
tags:
- trackers
- promotion
- cross-repo
- record-legibility
topic: record-legibility
---

> **What this is.** A one-time measured sweep for `Promote-when` criteria that have **fired**
> and were never harvested, across every repo on this machine carrying session-log trackers.
> Not a living tracker and not backed by a checker — the checker was considered and declined
> at n=1 (`docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md`).
> Its purpose is that the expensive half — *finding* them — is not re-derived.

## Population

Measured 2026-08-20 over **764 tracker files** in **33 tracker directories** across 28 repos,
worktree copies excluded.

| | count |
|---|---:|
| `**Promote-when:**` lines | 655 |
| entries whose Promote-when text indicates a **fired** criterion | 86 |
| — of those, `Status:` records a promotion / hold | 29 |
| — of those, **open candidates** | **57** |
| open candidates in codescout (triaged in full, below) | 12 |
| open candidates in other repos (listed, not triaged) | 45 |

**57 is an upper bound, and the residual error is one-directional.** Hand-reading codescout's
12 found **5** genuinely fired; the other 7 are future-tense criteria — *"A second X → promote"*
with `Status: validated — single datapoint`. No pattern separates those from *"criterion met"*,
because the difference is **tense**, and the classifier that produced 57 cannot read tense. Do
not treat 57, or any rate derived from codescout's 5/12, as a population figure for the rest.

### How the count moved, and why it is recorded

The number went **12 → 54 → 56 → 57** across four instrument refinements. Each was a different
wrong predicate, and the sequence is the useful part:

1. a flat `*.md` glob — missed `docs/superpowers/trackers/` and every `archive/` subdir;
2. per-**line** matching — the disposition usually lives on the entry's `**Status:**` line, not
   its `**Promote-when:**` line, so "already promoted" was invisible;
3. first-word-of-Status — `validated — promoted to X` reads as `validated`;
4. relative paths handed to `awk` — it read nothing, printed `fired entries: 0`, and **exited
   0**. A clean, plausible zero from an instrument that never ran.

## codescout — triaged in full

| entry | verdict | destination |
|---|---|---|
| `bug-fix-session-log:W-32` | **promoted** | `CLAUDE.md` § Bug Tracking — *"run the reproduction before reading the fix plan"* |
| `bug-fix-session-log:W-36` | **promoted** | recon `SKILL.md` Phase 1 — trait method with no default impl |
| `reconnaissance-patterns:R-89` | **promoted** | recon `SKILL.md` Phase 1 — build vs process freshness |
| `reconnaissance-patterns:R-49` | **promoted** | recon `SKILL.md` Phase 1 — re-scout your own bug file |
| `archive/dzo-legibility-session-log:W-4` | **declined** | rule is false — see below |
| 7 others | **not fired** | future-tense criteria; left untouched |

**The declined one is the reason this is a triage and not a harvest.** `dzo-legibility:W-4`
wanted to promote *"before refactoring a method body, `count_symbols_by_name_path` > 1 means a
trait forwarder shares the name_path and `edit_code` is blocked — relocate the trait-impl block
first."* Its own `Status:` carries a CORRECTION two paragraphs down: `edit_code` resolves the
qualified `impl Trait for Type/method` form, the body was editable in place, the collision was
never a block, and the defect class was removed entirely (ADR
`docs/adrs/2026-06-13-drop-name-collision-defect.md`). Rubber-stamping the fired criteria would
have shipped a false rule as standing guidance.

**A fired `Promote-when` is a proposal, not a debt.** The harvest step is where it is judged.

## The other 45 — listed, not triaged

Not triaged here for two reasons: they are in repos whose standing instructions are not
codescout's to edit, and the same ~40% false-positive rate applies, so acting on the list
without reading each entry would promote unfired criteria.

| repo | open candidates | notes |
|---|---:|---|
| `mirela/backend-kotlin` | 14 | hand-read: **8 fired**. 5 of the 8 sit in `reconnaissance-patterns.md` with a **blank `Status:`** |
| `stefanini/southpole/MRV-poc` | 14 | shares tracker content with `mrv-chat-rewrite` — entries are **double-counted** across the two |
| `stefanini/southpole/advisory-proposal-generator` | 6 | 5 in `reconnaissance-patterns.md`, all blank `Status:` |
| `stefanini/invest-europe/ie-pal-engine` | 3 | all future-tense on inspection |
| `mirela/eduplanner-ui` | 2 | `tracker-hygiene-log` |
| `claude/code-explorer.old` | 2 | one duplicates codescout's archived `i1-session-friction` |
| `mrv-chat-rewrite`, `extended-crawler`, `manger-agent`, `prompt-engineering` | 1 each | |

### Two structural findings that change how the list reads

- **`reconnaissance-patterns.md` is a promotion TARGET, not a source.** Session-log `W-N` →
  promoted → `R-N` there. An `R-N` whose Promote-when says *"met — 3 datapoints, at the default
  bar"* is describing **why it exists**, not an unpaid debt; its *onward* promotion (to
  `SKILL.md` or memory) is the open question. Counting every one as a miss inflates the list.
- **Blank `Status:` clusters in exactly those files** — 5 of backend-kotlin's 8 fired, and all
  of advisory's. `get_guide("tracker-conventions")` calls `Status:` the field that makes a
  fired criterion harvestable, and it is absent precisely where criteria fire most.

## Reproducing this

```
find <roots> -maxdepth 6 -type d -name trackers | grep -v worktree     # 33 dirs
# then, per entry section (NOT per line — the disposition is on Status:):
#   fired    := Promote-when text matches reached|FIRED|met|already at|now at|REACHED|MET
#   harvested:= Status  text matches promoted|graduated|landed|written to|discharged|BLOCKED
```

Use **absolute** paths. The relative-path version reads nothing and reports `0` with exit 0.


## Appendix — the 45 raw candidates

Verbatim from the sweep: `path:line`, then the `Promote-when` opening. Read the entry before
acting — roughly 40% are future-tense and not fired.

```
claude/code-explorer.old/docs/trackers/archive/i1-session-friction.md:420
    er a single rule — the lesson is meta: **`merge=false` augment is partly surgical, partly destr
claude/code-explorer.old/docs/trackers/prompt-guide-refactor-session-log.md:274
    recon-before-build prevents another duplicate-mechanism build in a future refactor (≥1 more dat
claude/prompt-engineering/docs/trackers/harness-backlog-session-log.md:73
    econd pre-change scout that averts a broad test-call-site break → promote to codescout memory `
mirela/backend-kotlin/docs/trackers/archive/architecture-review-session-log.md:286
    te an ADR documenting the manual-DI per-domain composition-root pattern (`api/di/<Domain>Wiring
mirela/backend-kotlin/docs/trackers/archive/day-start-latency-session-log.md:69
    econd design-time scout catches a metric-vs-agency
mirela/backend-kotlin/docs/trackers/archive/optaplanner-removal-session-log.md:643
    eady twice in one session. If a third effort hits it, this belongs in the reconnaissance skill 
mirela/backend-kotlin/docs/trackers/branch-cleanup-audit-session-log.md:264
    's promote-when threshold was already met at 3 datapoints (R-1/R-2/R-3);
mirela/backend-kotlin/docs/trackers/iel-prevalidation-support-session-log.md:188
    econd instance of a prod-diagnostic SQL query diverging from the Kotlin predicate it reimplemen
mirela/backend-kotlin/docs/trackers/innovaplan-reconciliation-session-log.md:705
    econd data/fixture move turns up a presence-guarded consumer. Craft-shaped by the routing test 
mirela/backend-kotlin/docs/trackers/issue-triage-session-log.md:121
    eshold reached at 3 datapoints. Route to the project's `reconnaissance` codescout memory as: *"
mirela/backend-kotlin/docs/trackers/its-integration-session-log.md:1178
    eady earned. Promote to the `reconnaissance` codescout memory as: *brief every
mirela/backend-kotlin/docs/trackers/its-integration-session-log.md:2198
    econd counterparty-supplied sample set where measuring the claims changes the plan. At 2 datapo
mirela/backend-kotlin/docs/trackers/reconnaissance-patterns.md:342
    eshold met (R-1, R-2, R-3 all "read reality not the lagging signal"). Promote to
mirela/backend-kotlin/docs/trackers/reconnaissance-patterns.md:479
     project-shaped rule has met its threshold at 2 datapoints (F-8, F-13) — argued down from the d
mirela/backend-kotlin/docs/trackers/reconnaissance-patterns.md:505
    et (3 datapoints).** Two destinations — (a) project memory, for the outbound-to-Kedos specialis
mirela/backend-kotlin/docs/trackers/reconnaissance-patterns.md:575
    eady at threshold as a fourth datapoint on the family (R-20, R-21, R-23). The
mirela/backend-kotlin/docs/trackers/reconnaissance-patterns.md:709
    eady at three datapoints in one phase. Craft-shaped, not project-shaped — route to the skill's 
mirela/eduplanner-ui/docs/trackers/tracker-hygiene-log.md:185
    e shape confirmed across 2+ sweeps (already at 1, this sweep, 3 datapoints within it).
mirela/eduplanner-ui/docs/trackers/tracker-hygiene-log.md:197
    e shape confirmed across 2+ sweeps, OR immediately if `memory(write)` gains no merge/lock seman
stefanini/AI/albert/manger-agent/docs/trackers/manager-agent-session-log.md:654
    eady at three datapoints within one project. Promote to
stefanini/invest-europe/extended-crawler/docs/trackers/team-scoped-extraction-session-log.md:255
    econd IECRM-26-style investigation independently re-derives ground truth (a live fetch/diff) ra
stefanini/invest-europe/ie-pal-engine/docs/trackers/eval-calibration-session-log.md:138
    econd “guard on count/len before asserting on an aggregate that has an empty-sentinel” scout. A
stefanini/invest-europe/ie-pal-engine/docs/trackers/eval-calibration-session-log.md:77
    econd "spec named the low-level API; a higher-level method already
stefanini/invest-europe/ie-pal-engine/docs/trackers/refactor-recon-session-log.md:76
    econd instance where controller call-site verification overturns a subagent reachability verdic
stefanini/southpole/advisory-proposal-generator/docs/trackers/custom-instructions-session-log.md:178
    h the W-4 datapoint above this is already at 2 occurrences across two
stefanini/southpole/advisory-proposal-generator/docs/trackers/reconnaissance-patterns.md:107
     — 2 datapoints (the original 2026-07-18 miss, and this session's catch),
stefanini/southpole/advisory-proposal-generator/docs/trackers/reconnaissance-patterns.md:170
     more instance where a new discriminator value met a non-exhaustive dispatch.
stefanini/southpole/advisory-proposal-generator/docs/trackers/reconnaissance-patterns.md:41
     — 3 datapoints in a single session, at the default bar.
stefanini/southpole/advisory-proposal-generator/docs/trackers/reconnaissance-patterns.md:57
     — 2 datapoints per the source log's own stated bar, plus a 3rd independent confirming instance
stefanini/southpole/advisory-proposal-generator/docs/trackers/reconnaissance-patterns.md:73
     — 3 datapoints in a single session, at the default bar.
stefanini/southpole/mrv-chat-rewrite/docs/trackers/xlsx-figure-hints-session-log.md:46
    econd pre-spec scout of a store/loader interface prevents a fictional-spec component → promote 
stefanini/southpole/MRV-poc/docs/trackers/data-readiness-detached-app-session-log.md:1529
    . Two datapoints; the rule is craft-shaped, not project-shaped. Propose to
stefanini/southpole/MRV-poc/docs/trackers/data-readiness-detached-app-session-log.md:1661
    . Promote to the project's standing execution rule: a multi-task branch is
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:1101
     at three data points, but the two priors are retrieval-mechanism cases rather
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:1148
    ft-shaped and already at 3 datapoints in one session — propose for SKILL.md
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:1347
    hird instance of a tracker-recorded metric being re-derived at a different
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:1596
     third instance~~ **MET, same day — promote.** See the third instance below. Related: R-24 (the
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:216
    atapoints (this is the first). Need ≥2 more instances of a buffer-grep silent-negative before p
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:370
    econd work stream produces a set-valued check (C1 duplicate clustering and C2 figure reconcilia
stefanini/southpole/MRV-poc/docs/trackers/reconnaissance-patterns.md:394
    econd worktree session hits this same write-target drift. At two datapoints, promote as a manda
stefanini/southpole/MRV-poc/docs/trackers/sections-3-5-data-doc-session-log.md:380
    econd UR/requirements-doc section-structure claim is checked against `parse_template` and found
stefanini/southpole/MRV-poc/docs/trackers/shared/session-log.md:2008
    hird case where a cheap direct measurement of the *hypothesis* (not the symptom) overturns a we
stefanini/southpole/MRV-poc/docs/trackers/tracker-hygiene-log.md:845
    s is the third instance of the HY-7 class (HY-7 → HY-11 → here) — HY-11's promote-when already 
stefanini/southpole/MRV-poc/docs/trackers/tracker-hygiene-log.md:871
    eady fired once (this sweep). Recommend excluding `docs/trackers/retrieval-issues/**` from D3's
stefanini/southpole/MRV-poc/docs/trackers/xlsx-figure-hints-session-log.md:73
    econd pre-spec scout of a store/loader interface prevents a fictional-spec component → promote 
```

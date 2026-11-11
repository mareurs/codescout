---
id: '8c0392208b802da4'
kind: tracker
status: active
title: Mechanism candidates (MX ledger)
owners:
- marius
tags:
- mechanisms
- ledger
entry_prefix: MX
entry_high_water_MX: 3
---

# Mechanism candidates (MX ledger)

Each entry names a defect class from `opus-review-mining-wave-1.md`, a candidate mechanism, and its status. Nothing here is tried yet. `**Valid:**` is `dated 2026-11-11` for all.

## MX-1 — Implementer runs the mutation probe on each new test before reporting

**Status:** candidate — not tried.
**Valid:** dated 2026-11-11
**Rests on:** `docs/trackers/mechanisms/opus-review-mining-wave-1.md` § Observations 1.

**Defect class:** test-weak. 137 of 624 verified findings; 58 of 80 reviews; 41 of the 146 top-ranked findings.

**Mechanism:** the implementer prompt requires that each new or changed test is checked with `./scripts/mutation-probe.sh` against the one change it claims to guard. The report must paste the real output. A surviving mutant is fixed before the review.

**Why it might work:** many findings describe a mutant that survives (an assertion pins less than its message claims; a guard has no test). The repo already has the probe script.

**Unverified:** haiku labelled 100 of 137 test-weak findings `mechanical: yes`; only 32 of the 100 `mechanical_how` texts name a mutation probe. No hand check yet. No trial run: the cost in implementer time and the drop in test-weak findings are unmeasured.

## MX-2 — Stale-wording grep after every change that renames or re-sizes something

**Status:** candidate — not tried.
**Valid:** dated 2026-11-11
**Rests on:** `docs/trackers/mechanisms/opus-review-mining-wave-1.md` § Observations 2.

**Defect class:** doc-drift. 86 of 624 verified findings; 44 of 80 reviews; 13 of the 146 top-ranked findings.

**Mechanism:** a checklist line (or a script) for the implementer: for each term, number or name the change replaces, grep the repo for the old form (comments, messages, docs, tests) and fix or justify each hit.

**Unverified:** haiku labelled 28 of 86 doc-drift findings `mechanical: yes`; the other 58 are drift of meaning, which a grep does not find. The ceiling of this mechanism is therefore low. No trial run.

## MX-3 — Fix and implementer reports must carry real command output, checkable by the controller

**Status:** candidate — not tried.
**Valid:** dated 2026-11-11
**Rests on:** `docs/trackers/mechanisms/opus-review-mining-wave-1.md` § Observations 3.

**Defect class:** report-inaccuracy. 42 of 624 verified findings; 17 of 80 reviews.

**Mechanism:** the report records the command, the exit code and a log path for each claimed result (gate, probe, test run). The controller (main agent) re-runs or diffs one claimed result before dispatching the reviewer. In the pilot, a re-review rejected a fix round because its pasted probe outputs were reconstructed, not real.

**Unverified:** only 6 of the 42 findings are `mechanical: yes`; the miner judges most as claim-versus-evidence gaps that need a reader. No trial run. The check may cost as much as it saves.

## Template for new entries

---
id: '7985f4604fb9ee66'
kind: research-note
status: archived
title: 'Historical: 2026-10-30-natural-failure-feedback'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-30-natural-failure-feedback.md
source_body_sha256: 2009ea528751aeaf70eec598766b6b280f3256c6b6596490b069e70f1f4fb88d
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-30-natural-failure-feedback.md
  sha256: '2009ea528751aeaf70eec598766b6b280f3256c6b6596490b069e70f1f4fb88d'
  body_sha256: '2009ea528751aeaf70eec598766b6b280f3256c6b6596490b069e70f1f4fb88d'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-30-natural-failure-feedback.md
  sha256: '2009ea528751aeaf70eec598766b6b280f3256c6b6596490b069e70f1f4fb88d'
  body_sha256: '2009ea528751aeaf70eec598766b6b280f3256c6b6596490b069e70f1f4fb88d'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-30-natural-failure-feedback.md

---

# Natural-failure feedback diagnostic — 2026-10-30

The three fresh low reviews repair the known empty-suffix crash before any feedback is delivered. The xhigh review does too. All four pass the fixed primary contract, but a separate overlap probe distinguishes them: U and V still mishandle overlapping custom/built-in suffixes, while F, X and upstream gold preserve the expected JSON highlighting. This is useful development evidence for cheap review plus independent checks, with no established feedback effect or general low/xhigh parity.

## Fixed comparison

All four arms start from the same exact saved low C2 implementation and receive the same merge-preparation review prompt, which does not name the empty-input defect. They use isolated profiles/workspaces, privately pinned CLI2.1.309 and claude-sonnet-5-5. No model subagents, paid retries or task substitutions occur.

| Arm | Assigned workflow | First/final CLI cases | First/final upstream tests | Feedback blocks | Client USD | CLI seconds |
|---|---|---|---|---|---|---|
| U | low, silent observer | 6/6 both | 80/80 both | 0 | 0.1081944 | 117.85 |
| V | low, generic recheck if failing | 6/6 both | 80/80 both | 0 | 0.1211422 | 107.52 |
| F | low, precise failure if failing | 6/6 both | 80/80 both | 0 | 0.0955674 | 76.93 |
| X | xhigh, silent observer | 6/6 both | 80/80 both | 0 | 0.3475444 | 119.80 |

Here “first” means the first Stop after that arm's review, not the broken source before any model work. Each has one successful Stop, unchanged first/final patch hashes and source-bound parent semantic review. Primary acceptance explicitly permits either harmlessly ignoring empty suffixes or cleanly rejecting them. Low solutions ignore them; X rejects empty CLI values while also protecting library recursion. Accepted contracts do not imply identical behavior.

Because V and F receive no message, their configured feedback treatments are unexercised. Their better or worse results cannot be attributed to generic or precise feedback. These are three low trajectories on one known defect, plus one xhigh trajectory, rather than four independent problem samples.

## Broader behavior found after the primary evaluation

The F and X diffs also continue trying matching suffixes when stripping an earlier match finds no syntax. X adds explicit tests for a custom `-old` suffix overlapping the built-in `.dpkg-old`. A parent probe independently builds each exact snapshot in a fresh source-bound container and checks `--ignored-suffix=-old` with `test.json.dpkg-old`, plus a nonoverlapping control. All corrected probe processes exit 0; the predicate compares their actual stdout against the frozen JSON syntax reference.

| Snapshot | Nonoverlapping control | Overlap preserves JSON highlighting |
|---|---|---|
| Original low C2 draft | pass | fail |
| Upstream gold | pass | pass |
| U low review | pass | fail |
| V low review | pass | fail |
| F low review | pass | pass |
| X xhigh review | pass | pass |

This is a post-hoc development probe, not a replacement acceptance denominator. All four primary verdicts remain unchanged. The six initial overlap invocations incorrectly passed the leading-hyphen value as a separate argument; those six rejected arguments before behavior, while their six nonoverlapping controls passed. Those invalid probe records are retained separately from the corrected equals-form executions. They are not behavioral failures.

F demonstrates that low can find both defects here; U and V demonstrate variation that the initial finite oracle does not catch. X's added tests helped expose this second case. Selecting the cheapest successful low trajectory after seeing this probe would not establish a general cost advantage.

## Verification boundaries

Complete diffs were reviewed against the measured initial source inventory. New regressions preserve existing assertions; no oracle bypass was found. Actual Cargo receipts show U/V run full Cargo tests before their final Rust changes and targeted tests afterward; F runs Cargo tests successfully on its final Rust source. X has no successful Cargo execution in its actor trace: its build/format attempts were denied by the fixed tool policy. Parent checks independently compile and grade every first/final snapshot, including X.

All actors have the same restricted tool policy. This evaluates that specific workflow, not unrestricted xhigh tooling. The recorded CLI wall times include Stop checks on a shared host; neither the fixed execution order nor one case supports a general latency claim. File-request audit finds requested paths inside each own workspace; Bash request review finds repository-local operations and denied commands. Request records do not prove every requested command executed.

## Accounting

New CLI estimate: USD0.67244840000000001. Cumulative: USD16.778193899999999875. The user added USD20 after registration, making the global cap USD40 and leaving USD23.221806100000000125. The current phase preserves its frozen USD20 admission rule, four allowances and no-retry policy. Native orchestration/review dollars and billed totals are unknown.

The original low draft plus U's fresh review totals USD0.31241130000000001 in client estimates for this lineage. X also starts from the draft, so its repair cost is not an end-to-end xhigh generation baseline.

## Evaluator preparation and checks

A color-never control falsely accepted the broken implementation because it bypassed syntax detection. Corrected color-always controls reproduce four empty-input aborts in both the original candidate and upstream gold, with two valid cases matching the frozen ANSI reference. A separate hand-fixed control passes six CLI cases and all 80 upstream tests. The invalid initial control is preserved and never used as a paid grade.

Independent review applied six guard mutations: four killed, two survived, zero invalid against nine tests. Regressions added after observed survivors make the same six fail against the then-current 12 protocol tests. Observer replay contracts reject six invalid session/workspace/process variants actually accepted before correction. Frozen prepayment verification passes 14 tests after explicitly normalizing the real Cargo parser's kind/count schema. Three later closure tests bring the final suite to 17 passing tests; two separate applied closure controls, removing the budget-guard call or reusing the old global cap, are both killed. These two controls are separate from the six protocol mutants. The original free-control wrapper's metadata KeyError after 80 actual passing tests is retained separately from the Cargo result.

An admission preview created an exclusive receipt before V's CLI started. The worker refused the overwrite; the preview was retained separately, then the first actual V invocation started. No model call or outcome was discarded and no frozen code changed.

The complete repository gate exits 0: PYTHON_LIGHT, FMT, CLIPPY, LEAN and DEFAULT all pass. Its printed limits remain: heavy Python suites, server-stack, local ONNX weights, and librarian code in LEAN; librarian tests run in DEFAULT. Rust/source code is unchanged from the parent commit. Final document/tracker integrity and reference audit are recorded separately.

## Research implication and retained evidence

A fresh low review is a promising repair component on this known case. A broader independent oracle is necessary to select among its variable results. For an actual test of diagnostic feedback, run the external check on a failing draft before starting the repair call, so the intervention is delivered rather than repeatedly waiting for a first Stop that already passes. Validate the frozen mechanism later on disjoint tasks; no such external validation is claimed here.

[Design](../superpowers/specs/2026-10-29-natural-failure-feedback-design.md), [plan](../superpowers/plans/2026-10-29-natural-failure-feedback.md), tracker LEM-16. The private phase natural-failure-feedback-20261029 retains registration SHA256819a87a68369dda63118fc6f0c74dfc75883c124c0d0089ee6a2dbe9c7f17243, analysis, exact inventories/patches, raw receipts/transcripts, controls, source reviews, both overlap probe versions, mutation logs and closure. Prior anchor external-feedback-20261029/final-outcome.json SHA256d202704b19aa0d29fecb50dbc71cf562e4c372bc4433ac5823d7b3b2f9a9902f.

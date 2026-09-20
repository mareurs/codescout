---
id: '1b770cb6462acde6'
kind: tracker
status: active
title: Deep-agent workflow observations and session coverage
tags:
- deep-agent
- observation
- prospective
- workflow
topic: deep-agent-observation
time_scope: '2026-09-18_to_2026-10-02'
entry_prefix:
- DWF
- DCS
snapshot_anchor: '| ID | Date UTC | Kind | Sampling | Capture key |'
entry_high_water_DWF: 2
entry_high_water_DCS: 1
---

# Deep-agent workflow observations and session coverage

**Status: collecting, 18 September–2 October 2026 (UTC).** First review: 25 September. Stop routine capture at 00:00 UTC on 2 October unless extended by the user. Companion: [context timing observations](deep-agent-context-observations.md). Design: [codescout deep agent](local-semantic-evaluator-design.md). Starting evidence: [baseline measurement](../research/2026-09-18-deep-agent-observation-baseline.md).

## Question and scope

Which current investigation/test/fix workflows are repeatable enough to delegate to a lightweight codescout worker, and what evidence would let it verify completion? Observe ordinary host-agent/human work before building the worker. A successful human-assisted sequence is not proof a local or API model can reproduce it. Record handoffs, authority, tool results and actual completion checks; keep that claim distinct from automation potential.

## Sampling protocol

The session's coordinating host agent owns one **routine-first** sample: the first substantive investigation, test/verification, or fix expected at initiation to need at least two task tool actions. Logging calls do not count as task steps. Select it when the task enters that loop, before the outcome; keep the sample even if it aborts after one action. Include workflows that complete normally or need no intervention. Capture pre-action evidence before the next dependent step; do not halt legitimate work to construct a full retrospective plan. Additional material failures, recoveries or unexpectedly effective workflows are **enrichment** and must be analyzed separately. Historical seeds are excluded from both.

One case may appear in both ledgers; link it and treat it as one grouped incident, not independent training examples. Delegate results flow to the session collector; record the actual actor, not an invented principal. Carry the capture key over compaction. Link canonical incident records instead of reproducing them. If collection starts after the outcome is known, declare `retrospective`; never reconstruct missing pre-decision facts from the later fix. Do not introduce new model calls, extra test runs or permission requests solely to populate this study.

## Workflow episode fields

| Field | Record |
|---|---|
| Status / Valid | `pending-outcome`, `observed`, or `needs-adjudication`; dated validity |
| Sampling / capture mode | `routine-first`, `enrichment`, or `historical-seed`; `prospective` or `retrospective` |
| Identity / key / times | Stable session+task key, actor/principal and collector, observed provider/model or unknown; start, capture and finish UTC (unknown when absent) |
| Task / authority / substrate | Objective; allowed work as actually instructed; workspace, base revision and dirty/worktree state; target paths |
| Pre-action evidence | Source spans/revisions or sanitized observations available then; constraints and uncertainty; later outcome kept separate |
| Initial next action / completion check | Intended next action and the independently observable check needed for this task; do not invent a detailed plan after execution |
| Trajectory | Ordered actual tools/actions with safe args or references, raw result slices/exit states; failed attempts and guidance; linked DCTX context timing; gaps explicitly marked |
| Effects / recovery | Diff or artifact revisions, test/probe command and observed result, retries, intervention, cancellation/concurrency, rollback attempted/result or not needed |
| Outcome / basis | `good`, `bad`, `mixed`, or `unknown`; `verified-complete`, `partial`, `blocked`, `abandoned`, or `unknown`; name what the check actually established |
| Delegation candidate | Candidate bounded workflow, deterministic alternative, missing inputs/checks; proposal only, never a measured autonomous success |
| Rests on / grouping / overhead | Canonical source/incident, durable evidence, session/task/lineage grouping, actual or estimated capture time explicitly distinguished |

Green tests establish only the behavior they exercise. A fix episode should retain its observed failing baseline and post-fix check when the task produced them; if either is missing, mark that fact rather than run unrelated tests for a score. Investigation completion may be a well-supported unresolved finding, not a code patch. The collector's interpretation is provisional unless a named check supports it; do not use confidence or a tracker status as the completion oracle.

Freeze pre-action facts and append dated outcome/correction paragraphs. Do not overwrite a failed attempt with the eventual success. A usage row with outcome `success` is a tool-record outcome, not task verification; native tools and child principals may be missing from that instrument. Keep minimal sanitized durable excerpts, hashes and exact locators when feasible. Full prompts/transcripts, credentials and raw databases do not belong in this repository. A transient output-buffer handle alone is insufficient for later adjudication.

## Session coverage receipts

DCS entries are **coverage records, not workflow episodes**. At session end/handoff, the coordinating collector writes one receipt, including a zero/unknown receipt when there was no selected episode. A resumed session updates the same receipt/capture key rather than counting another session.

Record: session/principal/collector identity (or unknown); UTC observed start/end; workspace; coverage `complete-observed-session` or `partial`; DCTX routine/enrichment IDs and DWF routine/enrichment IDs; for each missing routine sample, `none-observed`, `missed-capture`, or `unknown`; native/delegated/unobserved gaps; collection overhead and how measured; unresolved pending entries. `complete-observed-session` means the collector covered its stated session interval, not all machine activity. Do not invent an eligible-opportunity total without a contemporaneous tally. No DCS receipt is unknown participation, not zero failures. usage.db session IDs are a separate recorded population and cannot establish complete coverage of host-native activity.

These receipts make sampling compliance inspectable. Day-7/day-14 analysis reports participation/missingness first and never treats DCS rows as good/bad workflow cases.

## Append and outcome update

This prose ledger owns DWF episodes and DCS coverage receipts. `params` holds configuration, not duplicated observation rows. Server-allocated IDs and one atomic section/index write avoid duplicate numbering.

```python
doc(action="append_entry", id="1b770cb6462acde6", id_prefix="DWF",
    anchor_heading="## Template for new entries", title="<objective and observed result>",
    body="**Status:** pending-outcome\n**Valid:** dated YYYY-MM-DD\n\n<episode fields>",
    index_row="| {id} | YYYY-MM-DD | workflow | routine-first | <capture key> |")
```

For a session receipt use the same call with `id_prefix="DCS"`, a title identifying the observed session interval, the coverage fields as body, and `index_row="| {id} | YYYY-MM-DD | coverage | session-receipt | <capture key> |"`. The declared `snapshot_anchor` places the row at the index tail. For updates, use heading-scoped `doc(action="update", patch={"body_edits":[...]})`; preserve pre-action fields and add a dated outcome or correction. Index rows are static sample identity, not outcome status. No hand-built params arrays and no separate index mutation.

## Historical examples, excluded from prospective counts

[U-40](codescout-usage-frictions.md) records a discriminating edit-miss investigation; [PR-review F-4 and W-3](pr-review-session-log.md) are two accounts of the same adversarial review and belong to one incident group. [W-1](pr-review-session-log.md) shows a scope comparison changing a review path. These seeds identify data worth capturing; their later explanations are not pre-action training input. Neither repeated citations nor multiple records of one incident create independent examples.

## Review at day 7 and day 14

Use the frozen baseline query and declared UTC bounds for new usage aggregates; report schema/build/workload changes and retention limits. Then enumerate actual DWF/DCTX episodes and DCS receipts by record type, sampling mode, capture mode and missingness. Adjudicate routine and enriched cases separately against their recorded checks. Inspect ordinary completions, unnecessary intervention, failed checks, effective recovery, and incomplete outcomes. Group linked records before any evaluation split. Identify a concrete workflow with usable initial evidence and a completion check; if those are absent, improve capture or extend deliberately. Do not start training or implementation merely because two weeks elapsed.

## Index

| ID | Date UTC | Kind | Sampling | Capture key |
|---|---|---|---|---|
| DWF-1 | 2026-09-18 | workflow | historical-seed | seed-usage-U40 |
| DCS-1 | 2026-09-18 | coverage | setup / partial | setup-2026-09-18-root |
| DWF-2 | 2026-09-20 | workflow | enrichment | s48d1f0c8-round3-fixer-dispatch |

## DWF-1 — Historical seed — discriminate an edit-miss hypothesis

**Status:** needs-adjudication
**Valid:** dated 2026-08-17

**Sampling / capture mode:** historical-seed / retrospective; excluded from prospective collection counts.
**Identity / key / times:** original principal and exact timestamps unknown; seed-usage-U40; source dated 2026-08-17, captured here 2026-09-18.
**Task / authority / substrate:** investigate a scoped multiline edit miss; original grants, HEAD and worktree state not reconstructed.
**Pre-action evidence:** no frozen packet. The original request bytes and later diagnosis would need separate reconstruction; do not insert the diagnosis into a training prefix.
**Initial next action / completion check:** source reports a scratch multiline probe distinguishing unsupported multiline behavior from literal escape corruption; exact initial plan is not reconstructed.
**Trajectory:** the canonical U-40 record describes repeated reads, a scratch two-line edit with actual newlines, and a subsequent successful single-line anchor edit. Complete ordered raw calls are not preserved here.
**Effects / recovery:** successful scratch probe refuted the broad multiline-unsupported hypothesis; the original edit recovered through a single-line anchor. No claim of a newly fixed tool or autonomous worker run.
**Outcome / basis:** good, provisional historical-source label; reported recovery with a discriminating probe, not independently rerun in this collection. Task disposition: partial evidence of completion.
**Delegation candidate:** bounded reproduce-and-discriminate investigation; missing raw inputs, revision and independent terminal evidence must be addressed before evaluation.
**Rests on / grouping:** [codescout-usage-frictions:U-40](codescout-usage-frictions.md). All later summaries of U-40 remain one incident group.
**Overhead:** not measured.

## DCS-1 — Collection setup — partial-session coverage

**Valid:** dated 2026-09-18

**Status:** recorded

**Capture key:** setup-2026-09-18-root

**Coverage:** partial observed session; rules were introduced during this session. Coordinating principal `/root`; host session attribution not independently verified for this receipt. Terra research and Sol protocol review contributed to setup; this is not a prospective task sample.

**Routine context sample:** unknown — no contemporaneous selection under this protocol before setup.

**Routine workflow sample:** unknown — no contemporaneous selection under this protocol before setup.

**Historical seeds only:** DCTX-1 and DWF-1; excluded from routine denominators.

**Observed work:** reproduced frozen usage aggregates and created collection ledgers and project rules. See [baseline](../research/2026-09-18-deep-agent-observation-baseline.md).

**Capture gaps:** pre-rule work and native-tool activity cannot be reconstructed as complete prospective coverage. Missing receipt or missing sample must not be interpreted as zero eligible work.

**Recording effort:** not measured; no numeric estimate.

**Handoff:** future sessions follow first-eligible sampling and preserve pre-action evidence; review 25 September, routine collection ends 2 October at 00:00 UTC.

## DWF-2 — Five-agent fixer dispatch built from each bug file's own Fix ruling — pre-action packet

**Status:** pending-outcome
**Valid:** dated 2026-09-20

**Sampling / capture mode:** enrichment / prospective. **NOT routine-first**: this session's first substantive episode ran before a context compaction and was never selected under this protocol, so the routine slot is declared `missed-capture` in this session's DCS receipt rather than backfilled here. Outcome genuinely unknown at capture — the five agents were still running when this was written.

**Identity / key / times:** collector and coordinating principal = session `48d1f0c8-9f60-43bb-a15e-17ec7995813a`, profile `~/.claude-kat`, observed model Opus 5 (1M context). Five delegated subagent actors under that session: three Sonnet, two Opus (provider observed; no per-actor session ids are minted, so they are not independently attributable — see `ca77cfe338b3f789`, which is that gap filed as a bug). Capture key `s48d1f0c8-round3-fixer-dispatch`. Dispatch and capture 2026-09-20; finish unknown at capture.

**Task / authority / substrate:** objective — fix five filed bugs drawn from the open ledger, one agent each. Authority as actually instructed: the user's standing *"continue autonomously; when in doubt, check trackers and/or measure first"*, under CLAUDE.md's constraints (no push absent an explicit ask; never `git add -A`; `experiments` only). Workspace `/home/marius/work/claude/codescout`, base revision `170eac15`, tree carrying one modified file owned by another session (`.codescout/audit/ripper-65e654-202609.jsonl`, untouched). Target paths declared disjoint per agent: `scripts/probe_augmentation_restore.py`; `scripts/install-hooks.sh` + `tests/hooks-discrimination.sh`; `src/librarian/tools/audit_doc_refs/**`; `src/prompts/README.md` + `src/prompts/mod.rs`; `src/librarian/tools/doctor.rs`.

**Pre-action evidence:** (a) `doc(find, kind="bug", status in open/taken/investigating/zombie)` returned **62 rows** at dispatch. (b) Ten candidates' own `## Fix` sections read before any brief was written, and **five ruled a fixer OUT** and were not dispatched: `1fc9a6192a3f31b3` (*"the right answer is a decision, not a patch"*), `f47274c162774e8e` (*"not choosing between them here"*), `863fb5cf6bf011ef` (irreversible host-local `DELETE` wanting an operator decision), `c04e0d83106045d7` (the fix lives in a different repo), and `aa1110786cd2a8a4` / `1a0a6887f5998777` (both *"not designed"*). (c) `./scripts/gate.sh` observed green at `170eac15` **before** dispatch — `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` — so no agent would be handed a peer's red as its own. (d) File-ownership overlap **measured, not assumed**: `grep mode="files"` returned `src/librarian/tools/doctor.rs` for both `fix_anchor` (63 matches) and `docs/augmentations` (8), and `src/prompts/mod.rs` in three separate searches; `ls src/librarian/tools/` then established `audit_doc_refs` is a separate directory module, which is what made two of the five safe to run concurrently. Constraint and uncertainty held at capture: whether briefs built from each file's own `## Fix` section reduce the prior round's defect rate (4 of 11 briefs there prescribed remedies the bug files explicitly ruled out) is the open question; n is far too small to answer it and this episode is not designed to.

**Initial next action / completion check:** next action — receive five hand-backs and integrate. Independently observable check, fixed **before** the outcome: (1) `./scripts/gate.sh` green at the integrated tree, read as its four printed exit codes rather than as an absence of output — the `;`-chained form ends in `echo`, so its shell status is 0 whatever happened; (2) each agent's claimed new test present **by name** in the DEFAULT lane's output, since `--no-default-features` compiles no librarian code and a lean green there is silence, not a pass; (3) `git show --stat <sha>` per commit showing only that agent's declared paths — a clean `git status --short` is explicitly **not** accepted as evidence of exclusion, being equally what inclusion produces.

**Trajectory:** five concurrent `Agent` dispatches issued in one turn after the scouting above. Ordered per-agent trajectories not yet observed; to be appended on hand-back. Gaps marked rather than inferred. No linked DCTX entry — no separate context-timing decision was selected for this episode.

**Effects / recovery:** none observed at capture. Five bug files moved `open` → `taken` with `claimed_by` set to this session id, **verified at the bytes** by grepping the written frontmatter rather than trusting five `updated: true` responses — an earlier call in this same session returned `updated: true` for eleven files while writing a wrong deletion sentinel into all of them.

**Outcome / basis:** unknown at capture, deliberately left unset. The protocol's instruction not to overwrite pre-action facts with the eventual result is precisely why this paragraph is written now rather than after the hand-backs.

**Delegation candidate:** the bounded repeatable step here is **not** the fix — it is the triage gate in front of it: *read a filed bug's `## Fix` section and classify whether it prescribes a patch, a decision, or an out-of-repo change.* Five of ten candidates classified "not a patch" on their own text, and each classification is checkable against the file that produced it. Missing input for automating it: **no machine-readable marker distinguishes a prescriptive `## Fix` from a deliberative one** — today the difference is carried entirely in prose. Proposal only; no autonomous run was measured, and this episode establishes nothing about whether a lightweight worker could reproduce the classification.

**Rests on / grouping:** one grouped incident spanning this session and its five dispatched agents. The five bug files are the canonical records and are not reproduced here: `9a1998aaf4b09801`, `044e3c141cf80359`, `db80a4adc712c971`, `ea152af988811fa1`, `8713b680435c878a`.

**Overhead:** capture ~8 minutes, actual. Distinguished from the scouting above, which the task required regardless and is not collection overhead.

## Template for new entries

Choose either the workflow field table or the session coverage fields. This is the append anchor, not a recorded episode.

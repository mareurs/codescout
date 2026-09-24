---
id: '0cc578bbc332d699'
kind: tracker
status: active
title: Deep-agent context timing observations
tags:
- deep-agent
- observation
- prospective
- context
topic: deep-agent-observation
time_scope: '2026-09-18_to_2026-10-02'
entry_high_water_DCX: 2
entry_prefix:
- DCX
snapshot_anchor: '| ID | Date UTC | Sampling | Capture key | Observation |'
---

# Deep-agent context timing observations

**Status: collecting, 18 September–2 October 2026 (UTC).** First review: 25 September. Stop routine capture at 00:00 UTC on 2 October unless the user extends it. This ledger collects evidence before implementing the [codescout deep agent](local-semantic-evaluator-design.md). Its companion is [workflow observations and session coverage](deep-agent-workflow-observations.md). Baseline and instrument limits: [measurement report](../research/2026-09-18-deep-agent-observation-baseline.md).

## Question and scope

When does project context help a real next action, arrive too late, mislead it, or add no value? Observe current human/host-agent work; there is no deployed deep-agent worker to evaluate yet. Context includes codescout guides, operator rules, skills, memories, tracker precedents and source evidence. Record recipient and timing, not just retrieval similarity. A delivered packet is not proof it was used; a correct next action is not proof the packet caused it. Unobserved counterfactuals stay unknown.

## Sampling protocol

The coordinating host agent owns collection for its session and includes delegated observations under their actual principal. Delegates return evidence to that owner instead of independently duplicating it. If a delegate is itself the only collector, record that ownership explicitly. Keep the capture key stable across compaction; do not restart the first-sample rule on resume.

Select the **first eligible context opportunity** in the session regardless of its eventual outcome. An opportunity is a project-specific evidence/guidance choice immediately before a substantive investigation, edit, test, or conclusion: include ordinary manual lookup, automatic delivery, or a deliberate choice that current context is enough. Freeze the short pre-action fields before the dependent action when possible. Capture selected/available context and the next intended action; do not demand an exhaustive candidate search. If no candidate search ran, record that fact. Do not invent absent context at decision time because a later failure made it obvious.

Capture additional material misleading/late/missing-context incidents and unexpectedly useful interventions as **enrichment**, with the same fields. They are useful cases, not members of the routine-sample rate denominator. Do not manufacture equal good/bad counts, trigger errors for this study, or run extra experiments solely to fill an entry. Reference an existing U/F/W/R/T/bug record for the incident; the new record adds temporal decision evidence rather than a duplicate root-cause story.

At session handoff/end, write one DCS coverage receipt in the companion ledger even if no opportunity was observed. Distinguish `none-observed` from `unknown/incomplete`. Missing receipts mean unknown participation, never zero incidents. Session and subagent identity may be unknown; preserve that uncertainty. Capture overhead and any task displaced by collection. The study instructions can change behavior, so this is an observational collection, not a causal before/after experiment.

## Episode fields

Use these labels in each DCX entry. Short facts and durable citations are enough; do not copy an entire transcript or private reasoning.

| Field | Record |
|---|---|
| Status / Valid | `pending-outcome`, `observed`, or `needs-adjudication`; `dated YYYY-MM-DD` |
| Sampling / capture mode | `routine-first`, `enrichment`, or `historical-seed`; `prospective` or `retrospective` |
| Identity / capture key | Session, agent/principal, collector, provider/model if actually known; stable session+task+decision key or `unknown` |
| Time / substrate | Decision timestamp UTC and capture timestamp separately; workspace, HEAD, dirty/worktree state; source revision/hash where available |
| Objective / trigger | Immediate task and event that made context relevant; what the instrument could see |
| Pre-action evidence | Exact available source/span or sanitized output, its revision, limits/truncation, acquisition channel; exclude later fixes/diagnoses |
| Context decision | Selected context or none; known candidates/search coverage; recipient; intended next action and required-before event |
| Observed sequence | When context actually arrived, next action/result, concrete evidence of use if observable; link usage row IDs with database/window or a durable sanitized excerpt |
| Outcome / basis | `good`, `bad`, `mixed`, or `unknown`; specific result/check; separate delivery outcome `early`, `on-time`, `late`, `absent`, `unneeded`, or `unknown` |
| Counterfactual / missingness | Alternative actually tried and result, otherwise `not-observed`; missing evidence and deviations from sampling |
| Rests on / related | Durable source/entry citations, related DWF/DCS IDs, canonical incident key for grouping |

`good` means a recorded beneficial outcome (for example an observed useful correction); it does not establish causal lift. `bad` needs an observed misleading result, delay or interference. Mixed and unknown are first-class outcomes. A self-reported benefit with no external check remains `needs-adjudication`, with its label explicitly provisional. Preserve useful/no-change cases; no error is not by itself success.

Do not edit pre-action fields after seeing the result. Add an `Outcome update — <UTC>` paragraph; correct factual transcription through a dated correction retaining the old value. Reclassifications include their evidence and previous label. For source data containing secrets or private content, retain a minimal sanitized excerpt plus safe locator/hash; never commit credentials, full transcripts, or raw usage.db dumps. Opaque process-local buffers are not durable evidence: preserve the necessary sanitized content before it expires, or mark the evidence missing.

## Append and outcome update

This is a prose ledger with prefix DCX; `params` holds collection settings only. Do not create a second observation array. The server allocates IDs. Read this section once, then use the actual returned ID.

```python
doc(action="append_entry", id="0cc578bbc332d699", id_prefix="DCX",
    anchor_heading="## Template for new entries", title="<decision and observation>",
    body="**Status:** pending-outcome\n**Valid:** dated YYYY-MM-DD\n\n<fields above>",
    index_row="| {id} | YYYY-MM-DD | routine-first | <capture key> | <short title> |")
```

`snapshot_anchor` places the index row at the table tail atomically with the section; do not hand-allocate IDs or run a separate index write. For outcome updates, use `doc(action="update", id=..., patch={"body_edits":[{"heading":"<returned DCX heading>","action":"edit","old_string":"<unique pending text>","new_string":"<updated disposition and dated outcome>"}]})`. Keep the index static: it describes the sample, not mutable outcome status. On resume, find your existing capture key and update it rather than append a duplicate.

## Historical examples, excluded from prospective counts

[Context-injection W-3](context-injection-session-log.md) motivates recipient-specific delivery; the historical experiment measured parent redelivery and a positive control, not comprehension. [TU-7](2026-08-15-tool-usage-investigation.md) is the healthy-guard counterexample: a frequent error may already supply an effective remedy. These are dated leads for sampling and adjudication; later diagnoses cannot be inserted into prospective inputs. The baseline report records the bounded seed review and its overlap.

## Review at day 7 and day 14

Group by canonical incident, session/task and source lineage. First report coverage receipts, routine vs enrichment vs historical counts, prospective vs retrospective capture, missing pre-action evidence, unknown identity/outcomes, and collection overhead. Then summarize observed useful/harmful/no-change patterns with evidence; do not pool enriched cases into a prevalence rate. Keep delivery timing separate from task outcomes. Ask whether context was available early enough and whether any actual decision changed. Decide whether to extend capture, narrow a trigger, or select a workflow experiment; the calendar alone does not make data training-ready.

## Index

| ID | Date UTC | Sampling | Capture key | Observation |
|---|---|---|---|---|
| DCX-1 | 2026-09-18 | historical-seed | seed-context-injection-W3 | Recipient-specific guide delivery |
| DCX-2 | 2026-09-24 | routine-first | sebf651ec-open-bug-verify-sweep-brief | verifier brief contents for 34-bug sweep |

## DCX-1 — Historical seed — recipient-specific guide delivery

**Status:** needs-adjudication
**Valid:** dated 2026-09-14

**Sampling / capture mode:** historical-seed / retrospective; excluded from prospective collection counts.
**Identity / capture key:** original session/principal not reconstructed; seed-context-injection-W3.
**Time / substrate:** source observation dated 2026-09-14; captured here 2026-09-18. Exact original event timestamps and workspace hashes unknown.
**Objective / trigger:** preserve child bootstrap while avoiding redundant parent guide delivery after child dispatch.
**Pre-action evidence:** no frozen decision-time packet recovered. The source's later narrative is outcome evidence, not a model input.
**Context decision / observed sequence:** source records parent calls before/after stamped subagent dispatch and a fresh-guide positive control. See the canonical record for experiment configuration and its historical comparison limits.
**Outcome / basis:** good, provisional historical-source label. Source W-3 records parent redeliveries 2 to 0 while child delivery and a fresh parent guide remained available. This measures delivery precision, not comprehension or task improvement. Delivery timing for a substantive task: unknown.
**Counterfactual / missingness:** this collection did not rerun the experiment; exact pre-decision context, actor attribution and downstream task outcome are missing.
**Rests on:** [context-injection-session-log:W-3](context-injection-session-log.md). Group all records of that experiment together.

## DCX-2 — Verifier brief for the 34-bug open-issue sweep: doctor citations, the two-mode lesson, and read-only limits

**Status:** pending-outcome
**Valid:** dated 2026-09-24

**Sampling / capture mode:** routine-first / prospective. This is the first context decision this session made before a dependent action and captured in time. Earlier decisions (whether to trust a peer's slot-file claim; what to put in a fork brief) were not captured and are declared missed in this session's DCS receipt.

**Identity / capture key:** session `ebf651ec-5ab7-42d9-a526-dcf9758692e1` (collector and coordinator), model Opus 5.5; recipients are four general-purpose Opus subagents. Key `sebf651ec-open-bug-verify-sweep-brief`.

**Time / substrate:** decided and captured 2026-09-24T11:41Z; HEAD `436a8ff6`, dirty tree (see `DWF-7`).

**Objective / trigger:** brief the delegates that will verify 34 old bug files. Iron Law 6 says delegates see only what their brief carries.

**Pre-action evidence:** the coordinator holds the doctor output (`open_bug_cited_from_source` rows naming the source files that cite 5 of the bugs), the per-bug id/path list, the part-1 finding that bug files can describe more than one failure mode and that fixes often land under other bug files, and CLAUDE.md's "run the reproduction before reading the fix plan" rule and shared-target hazards.

**Context decision:** include in each brief: the bug id/path list for its batch; the doctor-cited source files per bug; the two-failure-mode lesson (verify every claimed mode, not only the headline); where fixes hide (sibling archived bug files, `git log -S`, `git log --since=<opened> -- <cited paths>`); read-only limits (no edits, no git index/HEAD moves, no `--no-default-features` builds, no mutations in the shared tree); patch-id recipe; the verdict vocabulary. Told to fetch `get_guide("tracker-conventions")` themselves if needed. **Not included:** the full doctor output and unrelated open bugs. Intended next action: dispatch.

**Observed sequence:** the briefs were delivered at dispatch; all four agents stayed read-only; no edits or git state changes were reported or observed. The two-mode lesson was visibly used: batch C split `f47274c1` into three modes with separate verdicts, and batch A split `523233935` A/B and found B recurring on a branch the earlier fix did not cover. Guide delivery to the recipients was **not** as intended: two of the four verifiers were silently starved of `symbol-navigation` by the race filed as `e76556484627a41a`, while a third got it three times. **Outcome / basis:** mixed. The brief context was useful (per-mode verdicts observed), but the auto-delivered context was misrouted; delivery outcome `absent` for 2 of 4 recipients. **Counterfactual / missingness:** no alternative brief was tried. Whether the starved verifiers' reports suffered is not established; batch C's report was solid, and its work leaned on scripts rather than symbol navigation. **Rests on / related:** `DWF-7`, `e76556484627a41a`.

## Template for new entries

Use the field table above. This heading is the insertion anchor, not an observation.

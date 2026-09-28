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
entry_high_water_DCX: 5
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
| DCX-3 | 2026-09-25 | routine-first | se4fbc7ef-medium-tier-verify-brief | Verifier brief for the 18-bug medium-tier re-verification |
| DCX-4 | 2026-09-26 | enrichment (retrospective) | 571eb3d6/explore-brief-splade-vram | Explore brief for SPLADE's 2.9 GiB; its one log-testable VERIFIED claim did not hold |
| DCX-5 | 2026-09-28 | routine-first (retrospective) | 82cff72e/phase1b-s2/gpu-claim-warning | GPU-claim warning routed on the fork notice's self-reported name; reached a different process from the twin |

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

**Observed sequence:** the briefs were delivered at dispatch; all four agents stayed read-only; no edits or git state changes were reported or observed. The two-mode lesson was visibly used: batch C split `f47274c1` into three modes with separate verdicts, and batch A split `523233935` A/B and found B recurring on a branch the earlier fix did not cover. Guide delivery to the recipients was **not** as intended: two of the four verifiers were silently starved of `symbol-navigation` by the race filed as `c161cc27ddff5672`, while a third got it three times. **Outcome / basis:** mixed. The brief context was useful (per-mode verdicts observed), but the auto-delivered context was misrouted; delivery outcome `absent` for 2 of 4 recipients. **Counterfactual / missingness:** no alternative brief was tried. Whether the starved verifiers' reports suffered is not established; batch C's report was solid, and its work leaned on scripts rather than symbol navigation. **Rests on / related:** `DWF-7`, `c161cc27ddff5672`.

## DCX-3 — Verifier brief for the 18-bug medium-tier re-verification: read-only limits, pin the workspace, fetch guides explicitly

**Status:** observed
**Valid:** dated 2026-09-25

**Sampling / capture mode:** routine-first / prospective. The first context decision this session captured before its dependent action. An earlier one — which sections of the six high-severity bug files to read before re-verifying them — was not captured and is declared missed in this session's DCS receipt.

**Identity / capture key:** session `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92` (collector and coordinator), model Opus 5.5; recipients four general-purpose subagents on Opus. Key `se4fbc7ef-medium-tier-verify-brief`.

**Time / substrate:** decided and captured ~2026-09-24T21:35Z; HEAD at or after `a3781d2a` on `experiments`, main checkout shared with 6+ live sessions; a `scripts/gate.sh` run in flight in a leased slot.

**Objective / trigger:** re-verify the 18 open medium-severity bugs a live session does not hold, before choosing any fix — the same method this session applied to the six highs (`DWF-9`). Iron Law 6: delegates see only their brief.

**Pre-action evidence:** the coordinator holds the refreshed medium list (status, id, class per file), `DCX-2`'s outcome (per-mode verdicts paid off; auto-delivered guides reached 2 of 4 recipients), this session's measured hazards (MCP respawns drop activation and pre-restart buffers; a peer's in-flight `.rs` reds `FMT` in the gate; `fmt-mine` / shared `target/` hazards), and the house rule "run the reproduction before reading the fix plan".

**Context decision:** each brief carries its batch's id/path list; the read-only limits (no edits, no git index/HEAD moves, no cargo in the shared `target/` — `scripts/with-slot.sh` if a run is essential — no mutations in the shared tree); pin `workspace=` on every call because the server can respawn under them; verify EVERY failure mode a file claims; where fixes hide (`git log --since=<opened>` over cited paths, archived siblings, `-S` on a symbol); the verdict vocabulary; and an explicit instruction to FETCH `get_guide("symbol-navigation")` and, only if needed, `get_guide("tracker-conventions")`, rather than rely on auto-delivery — the `DCX-2` lesson. **Not included:** the other 83 open bugs, the session's commit history. Intended next action: dispatch four batches in parallel.

**Observed sequence:** (2026-09-25, coordinator's reading of the returns plus its own spot-checks) Four batches dispatched in parallel. **Batches B, C, D returned** and stayed inside the read-only limits: at return the shared index held no staged path of theirs. `git worktree list` showed only the pooled `mutation-slot-0`, which predates the sweep and is re-synced by `mutation-probe.sh`, so a probe run cannot be ruled out from that listing alone, and none was reported. Their returns reported fetching `symbol-navigation` explicitly rather than relying on auto-delivery, which is the brief's `DCX-2` provision working as intended. One recipient (batch D) followed the brief over a conflicting startup-hook suggestion, which is the precedence the brief intended but did not state. The coordinator spot-checked each return before writing notes, and **narrowed one over-read claim**: the reindex-in-a-worktree `unverified:` caveat. A return read the guarded worktree skip (`indexer.rs:291-297`) as resolving it, but the skip explains the zero-file walk and not the caveat's actual question (why another worktree has rows). The note records it as NARROWED, not resolved. 13 notes landed in `3db91b2e`. **Batch A (5 bugs) was lost to an API 429** mid-run (its last line: "Now I'll run the reproduction for bug 4"), and nothing of its work was returned. **Re-dispatched 2026-09-25 at HEAD `9ec5f07e` as ONE agent** rather than several, to lower 429 exposure, with the brief **revised at the point of failure**. Bug 4 is about a stopped session holding a catalog lock, so the re-brief adds: never signal a process you did not spawn, and never lock or open-for-write the shared `catalog.db` (a throwaway catalog via a code-confirmed override, or no reproduction). It also forbids `cargo rb` and `rb.sh` (bug 5 concerns rebuilds), and it passes this session's `commit-mine.sh` (`d859d04b`) and the lingering-server bug (`177695780d080014`) as prior results, so they are not re-discovered. **Batch A outcome** (returned 2026-09-25 about 04:40Z, after 110 tool calls and about 17 minutes). **The revised limits shaped the method rather than blocking it.** The verifier:

- ran every probe in throwaway repos and databases under its scratch directory, and sent SIGSTOP only to its own child;
- answered bug 4's lock question with a SQLite-semantics probe on a throwaway WAL database, not against the shared catalog;
- left no trace in the tree: `file-provenance` attributes no uncommitted repo write to this session beyond the coordinator's own, and it reads subagent transcripts, so a verifier write would have shown as this session's.

It returned five verdicts: 1 partially fixed, 3 still live with narrowed claims, and 1 not reproducible here. It accounted for both prior-results provisions (`commit-mine.sh`, and `177695780d080014` as a sibling rather than a duplicate), which is the only evidence that those provisions paid; whether it would have re-discovered them unaided is unmeasured.

**Coordinator spot-checks:** about 18 claims, among them 3 patch-ids, an ancestor check, the WAL pragmas, three profiles' deny lists, the stale-server counts, the `codex` parents, `guard_stale_binary`'s scope, citation drift, and two `claude-plugins` facts. All held. The one apparent mismatch was the coordinator's own grep pattern. The most consequential finding, `git commit -a` passing both ownership guards, was re-run independently with a control arm; the mechanism (`index.lock`) was found, and the finding filed as `ecb8e59d7be06c01` (its id since it was fixed and archived the same day). The coordinator also added two facts the verifier could not know: parent `2834158` is the coordinator's own process, and parent `1180549` no longer exists. Findings the notes carry that only the verifier ran, among them the WAL timings, the same-file and `-A` capture arms, and the `restore --staged` arms, are described in the notes without a coordinator re-run.

**Rests on / related:** `DWF-9`, `DCX-2`.

## DCX-4 — Explore brief for SPLADE's 2.9 GiB carried the measured facts and the vanished compose path; the one VERIFIED claim the logs could test did not hold

**Status:** observed
**Valid:** dated 2026-09-26

| Field | Record |
|---|---|
| Status / Valid | `observed`; dated 2026-09-26 |
| Sampling / capture mode | `enrichment`, `retrospective`. Captured about 15:45Z, after the outcome. Session `571eb3d6`'s routine-first DCX sample is already `missed-capture` (DWF ledger DCS-5), so this is not a new routine sample |
| Identity / capture key | Key `571eb3d6/explore-brief-splade-vram`. Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`, resumed as `0cbae2f0-9c0a-40e0-bf04-7612432a3233` on `~/.claude-sdd` (codescout server env). Principal: the operator, through this coordinating session. Recipient: a background Explore subagent. Collector: the coordinating session. Model: claude-opus-5.5 per system context; the subagent's model was not recorded |
| Time / substrate | Decision about 12:25Z; capture about 15:45Z. codescout `experiments`, shared checkout, HEAD in the `84dba2bd`–`2f9e3a2c` range during the episode. Also targeted `~/agents/llm` and `~/agents/llm-proxy`, read-only |
| Objective / trigger | After a plain restart left SPLADE's TEI backend at 2.89 GiB on the RX 7800 XT, the operator said *"still 2.9G is a lot. you can send an codescout explorer subagent into llm-proxy or ~/agents/llm to understand what happens"* |
| Pre-action evidence | The router cmdline (`--max-batch-tokens 2048 --max-client-batch-size 8`). The container env (`PYTORCH_HIP_ALLOC_CONF=expandable_segments:True`). Compose labels naming `/home/marius/work/claude/code-explorer/docker-compose.yml`, which no longer exists. Restart numbers 5.30 → 2.89 GiB. Neighbour VRAM (1.30 / 0.39 GiB). The manual's gfx1100 build note against a gfx1101 card |
| Context decision | The brief passed all of the above as *"established, do not re-derive"*, plus the probable successor paths (`codescout/docker-compose.yml`, `docker/sparse-amd/`) marked *"verify both"*. It named six codescout docs and issues, forbade container operations and edits, and asked for VERIFIED (file:line or read-only measurement) versus INFERRED on every claim. No candidate search beyond those paths ran |
| Observed sequence | **Returned in about 17 min:** a component breakdown (weights about 0.2, warmup output tensors up to 0.93, fragmentation 0.5–0.9, ROCm overhead about 1.0–1.3 GiB by subtraction), 8 levers, and 7 drift findings. It asked for two log checks it had skipped as container operations. **The coordinator ran them:** `finish rocm warmup` appeared in 276 lines (the ROCm warmup path, confirmed), but `expandable_segments not supported` appeared in **0**, against the agent's source-derived claim that the feature is compiled out with that warning. The coordinator recorded it as unresolved (gpu-tuning `docs/trackers/research.md` Q-2) and did not adopt it. It then independently re-verified each drift claim it filed (`e83e92ce`) |
| Outcome / basis | `mixed`, delivery `on-time`. **Good:** the agent built on the measured numbers instead of re-measuring, and its drift findings became bug `2026-09-26-running-retrieval-stack-is-defined-nowhere`, each re-verified by the coordinator. **Bad:** one claim labelled VERIFIED (by source read) did not survive the one runtime check available; its label overstated its evidence class |
| Counterfactual / missingness | `not-observed`. No briefless dispatch was tried. The subagent's own tool calls were not inspected |
| Rests on / related | gpu-tuning `docs/trackers/research.md` R-5, Q-2 (commit `08e7acb`). codescout `docs/issues/archive/2026-09-26-running-retrieval-stack-is-defined-nowhere.md` (filed `e83e92ce`, fixed `7a51ebd1`). Companion workflow episode in the DWF ledger, key `571eb3d6/amd-second-card` |

## DCX-5 — Which session to warn before taking the GPU: routed on the fork notice's self-reported name, which resolved to a different process from the twin the operator identifies

**Status:** observed
**Valid:** dated 2026-09-28

| Field | Record |
|---|---|
| Status / Valid | `observed`; dated 2026-09-28 |
| Sampling / capture mode | `routine-first`, `retrospective`: the first context decision after this session's fork, captured 2026-09-28 from its transcript after the outcome was known |
| Identity / capture key | Session `82cff72e-0245-48cb-ab07-45a1c3d0d388`, main loop, self-collected; model claude-opus-5-5 per system context. Key `82cff72e/phase1b-s2/gpu-claim-warning` |
| Time / substrate | Decision 2026-09-26 11:55:01Z; capture 2026-09-28. Shared checkout `/home/marius/work/claude/codescout` on `experiments`, HEAD `0ff91320` (per `git log` at 11:54:44Z) |
| Objective / trigger | About to launch six GPU runs on the machine's only A5000. The session this one was forked from shared the plan and the operator's "continue", so a duplicate launch would collide on the GPU and in the run directories |
| Pre-action evidence | (1) The harness `<fork-source>` notice at 11:53:24Z: forked from "a session whose self-reported name is 'codescout-f8'", with history shared up to 11:52:04Z. (2) `ListAgents`: own row "Automated onboarding documentation roadmap item ⑂", no `codescout-f8` (the listing is profile-scoped). (3) Socket table at 11:54:46Z: pid 2264384, `.claude-sdd`, idle, `codescout-f8`, sid `571eb3d6`; and pid 1715810, `.claude-sdd`, busy, "Automated onboarding documentation roadmap item", sid `0cbae2f0`. Both had cwd in this checkout. (4) `git log`: `82d18742` (Steps 1–2) carries `Session-Id: 571eb3d6`. (5) GPU idle, no training process |
| Context decision | Routed on the notice's name, corroborated by the commit trailer: messaged `uds:.../2264384.sock` that this session was taking the N/NC runs and to reply if it had started anything. The busy name-twin (1715810, `0cbae2f0`) was in the same table and was not messaged. No further candidate search. `CLAUDE.md` § *Reaching a Peer Session* ("take the sessionId, not the name") was in context |
| Observed sequence | `SendMessage` returned success (queued; the recipient runs another permission mode). Soon after, the operator said this is the main session and "the other one is the fork … exploring whether we can run both AMD and NVIDIA at the same time", so the twin was doing GPU work too. The message was never approved: a delivery notice on 2026-09-27 reported it expired, after the operator closed that session ("a fork that I didn't know what it was doing"). The same day the operator named the other session as `0cbae2f0` (socket then 1604523), and DCS-12 records "571eb3d6, resumed as 0cbae2f0" |
| Outcome / basis | `bad`, delivery `on-time`. The warning went to a process other than the one the operator identifies as the twin and was never read. No collision followed, because the twin did not launch N/NC runs (its recorded work was the AMD card, DWF-12) and this session had independently verified the A5000 idle. Which process was the fork's source is **not established**: the notice's name resolved to 2264384, and the operator's identification points to 1715810's session |
| Counterfactual / missingness | `not-observed`: no message to 1715810 was tried. The recipient's own state was never inspected |
| Rests on / related | `CLAUDE.md` § *Reaching a Peer Session*; IC-8's member `sendmessage-returns-success-for-a-message-held-for-approval`; DCS-12; companion DWF-14 (key `82cff72e/phase1b-s2-step3-4`) |

## Template for new entries

Use the field table above. This heading is the insertion anchor, not an observation.

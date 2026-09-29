---
id: dd69959ffc06248a
kind: spec
status: draft
title: System 1 base-rate measurement — design
tags:
- measurement
- system1
- rule-tell
- telemetry
- deep-agent
topic: system1-measurement
---

# System 1 base-rate measurement — design

**Valid:** dated 2026-09-26

**Status:** design approved section by section with the operator in session `3c5b02df-b6ce-45f5-9d03-1194e38465c0`, 2026-09-26. This is a design, not a registration. The judge prompt text and the pilot's results are registered in amendments to this file **before** the stage they govern runs. Nothing below has been run.

**Origin:** the operator's instruction to measure before building ("even now we are biased on what we think is possible"), following `docs/research/2026-09-26-codex-three-role-intervention.md` and its § *Agreed extension: System 2 can direct System 1*.

## The question, and the decision it feeds

**Is a System 1 worth building?** Answer it descriptively, before any detector exists: how often a lesson that already existed applied and was missed, and what each miss cost. The decision is go / no-go / inconclusive on building, under the rule in § *Go/no-go rule*, fixed here before any data is read.

This measures a **ceiling**: the misses a *perfect* System 1 with a *perfect* intervention could have prevented. A real detector recovers only part of it. The best selector measured so far, S0, recalled 5 of 17 known violations (`docs/evals/rule-tell-scoring-2026-09-23.md`).

## Decisions made with the operator

| Decision | Choice |
|---|---|
| Purpose | Base rates: how often a lesson applied, how often it was missed, cost per miss. Descriptive; no behaviour change |
| Population | Corrections traced back to their origin, **plus** a blind random audit for the misses nobody caught |
| Judge | Codex, with a blind operator spot-check of 25 items |
| Scope | codescout, plus one contrast project (§ *Scope*) |
| Retention | `cleanupPeriodDays: 3650` set in all three profiles on 2026-09-26 (done); a frozen archive with a hash manifest |
| Approach | Offline pipeline over the frozen archive, **plus** two record-only codescout fields (`tool_use_id`, `deliveries_json`) |
| Go/no-go basis | Operator-caught addressable misses, **N = 3 per 10 sessions** |
| Corpora | Outside the repo; only manifests and results are committed |

## What a result cannot establish — read before any number below

1. **A GO measures the ceiling, not the benefit.** Whether a real System 1 helps is the next experiment, a phase-2-style replay with an actual detector (`docs/evals/rule-injection-timing-preregistration.md`).
2. **"Operator-caught" undercounts the operator's attention.** It sees only redirects that appear as text in a transcript. Silent fixes, fixes made outside a session, and the effort of *noticing* are invisible. So a NO-GO means "few misses reached the operator through a transcript". It does not mean the operator spent little attention. This is the limit that most changes how the result reads, which is why it sits here and not in a footnote.
3. **The retrospective window is short and moving.** Transcripts before 2026-08-26 were deleted by the default 30-day cleanup and cannot be recovered. The codebase changed fast inside the window, so rates are reported by week with codescout and Claude Code versions attached.
4. **Lessons in the operator's private global `CLAUDE.md` cannot be dated.** That file is not in git, so they are counted separately as `undated` and never as "existing at the origin turn".
5. **The judge is a model.** The spot-check catches a bad judge; it cannot certify a good one (§ *Operator spot-check*).
6. **Companion-hook injections do not pass through codescout.** They are recovered from transcripts where present, and otherwise reported as a coverage gap.

## Sources, and what each sees (measured 2026-09-26)

| Source | Sees | Blind to |
|---|---|---|
| Transcripts: 3 profiles, read with the claude-traces reader | Agent prose, every tool call and result with its `tool_use_id`, operator messages, `entrypoint`, `isSidechain` | Whether hook-injected text is recorded is not yet verified (§ *Verifications owed*) |
| `usage.db`, per project | Every codescout MCP call: args, result (95% of rows since 09-19 carry `output_json`), outcome, `agent_id`, `started_at`, buffer handles, `effect_class`. Engine injections are recoverable from `output_json`: the recorder wraps `tool.call_content` (`src/server.rs`), which runs the engine coordinator before returning (`src/tools/core/types.rs`) | Native `Bash`/`Read`/`Edit`, prose, operator messages. **No `tool_use_id` column** |
| git | `Session-Id` trailers (codescout from September; none in MRV-poc or backend-kotlin), correction diffs | Uncommitted work; claims made only in chat |
| llm-proxy → Langfuse | Every API request passes through the proxy (all three profiles route through `:8082`) | **Excluded.** Whether ingestion is complete is unverified; it enters the pipeline only after a completeness check |

At census, the codescout project dirs held 131 top-level transcripts across three profiles (124 open with `entrypoint: cli`, 7 with `sdk-cli`). All projects together held 3.2 GB.

## Scope

- **Primary:** codescout. The go/no-go is decided on it.
- **Contrast:** MRV-poc, which had more transcripts at census (35 against backend-kotlin's 17). If MRV-poc yields fewer than 15 sessions after exclusions, backend-kotlin replaces it.
- Neither project has `Session-Id` trailers, so the contrast is compared **only on corrections from operator messages and reviews**. It is reported beside codescout, never pooled. If the two rates differ by more than 2×, the readout says the decision may not generalise.
- **Session:** one top-level transcript with at least one decision point. Its subagent entries (`isSidechain`) count inside it. Excluded, each listed in the manifest with its reason:
  - `entrypoint == sdk-cli`;
  - `-tmp-…scratchpad` project dirs;
  - **forks**: a transcript whose opening message uuids repeat another transcript's. The original is kept;
  - **this design session** (`3c5b02df`).

## Architecture

```
 SOURCES (read-only)             FROZEN CORPUS (outside repo)       COMMITTED (repo)
 transcripts ×3 profiles ─┐
 usage.db (cs + contrast) ─┼─► 0. ARCHIVE ─► corpus/<id>/ + manifest ─► manifest.json
 git log + trailers ───────┘                     │
                                                 ▼
                                   1. JOIN → events.db
                                      turns · tool_events · deliveries · commits
                                                 │
                              ┌──────────────────┴──────────────────┐
                              ▼                                     ▼
                   2. CORRECTION MINER                    3. BLIND AUDIT SAMPLER
                              └──────────────────┬──────────────────┘
                                                 ▼
                                   4. READOUT → docs/evals/
```

- **Stage 0: archive.** A named, frozen snapshot per corpus, stored outside the repo, following the precedent of the rule-tell checkpoints under the rule-tell-runs directory beside this checkout, since transcripts contain the operator's private `CLAUDE.md`. The committed manifest records:
  - per-file sha256;
  - time bounds and row counts;
  - Claude Code and codescout versions;
  - every exclusion, with its reason.

  Every number in the readout names its corpus id.
- **Stage 1: join.** One events database:
  - `turns`: transcript messages;
  - `tool_events`: transcript tool calls joined to `usage.db` rows;
  - `deliveries`: injected blocks parsed from `output_json`, plus hook text if transcripts carry it;
  - `commits`: with their trailers.

  **Every joined row records how it was joined**, `exact` (on `tool_use_id`) or `heuristic` (session, tool, arguments, time). The retrospective window is heuristic throughout.
- **Stage 2: correction miner.** Finds corrections and traces each to its **origin turn**. It reuses the correction markers and parent-commit blame from `docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py`.
- **Stage 3: blind audit sampler.** Draws decision points uniformly, independently of Stage 2.
- **Stage 4: readout.** Metrics and the go/no-go below, in a results doc under `docs/evals/`.

The pipeline code goes in a new scripts/measure directory with its own tests.

## Definitions

| Term | Definition |
|---|---|
| **Decision point** | A sampled assistant message, parent or subagent, that makes a checkable claim (a count, an absence, an identity, a cause, "done", "verified") or takes a consequential action (edit, commit, dispatch, destructive command, recommendation to the operator). Sampling is uniform over assistant messages; the judge decides whether a sampled message qualifies. |
| **Lesson** | A rule in the frozen lesson list: `CLAUDE.md`, codescout memories, and promoted `R-N`/`T-N`/`OP-N` entries, **as they stood at the origin turn's commit**. A lesson written after its incident does not count for that incident. |
| **Mistake** | A decision point that a later correction reversed or amended, or that the audit judges wrong. |
| **Miss** | A mistake that an existing lesson applied to. A mistake no lesson covered is `uncovered`: it needs a new lesson, not a System 1. |
| **Detectability** | `in-trace` (the counter-evidence was visible before the decision), `obtainable` (one bounded lookup would have found it), or `external`. It mirrors `text_detectable` in `docs/evals/rule-tell-detection.md`, widened from the text to the whole trace. |
| **Correction** | An operator redirect, a commit that changes text another turn or session wrote (blamed at the parent), a review finding, or a retraction. Each records who caught it (`operator` / `self` / `peer-session` / `review`), the delay in turns and wall time, and the rework cost. Several corrections of one mistake are grouped by origin. |
| **Addressable miss** | A miss whose detectability is `in-trace` or `obtainable`. |

## Metrics

Each is reported per corpus and per project. Proportions carry Wilson 95% intervals; per-session rates carry session-bootstrap intervals.

1. Correction rate per 100 decision points, split by who caught it.
2. Operator-caught share of corrections.
3. Lesson coverage: misses divided by mistakes.
4. Detectability mix among misses.
5. Uncaught-miss rate, from the audit.
6. Cost per miss:
   - rework tokens (from the claude-traces reader, deduplicated by message id), tool calls, operator turns and wall time, from the origin turn to resolution;
   - `unknown` for uncaught misses, marked as such.
7. Deliveries, descriptive only: count, and whether the next action changed.

## Go/no-go rule — fixed before any data is read

**Quantity:** addressable operator-caught misses per session, on codescout, **over the decision window defined in Amendment 1, A1.2**: the latest 7 days before the freeze instant.

**Interval:** 95% percentile interval from 10,000 bootstrap resamples **of sessions**, with the seed recorded. Sessions are resampled because misses cluster within them.

| Result | Condition |
|---|---|
| **GO** | lower bound ≥ 0.3 per session (N = 3 per 10 sessions) |
| **NO-GO** | upper bound < 0.3 |
| **INCONCLUSIVE** | anything else, **or** the judge fails its gate, **or** the spot-check agreement is below 80% |

- **After INCONCLUSIVE:** the next window is prospective. It runs 21 days, starting the day after both codescout fields are verified live. At the observed rate of about four codescout transcripts a day that is roughly 85 transcripts **before** exclusions, comparable to the retrospective sample. It is read under this same rule, and **the retrospective data is never re-read under a revised rule.**
- **The audit is outside the rule.** It is descriptive, so it cannot become a second chance to clear the threshold.

## Judge protocol (Codex)

- **Setup:**
  - model and CLI version pinned and recorded;
  - a clean config channel, the equivalent of `JUDGE_CONFIG_DIR` in the rule-tell campaign;
  - subscription only, never the paid API;
  - three votes, majority wins; disagreement is recorded as a reliability measure.
- **Input:**
  - the decision point;
  - bounded context from before the decision;
  - the lesson list frozen at the origin commit;
  - for corrections only, the correction text.
- **Output, typed:**
  - in correction mode, whether the candidate really is a correction of the origin turn (yes/no);
  - decision point yes/no;
  - lesson id(s), `uncovered` or `abstain`;
  - detectability;
  - one evidence quote.
- **Quote check:** the quote is checked mechanically; it must occur verbatim in the pre-decision evidence, reusing phase 1's `verify_span` idea. An `in-trace` label whose quote fails the check becomes `abstain`.
- **Gate, before any corpus item is labelled**, with its thresholds fixed here. Positives are fed from the **pre-correction blob**, because seven of the 21 RTD corrections were appended and coexist with their positives at HEAD.
  - **Correction mode**, given origin plus correction: detectability agrees with the RTD corpus's `text_detectable` on at least **16 of 21**, under the mapping `yes → in-trace`, `partial → obtainable`, `no → external`.
  - **Audit mode**, with no correction given:
    - flags a mistake on at least **3 of the 4** `peer × yes` cases (RTD-3, RTD-8, RTD-9, RTD-10), the honest target cell named in `docs/evals/rule-tell-detection.md`;
    - flags a mistake on at least **6 of the 8** `yes` cases;
    - fires on **at most 5 of the 52** never-corrected controls in `docs/evals/rule-tell-controls.md`. That is reported as a firing rate, not a false-positive rate, per that file's ground-truth caveat.
  - **Disclosed limit:** the fixtures cover the claim-shaped class only, so passing the gate does not validate the judge for every kind of lesson.

## Operator spot-check

- **Sample:** 25 items, stratified: 10 operator-caught corrections, 10 audit items, 5 `abstain`/`uncovered`.
- **Blind:** the operator sees the judge's input and not its label.
- **Measured:** agreement on addressable yes/no (with `abstain` counted as no), reported as raw agreement and Cohen's κ.
- **Rule:** agreement below 80%, i.e. under 20 of 25, makes the decision INCONCLUSIVE.
- **Limit:** at 20/25 the Wilson interval is about 61–91%, so this catches a bad judge and cannot certify a good one.

## Sampling and sizes

- **Operator-caught corrections:** a census, up to 400. Past 400, a uniform sample registered before it is drawn, scaled back up by its sampling fraction.
- **Blind audit:** assistant messages are drawn uniformly, in an order fixed by a recorded seed, until **200 codescout and 100 contrast messages qualify as decision points**. Draws that do not qualify are counted and reported, and the qualifying share is itself a result.
- **Operator-caught corrections come in two steps.** The miner proposes candidates, aiming for recall; the judge then confirms or rejects each one (§ *Judge protocol*). Only confirmed corrections enter the census.
- **Pilot:** 20 items end to end, run first to measure Codex cost and time and to shake out the pipeline. Pilot items are excluded from every estimate. The sizes above are re-registered only if the pilot shows them unaffordable, and before any main-sample item is judged.

## Prospective codescout additions

Both only record: tool output bytes stay the same, and only `usage.db` rows gain fields.

**`tool_use_id`**
- Read from `_meta["claudecode/toolUseId"]` by a new function beside `conversation_from_meta` (`src/tools/session_key.rs`).
- **Kept separate from conversation resolution.** The `MEASURED_TOO_FINE` denylist exists so a per-call id never becomes a conversation key, and it stays unchanged.
- Stored as a `tool_calls.tool_use_id TEXT` column plus an index, added with the probe-then-`ALTER TABLE … ADD COLUMN` pattern in `src/usage/db.rs`.
- **Written unconditionally**, like the buffer-linkage columns. `NULL` means the client sent none, as with Codex CLI or Pi.

**`deliveries_json`**
- A column on the same row: an array of `{engine, ledger_key, sha256, bytes}`, one entry per block the engine coordinator (`src/engines/coordinator.rs`) attached to that call's result.
- The coordinator's per-engine metadata must reach the recorder as data; the recorder must never re-parse rendered text.
- Written unconditionally. **`[]` and `NULL` mean different things:** `[]` means the coordinator ran and delivered nothing; `NULL` means it did not run, or the row predates the column. Without that difference, "a delivery that changed nothing" cannot be told apart from "no delivery".

**Tests:**
- Each field lands in its row; `NULL`, `[]` and a populated value are separate cases.
- Mutation at each site: dropping one engine's record in the coordinator must turn a test red, and so must dropping the recorder's write (`scripts/mutation-probe.sh`).
- `conversation_from_meta` still ignores `claudecode/toolUseId`.
- Tool result bytes are identical before and after.
- Then `./scripts/gate.sh`.

**Live check:** after `cargo rb` and `/mcp`, a session's own next calls must join exactly to their transcript `tool_use.id`.

## Leakage and contamination controls

- **Every judge input is checked mechanically:** its latest included timestamp must precede the decision's. An input that fails is refused, not judged. This is the enforceable form of the Codex doc's "reconstruct what was observable before the decision".
- **Lessons are frozen at the origin commit** (§ *Definitions*).
- **Forks and headless runs are excluded** (§ *Scope*).
- **Sessions whose task was this measurement are excluded.**

## Testing the pipeline

| Stage | Check |
|---|---|
| 0 | Re-hashing a frozen corpus reproduces its manifest. |
| 1 | **Positive controls:** the OP-4 delivery on 2026-09-26 05:22:48 UTC, on an `edit_file` call, parses out of `output_json`; the fork test flags every known 2026-09-23/24 fork run listed in `docs/evals/rule-tell-scoring-2026-09-23.md`. **Completeness:** every session appears in the readout or on the exclusion list, with no silent drops. |
| 2 | The correction commits named in the rule-injection census are found. Each filter is mutated separately, as `96b52f0c` did (9 of 9 killed). **Secondary check:** incidents recorded as DWF/DCX entries in the observation window are found or listed as missed with a reason. That is a recall check on a *selected* sample and is reported as such. |
| 3 | The leakage check refuses a hand-built input that carries a later timestamp. The judge gate runs before any corpus labelling. |
| 2 × 3 | **Miner recall, estimated from the audit.** For every audit-found miss that the operator later corrected in the same session, check whether the miner proposed that correction. The share it found is the miner's recall on operator corrections, reported with its Wilson interval. **Why this is load-bearing:** a miner that misses operator redirects biases the go/no-go silently toward NO-GO, and this is the only check in the design that can see it. |

## Verifications owed before the plan's first task

Each is an open measurement, not a design gap, and each names the decision it changes.

1. **Do transcripts record hook-injected text?** If yes, companion deliveries enter `deliveries` retrospectively; if no, they stay a declared gap.
2. **Does the fork test catch the known forks?** If not, a different fork signal is found before Stage 1 runs.
3. **Codex cost per judged item.** Measured by the pilot; it decides whether the sizes stand.
4. **Is Langfuse ingestion complete?** Only a complete ingestion admits it as a source. Absent that check it stays excluded, and nothing in this design depends on it.

## Disclosures

Before writing this design, the designer read these counts:
- `usage.db` row and `output_json` counts;
- 5 `operator-rule` deliveries between 2026-09-19 and 2026-09-26;
- transcript counts, sizes and `entrypoint` values per profile and project;
- `Session-Id` trailer counts per repo.

No correction was counted, no correction content was read, and no rate that enters the go/no-go was computed.

## Sources

- `docs/research/2026-09-26-codex-three-role-intervention.md`: the three roles and the constraints inherited here.
- `docs/evals/rule-tell-detection.md` and `docs/evals/rule-tell-controls.md`: the gate fixtures.
- `docs/evals/rule-injection-timing-preregistration.md` and `docs/evals/rule-tell-scoring-2026-09-23.md`: the replay route that owns benefit measurement, the correction-commit census, the fork runs.
- `docs/evals/phase1-local-classifier-preregistration.md`: `verify_span`, Codex as independent labeller, the judge-channel lesson.
- `context-injection-session-log:F-14` and `context-injection-session-log:F-15`: the two hook-surface and gate-population corrections made while designing this.

## Amendments

### Amendment 1 — 2026-09-26, operator-approved, before any data was read

**Source:** the operator's update to `docs/research/2026-09-26-codex-three-role-intervention.md` (§ *Agreed priority: all three outcomes, measurement first*, and § *Proposed extension: a Codescout-specialised background miniagent*), and two operator decisions taken in session `3c5b02df` after reading it. No corpus had been frozen and no correction, rate or judge output existed. Where this amendment and the body disagree, this amendment wins.

- **A1.1 Scope: three outcomes plus an execution baseline.** The measurement now baselines all three outcomes the operator named:
  - fewer mistakes and repeated reviews;
  - relevant context at the moment of decision;
  - transfer of lessons across sessions and projects;
  - plus the opportunity size for a background worker.

  **Only outcome 1 carries a decision rule**, the go/no-go below. The others are descriptive baselines, and they feed the choice of the first intervention: frequency, observed cost, addressability and measurement confidence (Codex doc, § *Choose and evaluate interventions after the baseline*).
- **A1.2 Window.**
  - **Decision window:** the go/no-go reads only `[T − 7 days, T)`, where T is the freeze instant.
  - **Retained window:** everything since the oldest surviving transcript (2026-08-26). It is reported as a precedent and stability check and **never decides**.
  - **Why:** the operator's standing guidance that recent fixes make older aggregate rates misleading, and the Codex doc's *"older incidents supply precedents rather than current frequency estimates"*.
  - **Cost:** about 30 codescout transcripts were written in the 7 days before 2026-09-26, before exclusions, so INCONCLUSIVE is the expected outcome unless the true rate is far from 0.3/session.
  - **INCONCLUSIVE:** the prospective read covers `[T − 7 days, T_live + 21 days)`, where T_live is the day after Part A is verified live. It is read under the same rule.
  - **The contrast project** is too thin for 7 days (MRV-poc: 9 transcripts), so it is compared on its own retained window, descriptive only, and labelled as such.
- **A1.3 Outcome 2, context at the right moment.** For each qualifying audit decision point, the judge also returns:
  - `evidence_present_before`: yes / no / unknown. Was the evidence this decision needed present in the trace before it?
  - `evidence_used`: yes / no / unknown.

  Metric: the share of decision points whose needed evidence was present before the decision and used, with Wilson intervals. Deliveries whose `next_action_aligned` is `not-aligned` are reported as candidate unnecessary interruptions. That is descriptive; it is not an established harm.
- **A1.4 Outcome 3, transfer.** For each lesson the judge finds applicable at a decision point, it returns `applied` or `missed`.
  - **Metric:** transfer rate = applied / (applied + missed) over lesson-applicable decision points in the audit. It is split by project and kept separate for `undated` lessons.
  - **Why this matters:** misses alone had no denominator, which is the Codex doc's warning against letting failure trackers become the denominator.
  - **Not measured:** repeated investigation, or rediscovery. It needs semantic matching of investigations to earlier results, so it is listed as `needs adjudication` in the observability map.
- **A1.5 Background-worker baseline, deterministic and with no model.**
  - **The classifier:** each tool event is assigned to a task family by tool and action:
    - `tracker-maintenance`: `doc` `append_entry` / `update_entry` / `update` / `create` on `docs/trackers/`;
    - `specified-run`: `run_command` invoking `./scripts/gate.sh`, `cargo test`, `pytest` or a `scripts/` entry point;
    - `scoped-refactor`: `edit_code` `rename`;
    - `other`.
  - **The metric:** the share of tool calls and of recorded latency in each family, per session.
  - **What it is not:** it measures opportunity size, not delegability or saving. Latencies are never summed across overlapping calls and never presented as elapsed time saved.
- **A1.6 Observability map, published before any Codex call.** After the join, a committed document lists, for each outcome, each link of the chain (opportunity → signal or request → delivery or delegated action → observed use → independently checked outcome) as `measurable now`, `needs adjudication` or `unobservable`. It carries coverage numbers from the events database: joins that are exact, heuristic or unmatched; delivery coverage by source; sessions per project and window; and exclusions by reason.
- **A1.7 Spot-check.** The same 25 items; the operator also labels `evidence_present_before`, `evidence_used` and the applied/missed labels. Agreement is reported per field. Only the addressable yes/no label is decision-bearing (≥ 20/25); the others qualify their own baselines.
- **A1.8 Existing instruments first.** Before building each pipeline stage, check `docs/PROBES.md` for an instrument that already measures it. Reuse one only after checking its current predicate against the live schema, or record why it was declined.
- **A1.9 Telemetry justified by named measurements.** The Codex doc asks that telemetry be added only for a named, unresolved measurement that informs a decision. Part A's fields each name theirs:
  - `tool_use_id`: the exact joins that the prospective window depends on (A1.2);
  - `deliveries_json`: "was context delivered before the decision" for codescout's own injections (A1.3).

### Amendment 2 — 2026-09-26, recorded when Part A shipped

**Source:** Task 3, shipping Part A (`tool_use_id` commits 18e15470, decfd71f; `deliveries_json` commits 2884ae0d, 705bca92) and Task 2's Opus review (Minor 5). Where this amendment and the body disagree, this amendment wins.

- **(a) V1 answered 2026-09-26.** Transcripts record hook executions as `attachment` entries (`hook_success`: hook name, event, exit code, duration, `toolUseID`). They record injected text as `hook_additional_context` (SessionStart carried 5,692 chars in session `3c5b02df`). A Stop hook that emits nothing records no content. So companion deliveries enter `deliveries` from transcripts.
- **(b) The `deliveries_json` shape is one entry per claiming engine,** `{engine, ledger_keys[], blocks[{sha256, bytes}], hint}`, not one per block. The coordinator can attribute keys to an engine but not to a block. The spec's purpose, which engine delivered what with `[]` versus NULL, is unchanged.
- **(c) Metric 7 is `next_action_aligned`, not "changed".** Without a counterfactual, a pipeline can only observe whether the next action matched what the delivery said, as `aligned|not-aligned|unknown`. The readout labels it that way, never as effect.
- **(d) What `hint: true` in `deliveries_json` means.** It records that the engine coordinator KEPT that engine's hint for the response's `_guide_hint` field. It does NOT establish that the caller saw the hint:
  - `inject_hint` does nothing on a non-object value (`src/tools/core/guide_emit.rs`);
  - no `format_compact` renderer reads `_guide_hint`, so text-form tool responses drop it.

  Part B must never read `hint: true` as "hint shown". (Source: Task 2's Opus review, Minor 5.)

### Amendment 3 — 2026-09-26, recorded when Stage 1a (Task 5) landed

**Source:** Task 5 (`scripts/measure/transcripts.py`, commits 46b267b4, 66ea4e0b, 84dba2bd, 0ff91320, 4a09e95d), its three Opus reviews, and the controller's rulings R22–R32, R36–R38 in the SDD ledger. Where this amendment and the body disagree, this amendment wins.

**Disclosure, because the go/no-go rule is pre-registered.** Items (b), (c) and (d) change definitions after the controller probed the real corpus. The probes counted *shapes* only:

- sessions per sid across profiles;
- shared uuid prefixes;
- task-notification entries, command wrappers, bare slash commands and interrupt markers.

No probe read a correction, a miss, a judge verdict, or any quantity the go/no-go rule is computed from. The go/no-go rule and its thresholds are unchanged.

- **(a) V2 re-pointed (R24).** The spec's known forks (the phase-2 fork-route replays) are in no profile's `projects/` dir: they ran from a copied prefix under a throwaway config dir. No detector can find them, because they are absent. The positive controls are therefore:
  - `d8a1f024`, a genuine in-corpus fork of `571eb3d6`;
  - the `.claude-sdd` copy of `571eb3d6`, an exact prefix of the `.claude-kat` copy;
  - a synthetic fork fixture.
- **(b) One sessionId in several profiles is ONE session (R22).** A session resumed under another profile leaves a transcript in each, identified as `<profile>/<sid>`. The copy whose uuid list is a superset is kept, and the prefix copy is excluded as `duplicate-prefix-of:<profile>/<sid>`. When neither is a prefix of the other, the longer is kept and the other is excluded as `divergent-duplicate-of:<profile>/<sid>`, a case counted in the observability map.
- **(c) Forks are attributed, not excluded (R28, superseding the body's `fork-of` exclusion and R23).** A fork is a transcript with a different sessionId whose first 5 uuids equal another's. `relations()` labels it `fork-of:<copy_id>` for reporting only; the original is the member whose first entry after the common prefix is EARLIER.

  Counting goes through `attribute_entries`, which assigns EVERY entry uuid, of every entry type, to exactly one non-excluded transcript containing it: the one with the most TOTAL uuids, with ties broken by earliest `first_ts`, then `copy_id` (R31). An excluded copy owns nothing, and Stage D and `relations()` choose keepers with one function over one population, the non-excluded copies (R32, R36). So `relations(sessions, exclusions)` never names an excluded copy on either side. A consequence of R22 worth stating: a `divergent-duplicate-of:` copy's UNIQUE uuids belong to no transcript and are not counted. That is the documented cost of keeping one copy, and the observability map counts it. Every later stage counts an entry only for its owner. Measured on the 135-transcript live corpus (2026-09-26): 436,943 uuids, 0 with no owner or an owner that lacks them, and 1,657 assistant uuids shared across transcripts, each owned once. Excluding the fork would instead have discarded a near-superset holding 47 real operator prompts.
- **(d) What an operator message is.** On top of the body's exclusions (tool results, `isMeta`, `isCompactSummary`, command wrappers), `operator_messages()` excludes:
  - task notifications: `promptSource == "system"`, or `origin.kind == "task-notification"`, or, for older entries, text starting `<task-notification>` (R26; 1,409 entries);
  - wrappers starting `<command-message>` (R27; 66). A prompt that merely MENTIONS a wrapper tag stays a prompt;
  - bare slash commands, stripped text matching `^/[a-z][a-z0-9-]*$` (R29; 202, mostly `/compact`);
  - operator interrupts: text exactly equal to `[Request interrupted by user]` or `[Request interrupted by user for tool use]` (R25). These are returned separately by `operator_interrupts()`, recorded as `turns.kind='interrupt'`, and treated by the miner as a correction signal (source `operator_interrupt`, corrector `operator`), with the next prompt carrying the correction text. Exact equality, so a prompt QUOTING the marker stays a prompt.

  On 132 transcripts, R26, R27 and R29 took `operator_messages` from 4,528 (already net of R25's interrupts) to 2,851, exactly 4,528 − (1,409 + 66 + 202). Interrupts are counted apart from prompts, so the readout can show both.

### Amendment 4 — 2026-09-27, recorded when Part A was verified live

**Source:** Task 3 Step 2 (the live exact-join check), which first FAILED, then its fix, 6f6349ca (bug `922981c9afdd2a42`, archived). Where this amendment and the body disagree, this amendment wins.

- **(a) `tool_use_id` was NULL on every row until 6f6349ca.** rmcp moves a request's `_meta` into `RequestContext.meta`, and the shipped code read the params field, which is always empty on the wire. A call carries the id only when a binary at or after that commit serves it. Each Claude Code session gains that at its own `/mcp`, not at a single instant.

  So the exact join is available per session, from that session's reconnect onward. Every earlier row joins heuristically or not at all. The first exact row is 2026-09-27 04:58:01.690 UTC (session `0cbae2f0`). The observability map reports exact vs heuristic joins by period, and the readout must not assume exact joins anywhere in `[T_live, …)`.
- **(b) The live check was verified on another session (ruling R52).** The spec's check names this design session's own calls, but the property it checks belongs to the BINARY, and this session is excluded by spec. It was verified on `0cbae2f0`, the first session on the rebuilt binary: 85 of 85 rows carry a non-NULL `tool_use_id`, 85 are distinct, and each occurs exactly once as a `tool_use` id across its 7 transcript files. `deliveries_json` is non-NULL on 83; the other 2 are `recoverable_error`, the pinned NULL case. This also shows Claude Code 2.1.283 sends `claudecode/toolUseId`.

### Amendment 5 — 2026-09-27, recorded when Stage 1b (Task 6) landed

**Source:** Task 6 (`scripts/measure/join.py`, commits 2a5d6155, 1e32b228, bfab5fc4, 399a5ab9, 00286ae6), four Opus reviews that each ran the code over the live corpus, and rulings R40–R56. Where this amendment and the body disagree, this amendment wins. As with Amendment 3, these followed shape and count probes of the corpus only. No correction, miss, verdict or go/no-go quantity was read.

- **(a) `turns.kind`** is one of `prompt|interrupt|delegation|assistant_text|assistant_thinking|tool_use|tool_result|meta`.
  - `prompt` and `interrupt` come ONLY from Task 5's `operator_messages()` and `operator_interrupts()`, applied to top-level entries.
  - In a subagent file, a non-tool user entry is `delegation` (the parent model's brief), never `prompt`. Before this rule, briefs made up 19% of prompt rows. An interrupt marker there stays `interrupt`.
  - A thinking-only assistant line is `assistant_thinking` with text NULL. Thinking is never stored.
- **(b) The spec's exclusion is applied by default.** `build_events` excludes `3c5b02df` unless told otherwise.
- **(c) Every uuid passes one gate.** Top-level uuids are owned per `attribute_entries`. Subagent entries come from the union of a kept session's copies' subagent files, first writer wins across all kept sessions in attribution order. Every skip is counted.
- **(d) `tool_events.join_method` is one of `exact|heuristic|none|not_codescout`.** `not_codescout` marks a tool_use that cannot have a usage row by construction, so it is never a join failure. The heuristic key is: bare sid (plus the fork-of target's sid for a fork); agent, which is hard when recorded; tool; arguments, minus the principal stamp; and time within 120 s, each row used once.

  For time, the heuristic uses `started_at`, and when that is NULL, `called_at − latency_ms`. 81% of usage rows predate `started_at`, and on rows with both, the derivation is within 1 s in 99.8% of cases.

  Measured on a 70k-row snapshot: exact 85, heuristic about 59.3k (about 51.9k of them via `called_at`), none about 26k, not_codescout about 28.6k.
- **(e) One delivery per injection.** Claude Code records some hook injections twice, as `hook_success` stdout and as `hook_additional_context`. A `hook_success` is a twin only of a `hook_additional_context` in the same session copy, the same transcript file and the same hook event. The text must be equal, or equal to one element of a merged one, within 5 s. toolUseID is not part of the key. Measured: twins sit at most 1.7 s apart, and the nearest non-twin is at least 12 s away.

  An untwinned `hook_success` counts as a delivery (`hook_success_only`: compact reloads and UserPromptSubmit stdout, which did reach the model). Two `hook_additional_context` rows are never merged. `deliveries_json = '[]'` means delivered nothing, and never falls back to parsing `output_json`. Markers count only in their anchored opening form.
- **(f) Build counters live in `events_meta`,** so Task 13 reads them and never re-derives them. `build_events` refuses an existing DB, and every `ts` is normalized to ISO UTC.

### Amendment 6 — 2026-09-28, recorded during Task 13 (the observability map)

**Source:** Task 13's two reviews, and the controller's reading of `src/usage/db.rs`. Where this amendment and the body disagree, this amendment wins.

- **(a) The retained window is derived from data.** A1.2's parenthetical start "(2026-08-26)" was a spec-time estimate. The corpus's oldest surviving top-level entry is 2026-08-03T20:49:16Z. The retained window is `[earliest KEPT top-level entry, freeze instant)`, computed by `observability.finalize_bounds` after the freeze, and `coverage()` raises when data falls outside it. The decision window, `[T−7d, T)`, is unchanged in definition.
- **(b) `usage.db` keeps only 30 days.** `write_record` deletes every `tool_calls` row older than 30 days on each write (`src/usage/db.rs`), while transcripts are kept 3650 days. So a tool call older than 30 days at freeze time can never join to its usage row.

  Three consequences:
  - The go/no-go reads only the 7-day decision window, so it is unaffected.
  - The retained-window join coverage shrinks every day the real freeze waits, so Task 12 freezes as soon as the pipeline allows (ruling R72).
  - The INCONCLUSIVE path's prospective read spans `[T−7d, T_live+21d)`, which is 28 days or more. A single freeze at its end would already have lost that window's earliest usage rows. The prospective read therefore unions `usage.db` rows across every freeze by row `id`, and the observability map reports the per-freeze row ranges.
- **(c) A delivery is counted once, as a delivered ITEM.** For one `deliveries_json` engine record, the events DB holds a key row (`key` set) and a block-digest row (`key` NULL). Measured: all 35 NULL-key `usage_deliveries_json` rows are block digests, each paired with a key row, and the other two sources have no NULL-key rows. So delivery counts use key rows only, and every delivery cell names its unit. Counting rows had inflated the transfer delivery count by 7.4%, and the error grew with every post-Part-A delivery, since all of them go through `deliveries_json`.
- **(d) The freeze procedure (Task 12, ruling R78).** A session active during the freeze publishes wrong numbers without crashing. Measured: a usage snapshot taken before the transcripts were copied published 4 joinable calls as `none`. So:
  1. Freeze only while every kept session is idle; the operator coordinates this. Afterwards, check each kept session's last-row gap to the usage-snapshot instant, and re-freeze under a new `corpus_id` if any gap is shorter than the longest in-flight call.
  2. Use `archive.freeze`'s own order (transcripts, then the usage backup), and record the backup instant.
  3. Require `parse_errors_skipped == 0`, or report it.
  4. If `coverage()` raises on a window, stop for a ruling. Bounds are never hand-edited.
  5. For the prospective read, union usage rows across freezes by `id` (item (b)).

### Amendment 7 — 2026-09-28, recorded after a cross-session review, before Task 9

**Source:** a review requested by the operator and run by session `82cff72e-0245-48cb-ab07-45a1c3d0d388`. It used four Opus reviewers on Tasks 4–8 and 13 plus a design review of Tasks 9–12, on synthetic input only. The controller verified its claims and took count-only shape probes on the Task 8 scratch corpus (138 sessions, 119 kept). Rulings R103–R113 are in the SDD ledger. Where this amendment and the body disagree, this amendment wins. **Nothing here changes the go/no-go rule.**

- **(a) Who spoke: the operator population.**
  1. **Mid-turn messages count (R103).** A message the operator types while the agent runs is recorded as a `type:attachment` entry with `attachment.type == "queued_command"`. In the corpus, 2089 such attachments were split as follows:
     - 829 `commandMode: task-notification`;
     - 1089 `origin.kind: peer`;
     - 171 `origin.kind: human`. Only 25 of these equal a kept prompt, so about 146 operator messages were dropped. They skew toward corrections, which biased the result toward NO-GO.

     `operator_messages` now includes a `queued_command` attachment iff two conditions hold. Its `commandMode` must be `"prompt"`. And the attachment's own `origin.kind` must be `"human"`: the origin field on the attachment itself, not an entry-level one. An attachment with no origin never counts. In the corpus, every prompt-mode attachment carries an origin (1089 peer + 171 human = 1260). So a filter that read the wrong field would admit every peer message, and would turn the latent peer-leak into a live one. **No dedupe (R130; corrected the same day).** The first version of this item dropped a queued message whose text equalled a later kept prompt, on the premise that a queued message is echoed later as a prompt. Measured, that premise is false:
     - of 169 human queued attachments in kept sessions, 0 have an equal-text prompt within 10 minutes in either direction, and the 25 equal-text matches all lie more than 10 minutes away (20 of them more than an hour);
     - of 81 `promptSource: queued` user prompts, 79 have no equal queued attachment.

     The two are disjoint channels, and the matches were repeated short operator messages. Every qualifying attachment now counts.
  2. **Positive identification (R104).** An entry that carries an `origin` counts only if `origin.kind == "human"`. Tag exclusions remain the fallback for older entries without an origin. Measured: all 2592 kept prompts already had `origin.kind == "human"`, so today nothing changes.
  3. **Tool-rejection feedback is an operator correction (R105).** A `tool_result` carrying the harness marker `the user said:` becomes a candidate with source `operator_rejection` and corrector `operator`. The corpus holds 10 such blocks in 9 sessions.
  4. **Exclusions are recorded in the manifest and propagate to forks (R106).** "Sessions whose task was this measurement" is {`3c5b02df-b6ce-45f5-9d03-1194e38465c0` (the design and execution session), `82cff72e-0245-48cb-ab07-45a1c3d0d388` (review work inside the decision window, self-disclosed)}. A transcript that shares an excluded copy's uuid prefix is excluded too. 82cff72e shares 0 uuids with 3c5b02df.
  5. A kept session whose first and last `entrypoint` differ is counted and shown (R107). The corpus holds none.
- **(b) Lessons.**
  1. **Dating (R108).** A lesson existed at a decision point iff the AUTHOR date of the commit that first introduced the lesson's anchor line precedes the decision timestamp. The anchor line is the heading, the bold lead, or the `## R-N` line. A section-prose lesson's anchor is its section heading. A preamble lesson's anchor, for text above a file's first `##` (R114), is its first non-blank line that is not a heading. The search is limited to the lesson's own source path, and it matches the anchor with whitespace normalized. So rule text first drafted in a session log or bug file dates from its promotion into the lesson source, not from the draft. That closes the inflating direction. A lesson moved between sources dates from the move, a disclosed deflation. `experiments` is rebased after every ship, which restamps committer dates, so an origin timestamp is never mapped to a commit by committer date. The review showed that mapping errs in both directions, including inflation. A lesson deleted before the freeze is absent from the inventory, a disclosed deflation.
  2. **Assignment (R109).** The judge names the most specific applicable lesson. A catch-all rule (OP-1, "always verify") is credited only when no specific lesson applies. The gate REPORTS lesson-assignment agreement on the RTD cases. Every one of the 21 carries a `rule:` field naming the law violated; RTD-8, RTD-9 and RTD-15 record that no written law names their tell, so `uncovered` is the expected answer there. This is reported beside the gate's result and is NOT a pass condition, because the gate's pass conditions belong to the go/no-go rule, which is fixed. (Corrected the same day: this item first said the gate "requires" the served rule, which would have added a pass condition.) The readout reports catch-all-only misses separately.
- **(c) Windows and dates.**
  1. The decision window is effectively `[T−7d+1s, T+1s)`, where `T = floor(created_utc)`. `finalize_bounds` and `coverage()` raise unless decision end == retained end == `T+1s` and decision start == end − 7d (R111).
  2. **Commit timestamps are author dates.** The window is filtered in Python over the full history from the manifest SHA; `git --since` stops walking at the first older committer date. `Session-Id` is read from git's trailer parser, never from the first matching body line (R110).
  3. The go/no-go denominator is the body's own rule: a top-level transcript with at least one decision point in the window. A miss is placed by its origin timestamp. The observability map's any-turn session count is descriptive and labelled as such (R112).
- **(d) Disclosure.** Every probe behind this amendment counted shapes: leading tags, origin kinds, `commandMode`, and hash equality. None read a correction, a miss, or a rate.

### Amendment 8 — 2026-09-29, recorded before the gate first runs (Task 9b)

**Source:** Task 9a's implementation (commits 0fc330a0 and d608423b; fix rounds 1–2: 164e9d5d, cb431157, 0ee8cf34, c90e0d9b), its Opus reviews, and the controller's verification. Rulings R116–R126 and R133–R148 are in the SDD ledger. Where this amendment and the body disagree, this amendment wins. **Nothing here changes the go/no-go rule or the gate's thresholds and populations** (16/21; 3/4; 6/8; at most 5/52).

- **(a) The frozen prompt.** The file is `scripts/measure/judge_prompt.md`, sha256 `3137920a8d4645540b9cff7bced95671a9c6fbb282581cddc5eb5391d4de8c54`, as committed in `c90e0d9b`, rendered by `judge.render_prompt`. Any later edit is a new prompt and needs a new registration. Its detectability terms mirror `text_detectable`, as § Definitions requires (R134), written in general terms with no text from `docs/evals/rule-tell-detection.md`:
  - `in-trace`: the material shown is enough, with nothing looked up, to see that the decision point is wrong, in the way it is wrong;
  - `obtainable`: the material shows **at least one warning sign** pointing at the problem, and confirming it needs one bounded lookup;
  - `external`: the material shows no sign of the problem, **even if one lookup elsewhere would have revealed it**. It also covers a problem needing more than one lookup, or facts no bounded lookup supplies.

  **§ Definitions' `obtainable` is read on the signal axis (R147, confirmed by the operator 2026-09-29).** L120 defines `obtainable` by lookup cost ("one bounded lookup would have found it"), declares that detectability mirrors `text_detectable`, and L173 fixes the mapping `no → external`. `text_detectable: no` is exactly the case where there is no signal in the text and the falsifier is elsewhere, and RTD-6's falsifier is one committed file away. So the three sentences agree only if "would have found it" means a lookup the material gave the agent reason to make. This is the reading applied. Consequences:
  - an addressable miss (§ Definitions; the go/no-go quantity) requires a signal in the trace, which matches what a trace-reading System 1 can fire on;
  - relative to the lookup-cost reading, it counts fewer addressable misses, so it errs toward NO-GO.

  The prompt's first draft put a `partial`-style warning sign under `in-trace`, and its second allowed `obtainable` with no sign at all. Both were corrected before any gate output existed.
- **(b) The channel (R122, amended by R133 and R138).**
  - **Subscription and version:** Codex on the ChatGPT subscription only, with `forced_login_method = "chatgpt"` and `--strict-config`. The model and effort are imported from `docs/evals/data/2026-09-24-rule-tell/stage2/generate_synthetic.py` (`gpt-6-astra`/`medium`). `codex-cli 0.154.0` is pinned, and a live run refuses on any other version.
  - **Invocation:** the rendered prompt is the entire user message, delivered on stdin with no wrapper instruction.
  - **Jail:** codex runs inside a `bwrap` jail with tmpfs over the home directory, `/tmp` and `/run/user/<uid>`. Only the fresh `CODEX_HOME`, its auth target and an empty workdir are bound back.
  - **Precondition:** before the first vote, a local `codex sandbox` check inside the same jail must fail to see the repo the gate runs on (the realpath of its `--repo`), the RTD document, every `~/.claude*` transcript root and the session scratch root, and must see the workdir. Otherwise the run refuses to start.
  - Why: measured with no model call, `--sandbox read-only` grants read access to `:root`, so the RTD answer key was readable. The same exposure in the rule-tell campaign's committed call sites is bug `4b7cdb0cb35d12a7`.
  - **Votes:** 3 votes per item, at most 3 concurrent calls, each with its own `--json` log. The logs live outside the repo and are never committed.
  - **Failed votes:** a vote whose log shows a tool or exec event is a failed `tool_call` vote. An empty or unparseable `--json` stream is a failed `call_failed` vote. **Retry policy, pre-registered (R137):** a `call_failed`, `unparseable` or `tool_call` vote is re-issued up to 2 more times. Every attempt is recorded, and the first attempt that is none of these counts. A vote with 3 failed attempts stays failed and is recorded, never dropped.
  - A live run also refuses on a non-empty leak scan, on `votes ≠ 3`, and on `--any-population` (R135, R138).
- **(c) Inputs (R116, R117, R120, R124, R142).**
  - **Leakage:**
    - an audit context turn must be strictly earlier than the decision;
    - correction-mode context must be strictly earlier than the origin;
    - the correction text is a separate field, never pre-decision evidence for the quote check;
    - `build_input` raises on any violation.
  - **Session context:** the preceding top-level turns, up to 12 turns and 20,000 chars, dropping the oldest first. Correction mode offers up to 5 previous `assistant_text` turns as origin candidates.
  - **Gate items are built from documents:**
    - the positive and up to 1,500 chars **on each side** of it, from the **pre-correction blob**;
    - the correction is the case's `negative`;
    - controls: the passage and its surrounding text in its source.
  - **Lesson index:** each lesson appears as its id, source, `dated|undated`, anchor line and first sentence, at most 300 chars, never its full text.
- **(d) Lesson dating (R118, R125, R140, R143).**
  - **Date:** a lesson's date is the AUTHOR date of the first commit that introduced its anchor line. It is found by `git log --reverse --topo-order --format=%H%x09%aI -G <regex> -- <the lesson's own source>`, with the anchor whitespace-normalized. A repeated heading dates at the commit where the source first held that many copies.
  - **Freeze points:**
    - correction items freeze lessons at the positive's own commit when the correction was appended, and at the pre-correction sha when it was in place;
    - controls freeze their REPO lessons at the passage's latest introducing commit ("as they stood at the origin commit", R140/R144). The operator's global lessons are unversioned, so there is no "as they stood". They are read at one fixed reference sha (the controls' tree 27eded91). Because the global file carries BEGIN/END markers, that set is byte-identical at any sha that has `docs/trackers/operator-rules.md`.
    - 8 controls originate before `docs/trackers/operator-rules.md` existed (first added 21e60b81). For them the OP-N rules appear in neither set, and the gate report says so.
  - **Undated lessons:** the operator's global lessons are always listed and marked `undated`. A repo lesson whose date cannot be derived is listed `undated` and counted as `lesson_undatable`, which was 0 at every gate freeze sha in the dry run.
- **(e) Verdict and scoring (R119, R136, R139, R142).**
  - **Missing fields:** a missing field parses to None, `unknown` or `{}` and is flagged, never defaulted. A `null` boolean is an abstention. For `origin_uuid`, `null` means "the correction targets the decision point shown", and `"unknown"` is its abstention.
  - **Quote failure:** an `in-trace` answer whose quote is not verbatim in the pre-decision evidence has its detectability and lessons discarded; the rest of the answer stands.
  - **Fires:** a control "fires", and a `yes` case is "flagged", when the majority `is_mistake` is True. The gate report counts how many fires and flags carry at least one `quote_not_verbatim` vote.
  - **Lessons:** `lessons` names the most specific applicable lesson. `lesson_outcomes` covers every lesson the judge finds applicable (A1.4). Lesson-assignment agreement is REPORTED beside the gate result and is not a pass condition (Amendment 7 (b)2).
- **(f) Disclosures the gate report carries.**
  - Gate document items see text after the decision point, from the same pre-correction blob; session items never do. So the gate's agreement transfers to session items only for the claim-shaped class, and only for decisions judged without later context.
  - The gate's cost, from the fix round's dry run: 81 items, 5,549,022 prompt chars (about 1.39M estimated tokens per vote round at chars/4), dominated by the lesson index.
  - The 8 pre-operator-rules controls (item (d)), with their OP-N count.
- **(g) Carried to Task 10.** Before any session item is built, a pre-selection rule for turns that share the decision's timestamp or `message_id` is ruled and recorded. R116's guard stays a raise. `_majority` must also separate `origin_uuid` `"unknown"` abstentions from `null` ("the decision point shown") before session items are scored; the gate has no origin candidates, so it is unaffected.
- **(h) Pre-registered before any gate output (R148).** Nothing here changes the gate's pass conditions, populations, labels or thresholds. The decisive score remains detectability agreement on all 21 correction cases, needing at least 16.
  - **A structural disagreement, measured.** RTD-2 is labelled `text_detectable: partial` (so `obtainable`). In its pre-correction blob (`9822b98c^`), the two window end timestamps that falsify its containment claim sit 433 and 385 chars BEFORE the positive, inside the gate's 1,500-char window. A judge applying `in-trace` correctly will therefore answer `in-trace`. The cause is the widening that § Definitions names ("widened from the text to the whole trace"): `text_detectable` assessed the turn's own output, while the gate shows the surrounding document. RTD-2 is scored as it stands. The committed gate record carries a controller note, computed from the run's JSON result, that states the agreement count both with and without RTD-2, descriptively.
  - **Predicted from reading, not measured,** by the review that cleared the prompt:
    - RTD-13 is the next most likely disagreement, close to a coin flip on `obtainable` against `external`, followed by RTD-1;
    - RTD-4, RTD-11, RTD-12 and RTD-19 more weakly;
    - all 4 `text_detectable: no` cases (RTD-6, RTD-13, RTD-14, RTD-21) are most plausibly `external`.

    These are recorded so that no disagreement can be explained after the fact without being checked against a prediction made before it.

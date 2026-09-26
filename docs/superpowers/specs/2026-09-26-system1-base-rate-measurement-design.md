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

**Quantity:** addressable operator-caught misses per session, on codescout.

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

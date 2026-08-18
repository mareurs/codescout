---
id: '35de33286cd34f87'
kind: tracker
status: active
title: Fable Tuning — Findings (FND-N)
tags:
- fable
- prompt-tuning
- model-behavior
entry_prefix: FND
---

## What this tracks

How the Claude **Fable** model (`claude-fable-5`) behaved when it "ran great" (June 2026 GA launch / April Mythos Preview) versus now — distilled into numbered findings (FND-N in params). Feeds `docs/trackers/fable-tuning-tasks.md` (turns findings into eval-gated work) and the prompt-TDD apparatus in `docs/trackers/prompt-hamsa-audit-log.md` (id `59ebeebb6ed05c89`). Full evidence + sources live in `docs/trackers/fable-tuning-research.md`.

## Timeline (FND-1..3)

- **"The 1st Fable that ran great"** ≈ the **Jun 9 2026 GA launch** of `claude-fable-5`, with roots in the April invite-only **Mythos Preview** (Project Glasswing). Lineage: `claude-mythos-preview` → `claude-mythos-5` / `claude-fable-5`; a "v1 preview" API/fallback surface was discontinued.
- A **19-day export-control shutdown (Jun 12 – Jul 1)** revoked then restored access — much of the "nerfed" sentiment dates from the post-Jul-1 redeploy.

## How it ran great (FND-4..6)

- **Exploration / planning depth** (the priority dimension): navigating ambiguity, bug-finding recall above Opus 4.8 including repo-history search, autonomously driving investigations and fixing unrequested-but-related issues.
- **Coding quality**: praised API / test / doc quality; a day's output felt like several days'.
- **Restraint**: self-cleanup — unwound its own hacks into supported features once told the library was in scope.

## What changed / how it works now (FND-7..8, 13)

- **Mechanism (best-sourced):** safety classifiers (cyber / bio / reasoning-extraction) intercept security-adjacent and debugging requests → **silent Opus-4.8 fallback**. BridgeMind measured a ~70% TypeScript-debugging score drop from 9/12 tasks rerouted — "not degraded reasoning … the classifier intercepting requests." Core grievance is the lack of transparency about *when* a fallback occurred.
- **Model defaults that read as regressions:** overplanning / over-exploration at high effort, unrequested tidying, verbosity, unrequested actions, early stopping — Anthropic frames these as prompt-adjustment issues, not degradation.
- **Unverified:** the Wikipedia `TOO_DUMB_TO_NEED_FABLE` covert-throttling claim (FND-13) — flagged possibly-fictional; do not rely on it.

## The lever (FND-9..10)

- Anthropic's counterintuitive headline: prompts / skills tuned for prior models are **too prescriptive for Fable and reduce output quality** → recover quality by **removing** scaffolding, not adding more.
- codescout is already partway there: this harness embeds two Fable snippets verbatim (anti-overplanning "act when you have enough info"; grounded-progress-claims).

## Local-trace evidence (FND-11..12)

- **130 genuine `model=fable` sessions**, Jun 13 – Jul 6, in `~/.claude-sdd` — **Opus-4.8-dominant with Fable minority** (multi-model usage), plus ~40 `prompt-test` one-offs (active A/B testing).
- Silent-fallback signature **not found** in 2 sampled sessions (0 refusal / 0 fallback blocks; early Jun-15 & recent Jul-5) — hypothesis unconfirmed in local data; needs a Langfuse served-by / stop_reason check (see tasks FT-8/FT-9).

**FND-14 (2026-07-07, definitive):** full-corpus scan of served models (JSONL `message.model` = what the API actually returned) across all 3 profiles: 81 fable-containing sessions, ~145k assistant messages, **0 refusal stop_reasons, 0 per-call Opus interleaves**. All 16 mixed-model sessions mix in large contiguous blocks (manual `/model` switches / resumes), never the lone-Opus-response-mid-Fable-run reroute shape. Silent fallback (FND-7) **refuted for local usage** — FND-12 resolved, FT-9 done, FT-12 dropped. Method, numbers, and the honest-model-field caveat: `fable-tuning-research.md` § Local-trace forensics (2026-07-07 entry).
## Self-reflection lane — fable vs opus tool-use (FND-17)

**Method.** Bucket recent Langfuse traces by `served_model` (now logged per call — a trace's served_model == the model that drove the agent for that call). Scanned 2000 traces (2026-07-10): opus 1070 / sonnet 845 / **fable 84** (2 sessions) / 1 none.

**Engagement (stop_reason).** fable tool_use 83.3% / end_turn 15.5%; opus 82.1% / 9.6% (+7.8% stop_sequence = this session's harness); sonnet 90.5% / 9.4%. Fable ends turns modestly more often — mild, n=13 end_turns — **not** the externally-reported collapse. Call density comparable (fable 1.06, opus 1.11, sonnet 1.29 calls/trace).

**Tool discipline (within-subject).** Session `34c9183a` (mirela-backend-kotlin, ~/.claude) ran on fable 09:04–09:58 (62 calls) then switched models. Fable-window tools: read_file 11, run_command 10, artifact 9, symbols 8, read_markdown 6, edit_markdown 5, + task-tracking + 2 Agent dispatches (one a well-formed *opus* review brief). **0 native Bash, 1 native Read** (1.6% native). Rest-of-session (324 calls, opus/sonnet): 2.8% native. No IL-violation surge on fable; if anything the native leak was *lower* — though task-mix confounds it (the fable window skewed toward doc/recon work, which structurally uses codescout tools).

**Caveats.** Cohort thin (89 fable calls, 1 deep-audited session). This session's own fable window (`9175beae`, ~/.claude-sdd) was not deep-audited: `cc.py` is pinned to `~/.claude` (FT-10) and mis-encodes dotted worktree dir names (bug filed in llm-proxy `docs/issues/`). Verdict: **parity, not degradation** — corroborates FND-14 (fallback refuted) and FND-16 (anti-tidying ceiling).
## Findings — per-entry anchors

> **Added 2026-08-18.** No `FND-N` heading existed anywhere in this body, so `link_scan` bound none of the eighteen tokens and every citation of them — including the cross-file ones from `prompt-hamsa-audit-log` — resolved to nothing. The grouped sections above carry the narrative for FND-1..13 and FND-17; **FND-14, 15, 16 and 18 had no body section at all**, so their claims existed only in the machine-local catalog. Each anchor below carries the entry's claim, so the record travels with the repo.
>
> Mechanism: `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`.

### FND-1 — "The 1st Fable that ran great" is the 2026-06-09 GA launch window

**Dimension:** timeline · **Source:** web+official · **Confidence:** high · **Status:** confirmed

With roots in the April Mythos Preview. Narrative: § *Timeline (FND-1..3)*.

### FND-2 — Lineage runs Mythos Preview → `claude-mythos-5` / `claude-fable-5` GA

**Dimension:** timeline · **Source:** official · **Confidence:** high · **Status:** confirmed

`claude-mythos-preview` (April, invite-only, Project Glasswing) → `claude-mythos-5` / `claude-fable-5` at GA on 2026-06-09; a v1-preview API surface was discontinued. Narrative: § *Timeline (FND-1..3)*.

### FND-3 — A 19-day export-control shutdown colours the post-redeploy "nerfed" sentiment

**Dimension:** timeline · **Source:** press · **Confidence:** high · **Status:** confirmed

Access was revoked then restored, 2026-06-12 to 2026-07-01. Narrative: § *Timeline (FND-1..3)*.

### FND-4 — Early Fable excelled at deep systematic exploration

**Dimension:** exploration · **Source:** official+willison · **Confidence:** high · **Status:** confirmed

Navigating ambiguity, bug-finding recall above Opus 4.8 including repo-history search, and autonomously driving investigations. Narrative: § *How it ran great (FND-4..6)*.

### FND-5 — Early Fable was praised for API, test and doc quality

**Dimension:** coding · **Source:** willison · **Confidence:** high · **Status:** confirmed

A day's output felt like several days'. Narrative: § *How it ran great (FND-4..6)*.

### FND-6 — Early Fable showed restraint and unwound its own hacks

**Dimension:** behavior · **Source:** willison · **Confidence:** medium · **Status:** confirmed

It converted its own workarounds into supported features once told the library was in scope. Narrative: § *How it ran great (FND-4..6)*.

### FND-7 — The best-sourced "regression" cause is classifier interception, not degraded reasoning

**Dimension:** mechanism · **Source:** press+official · **Confidence:** high · **Status:** **plausible**

Safety classifiers intercept security-adjacent and debugging requests, producing a silent Opus-4.8 fallback; BridgeMind measured a ~70% debug-score drop from 9 of 12 tasks being rerouted rather than answered worse. **Note (2026-07-07): does NOT manifest in our local traffic — see FND-14.** Remains plausible as an ecosystem report only. Narrative: § *What changed / how it works now (FND-7..8, 13)*.

### FND-8 — Documented Fable defaults read as regressions; Anthropic frames them as prompt-adjustment

**Dimension:** behavior · **Source:** migration-guide · **Confidence:** high · **Status:** confirmed

Overplanning and over-exploration at high effort, unrequested tidying, verbosity, unrequested actions, early stopping. Narrative: § *What changed / how it works now (FND-7..8, 13)*.

### FND-9 — Prompts written for prior models are TOO PRESCRIPTIVE for Fable

**Dimension:** meta · **Source:** migration-guide · **Confidence:** high · **Status:** confirmed

Anthropic's headline: prescriptive scaffolding reduces quality, and the recovery is **removing** scaffolding rather than adding it. This is the lever the whole stream turns on. Narrative: § *The lever (FND-9..10)*.

### FND-10 — codescout already embeds two Fable snippets verbatim

**Dimension:** meta · **Source:** local-trace · **Confidence:** high · **Status:** confirmed

Anti-overplanning and grounded-progress-claims. Narrative: § *The lever (FND-9..10)*.

### FND-11 — 130 genuine `model=fable` sessions, Opus-4.8-dominant with a Fable minority

**Dimension:** local-trace · **Source:** local-trace · **Confidence:** high · **Status:** confirmed

2026-06-13 to 2026-07-06 in `~/.claude-sdd`, multi-model, plus ~40 prompt-test one-offs. Narrative: § *Local-trace evidence (FND-11..12)*.

### FND-12 — The silent-fallback signature was absent in the two sampled local sessions

**Dimension:** local-trace · **Source:** local-trace · **Confidence:** medium · **Status:** confirmed

0 refusal and 0 fallback blocks. **Note (2026-07-07): resolved by FND-14's full-corpus scan.** The Langfuse lane was impossible — `llm-proxy` logs the request-side model only, so the served model had to come from JSONL `message.model` instead. Narrative: § *Local-trace evidence (FND-11..12)*.

### FND-13 — The Wikipedia covert-throttling claim is unverified and possibly fictional

**Dimension:** mechanism · **Source:** web · **Confidence:** low · **Status:** **refuted**

`TOO_DUMB_TO_NEED_FABLE`. Do not rely on it. Narrative: § *What changed / how it works now (FND-7..8, 13)*.

### FND-14 — Full-corpus served-model scan REFUTES silent fallback for local usage

**Dimension:** local-trace · **Source:** local-trace · **Confidence:** high · **Status:** confirmed

81 fable sessions across 3 profiles, 2026-06-09 to 2026-07-07, ~145k assistant messages: **0 refusal `stop_reason`s and 0 per-call Opus interleaves.** Mixing appears only as large manual-switch blocks. This is what settles FND-7 and FND-12 for local traffic.

### FND-15 — All delivered prompt surfaces are clean of both Fable foot-guns

**Dimension:** meta · **Source:** local-audit · **Confidence:** high · **Status:** confirmed

Run as `fable-tuning-tasks:FT-7` / hamsa A-13. Neither reasoning-extraction nor token-countdown appears in any delivered surface; the only token language shipped is tool-OUTPUT sizing (progressive-disclosure), which is not a context countdown. Complements FND-10.

### FND-16 — The tidying-temptation A/B hit the ceiling: 10/10 surgical with no guidance

**Dimension:** local-trace · **Source:** local-eval · **Confidence:** high · **Status:** confirmed

Run as `fable-tuning-tasks:FT-2` / hamsa A-14, fable, runs:10. FND-8's "unrequested tidying" default does not manifest locally on surgical-fix tasks, so the snippet was **not shipped** — the pre-registered ceiling branch fired. Weakens the prior for the FT-3..FT-6 snippet additions; each still needs its own base-arm-first eval.

### FND-17 — Self-reflection lane: no fable degradation in local tool use

**Dimension:** local-trace · **Source:** local-trace · **Confidence:** medium · **Status:** confirmed

Traces bucketed by `served_model`, n=2000. Engagement comparable — fable 83% `tool_use` / 15.5% `end_turn` vs opus 82%/9.6% and sonnet 90%/9.4%; call density fable 1.06 vs opus 1.11 / sonnet 1.29. In a clean 62-call fable window (session `34c9183a`, within-subject against the same session's opus and sonnet portions) codescout tool discipline was fully intact: 0 native `Bash`, 1 glue `Read` (1.6% native vs the rest's 2.8%), idiomatic `symbols` / `read_file` / `artifact` / `edit_markdown`. **Cohort is thin** — 2 fable sessions, 89 calls. Corroborates FND-14 and FND-16. Narrative: § *Self-reflection lane — fable vs opus tool-use (FND-17)*.

### FND-18 — Fable does not decay a latent directive over 5 turns of conversational distance

**Dimension:** local-trace · **Source:** local-eval · **Confidence:** medium · **Status:** confirmed

Run as `fable-tuning-tasks:FT-6`, fable, runs:2. Re-pinned the guidance-decay scenario `F-nr-far` to fable: a formatting rule stated once in turn 1, buried under 5 non-code filler turns, with a distant code probe and a mechanical contains-check on `// reviewed` at `pass_threshold` 1.0. **2/2 HELD** — so the FT-6 anti-decay / anti-early-stopping snippet has nothing to fix and was **not shipped**, per the pre-registered rule. Extends FND-16's single-turn ceiling along the turn axis. Scenario kept at `prompt-engineering/scenarios/fable-latent-decay/far`. **UNTESTED facet:** autonomous multi-step task QUALITY (the `anthropic_mcp` `max_turns` adapter) — the BridgeMind debug shape. Scripted-history decay is not autonomous-loop degradation.
## History

### 2026-07-07 — tracker created
Seeded 13 findings (FND-1..13) from the Fable research session: web/forum research + local trace forensics + `claude-api` migration-guide grounding.


### 2026-07-07 — session passover
Tracker set created & verified (entry_filter queries return; `cites` edges wired). No code/prompt changes this session. Resume via the index (`ca8c26fecbbc4f37`) § Session passover.

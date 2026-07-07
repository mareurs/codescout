---
id: '35de33286cd34f87'
kind: tracker
status: active
title: Fable Tuning — Findings (FND-N)
owners: []
tags:
- fable
- prompt-tuning
- model-behavior
topic: null
time_scope: null
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
- Silent-fallback signature **not found** in 2 sampled sessions (0 refusal / 0 fallback blocks; early Jun-15 & recent Jul-5) — hypothesis unconfirmed in local data; needs a Langfuse served-by / stop_reason check (see tasks T-8/T-9).

**FND-14 (2026-07-07, definitive):** full-corpus scan of served models (JSONL `message.model` = what the API actually returned) across all 3 profiles: 81 fable-containing sessions, ~145k assistant messages, **0 refusal stop_reasons, 0 per-call Opus interleaves**. All 16 mixed-model sessions mix in large contiguous blocks (manual `/model` switches / resumes), never the lone-Opus-response-mid-Fable-run reroute shape. Silent fallback (FND-7) **refuted for local usage** — FND-12 resolved, T-9 done, T-12 dropped. Method, numbers, and the honest-model-field caveat: `fable-tuning-research.md` § Local-trace forensics (2026-07-07 entry).
## History

### 2026-07-07 — tracker created
Seeded 13 findings (FND-1..13) from the Fable research session: web/forum research + local trace forensics + `claude-api` migration-guide grounding.


### 2026-07-07 — session passover
Tracker set created & verified (entry_filter queries return; `cites` edges wired). No code/prompt changes this session. Resume via the index (`ca8c26fecbbc4f37`) § Session passover.

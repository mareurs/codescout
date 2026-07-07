---
id: ad1af8262fdce357
kind: tracker
status: active
title: Fable Tuning — Tasks (T-N)
owners: []
tags:
- fable
- prompt-tuning
- model-behavior
topic: null
time_scope: null
---

## Why this initiative exists

Recover "early-Fable" quality in codescout by tuning prompts / tools / trackers. Rooted in `docs/trackers/fable-tuning-findings.md` (esp. FND-7 silent fallback, FND-9 over-prescription) and evidenced in `docs/trackers/fable-tuning-research.md`. **Every prompt-surface change is eval-gated** — run it as a pre-registered A/B arm through `docs/trackers/prompt-hamsa-audit-log.md` (id `59ebeebb6ed05c89`) against the "original Fable captures" baseline before shipping.

## Surfaces

- **prompt** — `src/prompts/source.md` (`server_instructions` + `onboarding` slices), `CLAUDE.md`, `builders.rs`. Keep all three tool-name-consistent (gated by `prompt_surfaces_reference_only_real_tools`).
- **tool / infra** — `lf.py` / `cc.py` trace tooling, Langfuse keys.
- **methodology** — the subtract-and-measure A/B protocol.
- **tracker** — supporting trackers (e.g. a fallback audit).

## Guiding principle

De-prescribe and delete, don't stack instructions (FND-9). Prefer removing scaffolding plus a few targeted communication-style / boundary snippets, each A/B-verified against the Fable baseline.

## Task detail

- **T-1 (CLAUDE.md slim)** is the highest-leverage prompt lever: audit entry A-2 already flags the 42 KB per-session file; Fable's literalism + "too prescriptive reduces quality" turns hygiene into a quality gain.
- **T-8 (Langfuse keys)** unblocks the *definitive* silent-fallback test (T-9) — `lf.py` reports the actually-served model + stop_reason per call.
- **T-2..T-6** are candidate snippet additions; each is its own eval arm — do not batch-ship.

## History

### 2026-07-07 — tracker created
Seeded 12 tasks (T-1..12) from the research report's recommendations (§5).


### 2026-07-07 — session passover
All 12 tasks (T-1..12) `open`; none started. No code/prompt changes this session. Resume via the index (`ca8c26fecbbc4f37`) § Session passover; do-next: T-1, T-8.

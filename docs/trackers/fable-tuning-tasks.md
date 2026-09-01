---
id: ad1af8262fdce357
kind: tracker
status: active
title: Fable Tuning — Tasks (FT-N)
tags:
- fable
- prompt-tuning
- model-behavior
entry_high_water_FT: 12
entry_prefix: FT
expects_augmentation: docs/augmentations/docs-trackers-fable-tuning-tasks.yaml
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

- **FT-1 (CLAUDE.md slim)** is the highest-leverage prompt lever: audit entry A-2 already flags the 42 KB per-session file; Fable's literalism + "too prescriptive reduces quality" turns hygiene into a quality gain.
- **FT-8 (Langfuse keys)** unblocks the *definitive* silent-fallback test (FT-9) — `lf.py` reports the actually-served model + stop_reason per call.
- **FT-2..FT-6** are candidate snippet additions; each is its own eval arm — do not batch-ship.

## Tasks — per-entry anchors

> **Added 2026-08-18.** No entry heading existed anywhere in this body, so `link_scan` bound none of the twelve tokens. That mattered here more than most: `prompt-hamsa-audit-log` cites `FT-1`, `FT-2`, `FT-7` and `FT-11` — already disambiguated *in prose* as "fable-tuning FT-7" — and every one of them resolved to nothing.
>
> **Renamed `T` → `FT` on 2026-08-18, and the namespace is now exclusive.** This ledger previously declared `entry_prefix: T`, which `docs/trackers/tool-usage-patterns.md` also owns — spelling its first thirteen entries zero-padded (`T-001`…`T-013`) and its later ones unpadded (`T-14`…`T-24`). The two token spaces were disjoint only *by accident of that padding*; nothing recorded or enforced it, and `link_scan`'s `prefix_conflicts` check reported the overlap the moment that check existed. `tool-usage-patterns` has the stronger claim — `CLAUDE.md` hard-codes `id_prefix="T"` for it, it holds 24 entries to this ledger's 12, and it is cited far more widely — so this ledger gave up `T`. `FT` has a single definer and no ceiling: extend it freely.
>
> Mechanism: `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`. Prefix collision and this rename: `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`.

### FT-1 — Slim the 42 KB per-session CLAUDE.md (dead tool names, 4× rule duplication, relocate forensics)

**Priority:** high · **Surface:** prompt · **Eval gate:** yes · **Status:** done

Closed 2026-07-07 as **born zombie**: the cut had already shipped 2026-06-21 (`b603d86f`, on `master`; 42,175 B → 12,535 B, −70%) under hamsa audit A-2, before this task was written. What remained was A-2's pending measurement, run observationally over 2.5 weeks of real post-cut sessions — (a) 0 dead-name calls in 4,743 post-cut codescout `tool_use` calls; (b) relocated rules hold (20 `docs/issues/` captures, 15/15 conventional commits, trackers maintained through the librarian); (c) 12,535 B on disk. The eval gate was satisfied by real-session measurement, which is stronger than a synthetic A/B for an already-shipped cut.

### FT-2 — A/B an anti-tidying / anti-over-engineering snippet on the prompt surface

**Priority:** high · **Surface:** prompt · **Eval gate:** yes · **Status:** done (NOT-INDICATED)

Closed via pre-registered A/B (hamsa A-14). Base arm — fable, no snippet, runs:10, on a tidying-temptation fixture (an off-by-one fix in a file planted with 4 unused imports, `== None`, a TODO and verbose style; mechanical trace-diff check, mutation-tested) — came in **10/10 surgical**. FND-8's tidying default does not manifest locally, so the pre-registered ceiling branch fired (FND-9: don't stack unneeded instructions) and the snippet does **not** ship. Arm B skipped per protocol. Suite kept at `prompt-engineering/scenarios/fable-tidying/`; re-open on a field sighting of shipped unrequested tidying. See FND-16. Base-arm-first was promoted to prompt-hamsa Heuristic 12 (`claude-plugins:5202cca`).

### FT-3 — A/B a boundary block ("report findings and stop until asked")

**Priority:** medium · **Surface:** prompt · **Eval gate:** yes · **Status:** open

Prior weakened by FND-16 (FT-2's base arm at ceiling). If attempted, run the base arm FIRST per protocol P-3 and expect a ceiling — a documented-default claim is not local evidence.

### FT-4 — A/B verifier-subagent guidance (fresh-context verifiers beat self-critique)

**Priority:** medium · **Surface:** prompt · **Eval gate:** yes · **Status:** open

Prior weakened by FND-16. Base arm first, per P-3.

### FT-5 — A/B memory-file guidance (leverage codescout memory + trackers)

**Priority:** medium · **Surface:** prompt · **Eval gate:** yes · **Status:** open

Prior weakened by FND-16. Base arm first, per P-3.

### FT-6 — A/B async-subagent + anti-early-stopping guidance for autonomous runs

**Priority:** medium · **Surface:** prompt · **Eval gate:** yes · **Status:** open

Prior weakened by FND-16, but multi-turn is the likeliest ceiling-escape — **and the harness already exists** (2026-07-10 scout). prompt-tdd's `claude_code` adapter `_run_history_turns` replays scripted turns via `--resume` in ONE persisted session (schema: `input.history:[{message:…}]` plus a final message, assertions targeting the last turn), live in the 14-arm guidance-decay suite; there is also an `anthropic_mcp` adapter with autonomous `max_turns=20`. So FT-6 is scenario authoring plus a fable pin in `prompt_tdd.yaml`, NOT harness engineering — which supersedes A-7's "multi-turn harness is THE blocker". One arm has since run: FND-18 found 2/2 HELD at 5-turn distance, so the anti-decay snippet has nothing to fix. The untested facet is autonomous multi-step task *quality*.

### FT-7 — Audit prompt surfaces for the reasoning-extraction and token-countdown foot-guns

**Priority:** high · **Surface:** prompt · **Eval gate:** no · **Status:** done

Ran as hamsa A-13. **CLEAN on both axes across ALL delivered surfaces** — 3 slices, 9 guides, `builders.rs`, templates, the generated system prompt, session memories, CLAUDE.md, companion hook text, Rust hint strings. Every grep hit was benign (research memories describing methodology, a retry-budget counter, code comments). Boundary case pinned: progressive-disclosure's token numbers are tool-OUTPUT sizing, not a context countdown. Nothing to ship; the negative is recorded to prevent re-audits. See FND-15.

### FT-8 — Set Langfuse keys so `lf.py` reports served-by model and `stop_reason` per call

**Priority:** high · **Surface:** infra · **Eval gate:** no · **Status:** done

Root cause was `lf.py`'s `load_env()` stopping at the first existing `.env` — codescout's own `.env` shadowed `~/agents/llm-proxy/.env`. Fixed to merge all candidates via `setdefault`; verified `lf.py recent` works from the codescout cwd. See `skill-frictions:SKF-1` (claude-traces).

### FT-9 — Run served-by analysis on recent Fable debugging sessions to confirm or refute silent Opus fallback

**Priority:** medium · **Surface:** tool · **Eval gate:** no · **Status:** done

Done via JSONL rather than Langfuse: `llm-proxy` logged only the REQUEST model at the time, and JSONL `message.model` is the served model. Full-corpus scan (81 fable sessions, 3 profiles) found 0 refusals and 0 per-call Opus interleaves — silent fallback **REFUTED locally** → FND-14. Follow-up shipped the same day: `llm-proxy` now logs `served_model` (`llm-proxy:678778c`, deployed and verified live), operationalized as a one-command `lf.py mismatches` check (`llm-proxy:b72d0f6`, first run 0 mismatches / 300 traces), with a standing watch added 2026-07-10 (`llm-proxy:481b31e`, systemd `--user` timer). Addresses FND-7 and FND-12.

### FT-10 — `cc.py`: add `--config-dir` / `CLAUDE_CONFIG_DIR` support so it can read `~/.claude-sdd` Fable sessions

**Priority:** low · **Surface:** tool · **Eval gate:** no · **Status:** open

Root cause plus a second defect filed 2026-07-10 as `llm-proxy` `docs/issues/2026-07-10-ccpy-config-dir-hardcoded-and-path-encoding.md`: `cc.py` hardcodes `CLAUDE_DIR=~/.claude` (ignoring `CLAUDE_CONFIG_DIR`), **and** `path_to_project_key` is lossy for dotted directories (`.worktrees`), silently resolving `--project` to a non-projects dir. Surfaced by FND-17's self-reflection lane, which could not reach that session's `~/.claude-sdd` fable window.

### FT-11 — Codify the subtract-and-measure protocol for every prompt change

**Priority:** high · **Surface:** methodology · **Eval gate:** no · **Status:** done

Codified as **P-1..P-8** plus the P-8a baseline note in the `prompt-hamsa-audit-log` body (§ Protocol, placed right after § Index so it is read at audit-registration time), distilled from A-1..A-14: base-arm-first, pre-registered decision rules, mechanical mutation-tested checks, run-scoped model pin, inverted burden for deletions, outcome-filling discipline. Developer entry point is `src/prompts/README.md` § *Measure before shipping*. Worked example A-14; template `prompt-engineering/scenarios/fable-tidying/`. Baseline clarified: "original Fable captures" is a stimulus corpus, not an executable fixture. P-3 promoted to prompt-hamsa Heuristic 12 (`claude-plugins:5202cca`).

### FT-12 — Consider a fallback-audit tracker flagging sessions where Fable turns rerouted to Opus

**Priority:** low · **Surface:** tracker · **Eval gate:** no · **Status:** dropped (moot)

Dropped 2026-07-07: FND-14 refuted local silent fallback, so there is no signal to audit. Revive only on new evidence — the standing detector is now the daily `lf.py mismatches` watch (`llm-proxy:481b31e`) over the `served_model` capture (`llm-proxy:678778c`).
## History

### 2026-07-07 — tracker created
Seeded 12 tasks (FT-1..12) from the research report's recommendations (§5).


### 2026-07-07 — session passover
All 12 tasks (FT-1..12) `open`; none started. No code/prompt changes this session. Resume via the index (`ca8c26fecbbc4f37`) § Session passover; do-next: FT-1, FT-8.

---
id: ab2170158c7d264e
kind: tracker
status: active
title: Fable Tuning — Research (sources & evidence)
owners: []
tags:
- fable
- prompt-tuning
- research
topic: null
time_scope: null
---

## What this is

The evidence base behind `docs/trackers/fable-tuning-findings.md` (id `35de33286cd34f87`) and `docs/trackers/fable-tuning-tasks.md` (id `ad1af8262fdce357`): web/forum research + local trace forensics + `claude-api` migration-guide grounding, on how the Claude **Fable** model (`claude-fable-5`) behaved early vs now. Reflective tracker — sources are human-curated prose, not mechanical state.

## Question

How did Fable behave when it "ran great" (Jun 9 2026 GA / April Mythos Preview) vs now — on coding quality, behavior/style, reliability, and (the priority) exploration/planning depth — and what recovers early quality?

## Web / forum synthesis

- **Timeline:** leak (Mar 26–Apr 7) → Mythos Preview (`claude-mythos-preview`, Apr 7–Jun 9, invite-only via Project Glasswing) → GA `claude-fable-5` + Glasswing-gated `claude-mythos-5` (Jun 9) → 19-day export-control shutdown (Jun 12–Jul 1) → redeploy.
- **Ran great:** Simon Willison (Jun 9, firsthand, no early access) — API/test/doc quality, a day's work in an afternoon, self-cleanup of its own hacks; independently surfaced a non-obvious upgrade path and fixed four unrequested related bugs. Anthropic docs — long-horizon autonomy, first-shot correctness, higher bug-finding recall + repo-history search, navigating ambiguity.
- **Now:** (1) over-cautious safety classifiers + silent Opus-4.8 fallback (BridgeMind: ~70% debug-score drop, 9/12 tasks rerouted); (2) documented Fable defaults (overplanning, tidying, verbosity, unrequested actions, early stopping); (3) cost/access turbulence.
- **Fixes (official + community, unanimous):** de-prescribe/delete over stack; dial effort per task; anti-overplanning / anti-tidying / boundary snippets; ground progress claims; verifier subagents; memory file; async subagents + anti-early-stopping; avoid reasoning-extraction & token-countdown foot-guns; rip out prior-model-tuned scaffolding.

## Local-trace forensics (this machine)

- **130 genuine `model=fable` sessions** Jun 13–Jul 6 (`~/.claude-sdd`); **Opus-4.8-dominant, Fable minority**; ~40 `prompt-test` one-offs. `prompt-hamsa-audit-log` (id `59ebeebb6ed05c89`) already runs pre-registered A/B evals with "original Fable captures" as a baseline.
- **Fallback check:** 2 sampled sessions (Jun-15, Jul-5) → 0 refusal, 0 fallback blocks. Silent-fallback unconfirmed locally; needs a Langfuse served-by check (tasks FT-8/FT-9).
- **Tooling caveats:** `cc.py` is pinned to `~/.claude` (can't read the sdd sessions); `lf.py` Langfuse keys didn't auto-discover.

**2026-07-07 — FT-9 full-corpus served-model scan (definitive local answer to FND-7/FND-12).**

*Method:* the Langfuse lane turned out impossible — llm-proxy logs the **request-side** model (`passthrough.rs` `parsed.get("model")`) and discards the response's `/message/model` in both SSE and buffered paths, so "served-by" never reached Langfuse. Instead scanned CC JSONL directly: every assistant message stores the API response's `model` field = the served model. Scanner: session scratchpad `fallback_scan.py` (aggregates only; per-session model-run compression + refusal stop_reason counts) over `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`.

*Numbers:* 746 session files scanned; **81 fable-containing sessions** (8 / 69 / 4 per profile); ~145k assistant messages in those sessions. **0 `refusal` stop_reasons** (stop_reason universe: tool_use / end_turn / stop_sequence only). **16 mixed-model fable sessions — all mixing in large contiguous blocks** (smallest foreign block adjacent to a fable run: opus(36), which itself merges into a following opus(754) across a `<synthetic>` marker = one sustained switch). No single-call or few-call Opus interleave inside a fable run anywhere — i.e. no reroute signature; block shape matches manual `/model` switches / resumes.

*Caveat:* detection relies on the response `model` field being honest. That is the same channel BridgeMind used to count 9/12 rerouted tasks (FND-7), so the mechanism *as reported* would have been visible here.

*Verdict:* **silent Opus fallback REFUTED for local usage** → FND-14 (findings), FND-12 resolved, FT-9 done, FT-12 dropped as moot. Forward monitoring **SHIPPED same day** (user go-ahead): llm-proxy now captures `/message/model` as `served_model` — trace metadata `requested_model`/`served_model`, `lf.py find` SERVED column + `lf.py trace` mismatch marker; verified live post-restart. See llm-proxy `docs/issues/2026-07-07-langfuse-served-model-not-logged.md` (fixed). Operationalized as a one-command check 2026-07-07: **`lf.py mismatches`** (llm-proxy:`b72d0f6`, paginated scan, pre-capture traces counted as no-data) — first run: 300 scanned / 293 with model data / **0 mismatches**.
## Sources (reliability noted)

**Official / primary:**
- Anthropic — *Prompting Claude Fable 5* (`platform.claude.com/docs/.../prompting-claude-fable-5`) — canonical behavior + fixes
- Anthropic — *Introducing Fable 5 & Mythos 5*; *News*; *Mythos Preview* (`red.anthropic.com/2026/mythos-preview`); *Glasswing*
- UK AISI cyber eval (`aisi.gov.uk`)
- Bundled `claude-api` skill reference (local, authoritative here)

**Reliable independent firsthand:**
- Simon Willison, Jun 9 (`simonwillison.net/2026/Jun/9/claude-fable-5`) — best "ran great" evidence

**Reputable press / measurement:**
- CNBC / Business Insider / Forbes / GT Law — export-control shutdown & restore
- TechTimes — BridgeMind ~70% debug-score drop via classifier reroute (via researcher summary; direct fetch blocked)
- BleepingComputer / VentureBeat — "nerfed" sentiment + transparency grievance
- Vellum — benchmark breakdown

**Forum anecdote (sentiment only):** r/claude, r/ClaudeCode, r/claudexplorers threads.

**Use with care / UNVERIFIED:** Wikipedia *Claude Mythos* — clean timeline but flagged possibly-speculative; the `TOO_DUMB_TO_NEED_FABLE` covert-throttling claim is unverified (see FND-13). A large fraction of Fable web results are SEO/AI-generated blogspam — discarded.

## History

### 2026-07-07 — tracker created
Captured from the Fable research session: background web/forum agent + local trace forensics + `claude-api` migration-guide grounding.


### 2026-07-07 — session passover
Web + local-trace evidence fully captured here (safe to compact session context). Resume via the index (`ca8c26fecbbc4f37`) § Session passover.

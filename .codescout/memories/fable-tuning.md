# Fable tuning work stream

Recovering "early-`claude-fable-5`" quality in codescout via prompts/tools/trackers. Ran 2026-07-07 (2 sessions); **high-priority backlog complete — 9/12 tasks resolved.**

Trackers (docs/trackers/fable-tuning-*.md — query via `artifact(find, tags=["fable"])`, never raw read):
- index    `ca8c26fecbbc4f37` — start here; § Session passover has the full state
- findings `35de33286cd34f87` — FND-1..16
- tasks    `ad1af8262fdce357` — T-N with closure notes
- research `ab2170158c7d264e` — sources + local-trace evidence

Headline results: silent Opus fallback REFUTED locally (FND-14; llm-proxy now logs requested vs served model per call, llm-proxy:678778c, as the standing detector); CLAUDE.md diet measured + HELD (A-2 closed); foot-gun sweep CLEAN (A-13/FND-15); anti-tidying snippet NOT shipped — base arm 10/10 surgical at ceiling (A-14/FND-16).

**Durable output: the subtract-and-measure protocol (P-1..P-8)** — prompt-hamsa-audit-log `59ebeebb6ed05c89` § Protocol; developer entry `src/prompts/README.md` § Measure before shipping; template `prompt-engineering/scenarios/fable-tidying/`. Any new prompt-surface change enters via the protocol (base-arm-first, pre-registered decision rule), not via new fable-tuning tasks.

Remaining opportunistic: T-3..T-6 (medium; priors weakened by FND-16, expect ceiling; T-6 multi-turn is the likeliest escape) and T-10 (low; cc.py --config-dir).
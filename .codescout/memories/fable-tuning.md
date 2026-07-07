# Fable tuning work stream

Recovering "early-`claude-fable-5`" quality in codescout via prompts/tools/trackers. Started 2026-07-07.

Trackers (docs/trackers/fable-tuning-*.md — query via `artifact(find, tags=["fable"])`, never raw read):
- index    `ca8c26fecbbc4f37` — start here; see § Session passover
- findings `35de33286cd34f87` — FND-N, filter by dimension/status
- tasks    `ad1af8262fdce357` — T-N, filter by status/priority/surface
- research `ab2170158c7d264e` — sources + local-trace evidence

Session 2 (2026-07-07) results: T-8 done (lf.py .env-shadowing fix), T-9 done — **silent Opus fallback REFUTED locally** (FND-14: 81 fable sessions, 0 refusals, 0 per-call interleaves; llm-proxy never logged served-by, JSONL message.model was the source), T-12 dropped, T-1 closed as zombie-born (CLAUDE.md cut had shipped 2026-06-21 `b603d86f`; A-2 measurement run + HELD: 0 dead-name calls / 4,743 post-cut).

Do-next (open/high): T-7 (reasoning-extraction foot-gun audit, no gate), T-2 (anti-tidying snippet, eval-gated), T-11 (codify subtract-and-measure). Remaining lever per FND-8/9: remove over-prescription, don't add scaffolding. Gate prompt changes via prompt-hamsa-audit-log `59ebeebb6ed05c89` against the "original Fable captures" baseline.
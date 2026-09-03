# Fable tuning work stream

Recovering "early-`claude-fable-5`" quality in codescout via prompts/tools/trackers. Ran 2026-07-07 (2 sessions) + 2026-07-10 continuation. **High-priority backlog complete; single-turn inspection has hit ceiling — every probe returns "fable fine locally."**

Trackers (docs/trackers/fable-tuning-*.md — query via `doc(action="find", filter={"tags": {"contains": "fable"}})`, never raw read):
- index    `ca8c26fecbbc4f37` — start here; § Session passover has full state
- findings `35de33286cd34f87` — FND-1..17
- tasks    `ad1af8262fdce357` — FT-N with closure notes (renamed from T-N in `d3282868`; the prefix collided with two other ledgers)
- research `ab2170158c7d264e` — sources + local-trace evidence

Headline (all point one way — parity, not degradation):
- Silent Opus fallback REFUTED locally (FND-14, full-corpus JSONL).
- Anti-tidying snippet NOT shipped — base arm 10/10 surgical at ceiling (FND-16/A-14).
- Foot-gun sweep CLEAN (FND-15/A-13); CLAUDE.md diet HELD (A-2).
- **FND-17 (self-reflection lane, 2026-07-10):** bucket traces by `served_model`; local agent tool-use shows NO fable degradation — engagement parity (fable 83% tool_use / 15.5% end_turn vs opus 82%/9.6%), codescout tool discipline intact in a clean 62-call fable window (0 native Bash). Cohort thin (2 sessions, 89 calls).

Observability (the standing guard):
- llm-proxy logs requested vs served model per call (llm-proxy:678778c).
- `lf.py mismatches [--check]` — `--check` adds exit-code contract (exit 2 on mismatch).
- **Standing watch LIVE: `llm-mismatch-watch.timer` (systemd --user, daily) runs `lf.py mismatches --check`; a reroute leaves the oneshot in `failed` state = the alert. Shipped llm-proxy:481b31e, verified both paths. `systemctl --user status llm-mismatch-watch.service` to check.**

Durable method: subtract-and-measure protocol (P-1..P-8, prompt-hamsa-audit-log `59ebeebb6ed05c89` § Protocol; README § Measure before shipping; Hamsa SKILL.md H12). New prompt-surface changes enter via the protocol (base-arm-first, pre-registered decision rule), NOT new fable tasks.

Open: FT-3..FT-6 (medium, priors weakened by FND-16/17; FT-6 multi-turn is the one unexplored regime = likeliest escape from ceiling, needs harness work). FT-10 (cc.py --config-dir) — root cause + a second path-encoding bug filed in the llm-proxy repo as `llm-proxy:docs/issues/2026-07-10-ccpy-config-dir-hardcoded-and-path-encoding.md` (llm-proxy:40f1645) — cross-repo, does not resolve from here.

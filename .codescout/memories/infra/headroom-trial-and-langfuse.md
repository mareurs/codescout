# Headroom proxy trial + local Langfuse observability infra (set up 2026-06-19)

Cross-repo. Canonical spec: codescout `docs/superpowers/specs/2026-06-18-headroom-proxy-measurement-design.md`.

## Topology
All Claude Code → `llm-proxy` :8082 (systemd USER service `llm-proxy`, runs `~/agents/llm-proxy/target/release/llm-proxy`,
symlinked binary) → Anthropic (baseline) OR Headroom :8787 (treatment). llm-proxy logs every request to local Langfuse.
A/B switch = `ANTHROPIC_UPSTREAM_URL` in `~/agents/llm-proxy/.env` (baseline `https://api.anthropic.com`,
treatment `http://localhost:8787`) + `systemctl --user restart llm-proxy`.

## CC routing — ALL 3 profiles (settings.json env, added 2026-06-19)
`~/.claude`, `~/.claude-sdd`, `~/.claude-kat` each have `env.ANTHROPIC_BASE_URL=http://localhost:8082`.
⚠️ CRITICAL PATH: llm-proxy is now in front of ALL CC. Do NOT `systemctl --user restart llm-proxy`
with a broken build — verify `cargo build --release` succeeds FIRST, else all CC breaks. Langfuse down
is harmless (async ingestion). `claude-trial` fn in `~/.bash_aliases` is now redundant (plain `claude` routes).

## Baseline llm-proxy config (current)
`~/agents/llm-proxy/.env`: STRIP_TOOLS + TRIM_BASH_DESCRIPTION commented OUT (transforms off for clean
measurement); ANTHROPIC_UPSTREAM_URL=https://api.anthropic.com (direct). Pre-trial backup in session
scratchpad `llm-proxy.env.pretrial-bak`.

## Local Langfuse — RELOCATED to llm-proxy (was backend-kotlin)
- Compose: `~/agents/llm-proxy/docker-compose.langfuse.yml`, compose project `langfuse`. UI http://localhost:3000.
- WHY moved: was in backend-kotlin's shared compose project; a `docker compose down -v` there wiped it
  2026-06-19. Old `backend-kotlin/docker-compose.langfuse.yml` neutralized with a SUPERSEDED header (do not run; :3000 conflict).
- Persistence (4 layers, "never lose again"): own compose project; BIND MOUNTS at
  `~/.local/share/langfuse/{postgres,clickhouse,minio}` (NOT removed by `down -v`); `restart: unless-stopped`
  (crash + boot via Docker daemon); headless seed.
- Headless seed: `~/agents/llm-proxy/.env.langfuse-init` (GITIGNORED, holds API secret) re-creates on empty DB:
  org `local`, project `llm-proxy-local`, API key public `pk-lf-ab107f81-51f1-4252-a214-3b6168cb8e93`
  (matches llm-proxy/.env LANGFUSE_* keys so proxy auths unchanged), login `marius-traian.mart@stefanini.com` / `langfuse-dev`.
- Manage: `docker compose -f ~/agents/llm-proxy/docker-compose.langfuse.yml up -d|down`.

## Metrics
Query via `claude-traces` skill (`lf.py`/`cc.py` in `~/agents/llm-proxy/.claude/skills/claude-traces/`) or Langfuse
API with the keys. NOTE backend-kotlin chat traces to CLOUD (cloud.langfuse.com via `.env.chat`), not this local instance.

## Treatment phase (NOT yet done)

Install Headroom Python proxy: `headroom proxy --port 8787 --no-telemetry --log-file ~/.headroom/trial.jsonl`;
set ANTHROPIC_UPSTREAM_URL=http://localhost:8787; restart llm-proxy.

**Fail-open is OPT-IN as of 2026-06-20 (llm-proxy commit `2906647`).** DEFAULT = no fallback:
if the Headroom hop errors or hangs, llm-proxy returns a loud 504 — and since it fronts ALL CC,
every profile sees the error (you notice and fix, rather than silently polluting baseline stats).
Two choices for the treatment window:
- **Fail-loud (default, cleanest stats):** leave `ANTHROPIC_FALLBACK_URL` unset. Headroom
  flakiness surfaces as CC 504s; there are no contaminated treatment requests to exclude.
- **Resilient (opt-in):** set `ANTHROPIC_FALLBACK_URL=https://api.anthropic.com` in
  `~/agents/llm-proxy/.env`. Then a Headroom error/hang falls back to direct Anthropic with log
  marker `upstream_fallback` — exclude those from treatment stats.

Hung-upstream failover is bounded by `UPSTREAM_SEND_TIMEOUT_MS` (default 15000ms); the client also
has a 5s `connect_timeout`. A 5xx from a *reachable* Headroom is passed through (not failed over),
marked `upstream_5xx`.
## Revert
Routing: remove `env.ANTHROPIC_BASE_URL` from the 3 settings.json (backups: session scratchpad
`settings.{main,sdd,kat}.bak.json`). Transforms: uncomment STRIP_TOOLS / TRIM_BASH_DESCRIPTION in llm-proxy/.env.
Then `systemctl --user restart llm-proxy`.

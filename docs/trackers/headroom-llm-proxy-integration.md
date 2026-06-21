---
id: '3366f6ae253097bd'
kind: tracker
status: active
title: Headroom + llm-proxy integration & trial
owners:
- marius
tags:
- headroom
- llm-proxy
- observability
- compression
- trial
- infra
- langfuse
topic: null
time_scope: null
---

## What this is

The integration of **Headroom** (a context-compression reverse proxy) behind **llm-proxy** (our
instrumented Anthropic reverse proxy + Langfuse observability gateway), to measure on real Claude
Code traffic whether compression earns a permanent place in the stack. Full design, ADRs, and the
§9 decision rubric live in the spec (see Pointers). Live operational state — services, `.env`,
revert, Langfuse — lives in the codescout memory `infra/headroom-trial-and-langfuse`. **This
tracker is the discoverable hub**: phase, gates, and pointers; details delegate to those two.

## Topology

`all CC → llm-proxy :8082 (ours) → { baseline: api.anthropic.com | treatment: Headroom :8787 } → Anthropic`

The A/B switch is one env var on llm-proxy (`ANTHROPIC_UPSTREAM_URL`); the client never reconfigures.
llm-proxy sits *in front* of Headroom, so Langfuse logs the ORIGINAL prompt while billed
`usage.input_tokens` reflects the compressed count.

## What shipped (llm-proxy hardening — commit `2906647`)

- Configurable upstream (`ANTHROPIC_UPSTREAM_URL`) with documented path-join resolution.
- **Fail-open is opt-in; default is fail-LOUD** — empty `ANTHROPIC_FALLBACK_URL` → a downed/hung
  upstream returns 504, not a silent reroute. This *inverts* ADR-4's original default (see the spec's
  ADR-4 revision); rationale is measurement integrity.
- Send-timeout via `tokio::time::timeout` (`UPSTREAM_SEND_TIMEOUT_MS`, default 15s) + client
  `connect_timeout` 5s → a hung upstream fails over instead of hanging the request forever.
- `upstream_5xx` marker — a 5xx from a reachable upstream is passed through, not failed over
  (fail-open is connection-level by design).
- Langfuse logs the original prompt (built before request transforms apply).
- Hermetic fail-open tests (`tests/failopen.rs`): refused / hung / 5xx-passthrough / fail-loud.

## Current state — shakeout LIVE (2026-06-21)

A passthrough shakeout is running on the daily driver. Headroom runs as a systemd user service
(`headroom-proxy`). It validates the 3-hop integration and measures TTFT/hop overhead; it does
**not** measure savings — **compression is a byte-equal no-op until Headroom PR-B3+**. Operational
details (service, `.env`, revert, watch) are in the ops memory.

## Decision gates (§9 — confirmed 2026-06-21)

See the `[LIVE]` table above and the spec §9. Adopt Headroom permanently only if ALL hold over the
treatment window: net token saving ≥25%; **prompt-cache within ~10% (HARD GATE** — CC is unusually
cache-heavy, a broken cached prefix can make net savings negative); CCR retrieval <5%; TTFT added
latency <150ms median.

## Phased plan

1. ✅ Build + harden llm-proxy (configurable upstream, fail-open, send timeout, observability).
2. ✅ **Shakeout (passthrough)** — validate the integration + measure TTFT. ← *here*
3. ⏳ **Economics trial** — blocked on Headroom **PR-B3+** (real compression). Then set the unit's
   ExecStart to `--compression --compression-mode live_zone`, run ~2 weeks, score against §9.
4. ⏳ **Decide** — adopt / narrow to a codescout-specific integration / reject. If adopt → open the
   Appendix A follow-on (codescout tool-usage instrumentation).

## Pointers

- **Spec (design, ADRs, §9):** `docs/superpowers/specs/2026-06-18-headroom-proxy-measurement-design.md` (artifact `cbc09eca4c4ab04c`)
- **Ops memory (services, `.env`, revert, Langfuse):** codescout memory `infra/headroom-trial-and-langfuse`
- **llm-proxy:** `~/agents/llm-proxy` (master) — hardening commit `2906647`
- **Headroom:** `~/work/claude/headroom` — rebuilt Rust `headroom-proxy` CLI
- **Langfuse UI:** http://localhost:3000  •  **Headroom logs:** `journalctl --user -u headroom-proxy -f`

## History

### 2026-06-21 — Shakeout live; thresholds confirmed; tracker created
- §9 thresholds confirmed (commit `e4ef7d51`); prompt-cache reframed as the hard gate.
- Headroom rebuilt as the Rust binary `headroom-proxy`; new CLI (`--listen` / `--upstream`, `--upstream` required); compression a no-op until PR-B3+.
- Passthrough shakeout flipped live; Headroom made a systemd user service; ops memory updated (commit `46f48231`).

### 2026-06-20 — llm-proxy fail-open hardening shipped
- Commit `2906647`: opt-in fail-open (default fail-loud), send timeout, `upstream_5xx` marker, pre-transform Langfuse logging, hermetic fail-open tests.

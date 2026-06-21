---
title: Headroom proxy trial — measurement design
status: draft
date: 2026-06-18
authors: [Marius, Architecture Snow Lion]
spans_repos: [codescout, llm-proxy, headroom]
implements_in: llm-proxy
supersedes: none
---

# Headroom proxy trial — measurement design

## 1. Goal & the question

Run **Headroom** (a local-first LLM context-compression reverse proxy) in front of our
real Claude Code traffic for ~2 weeks and decide, on measured evidence, whether it earns
a permanent place in the agent stack.

The decision has two halves, and they need different instruments:

1. **Economics** — how many input tokens / how much cost does Headroom save?
2. **Quality** — does compression degrade the model's answers, tool use, or prompt-cache
   behavior?

We already own an instrumented Anthropic-API reverse proxy (`llm-proxy`, systemd, `:8082`).
This spec uses it as the experiment harness and, longer term, as a permanent observability
gateway.

## 2. Scope

**In scope.** Headroom's effect on the *entire* Claude Code → Anthropic request stream.
codescout's tool outputs are one slice of each prompt, observed in aggregate — not isolated.

**Explicitly out of scope.**
- Isolating codescout's slice from the rest of the prompt (structurally impossible at this
  vantage — codescout's tool_results are embedded in Claude Code's prompt, not a separate
  wire).
- Any codescout-side code change. codescout is a measurement *subject*, not a participant.
- The codescout-specific integration directions (CCR convergence, in-process `headroom-core`
  crate, coexistence signal). **This trial is the gate before any of those.** If Headroom
  does not earn its hop end-to-end, those questions are moot; if it does, they get their own
  brainstorm.

**What "the trial measures Claude Code, not codescout" means for the decision.** A positive
result answers *"is Headroom worth running in front of my agent?"* — the realistic deployment
question. It does **not**, by itself, answer *"should codescout emit compressed output?"*

## 3. Topology

llm-proxy stays the front door (Claude Code already points at `:8082`); Headroom is a
swappable middle hop selected by one env var on llm-proxy.

```
                         ┌─ BASELINE:   ANTHROPIC_UPSTREAM_URL = https://api.anthropic.com
Claude Code ──:8082──▶ llm-proxy ──────┤   (direct; no compression)
(config unchanged)     (ours)          └─ TREATMENT:  ANTHROPIC_UPSTREAM_URL = http://localhost:8787
                          │                                       │
                  Langfuse: ORIGINAL request                      ▼
                  + response + billed usage          Headroom :8787  (Python `headroom proxy`)
                  + cache tokens + tool_call_count      │  records orig→compressed→saved, CCR rate
                  + session id                          │  ANTHROPIC_TARGET_API_URL = api.anthropic.com
                                                         ▼
                                                  api.anthropic.com
```

In **treatment**, llm-proxy sits *in front of* Headroom, so it logs the **original** prompt
(what Claude Code sent) but the response's `usage.input_tokens` reflects the **compressed**
(billed) count. This split is intentional and is handled in §8.

## 4. Architecture decisions (ADRs)

### ADR-1 — llm-proxy in front, Headroom behind, env-switched

**Decision:** llm-proxy is the permanent front door. Its Anthropic upstream becomes
env-configurable (`ANTHROPIC_UPSTREAM_URL`, default `https://api.anthropic.com`). Flipping it
to `http://localhost:8787` inserts Headroom; flipping it back removes Headroom. The client is
never reconfigured.
**Context:** llm-proxy is ours and already the front door. The only force that previously
dictated "Headroom in front" was a hard-coded constant we now control. Verified: that constant
(`ANTHROPIC_MESSAGES_URL`, `src/passthrough.rs:20`) has exactly **one** consumer — the single
`.post(ANTHROPIC_MESSAGES_URL)` at `src/passthrough.rs:105` inside `handle`. One-const → one
config read.
**Alternatives considered:**
- *Headroom in front* (`Headroom → llm-proxy → Anthropic`): zero code change, but every
  baseline↔treatment flip reconfigures the *client*, and Langfuse then captures only the
  *compressed* prompts. Rejected — worse A/B ergonomics, loses original-prompt capture.
- *Headroom standalone* (no llm-proxy): yields the savings number only; no content/session
  quality forensics. Rejected — we chose the quality verdict.
**Consequences:**
- now easier: one-env-var A/B on a single instrument; original-prompt capture; no client
  reconfig; a configurable upstream worth keeping permanently.
- now harder: a three-hop hot path; a new partial-failure mode (see ADR-4); a cache-key
  authority that two nodes could touch (ADR-2); streaming that must survive two relays;
  llm-proxy transforms that must be disabled (§8).
**Change scenarios absorbed:** insert/remove a middle transform proxy without touching the
client or recompiling; future staging relays.
**Revisit-when:** a third route needs a configurable upstream (then consider a resolver,
ADR-3); or we want llm-proxy to independently tokenize (we don't — Headroom owns that math).
**Confidence:** high.

### ADR-2 — Headroom owns the prompt-cache key

**Decision:** `prompt_cache_key` / cache-control is mutated by exactly one node — Headroom.
llm-proxy passes cache controls through untouched.
**Context:** Headroom's entire value rests on preserving Anthropic prompt-cache via its
live-zone freeze. Two independent nodes mutating the same field makes cache behavior
unattributable — a shared-data-structure coupling with one legitimate owner.
**Consequences:** now easier — prompt-cache stays measurable (we read `cache_read_tokens` to
prove the freeze holds). now harder — llm-proxy must be audited to confirm it injects no
cache key on the passthrough path.
**Change scenarios absorbed:** prompt-cache regressions are diagnosable to one owner.
**Confidence:** high.

### ADR-3 — Defer the upstream-resolver abstraction

**Decision:** Add only the minimal `ANTHROPIC_UPSTREAM_URL` override on the Claude path. Do
**not** unify the three existing upstream resolutions (Anthropic const, `OPENROUTER_URL`
const `src/proxy.rs:25`, `LOCAL_MODELS` env map `src/main.rs:60-68`) into one resolver now.
**Context:** the rule-of-three is technically met, but the experiment needs only the Anthropic
override. A resolver extracted under experiment pressure freezes an interface around the wrong
sample.
**Alternatives considered:** unify now — rejected as premature abstraction (one wall in an
empty field).
**Revisit-when:** a genuinely new fourth upstream need appears; then let the duplication
dictate the resolver's shape.
**Confidence:** high.

### ADR-4 — The Headroom hop fails open

**Decision:** If llm-proxy cannot reach Headroom (connection refused / timeout), it falls back
to the direct Anthropic upstream rather than erroring. The fallback wraps the single send at
`src/passthrough.rs:105`.
**Context:** Headroom is fail-loud by design (exit 78 if its core is missing). Because the
gateway is now permanent and on the hot path for *all* agent traffic, a Headroom crash must
never take down Claude Code. The resilience must live in our node.
**Consequences:** now easier — Headroom restart/crash mid-trial is invisible to the user.
now harder — a silent fallback can mask "Headroom is down" and quietly end the treatment
window; therefore the fallback **must log** a distinguishable event (see §8) so a fallback
isn't mistaken for a compressed request.
**Change scenarios absorbed:** Headroom outage during a multi-week trial.
**Confidence:** high — shipped (commit `2906647`) and covered by hermetic smoke tests in
`tests/failopen.rs` (refused / hung / fail-loud-by-default, all asserted end-to-end). See the
revision below for the default change.

**Revision (2026-06-20) — fail-open is now opt-in; the default is fail-LOUD.** The original
decision (fall back to direct Anthropic *by default*) was inverted during implementation: with
no `ANTHROPIC_FALLBACK_URL` configured, the fallback is empty and a downed or hung upstream
returns a loud 504 rather than silently rerouting. Rationale: the very risk this ADR named — a
silent fallback masking "Headroom is down" and quietly ending the treatment window — is better
*eliminated* than merely logged. Operators opt into the safety net by setting
`ANTHROPIC_FALLBACK_URL=https://api.anthropic.com`. The timeout half of the decision shipped as
`tokio::time::timeout` around `send()` (bounds the response-header wait only; body streaming is
unaffected) plus a client `connect_timeout`. Fail-open stays connection-level: a 5xx from a
reachable upstream is passed through and marked `upstream_5xx`, never failed over.
## 5. llm-proxy changes (build-to-keep)

These serve the trial **and** survive as the permanent gateway's spine. Keep them production-grade.

1. **Configurable Anthropic upstream.** Replace the `ANTHROPIC_MESSAGES_URL` const
   (`src/passthrough.rs:20`, single consumer at `:105`) with a value read from
   `ANTHROPIC_UPSTREAM_URL` (default `https://api.anthropic.com/v1/messages`). Path-join
   semantics: if the env value lacks `/v1/messages`, append it; document the exact rule.
2. **Fail-open fallback** (ADR-4): on send error to the configured upstream, retry once
   against the hard default Anthropic URL and emit a `upstream_fallback` log marker.
3. **Cache-key passthrough audit** (ADR-2): confirm llm-proxy does not inject/rewrite
   `prompt_cache_key` on the Claude passthrough path; if it does, gate it off.
4. **No new instrumentation required for the trial** — `StreamAccumulator` already captures
   `tool_call_count`, cache tokens, TTFT, and full input/output into the Langfuse
   `GenerationLog` (`src/passthrough.rs`, `into_generation_log` / `build_langfuse_input` /
   `build_buffered_gen_log`).

**Implementation status (2026-06-20): SHIPPED + verified.** Commit `2906647` (`llm-proxy`,
master; verified `cargo test` green — 18 unit + hermetic fail-open suite). Per item:

1. **Configurable upstream — done.** `resolve_upstream_url` reads `ANTHROPIC_UPSTREAM_URL`
   (empty → direct Anthropic; bare base → `/v1/messages` appended; trailing slash trimmed;
   an already-messages URL used as-is). Pinned by `resolve_upstream_url_cases`. Line numbers
   drifted from this spec; the `ANTHROPIC_MESSAGES_URL` const is retained as the hard default.
2. **Fail-open fallback — done, with a deliberate default flip (see ADR-4 revision).** The
   fallback target is an injectable `AppState.anthropic_fallback_url` (env
   `ANTHROPIC_FALLBACK_URL`). **Default empty = NO fallback: failures are LOUD (504), not
   silently bypassed** — fail-open is opt-in. The send is wrapped in
   `tokio::time::timeout` (`UPSTREAM_SEND_TIMEOUT_MS`, default 15s) plus a client
   `connect_timeout` (5s), so an up-but-hung upstream fails over (or fails loud) instead of
   hanging forever — the gap the original "needs a smoke test" flagged. `upstream_fallback`
   marker emitted on each fallback hop.
3. **Cache-key passthrough — verified.** `transforms_preserve_cache_control` proves
   `cache_control` survives llm-proxy's own transforms untouched (ADR-2 holds).
4. **Instrumentation — confirmed sufficient**, and `langfuse_input` is now built *before*
   `apply_request_transforms`, so the logged prompt is always the original regardless of
   llm-proxy transform config (keeps savings attribution clean).

**New env knobs:** `ANTHROPIC_FALLBACK_URL`, `UPSTREAM_SEND_TIMEOUT_MS` (+ fixed
`connect_timeout` 5s). **Residual:** failover guards only the pre-headers phase — once
streaming starts, a mid-stream upstream stall cannot fail over (inherent; the response is
already committed). A 5xx from a *reachable* upstream is passed through, not failed over, and
marked `upstream_5xx`.
## 6. Headroom configuration

- Run the **Python** proxy (`headroom proxy`), **not** the Rust binary — the Rust compressors
  are still no-ops; the savings tracker, `/stats`, and CCR signals live in Python.
- `headroom proxy --port 8787 --no-telemetry --log-file ~/.headroom/trial.jsonl`
  - `--no-telemetry` — kills the external Supabase beacon (which sends aggregate numbers only,
    never content) per privacy posture (§10).
  - `--log-file` — local per-request JSONL (orig vs optimized tokens). Do **not** pass
    `--log-messages` (keeps prompt content out of Headroom's own logs).
- `ANTHROPIC_TARGET_API_URL` left at default (`api.anthropic.com`) — Headroom talks straight
  to the provider.

## 7. Measurement protocol

Same instrument, two windows, so the comparison is apples-to-apples on one pipeline.

1. **Baseline window (~3–5 days).** `ANTHROPIC_UPSTREAM_URL=https://api.anthropic.com`. No
   Headroom. Langfuse captures original prompts, real `usage`, cache tokens, tool counts,
   latency, by session.
2. **Treatment window (~1–2 weeks).** `ANTHROPIC_UPSTREAM_URL=http://localhost:8787`, Headroom
   running. Headroom records per-request savings; Langfuse records resulting quality/usage;
   CCR retrieval rate + cache health watched continuously.
3. **Switch = one env edit + `systemctl --user restart llm-proxy`.** No client change.

## 8. Metrics to pull

> **Query surface:** pull every Langfuse/JSONL number below via the **`claude-traces`** skill
> (`lf.py` / `cc.py`), not hand-rolled queries. Run each across both windows and diff.

**Economics (authoritative: Headroom).**
- `headroom perf --hours <N> --format json` and `GET /stats-history` →
  `input_tokens_original`, `input_tokens_optimized`, `tokens_saved`, compression %, USD saved.
  This is the per-request, exact savings number.
- **Independent cross-check (Langfuse).** Baseline-window `usage.input_tokens` (uncompressed
  billed) vs treatment-window `usage.input_tokens` (compressed billed), aggregated over
  comparable session types. Approximate (different traffic across windows) but a useful sanity
  check on Headroom's self-report. **Note:** in treatment, Langfuse `input` *content* is the
  ORIGINAL prompt while `input_tokens` is the COMPRESSED billed count — never read treatment
  `input_tokens` as "original."

**Quality (authoritative: Langfuse + Headroom CCR).**
- `GET /v1/retrieve/stats` (Headroom) → CCR retrieval rate. Headroom's own comment: *high =
  compression too aggressive.* Primary quality red-flag.
- Langfuse treatment vs baseline: `tool_call_count` distribution, `stop_reason` distribution,
  spot-checked response content on matched prompts.
- Distinguish `upstream_fallback`-marked requests (ADR-4) and exclude them from treatment
  stats — they were not actually compressed.

**Prompt-cache health (validates Headroom's core promise).**
- `cache_read_input_tokens` / `cache_creation_input_tokens` from Langfuse, baseline vs
  treatment. If cache_read collapses under treatment, Headroom's live-zone freeze is busting
  the cache — a disqualifying regression even if raw tokens drop.

**Latency.**
- TTFT (`first_token_ms`) and total latency, baseline vs treatment. The extra hop + compute is
  the cost side of the ledger.

## 9. Decision rubric

**Thresholds CONFIRMED with Marius 2026-06-21.** Adopt Headroom permanently **only if all
hold** over the treatment window:

1. **Net input-token saving ≥ 25%** on real traffic (Headroom `perf`), *after* netting out any
   prompt-cache loss. Below this, the three-hop complexity is not worth it.
2. **Prompt-cache preserved** — treatment `cache_read_input_tokens` within **~10%** of baseline.
   **This is the primary hypothesis under test — treat it as an effective hard gate, not a soft
   threshold.** CC traffic is unusually prompt-cache-heavy (cached system prompt + tools +
   history, billed at ~10% of input); if Headroom's compression mutates the cached prefix the
   cache breaks, and net savings can go **negative** even as raw token count drops. ADR-2's
   live-zone freeze is meant to prevent this but is unproven on our traffic — the trial is
   really testing whether the freeze holds.
3. **No material quality regression** — CCR retrieval rate **< 5%** of requests triggering a
   full retrieval, no adverse shift in `stop_reason` distribution, spot-checked answers
   unaffected.
4. **TTFT overhead acceptable** — median added latency **< 150 ms**.

If economics pass but quality fails → do not adopt globally; revisit the narrower
codescout-specific integration (compress only safe surfaces). If both pass → adopt, then open
the follow-on (Appendix A).
## 10. Privacy posture

- Headroom: `--no-telemetry` (no external beacon) and **no** `--log-messages` (no prompt
  content in Headroom's logs).
- Content forensics live **only** in our local Langfuse (`LANGFUSE_BASE_URL`), on our machine.
- **Retention (RESOLVED 2026-06-18):** keep full request/response content in Langfuse
  **indefinitely**; manual cleanup as needed. No automated rotation — Marius prunes the
  store by hand when it grows.

## 11. Risks & revisit-when

| Risk | Mitigation | Revisit trigger |
|---|---|---|
| Headroom outage takes down Claude Code | Fail-open (ADR-4) + logged fallback marker | repeated fallbacks → Headroom instability |
| llm-proxy transforms confound the result | Disable `STRIP_TOOLS` / `TRIM_BASH_DESCRIPTION` during the trial | any transform left on |
| Double cache-key injection | ADR-2: Headroom owns it; audit llm-proxy passthrough | cache_read anomaly in baseline |
| SSE breaks across the extra hop | Smoke-test streaming before the window opens | malformed/blank streamed responses |
| Rust-vs-Python confusion | Run the Python proxy (§6) | `/stats` returns empty |
| Permanent prompt log grows unbounded | Retention = keep forever, manual cleanup (§10) | Langfuse storage pressure → prune by hand |

## Appendix A — The codescout tool-usage instrumentation boundary (scopes the follow-on)

Marius's goal is broader than `usage.db`: see the **prompts around** a tool call and
understand full context. Verification shows the observability stack already has **three
complementary layers, each with a skill** — so the follow-on composes them, it does not build
a fourth store.

**Layer 1 — Tool health (server-side).** codescout `.codescout/usage.db` (`src/usage/db.rs`;
`usage` tool, `doctor://tool-usage`, dashboard) → the **`analyze-usage`** skill. Owns per-tool
calls, error_rate, overflow_rate, p50/p99 latency, outcome class. Sees `recoverable_error` vs
hard error and OutputGuard overflow — which the wire cannot.

**Layer 2 — Prompt context + token economics (the "full context" layer).** llm-proxy →
Langfuse, plus Claude Code session JSONL → the **`claude-traces`** skill
(`llm-proxy/.claude/skills/claude-traces/scripts/`): `lf.py` (Langfuse — tokens
in/out/cache_read/cache_write, latency, TTFT, cost, profile via `api_key_hint`; progressive
disclosure session→find→trace) and `cc.py` (JSONL — message+tool timeline, `tool-calls` actual
sequence, cost, text search). Answers "what was the conversation around this call, and what
did it cost." **This is the analysis surface for the Headroom trial** (§8) — we build no new
analysis.

**Layer 3 — Production agent traces (Arize).** Sibling skill **`arize-logs`** in
`prosus/service-m-python-agentex/.claude/skills/` (`ArizeClient` / `SearchFilters` / fetch /
search / similar / compare) queries Arize traces for the production AgentEx deployment.
Different domain, same skill pattern — listed so the family (one progressive-disclosure skill
per observability backend) is explicit.

**Already satisfied vs the one new slice.**
- *Full context around a codescout tool call, per session* — **available today** via `cc.py
  trace --slim` (surrounding messages) + `cc.py tool-calls` (codescout sequence) + `lf.py
  trace` (token cost). No new build.
- *Aggregate token-cost-per-codescout-tool, and its compression delta under Headroom* — the
  **only genuinely new slice**, attributable by `tool_use.name` on the wire. The
  per-invocation join to `usage.db` (latency/error) stays blocked by the missing shared id
  (the MCP call carries no Anthropic `tool_use_id`); defer it.

**Rule:** route each question to the layer that owns it — health → `analyze-usage`/`usage.db`;
prompt+cost → `claude-traces`; production → `arize-logs`. Do **not** build a fourth store; the
Headroom follow-on adds at most the by-tool-name cost rollup to Layer 2.

**Verify before the follow-on:** `claude-traces` exists in *both* codescout and llm-proxy
`.claude/skills/` — confirm which is canonical (CLAUDE.md: a repo file is the source of truth,
not a copy) before extending it.
## Appendix B — Open questions for review

1. Decision-rubric thresholds (§9) — proposed defaults stand (≥25% saving / cache ±10% / CCR full-retrieval <5% / TTFT <150 ms), adjustable any time before the trial concludes; not yet explicitly confirmed.
2. Window lengths (§7) — proposed defaults stand (3–5 d baseline / 1–2 wk treatment), adjustable before the trial opens.
3. Langfuse content retention (§10) — **RESOLVED (2026-06-18): keep full content indefinitely; manual cleanup as needed.**
4. Spec home — **RESOLVED (2026-06-18): canonical spec stays in codescout `docs/superpowers/specs/`; the proxy-side implementation excerpt is copied to `llm-proxy/docs/2026-06-18-configurable-upstream-headroom-trial.md`.**

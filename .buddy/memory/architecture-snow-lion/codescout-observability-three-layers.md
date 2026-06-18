---
specialist: architecture-snow-lion
scope: project
slug: codescout-observability-three-layers
created: 2026-06-18
updated: 2026-06-18
tags: [observability, instrumentation, usage-db, langfuse, arize, skills, boundaries]
---

**Lesson:** Observability for codescout work already lives in **three composable layers**, each
with its own skill. Route a question to the layer that owns it; do not build a fourth store.

**Why:** Found during the Headroom proxy-trial design. Marius's goal — "not just usage.db, but
see the prompts around a call, full context" — is mostly already served by surfaces that exist:
- **Layer 1, tool health (server-side):** `.codescout/usage.db` (`src/usage/db.rs`; the `usage`
  tool, `doctor://tool-usage` resource, dashboard) → the **`analyze-usage`** skill. Owns
  per-tool calls, error_rate, overflow_rate, p50/p99 latency, outcome class (`error` vs
  `recoverable_error`), OutputGuard overflow. The wire cannot see these.
- **Layer 2, prompt context + token economics:** llm-proxy → Langfuse + Claude Code session
  JSONL → the **`claude-traces`** skill (`llm-proxy/.claude/skills/claude-traces/scripts/`:
  `lf.py` for Langfuse tokens/cache/latency/TTFT/cost/profile; `cc.py` for the JSONL
  message+tool timeline and actual `tool-calls` sequence). The "full context around a call"
  layer.
- **Layer 3, production agent traces:** Arize → the **`arize-logs`** skill in
  `prosus/service-m-python-agentex/.claude/skills/` (ArizeClient/SearchFilters/fetch/search/
  similar/compare). Different domain, same one-skill-per-backend pattern.

My first framing ("don't rebuild usage.db on the wire") was right about non-duplication but too
narrow: the layers **compose**, they don't compete. "Full context around a codescout tool call,
per session" already ships via `cc.py trace --slim` + `cc.py tool-calls` + `lf.py trace`. The
only genuinely new slice is *aggregate* token-cost-per-tool (and its compression delta under a
layer like Headroom), attributable by `tool_use.name` on the wire; the per-invocation
usage.db↔Langfuse join stays blocked by a missing shared id (MCP carries no Anthropic
`tool_use_id`).

**How to apply:** Before building any new codescout observability instrument, check these three
layers and route to the owner — health → `analyze-usage`/`usage.db`; prompt+cost →
`claude-traces` (`lf.py`/`cc.py`); production → `arize-logs`. For any analysis of Claude Code
traffic (token/cost/cache/conversation), reuse `claude-traces` — do not hand-roll Langfuse/JSONL
queries. The Headroom proxy trial's analysis surface **is** `claude-traces`. Same restraint as
[[tool-registration-rule-of-three]]: don't build a fourth store until duplication earns it.
Caveat to resolve before extending: `claude-traces` exists in **both** codescout and llm-proxy
`.claude/skills/` — confirm which is canonical (repo file is source of truth, not a copy). Full
design: `docs/superpowers/specs/2026-06-18-headroom-proxy-measurement-design.md` Appendix A.

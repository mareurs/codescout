# Sakana Fugu — Integration Analysis for codescout

**Researched 2026-06-25** (Fugu launched ~2026-06-22). External-product intel — **will go stale; re-verify before relying on API specifics.**

## What Fugu is
Commercial, hosted, proprietary **multi-agent orchestration system exposed as a single OpenAI-compatible model API** ("one model to command them all"). Internally routes a task across a swappable pool of frontier LLMs, then verifies + synthesizes. Routing is intentionally opaque. Built on TRINITY (arXiv 2512.04695) + Conductor (arXiv 2512.04388); tech report arXiv 2606.21228. Not open source. Source: https://sakana.ai/fugu/

## Fugu API surface (the integration-relevant facts)
- Base URL `https://api.sakana.ai/v1`; Bearer API key from console.sakana.ai.
- Endpoints: `/v1/responses` (preferred), `/v1/chat/completions`, `/v1/models`. **NO `/v1/embeddings` and no embedding model.**
- **NO MCP support** — Fugu does not consume MCP servers; tool defs must live in the harness (OpenAI-style `tools` array only).
- Supports: tool/function calling, streaming (with `stream_idle_timeout_ms`/`stream_max_retries` resilience knobs), structured outputs (`json_schema`), vision (text+image input).
- Quirks: `temperature`/`top_p`/`stop`/`seed`/penalties/`parallel_tool_calls` accepted-but-ignored; `previous_response_id` rejected (resend full history); reasoning via `reasoning.effort` (high/xhigh/max).
- Models: `fugu`, `fugu-ultra`, `fugu-ultra-20260615`. Context window 1M tokens (272K is a pricing-tier boundary, not the window).
- Pricing (Ultra, per 1M): in $5/$10, out $30/$45, cached $0.50/$1.00 (low/high tier at 272K). Base `fugu` billed at underlying model's rate, not stacked. Subs: $20/$100/$200 = baseline/10x/30x.
- **EU/EEA: NOT available** (GDPR pending, no timeline). Privacy: agent opt-out is a console setting, base tier only (Ultra pool fixed); training opt-out via console.

## codescout's only model touchpoints (why integration is narrow)
- **Only outbound model call = embeddings** (`crates/codescout-embed/`: `remote.rs` → OpenAI-compatible `/v1/embeddings`; `local.rs` → ONNX/fastembed). **No chat-completions client anywhere.**
- All "LLM" work (system-prompt synthesis, tracker/artifact refresh, memory classification) is *delegated* to the calling agent as an emitted prompt — codescout never calls a chat model.
- codescout is agent-agnostic by design (a documented convention) and is itself an MCP server (tool *provider*).

## Integration verdict — three surfaces
1. **Fugu-powered harness driving codescout** — ONLY compelling fit, **zero codescout code change**. Topology: Codex/Cursor harness speaks MCP to codescout, translates MCP tools → OpenAI `tools`, points base_url at Fugu. Works today because codescout is agent-agnostic. Work (if any) = verification that prompt surfaces (Iron Laws, server_instructions — historically Claude-tuned) steer well under Fugu's opaque router; maybe a docs note. NOT a code feature.
2. **Fugu as embeddings backend** — **CLOSED.** No `/v1/embeddings`. Embeddings stay on local ONNX or separate provider.
3. **codescout grows internal chat client backed by Fugu** — speculative, weak ROI. Synthesis tasks are small/one-shot; Fugu's agentic value is overkill + expensive; quirks tolerable but no advantage over a cheap local/standard model.

## Blockers for direct use
- EU/EEA hard block (relevant if calling from an EU jurisdiction).
- Source code fans out to an opaque frontier-model pool; opt-out is coarse. Deliberate decision needed for proprietary code.

See also [[research/agent-memory-frameworks]].
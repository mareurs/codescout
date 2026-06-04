---
status: draft
kind: adr
opened: 2026-05-19
owner: marius
tags: [mcp, prompt-surfaces, claude-code, truncation]
related: []
---

# MCP Channel Caps in Claude Code

## Summary

Claude Code silently truncates the MCP `initialize.instructions` field and
each tool's `description` field at ~2 KB per block, appending the literal
marker `… [truncated]`. MCP resources are not exposed to the model at all
in this profile. Tool call results remain the only model-accessible channel
with significant content budget.

This finding invalidates the prior assumption that the codescout
`server_instructions.md` (~42 KB on the wire) reaches the model. ~95 % is
operationally dead before the first prompt token. The empirical evidence
and channel-by-channel measurements live below; design implications are at
the bottom.

## Status

**Investigation complete. Decision pending.** This document is the
evidence base for a redesign of how codescout delivers prompt content to
the model. The redesign itself is the next step — captured under
*Open Decision* at the end.

## Context

`src/prompts/source.md` is the source of truth for codescout's
`initialize.instructions` payload. It compiled to ~21 KB at build time;
at runtime `src/server.rs::from_parts` concat-appends
`librarian::INSTRUCTIONS` for a total of ~42 KB. Three prompt surfaces
in the repo (`server_instructions.md`, `onboarding_prompt.md`,
`builders.rs`) are kept consistent by an explicit invariant in
`CLAUDE.md` and a test
`server::tests::prompt_surfaces_reference_only_real_tools`.

All of that machinery assumes the full text reaches the model. That
assumption was never measured. This document measures it.

## Evidence — `instructions` field truncated at ~2 KB

### Direct stdio probe (server-side, ground truth)

```bash
{ initialize; notify; } | target/release/codescout start
```

Returns `instructions = 42,103 chars` end-to-end intact (terminating
sentence: "...the librarian did not see).").

### Langfuse trace (post-Claude-Code, model-side ground truth)

Trace `d19a4827-6871-4351-af9a-15fc71971ca7` (session
`1e371fec-...`, 2026-05-19). Direct extraction of the codescout
block from `input.system`:

```text
Start of codescout block:    pos 14896
End of codescout block:      pos 17041   (== start of next "## edu-planner...")
Codescout slice length:      2,145 chars  (JSON-escaped)
Block tail:                  "... For long-running comm… [truncated]"
```

Cut hits mid-paragraph in Iron Law #3, before the bounded-LHS pipes
clause. Iron Law #3 itself survives only the first paragraph; Law #4
onward, the librarian guide, the progressive-disclosure tips, the
anti-patterns table — all dead.

## Evidence — per-tool `description` field truncated at ~2 KB

To verify whether per-tool descriptions share the same cap, codescout
ships a debug-only `ProbeTool` (gated on `CODESCOUT_PROBE=1`) whose
`description` field is a generated 8,820-byte string containing
`SENTINEL_NNNN_XX` markers at known byte offsets (200, 500, 1000,
1500, 2000, 2500, 3000, 4000, 5000, 6000, 8000, plus
`SENTINEL_END_C0FFEE` at the tail).

Source: `src/tools/probe.rs`.
Registration: `src/server.rs::from_parts` (gated).

### Subagent recital (2026-05-19, after `/mcp` restart with probe)

Spawned a `general-purpose` subagent and asked it to read (not call)
the probe tool's description and list every visible sentinel.

```text
SENTINEL_0200_AA   ✓ visible
SENTINEL_0500_BB   ✓ visible
SENTINEL_1000_CC   ✓ visible
SENTINEL_1500_DD   ✓ visible
SENTINEL_2000_EE   ✓ visible
SENTINEL_2500_FF   ✗ not visible
SENTINEL_3000_GG   ✗ not visible
…
SENTINEL_END_C0FFEE ✗ not visible

Tail observed: "filler filler filler… [truncated]"
```

The cut is in the same place (≈2 KB) and uses the same explicit
marker. **Per-tool `description` and server-level `instructions` share
the same per-block ~2 KB ceiling.**

## Evidence — MCP resources not exposed to model

Codescout already serves MCP resources (`doc://progressive-disclosure`,
`doc://librarian-guide`, etc.) — these emit full content from
`build_resource_registry` at `src/server.rs:771-848`.

The probe extends this with two debug resources (gated):

- `probe://description-test` — 8,997-byte descriptor `description`
- `probe://body-test` — 19,996-byte body via `read`

Source: `src/mcp_resources/probe.rs`.

### Subagent attempt (same restart, post-probe)

Subagent reported:

> *"My available toolset (codescout MCP, researcher MCP, and a few
> auth/monitor tools) does not include any MCP resource-listing or
> resource-reading capability. No `ReadMcpResource` tool is exposed.
> `mcp__codescout__read_file` rejects `probe://...` URIs as filesystem
> paths."*

Main-agent tool list (from this session's tool-listing): no
`ReadMcpResource`, no `mcp__codescout__resources/*`, no analog.

**Resources reach the client intact but the client does not expose
them as model-callable tools. Resources are not a delivery channel for
content the model can autonomously read.**

## Evidence — tool call results respect the documented cap

Claude Code documents `MAX_MCP_OUTPUT_TOKENS` (default 25,000 tokens
≈ 100 KB) as the budget for a single tool call's result.

Codescout's own output discipline is well under that:

| Constant | Value | Source |
|---|---|---|
| `MAX_INLINE_TOKENS` | 2,500 | `src/tools/core/types.rs:18` |
| `TOOL_OUTPUT_BUFFER_THRESHOLD` (bytes) | 10,000 | `src/tools/core/types.rs:22` |
| `INLINE_BYTE_BUDGET` | 9,000 | `src/tools/core/types.rs:27` |
| `COMPACT_SUMMARY_MAX_BYTES` | 2,000 | `src/tools/core/types.rs:49` |
| `COMPACT_SUMMARY_HARD_MAX_BYTES` | 3,000 | `src/tools/core/types.rs:51` |

Above `MAX_INLINE_TOKENS`, content is stored in the `@tool_*` buffer
and a compact summary is returned. The buffered handle is then queried
in follow-up calls. The buffer system is the only model-autonomous
channel with `>2 KB` capacity, and it is already in production use.

## Channel inventory — final

| Channel | Model can read? | Cap (bytes) | Cap (tokens) |
|---|---|---|---|
| MCP `initialize.instructions` | yes (auto-injected) | ~2,000 | ~500 |
| Per-tool `description` | yes (auto-injected) | ~2,000 | ~500 |
| MCP **resources** | **no** (not exposed) | n/a | n/a |
| MCP **prompts** | no (user-triggered only) | n/a | n/a |
| Tool call **results** | yes (per call) | ~100,000 | 25,000 |
| Codescout `MAX_INLINE_TOKENS` | (self-imposed under above) | ~10,000 | 2,500 |

## Tracker audit (does this change behaviour?)

`docs/trackers/skill-frictions.md` (F-001 → F-010, 10 entries): all
entries scope project skills (`/claude-traces`, `/analyze-usage`,
`/onboarding`, recon). None reference codescout MCP server
instructions. Truncation is not the cause and not the cure.

`docs/trackers/tool-usage-patterns.md` (T-001 → T-010, librarian
artifact `b3fa993849ac83ab`): the four anti-pattern entries (T-005
`npm run build | grep` × 7, T-006 `cat file | head -50`, T-008
`edit_file` drift, T-009 onboarding gate, T-010 schema-migration bug)
all involve rules the model **did see** (Iron Laws 1–3 survive
truncation) and ignored anyway, or live on a different surface
(`onboarding_prompt.md`, tool description example).

**Zero observed frictions trace to a rule lost in truncation.** The
~40 KB of cut content was not load-bearing in observed sessions.
The truncation has been silently doing a cut we should have done
ourselves.

## Implications

1. The runtime `librarian::INSTRUCTIONS` concat at
   `src/server.rs::from_parts` always lands in the cut zone.
   The librarian guide never reaches the model via this channel.
2. The `prompt_surfaces_reference_only_real_tools` invariant in
   `CLAUDE.md` defends a phantom contract for ~95 % of the file —
   the model only sees the first ~500 tokens.
3. Per-tool `description` is not the escape hatch the diagram
   suggested. The 300-char self-imposed cap noted in the `Tool` trait
   docs (`fn description() -> &str`, "capped at 300 chars") was
   conservative *and* approximately right — the platform itself
   enforces a cap an order of magnitude higher, but still well below
   the prose-explanation thresholds we were assuming.
4. The `long_docs()` field on the `Tool` trait, exposed via
   `doc://codescout-tool-guide`, is **never read by the model**
   because resources are not model-accessible in this client. Every
   tool that wrote `long_docs()` was writing to a dead channel.
5. The only channel for `>2 KB` autonomous content is **tool call
   results**, mediated by codescout's `@tool_*` buffer system.
   "Get-on-demand" patterns must be tools, not resources.

## Open Decision (next step)

Convert this evidence base into a binding ADR with two specialists'
input already on record (see `Self-Trap #5` retraction in session
transcript):

- **Hamsa (prompt):** cut the dead content; tighten the surviving
  ~1,800 chars of `source.md` with paired do-instead clauses on every
  Iron Law (Heuristic #1).
- **Snow Lion (architecture):** sever the `librarian::INSTRUCTIONS`
  concat — it is a cross-crate trunk-reach into a channel that does
  not reach the model. Replace with per-tool description rules where
  they fit (≤ ~2 KB per tool) and a `get_guide(topic)` *tool* (not
  resource) for content that exceeds that budget.

This document does not yet make those decisions. It states the
constraints they must respect.

## Probe artifacts

Both probe sources are committed but registered only when
`CODESCOUT_PROBE=1` is set in the codescout server env:

- `src/tools/probe.rs` — `ProbeTool` with 8,820-byte sentinel-laden
  description.
- `src/mcp_resources/probe.rs` — `ProbeProvider` exposing
  `probe://description-test` (8,997-byte description) and
  `probe://body-test` (19,996-byte body).

Registration sites (both gated on env):

- `src/server.rs::from_parts` (tool side)
- `src/server.rs::build_resource_registry` (resource side)

To re-run the measurement, set `CODESCOUT_PROBE=1` in the codescout
entry of `~/.claude-kat/.claude.json` (or equivalent), restart the
client (`/mcp`), and ask any subagent to list the sentinels it sees
in `__probe_description_cap__`'s description.

## Proxy debug aid

The local `llm-proxy` strips tool definitions to a `tool_names` list
before logging to Langfuse (`/home/marius/agents/llm-proxy/src/passthrough.rs::build_langfuse_input`).
For this investigation the function was patched to optionally emit a
`tools_full` array (full schemas) and a `tools_digest` per-tool
record (`name`, `description_len`, `description_head`,
`description_tail`, `input_schema_len`, `truncation_marker`) when
`LOG_FULL_TOOLS=1` or `LOG_TOOL_DIGEST=1` is set in the proxy's
`EnvironmentFile`. Both flags are currently on. Remove them and
restart the service when the investigation closes.

## References

- Trace: Langfuse `d19a4827-6871-4351-af9a-15fc71971ca7`
- Session transcript (2026-05-19): truncation discovery →
  proxy/probe instrumentation → cap measurement.
- `CLAUDE.md` — "Prompt Surface Consistency" section (defends an
  assumption this finding invalidates).
- `docs/trackers/skill-frictions.md` — audited, no truncation-caused
  entries.
- `docs/trackers/tool-usage-patterns.md` — audited, no
  truncation-caused entries (artifact id `b3fa993849ac83ab`).
- `src/tools/core/types.rs:18` — `MAX_INLINE_TOKENS = 2_500`
- `src/server.rs::from_parts` — instructions composition site.
- `src/server.rs::build_resource_registry` — resource composition site.

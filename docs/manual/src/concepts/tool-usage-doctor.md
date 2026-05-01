# Tool Usage Doctor
The `doctor://tool-usage` MCP resource surfaces per-tool call statistics and
prune candidates, so codescout maintainers can quantify what the current
token-diet actually bought us and identify rarely-used tools for the next
prompt-surface review.

## Background

From `docs/trackers/mcp-integration-ideas-2026-04.md` idea #7: every tool
description is re-sent to the LLM on every turn. Tools that almost never get
called are pure context-window tax. Before pruning a tool we want to verify —
with data — that it's actually unused across sessions.

## Reading the resource

Any MCP client can read the resource. Example over HTTP:

```bash
curl -s -X POST http://127.0.0.1:PORT/mcp \
  -H "Authorization: Bearer $TOKEN" \
  -H "mcp-session-id: $SESSION" \
  -d '{"jsonrpc":"2.0","id":1,"method":"resources/read",
       "params":{"uri":"doctor://tool-usage"}}'
```

## Response shape

```json
{
  "window": "30d",
  "low_call_threshold": 5,
  "total_calls": 4217,
  "tools": [
    {
      "name": "find_symbol",
      "calls": 1830,
      "errors": 4,
      "overflows": 12,
      "error_rate_pct": 0.22,
      "overflow_rate_pct": 0.66,
      "p50_ms": 18,
      "p99_ms": 210
    }
  ],
  "prune_candidates": ["symbol_at", "git_blame"],
  "unused_tools": ["register_library"]
}
```

### Fields

| Field | Meaning |
|-------|---------|
| `window` | Time window analysed (default `30d`). |
| `low_call_threshold` | Tools called fewer than this many times are flagged as `prune_candidates`. |
| `total_calls` | Sum of calls across all tools in the window. |
| `tools` | Per-tool stats from `usage.db`, ordered by call count descending. |
| `prune_candidates` | Known tools with 1 ≤ calls < `low_call_threshold`. |
| `unused_tools` | Currently-registered tools that were **never** called in the window. |

## Behaviour

- If `usage.db` doesn't exist for the active project (fresh install), all
  counts are zero and every registered tool appears in `unused_tools`.
- The window is currently fixed at `30d`. Future work: accept a query
  parameter to change it.
- `prune_candidates` uses a strict `<` comparison, so a tool called exactly
  `low_call_threshold` times is not flagged.
- Sorting: `tools` mirrors usage-DB ordering (descending calls).
  `unused_tools` is sorted alphabetically for deterministic output.

## Typical workflow

1. Review the report monthly.
2. For every name in `unused_tools`: check if any known MCP client would ever
   plausibly use it. If not, consider unregistering.
3. For every name in `prune_candidates`: look at the `p99_ms` and
   `error_rate_pct`. Expensive + rarely-useful tools are the highest-value
   pruning targets.
4. When pruning a tool, update all three prompt surfaces (see
   `CLAUDE.md § Prompt Surface Consistency`) in the same commit.

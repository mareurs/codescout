# Tool Usage Monitor — Design

> **ORA-7** | Primary driver: debug agent behavior (tool selection quality, overflow patterns, error rates)

## Overview

A lightweight SQLite-backed recorder that wraps the central `call_tool` dispatch loop in `server.rs`. Every tool invocation is timed, classified, and written to `.code-explorer/usage.db`. A new `get_usage_stats` MCP tool surfaces the data as structured JSON.

---

## Architecture

### Module layout

```
src/usage/
    mod.rs     — UsageRecorder struct and record() method
    db.rs      — SQLite schema, open_db, write_record, query_stats
src/tools/
    usage.rs   — get_usage_stats tool implementation
```

**Modified files:**
- `src/server.rs` — construct `UsageRecorder`, one-line change in `call_tool`
- `src/tools/mod.rs` — register `GetUsageStats` tool

### Storage

`.code-explorer/usage.db` — same directory and pattern as `embeddings.db`:
- WAL mode
- `open_db()` creates tables on first call, safe to call per-request
- No shared connection — opened per-call, consistent with existing codebase pattern

---

## Data Model

```sql
CREATE TABLE IF NOT EXISTS tool_calls (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_name  TEXT NOT NULL,
    called_at  TEXT NOT NULL,   -- ISO 8601 UTC
    latency_ms INTEGER NOT NULL,
    outcome    TEXT NOT NULL,   -- "success" | "recoverable_error" | "error"
    overflowed INTEGER NOT NULL DEFAULT 0,  -- 1 if result JSON contained "overflow" key
    error_msg  TEXT             -- NULL on success
);
```

**Retention:** enforced on every write in the same transaction as the insert:
```sql
DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days');
```
No background scheduler needed; table stays small automatically.

---

## `UsageRecorder` API

```rust
pub struct UsageRecorder {
    agent: Agent,
}

impl UsageRecorder {
    pub fn new(agent: Agent) -> Self;

    /// Wraps a tool call: times it, classifies the result, writes to DB.
    /// Recording is best-effort — a DB failure never fails the tool call.
    pub async fn record<F, Fut>(&self, tool_name: &str, f: F) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Value>>;
}
```

### Outcome classification

Determined from the `Result<Value>` returned by `tool.call()`, before `route_tool_error`:

| Result | `outcome` | `overflowed` | `error_msg` |
|--------|-----------|--------------|-------------|
| `Err(e)` | `"error"` | 0 | `e.to_string()` |
| `Ok(v)` where `v["error"].is_string()` | `"recoverable_error"` | 0 | `v["error"]` |
| `Ok(v)` where `v["overflow"].is_object()` | `"success"` | 1 | NULL |
| `Ok(v)` otherwise | `"success"` | 0 | NULL |

### Integration in `call_tool`

`UsageRecorder` is constructed at the top of `call_tool` alongside `ToolContext`:

```rust
let recorder = UsageRecorder::new(self.agent.clone());

// one-line change — timeout wrapper stays unchanged around this:
let result = recorder.record(&req.name, || tool.call(input, &ctx)).await;
```

---

## `get_usage_stats` Tool

**Input schema:**
```json
{
  "window": {
    "type": "string",
    "enum": ["1h", "24h", "7d", "30d"],
    "default": "30d",
    "description": "Time window for aggregation"
  }
}
```

**Output:**
```json
{
  "window": "30d",
  "total_calls": 312,
  "by_tool": [
    {
      "tool": "semantic_search",
      "calls": 47,
      "errors": 3,
      "error_rate_pct": 6.4,
      "overflows": 12,
      "overflow_rate_pct": 25.5,
      "p50_ms": 320,
      "p99_ms": 1840
    }
  ]
}
```

- Sorted by `calls` descending — highest-traffic tools first
- Tools with zero calls in the window are omitted
- No active project → `RecoverableError` with hint to run `activate_project`
- Empty DB → `{ "window": "30d", "total_calls": 0, "by_tool": [] }`

**Percentile query:** SQLite has no native percentile function. Use a subquery per tool:
```sql
SELECT latency_ms FROM tool_calls
WHERE tool_name = ? AND called_at >= ?
ORDER BY latency_ms
LIMIT 1 OFFSET (SELECT COUNT(*) FROM tool_calls WHERE tool_name = ? AND called_at >= ?) * 50 / 100
```
Row counts are low enough that this is fast.

---

## Error Handling

Recording is **always best-effort**:
- No active project → skip silently
- DB open fails → skip silently
- Write fails → skip silently
- The original `Result<Value>` is always returned unchanged

This ensures a broken or missing `usage.db` never affects tool correctness.

---

## Testing

Unit tests in `src/usage/db.rs` using `tempdir` (same pattern as `embed/index.rs`):

- Insert a row, query it back — roundtrip
- Retention: insert a row with `called_at` 31 days ago, verify it's pruned on next insert
- Outcome classification: all four cases
- Window filtering: rows outside window excluded from stats
- Empty state: `get_usage_stats` returns zero totals with no rows
- Percentile: verify p50/p99 with a known set of latencies

---

## What This Unblocks

- **ORA-6 Project Dashboard** (Phase 2): tool call counts + error log from `usage.db`
- **ORA-11 Contributor Skills**: `log-stat-analyzer` skill reads `get_usage_stats` output

---

*Created: 2026-02-28*

# Usage Traceability & Debug Mode

**Date:** 2026-04-02
**Status:** Design
**Branch:** `experiments`

## Problem

`usage.db` records tool call metrics (latency, outcome, overflow) but lacks the context
needed for post-mortem debugging. When a tool call fails, we can't answer: what were the
inputs? What version of codescout was running? What was the project state? This makes it
impossible to reproduce and fix the failure.

## Goals

1. **Reproducible post-mortems** — given a failed tool call, be able to checkout both
   codescout and the project at the exact state, re-invoke the tool with the same inputs,
   and observe the same failure.
2. **Zero overhead in normal mode** — new tracing columns (SHAs, session ID) are always
   populated but cheap. Verbose data (input/output JSON) only recorded in debug mode.
3. **Consolidate `--diagnostic` into `--debug`** — one flag for all verbose/debug behavior.

## Design

### Schema Changes

New nullable columns on the existing `tool_calls` table:

```sql
ALTER TABLE tool_calls ADD COLUMN codescout_sha TEXT;
ALTER TABLE tool_calls ADD COLUMN project_sha TEXT;
ALTER TABLE tool_calls ADD COLUMN session_id TEXT;
ALTER TABLE tool_calls ADD COLUMN input_json TEXT;
ALTER TABLE tool_calls ADD COLUMN output_json TEXT;
```

| Column | When populated | Source |
|--------|---------------|--------|
| `codescout_sha` | Always | Compile-time via `build.rs` → `env!("CODESCOUT_GIT_SHA")` |
| `project_sha` | Always (None for non-git projects) | `git rev-parse --short HEAD` at project activation, cached on `ActiveProject` |
| `session_id` | Always | UUID generated once at server start |
| `input_json` | Debug mode only | `serde_json::to_string(&input)` of the tool's input `Value` |
| `output_json` | Debug mode + error/recoverable_error only | Full response JSON |

Migration happens in `open_db()`. SQLite `ALTER TABLE ADD COLUMN` is safe — no table
rewrite, existing rows get NULL for new columns.

### `--debug` Flag

Rename `--diagnostic` to `--debug` on the `Start` command. `--diagnostic` becomes a
hidden deprecated alias.

**What `--debug` enables:**
1. Diagnostic text logs (existing behavior) — `.codescout/diagnostic-*.log` with lifecycle
   + tool events, rotation at 6 files
2. Verbose usage recording (new) — `input_json` and `output_json` columns populated

**Flag propagation:**
- Parsed in `main.rs`, passed to `CodeScoutServer::from_parts()`
- Server stores as `debug: bool` (renaming existing `diagnostic` field)
- `UsageRecorder` accesses via `Agent` (which holds server config)
- `record_content()` checks the flag to decide whether to serialize inputs/outputs

### `codescout_sha` via `build.rs`

Add a `build.rs` that runs `git rev-parse --short HEAD` and sets `CODESCOUT_GIT_SHA`
as a compile-time env var. Fallback to `"unknown"` if not in a git repo (e.g., crates.io
install). Accessed via `env!("CODESCOUT_GIT_SHA")` — zero runtime cost.

### Project SHA Caching

- On `activate_project`, run `git rev-parse --short HEAD` in the project root
- Cache as `Option<String>` on `ActiveProject` (None for non-git projects)
- Reuse for all tool calls — no per-call git overhead
- No invalidation: server sessions are short-lived; if HEAD moves mid-session, the SHA
  at call time is still a valid reproduction point

### Updated `write_record` Signature

```rust
pub fn write_record(
    conn: &Connection,
    tool_name: &str,
    latency_ms: i64,
    outcome: &str,
    overflowed: bool,
    error_msg: Option<&str>,
    codescout_sha: &str,
    project_sha: Option<&str>,
    session_id: &str,
    input_json: Option<&str>,
    output_json: Option<&str>,
) -> Result<()>
```

### Updated `record_content` Flow

1. Start timer
2. Execute the tool call
3. Measure latency
4. Classify result (existing `classify_content_result` logic)
5. If debug mode: serialize `input` to JSON string
6. If debug mode AND outcome is error/recoverable_error: serialize response to JSON string
7. Call `write_record` with all fields

### Retention

Same 30-day pruning as today. Debug payloads are pruned alongside normal records in the
existing `DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')` statement.

## Replay Workflow

Given a failed call:

```sql
SELECT id, tool_name, input_json, output_json, codescout_sha, project_sha, called_at
FROM tool_calls
WHERE outcome != 'success' AND input_json IS NOT NULL
ORDER BY called_at DESC LIMIT 10;
```

To reproduce:
1. `git checkout <project_sha>` — restore project state
2. `git checkout <codescout_sha>` + `cargo build --release` — restore codescout version
3. Start codescout, activate the project
4. Re-invoke the tool with the stored `input_json`

A future `codescout replay <call_id>` CLI command could automate step 4, but is out of
scope for this design.

## Files to Change

| File | Change |
|------|--------|
| `build.rs` | New file — `git rev-parse --short HEAD` → `CODESCOUT_GIT_SHA` |
| `src/main.rs` | Rename `--diagnostic` to `--debug`, add deprecated alias |
| `src/usage/db.rs` | `open_db()` migration + `write_record()` new params |
| `src/usage/mod.rs` | `UsageRecorder` gains debug flag, `record_content`/`write_content` pass new fields |
| `src/agent.rs` | `ActiveProject` gets `head_sha: Option<String>`, populated on activation |
| `src/server.rs` | `session_id: String` field (UUID), rename `diagnostic` → `debug` |
| `src/logging.rs` | Update references from `diagnostic` to `debug` |
| Tests in `src/usage/db.rs` | New tests for migration, new columns, debug-mode recording |

## Out of Scope

- `codescout replay` CLI command (future)
- Dashboard/`GetUsageStats` tool extensions for debug records
- `lsp_events` table changes (no traceability need identified)

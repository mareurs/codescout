---
name: analyze-usage
description: Use when asked to analyze codescout tool usage, check for error patterns, audit tool health, generate a usage report, spot anti-patterns, or clear usage statistics to start fresh.
---

# analyze-usage

## Overview

Scan all codescout `usage.db` databases across workspace projects, run a fixed set of SQL
queries, and produce a comprehensive markdown report covering: tool popularity, error
patterns, latency distribution, overflow behavior, and session summaries. Saves report to
file and prints a compact inline summary.

## When to Use

- User invokes `/analyze-usage` → run the full analysis report
- User invokes `/analyze-usage clear` → clear all projects' usage data
- User invokes `/analyze-usage clear <project>` → clear one project's usage data
- User asks for a tool usage health check, error audit, or anti-pattern review
- User wants an actionable improvement list for codescout tools
- User wants to reset statistics before a new measurement period
## When NOT to Use

- Single ad-hoc query — just run `sqlite3` directly
- Real-time monitoring — this is a point-in-time snapshot

## Steps

### 1. Discover DBs

```bash
find ~/work -path "*/.codescout/usage.db"
```

`~/work` is the standard project root on this machine. Adjust if projects live elsewhere.

Optional: if a project name or path was given as argument, filter results to that path only.

**If no DBs found:** stop and report: "No usage.db files found under ~/work. Check that codescout projects have been activated at least once."

After running query A for each DB, skip projects with fewer than 20 calls — include them only as a row in a summary table at the end of the report rather than with full per-project sections.

Note: `find` may return nested DBs (e.g. `codescout/crates/librarian-mcp/.codescout/usage.db`). Include these as separate projects — each sub-project DB tracks its own activation context.

Store the list of DB paths — loop over each one in Steps 2–4.

### 2. For Each DB — Run SQL Queries (8 total)

Run each query below via `run_command`. Replace `<db>` with the full DB path.

Invoke pattern:

```bash
sqlite3 -line <db> "SELECT ..."
```

The `-line` flag formats output as `column = value` pairs — readable in markdown reports. Replace `SELECT ...` with the full query text for each section.

**A. Overview**
```sql
SELECT COUNT(*) as total_calls,
       ROUND(100.0 * SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) / NULLIF(COUNT(*), 0), 1) as error_pct,
       MIN(DATE(called_at)) as from_date,
       MAX(DATE(called_at)) as to_date,
       COUNT(DISTINCT session_id) as sessions
FROM tool_calls;
```

> If `total_calls` is 0, skip this project entirely — do not include it in the report.

**B. Tool popularity**
```sql
SELECT tool_name, COUNT(*) as calls,
       ROUND(AVG(latency_ms)) as avg_ms,
       MAX(latency_ms) as max_ms,
       SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) as errors,
       SUM(overflowed) as overflows
FROM tool_calls
GROUP BY tool_name ORDER BY calls DESC;
```

**C. Error breakdown** (top 20)
```sql
SELECT tool_name, error_msg, COUNT(*) as n
FROM tool_calls WHERE outcome='error' AND error_msg IS NOT NULL
GROUP BY tool_name, error_msg ORDER BY n DESC LIMIT 20;
```

**D. Overflow tools**
```sql
SELECT tool_name, COUNT(*) as overflow_calls
FROM tool_calls WHERE overflowed=1
GROUP BY tool_name ORDER BY overflow_calls DESC;
```

**E. Latency buckets** (success calls only)
```sql
SELECT tool_name,
       COUNT(*) as total,
       SUM(CASE WHEN latency_ms < 100 THEN 1 ELSE 0 END) as lt100ms,
       SUM(CASE WHEN latency_ms BETWEEN 100 AND 999 THEN 1 ELSE 0 END) as lt1s,
       SUM(CASE WHEN latency_ms BETWEEN 1000 AND 9999 THEN 1 ELSE 0 END) as lt10s,
       SUM(CASE WHEN latency_ms >= 10000 THEN 1 ELSE 0 END) as gt10s
FROM tool_calls WHERE outcome='success'
GROUP BY tool_name
ORDER BY SUM(CASE WHEN latency_ms >= 10000 THEN 1 ELSE 0 END) DESC;
```

**F. Slow run_command calls** (>10s)
```sql
SELECT latency_ms, json_extract(input_json,'$.command') as command
FROM tool_calls WHERE tool_name='run_command' AND latency_ms > 10000
ORDER BY latency_ms DESC LIMIT 15;
```

**G. Session summary** (top 15)
```sql
SELECT session_id, COUNT(*) as total_calls,
       SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) as errors,
       GROUP_CONCAT(DISTINCT tool_name) as tools_used
FROM tool_calls WHERE session_id IS NOT NULL
GROUP BY session_id ORDER BY total_calls DESC LIMIT 15;
```

**H. LSP events**
```sql
SELECT language, reason, COUNT(*) as starts,
       ROUND(AVG(handshake_ms)) as avg_handshake_ms,
       MAX(handshake_ms) as max_handshake_ms,
       ROUND(AVG(first_response_ms)) as avg_first_response_ms
FROM lsp_events GROUP BY language, reason ORDER BY avg_handshake_ms DESC;
```

### 3. Cross-Project Aggregation (in-context)

After collecting all per-DB results, compute in-context:
- **Total calls** = sum of all projects' `total_calls`
- **Combined error rate** = total errors / total calls × 100
- **Worst project by error rate** = highest `error_pct`
- **Worst project by latency** = highest `max_ms` across any tool
- **Systemic errors** = error messages that appear in >1 project

No additional SQL needed — data is already in context.

### 4. Build Markdown Report

Structure:

```
# Usage Analysis — YYYY-MM-DD

## Cross-Project Summary

Projects scanned: N
Total calls: X | Error rate: Y% | Sessions: Z

### Top Issues
[ranked list — prefix each with [project-name], ordered: high error counts > overflows > latency > LSP]

---

## Project: <project-name>

**DB:** `<db path>`

### Overview
<output of query A>

### Tool Popularity
<output of query B>

### Error Breakdown
<output of query C>

### Overflow Tools
<output of query D — OMIT this section if query returns no rows>

### Latency Buckets
<output of query E>

### Slow Commands
<output of query F — OMIT this section if query returns no rows>

### Session Summary
<output of query G>

### LSP Events
<output of query H — OMIT this section if table is empty>
```

Repeat the `## Project:` block for each DB. Cross-project summary **always at the top**.

### 5. Save Report

```
path: docs/usage-reports/YYYY-MM-DD-usage-analysis.md
```

Use `create_file` with `overwrite: true`. If `docs/usage-reports/` doesn't exist, it will
be created. Path is relative to the codescout project root — use absolute path if
running from a different project.

### 6. Print Inline Summary

After saving, output exactly this in the conversation:

```
## Usage Analysis — YYYY-MM-DD

Projects scanned: N (<db1 project name>, <db2 project name>, ...)
Total calls: X | Error rate: Y% | Sessions: Z

### Top Issues
1. [project] <issue>
2. [project] <issue>
...

Full report: docs/usage-reports/YYYY-MM-DD-usage-analysis.md
```

Limit top issues to 4–6, ordered by severity.



## Step 7: Session Drill-Down (when warranted)

usage.db tracks codescout tool calls only. For sessions that look anomalous in query G
(high call count, high errors, many overflows), drill into the full picture using
**claude-traces** (`cc.py` + `lf.py`).

**Trigger criteria — drill down on any session where:**
- `total_calls > 50`
- `errors / total_calls > 10%`
- Session appears in query D results (many overflows)

**Step 7a — Locate the session's JSONL**

Query G returns `session_id` values. Before using `--project`, verify the path:

```bash
# Locate actual JSONL file (avoids path-decode ambiguity)
find ~/.claude/projects -name "<session_id_prefix>*" 2>/dev/null
# The parent directory name encodes the project path: -home-user-work-project-name
# Decode: replace leading -  with /, then each - with / — BUT directory names with dashes are ambiguous.
# Always verify: ls /decoded/path 2>/dev/null || echo "path invalid"
```

**Step 7b — Get actual tool sequence**

```bash
python3 <skill_dir>/scripts/cc.py tool-calls <session_id> --project <verified_path>
```

Returns every tool call in order with input preview. This is the ground truth for what
the model actually did — codescout tools AND native CC tools (Skill, Agent, TaskCreate, etc.).

**Step 7c — Get cost + stop reasons**

```bash
# Langfuse keys live at ~/agents/llm-proxy/.env
# If lf.py reports missing keys, set them explicitly:
LANGFUSE_PUBLIC_KEY=<key> LANGFUSE_SECRET_KEY=<key> LANGFUSE_BASE_URL=http://localhost:3000 \
  python3 <skill_dir>/scripts/lf.py session <session_id>
```

Returns: API call count, time range, cost, stop reasons. A `max_tokens` stop reason
indicates context overflow — flag in report.

**Step 7d — Synthesize**

In the report, add a `### Session Drill-Down` subsection per drilled session:

```
### Session Drill-Down: <session_id_prefix>

**Cost:** $X.XX | **API calls:** N | **Stop reasons:** {tool_use: X, end_turn: Y, max_tokens: Z ⚠}

**Tool sequence summary:** [N codescout tools, M native CC tools]
Top tools: symbols ×N, grep ×N, edit_code ×N

**Findings:**
- [violation or anti-pattern observed]
- [efficiency note]
```

Flag `max_tokens` stops and sessions with >20% native-CC-to-codescout ratio (may indicate
the model is using native tools where codescout tools belong).

**Note:** `lf.py trace <id>` shows "Tools (N)" which is the full available schema, NOT
which tools were actually called. For actual call sequence always use `cc.py tool-calls`.

## Clear Mode

Invoked when the user passes `clear` as the argument. Resets usage data so future analysis starts fresh.

### 1. Discover DBs (same as analysis mode)

```bash
find ~/work -path "*/.codescout/usage.db"
```

If a project name or path was given after `clear` (e.g. `/analyze-usage clear codescout`), filter to that project only.

### 2. Show what will be cleared

List the DB paths that will be cleared, including their current `total_calls` count:

```bash
sqlite3 <db> "SELECT COUNT(*) FROM tool_calls;"
```

Print to the conversation:

```
About to clear usage data from N project(s):
- codescout       (5,050 calls)
- backend-kotlin (1,745 calls)
...

This cannot be undone. Confirm? (yes/no)
```

**Wait for explicit confirmation before proceeding.** If the user says anything other than "yes" / "y", abort and report "Cancelled."

### 3. Clear each DB

For each confirmed DB, run:

```bash
sqlite3 <db> "DELETE FROM tool_calls; DELETE FROM lsp_events; DELETE FROM call_edges; VACUUM;"
```

`VACUUM` reclaims disk space after the deletions. The schema is preserved — codescout does not need to recreate the file.

### 4. Report

Print:

```
Cleared N project(s):
- codescout       — 5,050 calls removed
- backend-kotlin — 1,745 calls removed
...
Total removed: X calls
```

## Common Mistakes

- **Using `find .`** — always `find ~/work`. CWD may not contain all projects.
- **Wrong path pattern** — must be `*/.codescout/usage.db`, not just `-name "usage.db"`.
- **Skipping LSP events** — `lsp_events` has its own schema; separate query required.
- **Cross-project summary at the bottom** — it always goes at the TOP.
- **Not passing `overwrite: true`** to `create_file` on re-run same day.
- **Printing empty sections** — if D/F/H return no rows, omit those sections entirely.
- **Clearing without confirmation** — always show what will be removed and wait for explicit "yes" before running DELETE.
- **Deleting the .db file** — use `DELETE + VACUUM`, not `rm`. Deleting the file works but forces codescout to recreate it on next activation; DELETE preserves the schema cleanly.
- **Forgetting `call_edges`** — three tables need clearing: `tool_calls`, `lsp_events`, `call_edges`.
- **Trusting `--all` project paths** — cc.py decodes project paths by replacing `-` with `/`, but directory names containing `-` (e.g. `backend-kotlin`) become `backend/kotlin`. Always verify the reconstructed path exists before using it in `--project` flags: `ls <path> 2>/dev/null || echo "path invalid"`. If invalid, use `find ~/.claude/projects -name "<session_prefix>*"` to find the real encoded path.
- **Stopping at usage.db for session analysis** — usage.db tracks codescout tool calls only. For the full picture (token cost, stop reasons, actual tool sequence including native CC tools), see Step 7.

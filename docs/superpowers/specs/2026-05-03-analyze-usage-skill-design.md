# analyze-usage Skill Design

**Date:** 2026-05-03
**Skill name:** `codescout-companion:analyze-usage`
**Location:** `../claude-plugins/codescout-companion/skills/analyze-usage/SKILL.md`

---

## Purpose

On-demand skill that scans all `usage.db` files across workspace projects, runs a fixed set
of SQL queries, and produces a comprehensive markdown report of tool usage patterns, errors,
latency, and session behavior.

Primary use: health checks, spotting anti-patterns, building a bug/improvement backlog for
code-explorer.

---

## Trigger

Slash command: `/analyze-usage`

- No args → scan all projects
- Optional arg: project name or path → scope to that single DB

---

## DB Discovery

```bash
find ~/work -path "*/.codescout/usage.db"
```

Targets only the canonical `.codescout/usage.db` location. Runs from anywhere — not
dependent on current working directory.

---

## Report Sections (per project)

Each DB produces these sections in the report:

| Section | Content |
|---------|---------|
| **Overview** | Total calls, error rate %, date range, unique sessions |
| **Tool popularity** | calls + avg_ms + error count per tool, DESC by calls |
| **Error breakdown** | `error_msg` grouped by tool + message, top 20 |
| **Overflow tools** | tools where `overflowed=1`, count |
| **Latency buckets** | <100ms / <1s / <10s / >10s per tool |
| **Slow commands** | `run_command` calls with `latency_ms > 10000`, input preview |
| **Session summary** | calls + errors + tools_used per session, top 15 |
| **LSP events** | handshake_ms avg/max per language, cold start count |

---

## Cross-Project Summary

Appears at the top of the report, after all DBs are scanned:

- Total calls across all projects
- Combined error rate
- Project with most errors / slowest tools
- Error types appearing in multiple projects (systemic vs local)

---

## Output

### Inline (conversation)

Compact summary: projects scanned, total calls, error rate, session count, top 4–6 issues
with `[project]` prefix. Path to full report at the end.

Example:
```
## Usage Analysis — 2026-05-03

Projects scanned: 3
Total calls: 5000 | Error rate: 4.1% | Sessions: 47

### Top Issues
1. [code-explorer] run_command blocked 39x — shell-on-source-files hook working but agent retries
2. [code-explorer] edit_file blocked 46x — structural edits on .rs files
3. [code-explorer] memory recall broken — 384 vs 768 dim mismatch
4. [code-explorer] grep overflow 19x — mega-pattern alternations

Full report: docs/usage-reports/2026-05-03-usage-analysis.md
```

### File

`docs/usage-reports/YYYY-MM-DD-usage-analysis.md`

Full markdown with all per-project sections and cross-project summary. If re-run on the
same day, overwrites the existing file. SQL queries included as fenced blocks for
reproducibility.

---

## Implementation Approach

**Pure SQL + shell loop (Approach A).** No external dependencies beyond `sqlite3` CLI.

The skill instructs Claude to:

1. Run `find ~/work -path "*/.codescout/usage.db"` to locate DBs
2. For each DB path, run `sqlite3 <path> "<query>"` via `run_command` for each section
3. Collect all output, build the markdown report string
4. Save to `docs/usage-reports/YYYY-MM-DD-usage-analysis.md` via `create_file`
5. Print inline summary

All SQL queries are embedded in the skill document. No helper scripts needed.

---

## SQL Query Set

Queries proven from the 2026-05-03 analysis session:

```sql
-- Overview
SELECT COUNT(*) as total_calls,
       ROUND(100.0 * SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) / COUNT(*), 1) as error_pct,
       MIN(DATE(called_at)) as from_date, MAX(DATE(called_at)) as to_date,
       COUNT(DISTINCT session_id) as sessions
FROM tool_calls;

-- Tool popularity
SELECT tool_name, COUNT(*) as calls,
       ROUND(AVG(latency_ms)) as avg_ms,
       MAX(latency_ms) as max_ms,
       SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) as errors,
       SUM(overflowed) as overflows
FROM tool_calls
GROUP BY tool_name ORDER BY calls DESC;

-- Error breakdown
SELECT tool_name, error_msg, COUNT(*) as n
FROM tool_calls WHERE outcome='error' AND error_msg IS NOT NULL
GROUP BY tool_name, error_msg ORDER BY n DESC LIMIT 20;

-- Overflow tools
SELECT tool_name, COUNT(*) as overflow_calls
FROM tool_calls WHERE overflowed=1
GROUP BY tool_name ORDER BY overflow_calls DESC;

-- Latency buckets
SELECT tool_name,
       COUNT(*) as total,
       SUM(CASE WHEN latency_ms < 100 THEN 1 ELSE 0 END) as lt100ms,
       SUM(CASE WHEN latency_ms BETWEEN 100 AND 999 THEN 1 ELSE 0 END) as lt1s,
       SUM(CASE WHEN latency_ms BETWEEN 1000 AND 9999 THEN 1 ELSE 0 END) as lt10s,
       SUM(CASE WHEN latency_ms >= 10000 THEN 1 ELSE 0 END) as gt10s
FROM tool_calls WHERE outcome='success'
GROUP BY tool_name ORDER BY (SUM(CASE WHEN latency_ms >= 10000 THEN 1 ELSE 0 END)) DESC;

-- Slow run_command calls
SELECT latency_ms, SUBSTR(input_json,1,200) as input
FROM tool_calls WHERE tool_name='run_command' AND latency_ms > 10000
ORDER BY latency_ms DESC LIMIT 15;

-- Session summary
SELECT session_id, COUNT(*) as total_calls,
       SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) as errors,
       GROUP_CONCAT(DISTINCT tool_name) as tools_used
FROM tool_calls WHERE session_id IS NOT NULL
GROUP BY session_id ORDER BY total_calls DESC LIMIT 15;

-- LSP events
SELECT language, reason, COUNT(*) as starts,
       ROUND(AVG(handshake_ms)) as avg_handshake_ms,
       MAX(handshake_ms) as max_handshake_ms,
       ROUND(AVG(first_response_ms)) as avg_first_response_ms
FROM lsp_events GROUP BY language, reason ORDER BY avg_handshake_ms DESC;
```

Cross-project aggregation is done by Claude in-context after collecting per-DB results:
accumulate totals, identify error messages that appear in >1 project, rank projects by
error rate and max latency. No SQL needed — the data is already in the context window.

---

## Out of Scope

- Trend detection over time (no baseline stored)
- Auto-creating GitHub issues or tracker entries
- Periodic/scheduled runs (use `/loop` separately if wanted)
- Filtering by date range (all-time only)

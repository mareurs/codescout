# analyze-usage Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `codescout-companion:analyze-usage` — a skill that discovers all workspace `usage.db` files, runs a fixed SQL query set, and produces a comprehensive markdown report plus inline summary.

**Architecture:** Pure SQL + shell. No external deps beyond `sqlite3` CLI. Skill is a single `SKILL.md` file in the companion plugin. Follows RED-GREEN-REFACTOR: write skill, run baseline test without skill (document gaps), run with skill (verify compliance), close loopholes.

**Tech Stack:** SQLite (`sqlite3` CLI), Markdown, Claude Code skill system

---

### Task 1: Write the SKILL.md

**Files:**
- Create: `.claude/skills/analyze-usage/SKILL.md`

- [ ] **Step 1: Create the skill file**

```bash
mkdir -p /home/marius/work/claude/code-explorer/.claude/skills/analyze-usage
```

Then create `SKILL.md` with this exact content:

````markdown
---
name: analyze-usage
description: Use when asked to analyze codescout tool usage, check for error patterns, audit tool health, generate a usage report, or spot anti-patterns across workspace projects.
---

# analyze-usage

## Overview

Scan all codescout `usage.db` databases across workspace projects, run a fixed set of SQL
queries, and produce a comprehensive markdown report covering: tool popularity, error
patterns, latency distribution, overflow behavior, and session summaries. Saves report to
file and prints a compact inline summary.

## When to Use

- User invokes `/analyze-usage`
- User asks for a tool usage health check, error audit, or anti-pattern review
- User wants an actionable improvement list for code-explorer tools

## When NOT to Use

- Single ad-hoc query — just run `sqlite3` directly
- Real-time monitoring — this is a point-in-time snapshot

## Steps

### 1. Discover DBs

```bash
find ~/work -path "*/.codescout/usage.db"
```

Optional: if a project name or path was given as argument, filter results to that path only.
Store the list of DB paths — loop over each one in Steps 2–4.

### 2. For Each DB — Run SQL Queries (8 total)

Run each query below via `run_command`. Replace `<db>` with the full DB path.

**A. Overview**
```sql
SELECT COUNT(*) as total_calls,
       ROUND(100.0 * SUM(CASE WHEN outcome='error' THEN 1 ELSE 0 END) / COUNT(*), 1) as error_pct,
       MIN(DATE(called_at)) as from_date,
       MAX(DATE(called_at)) as to_date,
       COUNT(DISTINCT session_id) as sessions
FROM tool_calls;
```

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
be created. Path is relative to the code-explorer project root — use absolute path if
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

## Common Mistakes

- **Using `find .`** — always `find ~/work`. CWD may not contain all projects.
- **Wrong path pattern** — must be `*/.codescout/usage.db`, not just `-name "usage.db"`.
- **Skipping LSP events** — `lsp_events` has its own schema; separate query required.
- **Cross-project summary at the bottom** — it always goes at the TOP.
- **Not passing `overwrite: true`** to `create_file` on re-run same day.
- **Printing empty sections** — if D/F/H return no rows, omit those sections entirely.
````

- [ ] **Step 2: Verify file exists**

```bash
ls /home/marius/work/claude/claude-plugins/codescout-companion/skills/analyze-usage/SKILL.md
```

Expected: file listed, no error.

- [ ] **Step 3: Verify SQL queries are intact**

```bash
grep -c "FROM tool_calls\|FROM lsp_events" /home/marius/work/claude/claude-plugins/codescout-companion/skills/analyze-usage/SKILL.md
```

Expected: `9` (8 queries hit `tool_calls` or `lsp_events`; query A hits both tables → 8 FROM clauses total across the 8 queries, plus 1 for lsp_events = 9).

- [ ] **Step 4: Commit the skill**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add skills/analyze-usage/SKILL.md
git commit -m "feat(skills): add analyze-usage skill"
```

---

### Task 2: Baseline Test (RED phase)

Run a subagent **without** invoking the skill. Document what it does naturally vs what the skill specifies.

**Files:** None created — observation only.

- [ ] **Step 1: Dispatch baseline subagent**

Spawn a general-purpose subagent with this prompt exactly:

```
Analyze codescout tool usage data across my workspace projects and tell me what patterns you see.

Context: codescout projects live under ~/work. Each project may have a .codescout/usage.db SQLite database.

Do NOT load any skills. Just do whatever seems natural to you.
```

- [ ] **Step 2: Record baseline behavior**

Watch for these specific gaps (these are the things the skill teaches):

| Behavior | Skill requires | Did subagent do it? |
|----------|---------------|---------------------|
| DB discovery | `find ~/work -path "*/.codescout/usage.db"` | |
| All 8 SQL queries | Queries A–H in order | |
| Cross-project aggregation | Systemic errors, worst project | |
| Save report to file | `docs/usage-reports/YYYY-MM-DD-usage-analysis.md` | |
| Inline summary format | Projects + totals + top issues + file path | |
| Empty section skipping | Omit D/F/H if no rows | |

Document which rows were "No" — these are the loopholes to close in Task 3.

- [ ] **Step 3: Verify RED is RED**

Confirm at least one of these gaps exists. If subagent did everything correctly without the skill, the skill may be unnecessary (unlikely — it would have to invent all 8 queries and the exact output format).

---

### Task 3: Verify with Skill (GREEN phase)

- [ ] **Step 1: Invoke the skill**

Run `/analyze-usage` in the main session (not a subagent). The skill should now be loaded from the companion plugin.

- [ ] **Step 2: Verify each step was followed**

Check against the same table from Task 2 Step 2. All rows should now be "Yes".

- [ ] **Step 3: Verify output file created**

```bash
ls /home/marius/work/claude/code-explorer/docs/usage-reports/
```

Expected: file named `YYYY-MM-DD-usage-analysis.md` with today's date.

- [ ] **Step 4: Spot-check file contents**

```bash
head -40 /home/marius/work/claude/code-explorer/docs/usage-reports/$(date +%Y-%m-%d)-usage-analysis.md
```

Expected: `# Usage Analysis — YYYY-MM-DD` header, `## Cross-Project Summary` near the top.

---

### Task 4: Refactor — Close Loopholes (REFACTOR phase)

- [ ] **Step 1: List gaps from Task 2**

For each "No" row from the baseline table, write one bullet:
- What the agent did instead
- Which exact wording in the skill would prevent it

- [ ] **Step 2: Update SKILL.md for each gap**

Add explicit counters to `## Common Mistakes` or inline in the relevant step. Example: if agent used `find .` instead of `find ~/work`, add:

```markdown
> **Never use `find .`** — the skill MUST use `find ~/work` regardless of current directory.
```

- [ ] **Step 3: Re-run verification**

Re-invoke `/analyze-usage`. Confirm all gaps from Task 2 are now closed.

- [ ] **Step 4: Commit refactored skill**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add skills/analyze-usage/SKILL.md
git commit -m "fix(skills): close loopholes in analyze-usage from baseline test"
```

If no loopholes found in Task 2, skip Steps 2–4 and commit a no-op note:

```bash
git commit --allow-empty -m "test(skills): analyze-usage baseline passed — no loopholes found"
```

---

### Task 5: Create `docs/usage-reports/` .gitkeep

The output directory should exist in git so the first report isn't a surprise.

- [ ] **Step 1: Create the directory and .gitkeep**

```bash
mkdir -p /home/marius/work/claude/code-explorer/docs/usage-reports
touch /home/marius/work/claude/code-explorer/docs/usage-reports/.gitkeep
```

- [ ] **Step 2: Commit**

```bash
cd /home/marius/work/claude/code-explorer
git add docs/usage-reports/.gitkeep
git commit -m "chore: add docs/usage-reports/ output directory"
```

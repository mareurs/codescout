---
kind: tracker
status: active
title: Skill Frictions Tracker
owners: []
tags:
  - skills
  - frictions
---

# Skill Frictions Tracker

Running log of rough edges found while using project skills. Feed into refactor passes.

---

## `/claude-traces`

### F-001 — lf.py env auto-discovery silently fails
**When:** `lf.py session <id>` on first run  
**Expected:** Auto-load keys from `~/agents/llm-proxy/.env` (documented behavior)  
**Got:** `ERROR: LANGFUSE_PUBLIC_KEY / LANGFUSE_SECRET_KEY not set`  
**Probable cause:** Script's `.env` walk doesn't reach `~/agents/llm-proxy/.env`, or path resolution breaks when CWD isn't home  
**Fix idea:** Make the search path explicit in skill docs; add a diagnostic mode (`lf.py check-env`) that prints where it looked  
**Status: FIXED 2026-05-03** — `get_client` error message now prints key location (`~/agents/llm-proxy/.env`) and explicit env-var invocation example

### F-002 — cc.py stats fails silently on cross-project sessions
**When:** `cc.py stats 64618681-de62-4bf7-abad-0e0d93de005a`  
**Expected:** Find session JSONL and return token/cost summary  
**Got:** `ERROR: session not found`  
**Probable cause:** `cc.py` only scans the current project dir by default; session may live in a different project  
**Fix idea:** Skill should instruct: run `cc.py sessions --all` first to locate which project owns the session before running `stats`/`trace`; or add auto-fallback to `--all` on not-found

### F-003 — No guidance on env setup prerequisite
**When:** Starting any Langfuse workflow  
**Observation:** Skill docs describe key locations but don't surface a "setup check" step. New users will hit F-001 first and not know why.  
**Fix idea:** Add a `## Prerequisites` section at the top: "Run `lf.py check-env` (or equivalent) to verify keys resolve before starting analysis."

### F-004 — `cc.py sessions --all` permission friction
**When:** Trying to locate session across all projects  
**Observation:** `--all` scans broadly (`~/.claude/projects/`); triggers user confirmation in some permission modes  
**Fix idea:** Skill should note this as a known prompt; suggest narrowing with `--project` when the project is known

---


### F-005 — `lf.py find` prints 12-char truncated IDs; `lf.py trace` needs full UUID
**When:** Running `lf.py trace <id>` with the ID copied from `lf.py find` output  
**Got:** `404 Client Error: Not Found for url: .../api/public/traces/12c504ad-d66`  
**Root cause:** `cmd_find` prints `t['id'][:12]` (line ~201 in lf.py), but `cmd_trace` passes the value directly to `GET /api/public/traces/{id}` which requires full UUID  
**The footer hint says:** `→ Drill down: lf.py trace <full_trace_id>` — but `full_trace_id` was never shown  
**Skill doc says:** "Use the short trace_id prefix shown to pick which call to inspect" — wrong; API doesn't support prefix matching  
**Fix options:** (a) print full UUIDs in `find` (widen column), (b) make `trace` do a prefix lookup via `GET /traces?sessionId=...` and filter client-side  
**Status: FIXED 2026-05-03** — `cmd_find` now prints full 36-char UUIDs; column widened to 36; footer updated to "copy as-is"

### F-006 — `lf.py trace` "Tools (N)" shows available schema, not actual calls
**When:** Reading `lf.py trace` output to understand what tools were used  
**Got:** `Tools (34): Agent, AskUserQuestion, ...` — lists all 34 tools in the schema  
**Expected:** Which tools were actually called in that API turn  
**Root cause:** Proxy logs `output: { text: "" }` — tool_use blocks are stripped from logged output. Tool names aren't recoverable from Langfuse observations alone.  
**Workaround:** Use `cc.py tool-calls <session_id> --project <path>` for actual sequence  
**Fix idea:** Proxy should log tool_use block names (not inputs) into observation metadata, e.g. `tools_called: ["mcp__codescout__symbols", "TaskCreate"]`

### F-007 — Session belongs to different project; cc.py stats/trace fails silently
**When:** `cc.py stats 64618681-...` from code-explorer project  
**Got:** `ERROR: session not found`  
**Root cause:** cc.py scans only the current project's JSONL by default; session was in `/home/marius/work/mirela/backend/kotlin`  
**No guidance** in skill on how to locate session → user must either know the project or run `--all`  
**Fix idea:** On not-found, auto-suggest `cc.py sessions --all | grep <session_prefix>` as next step; or add `cc.py locate <session_id>` that scans all projects and returns the owning path


### F-008 — cc.py path decoding ambiguous: `-` in dir names decoded as `/`
**When:** `cc.py sessions --all` shows `project: /home/marius/work/mirela/backend/kotlin`  
**Reality:** Actual path is `/home/marius/work/mirela/backend-kotlin`  
**Root cause:** JSONL encodes project paths as `~/.claude/projects/<path-with-slashes-as-dashes>/`. cc.py reverses this by replacing `-` with `/`, but directory names that contain `-` (e.g. `backend-kotlin`) are indistinguishable from path separators.  
**Impact:** Any `--project` flag or `cc.py stats/trace` call built from `--all` output will have the wrong path and fail silently.  
**Fix idea:** cc.py should verify the reconstructed path exists; if not, try heuristics (longest existing prefix). Also document the ambiguity in skill docs so users know to verify before using a `--project` path from `--all` output.  
**Status: FIXED 2026-05-03** — `project_key_to_path` now uses filesystem-guided bitmask decode: tries all `-`-vs-`/` splits ordered by most separators first, picks first path that exists on disk

### F-010 — artifact(update, rel_path) updates metadata but doesn't rename file on disk (FIXED 2026-05-23, code-explorer:1cb123d1)
**When:** `artifact(action="update", patch={rel_path: "new/path.md"})` after manually moving a file  
**Got:** Artifact metadata updated (confirmed `"updated": true`), but file stays at old path  
**Impact:** Subsequent `edit_markdown(path="new/path.md")` fails with "No such file or directory"  
**Fix:** `update` now rejects `patch.rel_path` with a RecoverableError hinting at `artifact(action="move", id=..., new_rel_path=...)`. Two-call APIs must reject the wrong input shape explicitly, not accept silently — silent divergence is the worst failure mode here because `updated: true` reads as proof of action. Test: `update_rejects_rel_path_with_move_hint` in `src/librarian/tools/update.rs`. The `move` action (`src/librarian/tools/mv.rs`) covers the file-rename use case atomically.

## `/analyze-usage`

### F-005 — `find ~/work` assumption not portable
**When:** Step 1 (Discover DBs)  
**Observation:** Skill hardcodes `~/work` as standard project root with a note to adjust — but doesn't tell the model HOW to detect the right root. If the user's projects live elsewhere, step 1 produces zero results with no actionable error.  
**Fix idea:** Add discovery fallback: check `git rev-parse --show-toplevel`, then `~/work`, then `~` — or ask user for root on zero results

### F-006 — No per-session filter mode
**When:** User wants to analyze a specific session (as in this conversation)  
**Observation:** Skill is report-only (all-time). There's no way to scope queries to a session_id even though `tool_calls.session_id` exists.  
**Fix idea:** Add `/analyze-usage session <id>` mode that runs the same queries with `WHERE session_id=?`

### F-007 — Skill doesn't coordinate with claude-traces
**When:** User asks for session-level efficiency analysis  
**Observation:** `/analyze-usage` covers usage.db (codescout-side), `/claude-traces` covers JSONL+Langfuse (Claude-side). Neither skill mentions the other or describes how to combine them for a full picture.  
**Fix idea:** Add a "See also" cross-reference in both skills; document the complementary data model (usage.db = tool call metrics, Langfuse = token/cost/tool sequence)


### F-009 — analyze-usage operates in isolation from claude-traces
**Observation:** `/analyze-usage` scans usage.db (codescout-side: tool call counts, latency, errors) but has no awareness of `/claude-traces` (Claude-side: token cost, stop reasons, actual tool sequences from JSONL/Langfuse). A full session audit requires both — usage.db tells you *what codescout saw*, JSONL/Langfuse tells you *what the model decided*.  
**Current state:** Neither skill references the other. A user wanting session-level efficiency analysis has to manually combine them.  
**Direction:** `/analyze-usage` should be the driver — it owns the audit workflow. It should:
1. Run its SQL queries as today
2. For sessions of interest (high error rate, high tool count), call into `/claude-traces` to pull the actual tool sequence and token cost
3. Synthesize both into a unified verdict (efficiency + correctness)  
**Fix idea:** Add a `## Cross-referencing with session traces` section to the analyze-usage skill that explains when and how to invoke `cc.py tool-calls` + `lf.py session` for drill-down, and what signals from usage.db should trigger the drill-down (e.g. sessions with >50 calls, error rate >10%, or overflows).

### F-008 — Skill doesn't mention librarian for tracker creation
**When:** User asked to create a tracker for grep usage patterns  
**Got:** File created manually with `create_file` instead of `artifact(action="create", kind="tracker")`  
**Prompt gap:** Neither `/claude-traces` nor `/analyze-usage` skill mentions that trackers should go through the librarian. A one-liner "create any tracker via `artifact(action=create, kind=tracker)` — call `librarian(tracker_design)` first" would prevent this.


### F-010 — Step-2 query-battery output overflows; natural `grep | sed` post-processing trips the companion IL3 gate
**When:** Step 2 (per-DB SQL queries), running under the `codescout-companion` PreToolUse hook (the normal dev environment for this repo).
**Observation:** The documented invoke pattern loops `sqlite3 -line "$db" "..."` across every DB. With ~10 active DBs the combined output exceeds the inline budget and lands in a `@cmd_*` buffer (440+ lines buffered). The obvious next step — `grep -E "..." @cmd_xxx | sed 's/^ *//'` to extract the per-DB error rows — trips the IL3 advisory ("piped `grep` to a log-trimmer"), re-buffers the result, and truncates it again, forcing a fallback to `cat @cmd` + multiple `sed -n 'A,Bp' @cmd` paging calls. Net: ~3 extra round-trips per analysis to read data the skill already produced.
**Got:** Skill Step 2 says nothing about (a) expecting overflow on multi-DB loops, or (b) that buffered output must be paged with a single bounded-LHS command (`sed -n`, `cat`, bare `grep @ref`) — never a chained pipe to `sed`/`head`/`tail`, which the companion gate blocks.
**Fix idea:** Add a note to Step 2: "Multi-DB loop output overflows into a `@cmd_*` buffer. Page it with `sed -n 'N,Mp' @cmd_id` or bare `grep PATTERN @cmd_id` — do NOT chain `| sed`/`| head`/`| tail`, which the codescout IL3 gate blocks. Or scope each query tighter (single DB, `LIMIT`, date filter) so results fit inline." Pairs with the existing buffer guidance in `get_guide("progressive-disclosure")`.
**Note:** The *cross-project* IL3 pipe-to-`head` recurrence seen in the usage data (deployment / claude-plugins / researcher piping `git log | head`, `find | head`) is a **tool-usage pattern**, not a skill friction — track that as a T-N in `docs/trackers/tool-usage-patterns.md`, not here.
## `/onboarding`

### F-001 — workspace onboarding silently over-reported per-project memory writes
**When:** Multi-project workspace with `force=true`. HARD-GATE only verified `project-overview` per project, allowing subagents to pass with 2 of 6 memories.
**Got:** Final summary claimed 6/6 coverage; in reality some projects had 2–3 memories.
**Fix idea (FIXED 2026-05-07):** Phase 4 Coverage Verification reads back all 6 topics per project; subagent MANIFEST line is advisory only.

### F-002 — onboarding root-layer content not captured
**When:** Monorepo with real root-layer code (dev scripts, docker-compose, top-level scripts).
**Got:** Workspace prompt explicitly forbade a root subagent and had no fallback to capture root content.
**Fix idea (FIXED 2026-05-07):** workspace `architecture` template grew Top-Level Code Map + Generic Navigation subsections; the no-root-subagent rule now states the reason.


## `/superpowers:writing-plans` + `/codescout-companion:reconnaissance`

### F-001 — writing-plans writes test assertions naming types without scouting them; recon's triggers don't catch this

**When:** Plan `docs/superpowers/plans/2026-05-18-jsonpath-negative-slice.md`
written via the writing-plans skill, then re-scouted via `/codescout-companion:reconnaissance`
before subagent dispatch. Recon caught a defect that the plan-writing phase
had baked in.

**Got:** Plan's Task 2 + Task 3 tests asserted on `err.hint.as_deref()` — a
field that doesn't exist. The actual `RecoverableError` (at
`src/tools/core/types.rs:169`) has `pub message` + `pub guidance:
Option<Guidance>` + a `.hint()` *method* on the impl block. The Display
impl's own comment documents `to_string().contains(...)` as the canonical
test-assertion form. The plan-writing skill never read the type — it
inferred the shape from the design spec, which itself didn't pin the
assertion form.

**Two-skill tension:**

1. **writing-plans** has no "scout types named in test assertions" step.
   Its self-review checklist covers placeholders / type consistency /
   spec coverage — but `type consistency` only catches inconsistencies
   *within* the plan, not between the plan and the codebase.
2. **reconnaissance** lists triggers like "before subagent dispatch" and
   "before editing code that changes a struct, function signature, or API
   contract" — but does NOT list "before writing a plan that asserts on a
   type the planner has not read". If the user hadn't re-invoked recon
   between writing-plans and subagent dispatch, F-3 (in
   `docs/trackers/bug-fix-session-log.md`, 2026-05-18) would have surfaced
   as the first subagent's compile error.

**Fix ideas (both surfaces are candidates):**

- **In writing-plans:** add a pre-write step "for every type T whose
  accessors appear in plan test code, `symbols(name=T, include_body=true)`
  once; cite the file path + exposed accessors in a footnote of the task
  that names T". Tightens Section 9 (Self-Review) "type consistency" to
  *external* consistency.
- **In reconnaissance:** add a `When to Use` bullet — "before writing
  test code in a plan, scout every type named in an assertion". Recon's
  current model is "at the seam"; this expands it to "before the seam
  becomes a plan token".
- **Composition fix:** writing-plans could declare reconnaissance as a
  REQUIRED SUB-SKILL for any plan whose tasks include test code that
  asserts on a type's accessors. Currently the cross-skill linkage is
  user-mediated.

**Severity:** med — caught this round, cost ~6 tool calls. Without the
user's mid-plan recon invoke, would have cost a failed subagent task +
controller drift mid-dispatch.

**Confirming data:** F-3 + W-2 in `docs/trackers/bug-fix-session-log.md`
(2026-05-18). Both surface the same gap from different angles — F-3 is
the drift, W-2 is the win-from-catching-it. The fact that both could be
*one* friction in the right place (writing-plans pre-write scout) and
not just a recon-saves-the-day story is the substantive complaint.

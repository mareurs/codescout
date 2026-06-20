# Session Log — pi-integration

> Two-sided observation log for the codescout<->Pi integration work stream.
> Frictions (F-N) and wins (W-N) captured during reconnaissance so future
> sessions inherit the lesson. Append above the template marker; update the
> Index. Status vocabulary: see `docs/templates/session-log.md`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-20 | med | plan-prose | fixed-verified | codescout `grep` directTool collides with Pi built-in `grep`; setActiveTools rejects on bad input |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| _none yet_ | | | | | |

---

## F-1 — codescout `grep` directTool collides with Pi's built-in `grep`; setActiveTools rejects on bad input

**Observed:** 2026-06-20, pre-execution reconnaissance of the codescout<->Pi integration plan (`docs/superpowers/plans/2026-06-19-codescout-pi-integration.md`), before any install/setup ran.

**When:** Scouting Pi's extension API + tool registry against the cloned source (`../pi`) to confirm the plan's `mcp.json` directTools and `codescout-mode.ts` API calls were real.

**Expected (plan):** codescout's hot-set — incl. `grep` — registers as first-class Pi directTools under bare MCP names with no collisions; `pi.setActiveTools([...])` is a safe fire-and-forget.

**Got (scouted reality):**
- Pi's tool registry contains built-in `grep`/`find`/`ls` (`packages/coding-agent/CHANGELOG.md:3361` — "Tool registry now contains all built-in tools (read, bash, edit, write, grep, find, ls)…"). codescout's `grep` directTool therefore collides by name — `has("grep")` / `setActiveTools` resolution is ambiguous (codescout's vs Pi's). `grep` is the ONLY hot-set name that collides (symbols/symbol_at/tree/semantic_search/references/read_file/read_markdown/edit_* are distinct from read/write/edit/bash/grep/find/ls).
- `setActiveTools` is async and REJECTS with `invalid_argument` on unknown OR duplicate tool names (`packages/agent/src/harness/agent-harness.ts:941`; `packages/agent/test/harness/agent-harness.test.ts:498-501`). The plan's extension called it fire-and-forget.

**Probable cause:** Plan written from `extensions.md` docs prose + Claude Code's `mcp__codescout__`-prefixed tool names; did not scout Pi's own built-in tool registry or `setActiveTools` failure modes.

**Workaround / fix (landed this session, pre-execution):**
- Dropped `grep` from `directTools` (mcp.json) and `CODESCOUT_HOT_SET` (extension). codescout's `grep` stays reachable via the `mcp` proxy. To keep it first-class, use the adapter's server-wide `toolPrefix` (renames all codescout tools `cs_*`) — documented as the contingency.
- Wrapped the `setActiveTools` call in `await` + `try/catch` so a stale/ambiguous name degrades to "native tools kept" instead of an unhandled rejection.

**Severity:** med — would have caused an ambiguous/failed tool registration or a `setActiveTools` rejection at `session_start`, silently defeating curation (Pi's native `edit` would stay active) with no error surfaced to the user.

**Status:** fixed-verified — plan corrected before any execution (directTools/hot-set drop + try/catch landed in `2026-06-19-codescout-pi-integration.md`, this session). Behavioral confirmation deferred to the plan's Task 7 dogfood.

**Fix idea / Pointer:** plan Task 4 (mcp.json) + Task 5 (extension), this session. Reconnaissance hit.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via
     edit_markdown(action="insert_before", heading="## Template for new entries", ...)
     and update the Index / Wins Index tables. Status vocabulary: docs/templates/session-log.md -->

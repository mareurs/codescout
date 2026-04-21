# Research Skills Design

**Date:** 2026-04-20
**Status:** draft

## Overview

Two slash commands — `/research` (inline) and `/research-subagent` (isolated) — backed by the
`researcher` MCP. A shared reference skill `researcher-mcp` holds tool-selection logic and
context-budget defaults used by both.

## Structure

Three skills in `~/.claude/skills/`:

| Skill dir | Slash command | Role |
|---|---|---|
| `researcher-mcp/` | not user-invokable | Shared tool-selection + mode guide |
| `research/` | `/research [query]` | Inline — results land in main context |
| `research-subagent/` | `/research-subagent [query]` | Isolated — subagent absorbs bulk, returns synthesis |

## Shared Reference: `researcher-mcp`

Not intended for direct invocation. Both action skills load it with `REQUIRED SUB-SKILL: researcher-mcp`.
Its `description` frontmatter should read: "Reference guide for /research and /research-subagent — do not invoke directly."
This deters auto-invocation while keeping it discoverable in the skill namespace.

### Tool selection matrix

| Query type | MCP tool |
|---|---|
| General web topic | `mcp__researcher__research` |
| Person background | `mcp__researcher__research_person` |
| Company intel | `mcp__researcher__research_company` |
| Library / framework | `mcp__researcher__research_code` |
| Stock / crypto / macro | `mcp__researcher__market_insight` |
| Job search | `mcp__researcher__search_jobs` |

### Mode guide

| Mode | Output | Token cost | When |
|---|---|---|---|
| `quick` | Links + snippets | Very low | Just need URLs |
| `summary` | Bullet facts | Low | Inline default |
| `report` | Full markdown | Medium | Subagent default |
| `deep` | Exhaustive (2× queries+sources) | High | Only when thorough research required |

### Context budget defaults

- **Inline (`/research`):** `max_queries: 3`, `max_sources: 5`, mode `summary`
- **Subagent (`/research-subagent`):** uncapped, mode `report`

### Intent options (for `research` tool)

Pass `intent` when query nature is clear: `developer-docs`, `news`, `academic`, `competitive`, `general`.

### Domain profiles (for `research` tool)

`tech-news`, `llm-news`, `academic`, `news`, `travel`, `shopping-ro`.
Use when query is domain-specific. Otherwise omit.

## `/research` Skill

**File:** `~/.claude/skills/research/SKILL.md`

### Flow

1. **Parse input** — args become the query; if none, prompt for it
2. **Load `researcher-mcp`** — use tool matrix to identify the right MCP tool
3. **Clarify tool** — if query is ambiguous between tools, ask user (multiple choice)
4. **Clarify mode** — ask if not obvious; default `summary`
5. **Optionally clarify domain** — ask only if query is domain-specific
6. **Call MCP tool** with `mode: summary`, `max_queries: 3`, `max_sources: 5`
7. **Present results inline** — raw MCP output, no additional synthesis

Context budget (max_queries, max_sources) is fixed — not user-configurable per invocation.

## `/research-subagent` Skill

**File:** `~/.claude/skills/research-subagent/SKILL.md`

### Flow

1. **Parse input** — args become query; if none, prompt for it
2. **Load `researcher-mcp`** — same tool identification
3. **Clarify tool and mode** — default mode `report`
4. **Build subagent prompt** — include: query, MCP tool to call, mode, synthesis instruction
5. **Spawn `general-purpose` agent** — passes full researcher context; agent calls MCP internally
6. **Subagent returns synthesis only** — main context never sees raw research output

### Subagent synthesis format

```
## Findings: <query>
- <bullet 1>
- <bullet 2>
...
**Confidence:** high / medium / low
**Follow-up:** <suggested next queries if useful>
```

## Shared Research Brief (prompt template)

Both skills build this brief before calling the MCP. In `/research` it guides Claude's
own reasoning and result interpretation. In `/research-subagent` it is passed verbatim
as the subagent's context.

```
## Research Brief

### Context
- Project: <inferred from ambient conversation>
- Working on: <current task / why this research matters>
- Prior knowledge: <what we already know / current assumptions>

### Query
<refined, specific, testable>

### What to look for
- Canonical sources, authoritative guides
- Recency markers (within N months if time-sensitive)
- Version alignment (e.g. matches lib version X)
- <task-specific signals>

### What to invalidate
- Outdated info predating relevant version
- Vendor marketing without benchmarks
- <assumptions we might have wrong>

### Output target
- Mode: quick / summary / report / deep
- Decision this informs: <what we do with findings>
```

### Field sources

| Field | How populated |
|---|---|
| Context, Prior knowledge | Inferred from ambient conversation — not asked |
| Query | From args, or one prompt if args missing |
| What to look for / invalidate | Drafted from project context; user asked only if brief is thin or stakes high |
| Output target | One multiple-choice question |

## Clarifying Questions Flow

Both skills follow this flow. Minimal — infer from ambient context, ask only true unknowns.

1. **Query** — ask once if no args, otherwise skip
2. **Tool disambiguation** — multiple choice, only if query maps to multiple MCP tools
3. **Mode** — default per skill; ask only if user hints at depth
4. **Invalidation targets** — only if stakes are high or ambient context is thin

**Hard cap:** max 3 questions. Beyond that, build the brief with best-effort inference.

After gathering: Claude shows the compact brief and asks "proceed?" — one confirm step
so the user can correct a wrong inference before the search runs.

## Subagent Prompt Template

Used only by `/research-subagent`. Passed verbatim when spawning the `general-purpose` agent.

````
You are a research subagent. Use only the `researcher` MCP server tools.

## Research Brief
<full brief from shared template>

## Instructions
1. Call `<tool_name>` with the parameters below.
2. Do not dump raw search results. Synthesize against the brief.
3. Apply the "What to look for" and "What to invalidate" filters
   — drop sources that fail these.
4. Flag confidence based on source quality and consensus.
5. If initial results are thin, you may run ONE follow-up call
   with a refined query. No more.

## Tool parameters
- tool: mcp__researcher__<tool>
- mode: <mode>
- max_queries: <n or "default">
- max_sources: <n or "default">
- <tool-specific params, e.g. intent, domain_profile>

## Response format — return ONLY this

## Findings: <query>
- <bullet 1 with source domain in parens>
- <bullet 2>
...

**Confidence:** high / medium / low
**Caveats:** <what couldn't be verified / gaps>
**Follow-up:** <suggested next queries, or "none">

Do not include the brief, raw search output, or meta-commentary.
````

### Key design choices

- **One follow-up allowed** — subagent can refine if first search is thin, but capped
- **Source attribution inline** — `(domain)` after each bullet so main context can
  gauge credibility without loading full URLs
- **Strict output schema** — prevents the subagent from leaking raw search dumps into
  the main context

## Differences Summary

| Aspect | `/research` | `/research-subagent` |
|---|---|---|
| Default mode | `summary` | `report` |
| Context impact | Raw results in main context | Synthesis only |
| Context budget | max_queries=3, max_sources=5 | Uncapped |
| Output | MCP raw output | Agent synthesis |
| Best for | Quick lookups, lightweight queries | Deep research, context-sensitive sessions |

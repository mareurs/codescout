# Research Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `/research` and `/research-subagent` slash commands backed by a shared `researcher-mcp` reference skill, all driven by a research brief prompt template shared between both paths.

**Architecture:** Three markdown skills under `~/.claude/skills/`. `researcher-mcp` is a reference guide (tool matrix, mode guide, context budgets). `research` calls the researcher MCP directly — results land inline. `research-subagent` spawns a `general-purpose` agent with a templated subagent prompt — only the synthesis returns to main context.

**Tech Stack:** Markdown + YAML frontmatter. Targets the `researcher` MCP server (already configured in this environment). No build system, no tests beyond manual smoke tests.

**Spec:** `docs/superpowers/specs/2026-04-20-research-skills-design.md`

**Note on persistence:** `~/.claude/skills/` is not a git repo. No commits for skill files; implementation is pure file authoring. The spec and plan live in the code-explorer repo and ARE committed.

---

## File Structure

Files to create:

| File | Responsibility |
|---|---|
| `~/.claude/skills/researcher-mcp/SKILL.md` | Shared reference: tool matrix, mode guide, context budgets, brief template |
| `~/.claude/skills/research/SKILL.md` | Inline action skill: builds brief, calls MCP directly, returns results to main context |
| `~/.claude/skills/research-subagent/SKILL.md` | Subagent action skill: builds brief, spawns general-purpose agent with subagent prompt, returns synthesis |

No existing files modified.

---

## Task 1: Create `researcher-mcp` shared reference skill

**Files:**
- Create: `~/.claude/skills/researcher-mcp/SKILL.md`

- [ ] **Step 1: Create directory**

Run:
```bash
mkdir -p ~/.claude/skills/researcher-mcp
```

- [ ] **Step 2: Write `SKILL.md` verbatim**

Path: `~/.claude/skills/researcher-mcp/SKILL.md`

Content:
````markdown
---
name: researcher-mcp
description: Reference guide for /research and /research-subagent — do not invoke directly. Provides tool-selection matrix, mode guide, context budgets, and the shared research brief template used when calling the researcher MCP server.
---

# Researcher MCP — Shared Reference

**Not intended for direct invocation.** Loaded by `/research` and `/research-subagent` as `REQUIRED SUB-SKILL` to share tool selection logic, context budgets, and the research brief template.

## Tool Selection Matrix

Pick the right MCP tool based on query type:

| Query type | MCP tool | Notes |
|---|---|---|
| General web topic | `mcp__researcher__research` | Default for most queries |
| Person background | `mcp__researcher__research_person` | Meeting prep on individuals |
| Company intel | `mcp__researcher__research_company` | Meeting prep on organizations |
| Library / framework | `mcp__researcher__research_code` | Bugs, releases, breaking changes |
| Stock / crypto / macro | `mcp__researcher__market_insight` | Markets; web research only, no price APIs |
| Job search | `mcp__researcher__search_jobs` | Remote AI engineering jobs |

If ambiguous (e.g. "Axum" could be `research` or `research_code`), ask the user with a multiple-choice question.

## Mode Guide

All tools accept a `mode` parameter:

| Mode | Output | Token cost | When |
|---|---|---|---|
| `quick` | Links + snippets only | Very low | Just need URLs |
| `summary` | Bullet facts | Low | Inline default |
| `report` | Full markdown analysis | Medium | Subagent default |
| `deep` | Exhaustive (2× queries+sources) | High | Only when thorough research required |

## Context Budget Defaults

| Path | mode | max_queries | max_sources |
|---|---|---|---|
| `/research` (inline) | `summary` | 3 | 5 |
| `/research-subagent` | `report` | default (uncapped) | default (uncapped) |

## Intent Options (research tool)

Pass `intent` when the query nature is clear:
- `developer-docs` — API/SDK/library docs; keyword-dense planner queries
- `news` — recent events; pair with `domain_profile: news`
- `academic` — research papers, formal studies
- `competitive` — comparisons, market positioning
- `product-research` — product features, reviews
- `general` — default / unspecified

## Domain Profiles (research tool)

Pass `domain_profile` when query is domain-specific:
- `tech-news`, `llm-news`, `academic`, `news`, `travel`, `shopping-ro`

Or pass `domains: ["example.com", ...]` to pin specific sites.

## Research Brief Template

Both action skills build this brief before calling the MCP. In `/research` it guides Claude's internal reasoning. In `/research-subagent` it is passed verbatim as the subagent's context.

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

Minimal — infer from ambient context, ask only true unknowns.

1. **Query** — ask once if no args, otherwise skip
2. **Tool disambiguation** — multiple choice, only if query maps to multiple MCP tools
3. **Mode** — default per skill; ask only if user hints at depth
4. **Invalidation targets** — only if stakes are high or ambient context is thin

**Hard cap:** max 3 questions. Beyond that, build the brief with best-effort inference.

After gathering: show the compact brief and ask "proceed?" — one confirm step so the user can correct wrong inferences before the search runs.
````

- [ ] **Step 3: Verify file exists and frontmatter is valid**

Run:
```bash
head -5 ~/.claude/skills/researcher-mcp/SKILL.md
```

Expected: YAML frontmatter with `name: researcher-mcp` and a description starting with "Reference guide".

---

## Task 2: Create `/research` inline skill

**Files:**
- Create: `~/.claude/skills/research/SKILL.md`

- [ ] **Step 1: Create directory**

Run:
```bash
mkdir -p ~/.claude/skills/research
```

- [ ] **Step 2: Write `SKILL.md` verbatim**

Path: `~/.claude/skills/research/SKILL.md`

Content:
````markdown
---
name: research
description: Use when the user runs /research or asks for web research on a topic, person, company, library, market, or jobs and wants results inline in the current conversation. Calls the researcher MCP directly; results land in main context. Prefer /research-subagent if context is precious or the user asked for a deep/report mode search.
---

# /research — Inline Research

Direct-call researcher MCP skill. Results land in the main context — keep the budget small.

**REQUIRED SUB-SKILL:** researcher-mcp — load it to pick the right tool and mode, and to use the shared research brief template.

## When to Use

- User ran `/research [query]` or asked for a quick lookup
- The research output is short enough to live inline (summary/quick modes)
- Main context is not tight

## When NOT to Use

- User wants a deep report, or research will return multi-page output → use `/research-subagent`
- Context window is already near capacity → use `/research-subagent`

## Flow

1. **Parse input.**
   - If args provided on invocation, treat as the query.
   - If no args, ask the user for the query.

2. **Build the research brief** (see `researcher-mcp` skill for template).
   - Infer Context and Prior knowledge from ambient conversation — do not ask.
   - Draft "What to look for" and "What to invalidate" from project context.
   - Ask the user for clarifications only if strictly needed (hard cap: 3 questions).

3. **Pick the MCP tool** using the matrix in `researcher-mcp`.
   - If query maps unambiguously to one tool, skip asking.
   - If ambiguous, ask with multiple choice.

4. **Confirm the brief.** Show the user the compact brief and ask "proceed?" before spending tokens on the search.

5. **Call the MCP tool.** Defaults:
   - `mode: "summary"` (override only if user explicitly asked for `quick`, `report`, or `deep`)
   - `max_queries: 3`
   - `max_sources: 5`
   - Pass `intent` and `domain_profile` if relevant (see `researcher-mcp`)

6. **Present results inline** — the MCP tool output goes straight to the user. Do not re-synthesize unless the user asks.

## Context Budget

Hard caps are intentional. Do not raise `max_queries` or `max_sources` for an inline call. If the user needs more depth, route them to `/research-subagent`.

## Example Invocation

User: `/research rust async cancellation patterns`

You:
1. Infer context (Rust project, currently working on async code).
2. Draft brief — What to look for: canonical patterns, tokio docs, recent (2024+). What to invalidate: pre-tokio-1.0 info.
3. Pick tool: `mcp__researcher__research_code` (framework-specific).
4. Show compact brief, ask "proceed?"
5. On confirm: call `research_code(framework="tokio", aspects=["community","changelog"], query="async cancellation patterns")` with `mode: summary`, `max_queries: 3`, `max_sources: 5`.
6. Present the MCP output.

## Common Mistakes

- **Asking too many clarifying questions.** Hard cap at 3. Infer aggressively from ambient context.
- **Skipping the brief confirm step.** The confirm step is cheap insurance against a wasted search.
- **Raising the context budget "just this once".** Don't. Route to `/research-subagent` instead.
- **Re-synthesizing MCP output.** The user already sees the tool output; adding your own synthesis burns tokens for no gain.
````

- [ ] **Step 3: Verify**

Run:
```bash
head -5 ~/.claude/skills/research/SKILL.md
grep -c "REQUIRED SUB-SKILL" ~/.claude/skills/research/SKILL.md
```

Expected: valid frontmatter; at least 1 match for `REQUIRED SUB-SKILL`.

---

## Task 3: Create `/research-subagent` skill

**Files:**
- Create: `~/.claude/skills/research-subagent/SKILL.md`

- [ ] **Step 1: Create directory**

Run:
```bash
mkdir -p ~/.claude/skills/research-subagent
```

- [ ] **Step 2: Write `SKILL.md` verbatim**

Path: `~/.claude/skills/research-subagent/SKILL.md`

Content:
````markdown
---
name: research-subagent
description: Use when the user runs /research-subagent or asks for deep research, a full report, or research where the main context should not absorb raw search results. Spawns a general-purpose subagent that calls the researcher MCP and returns only a synthesized findings block. Prefer /research for quick inline lookups.
---

# /research-subagent — Isolated Research

Spawn a subagent that calls the researcher MCP. Main context only sees the synthesis.

**REQUIRED SUB-SKILL:** researcher-mcp — load it to pick the right tool and mode, and to use the shared research brief template.

## When to Use

- User ran `/research-subagent [query]` or asked for a deep/report-mode search
- Main context is tight and raw research output would blow the budget
- The research output is likely multi-page (full report mode)

## When NOT to Use

- Quick lookup where inline output is fine → use `/research` instead
- Query is a one-liner where the MCP tool output is already compact

## Flow

1. **Parse input.** Args → query. No args → ask.

2. **Build the research brief** (same template as `/research`, see `researcher-mcp`).
   - Infer Context / Prior knowledge from ambient conversation.
   - Draft "What to look for" / "What to invalidate" from project context.
   - Hard cap: 3 clarifying questions.

3. **Pick the MCP tool** using the matrix in `researcher-mcp`. Ask multiple-choice if ambiguous.

4. **Confirm the brief.** Show compact brief, ask "proceed?".

5. **Spawn a `general-purpose` subagent** via the Agent tool. Pass the subagent prompt (template below). Defaults:
   - `mode: "report"` (override if user asked for `deep`)
   - `max_queries` / `max_sources`: MCP defaults (uncapped)

6. **Return the subagent's synthesis to the user.** The synthesis already follows the `## Findings` format — present it as-is.

## Subagent Prompt Template

Pass verbatim to the subagent. Substitute the `<...>` placeholders.

```
You are a research subagent. Use only the `researcher` MCP server tools.

## Research Brief
<full brief built in step 2>

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
- <tool-specific params, e.g. intent, domain_profile, aspects, framework>

## Response format — return ONLY this

## Findings: <query>
- <bullet 1 with source domain in parens>
- <bullet 2>
...

**Confidence:** high / medium / low
**Caveats:** <what couldn't be verified / gaps>
**Follow-up:** <suggested next queries, or "none">

Do not include the brief, raw search output, or meta-commentary.
```

## Key Design Notes

- **One follow-up allowed.** The subagent may run one refinement call if the first search is thin. No more — cap prevents runaway spend.
- **Source domain inline.** Each bullet cites `(example.com)` so the main context can gauge credibility without loading full URLs.
- **Strict output schema.** The subagent must not leak raw search output back into the main context.

## Example Invocation

User: `/research-subagent embedding model benchmarks 2025`

You:
1. Infer context (embedding benchmarking work in this project).
2. Draft brief — What to look for: MTEB benchmarks, 2025 model releases. What to invalidate: pre-2024 comparisons.
3. Pick tool: `mcp__researcher__research` with `intent: academic`, `domain_profile: academic`.
4. Confirm brief, "proceed?"
5. On confirm: spawn general-purpose agent with the template above filled in.
6. Present the subagent's `## Findings` block to the user.

## Common Mistakes

- **Including raw MCP output in the subagent response.** The whole point is isolation — enforce the strict schema.
- **Letting the subagent run multiple follow-up calls.** Hard cap: ONE follow-up.
- **Asking clarifying questions after the subagent returns.** Questions happen BEFORE spawning. Once the subagent is running, commit.
- **Using `/research-subagent` for trivial queries.** Subagent spawn cost is not free — route quick lookups to `/research`.
````

- [ ] **Step 3: Verify**

Run:
```bash
head -5 ~/.claude/skills/research-subagent/SKILL.md
grep -c "REQUIRED SUB-SKILL" ~/.claude/skills/research-subagent/SKILL.md
```

Expected: valid frontmatter; at least 1 match for `REQUIRED SUB-SKILL`.

---

## Task 4: Smoke-test `/research`

**Files:** none (manual verification)

- [ ] **Step 1: Restart Claude Code session** so new skills are picked up.

- [ ] **Step 2: Invoke `/research` with a simple query**

Command in chat: `/research tokio async cancellation patterns`

Expected behavior:
- Claude loads `researcher-mcp` (announces "Using researcher-mcp").
- Claude infers project context (Rust, async work).
- Claude either proceeds or asks at most one multiple-choice question (tool disambiguation).
- Claude shows a compact research brief and asks "proceed?".
- On confirm, Claude calls `mcp__researcher__research_code` with `mode: summary`, `max_queries: 3`, `max_sources: 5`.
- Inline bullet-summary result lands in the chat.

- [ ] **Step 3: Invoke `/research` with no args**

Command in chat: `/research`

Expected: Claude asks for the query (only one question), then follows the same flow.

- [ ] **Step 4: Invoke `/research` on an ambiguous term**

Command in chat: `/research axum`

Expected: Claude asks a multiple-choice question to disambiguate (web framework via `research_code`, vs general web topic via `research`), then proceeds.

---

## Task 5: Smoke-test `/research-subagent`

**Files:** none (manual verification)

- [ ] **Step 1: Invoke `/research-subagent` with a deeper query**

Command in chat: `/research-subagent embedding model benchmarks MTEB 2025`

Expected behavior:
- Claude loads `researcher-mcp`.
- Claude drafts brief; asks ≤3 clarifying questions.
- Claude shows compact brief, asks "proceed?".
- On confirm, Claude spawns a `general-purpose` subagent via the Agent tool. The Agent invocation's prompt should be the filled subagent template (verify by checking tool-call logs or the skill's description of its actions).
- Subagent calls the researcher MCP internally. Main context does NOT see raw search dumps.
- Claude returns the `## Findings` block to the user with source domains inline, confidence, caveats, follow-up.

- [ ] **Step 2: Invoke `/research-subagent` with no args**

Command in chat: `/research-subagent`

Expected: Claude asks for the query, then follows the same flow.

- [ ] **Step 3: Confirm main-context isolation**

After Step 1 completes, compare the token footprint in the main conversation against the equivalent `/research` call with `mode: report`. The subagent path should have a markedly smaller footprint (synthesis only, no raw sources).

Manual check: `/context` (if available) or eyeball the chat length.

---

## Task 6: Commit spec and plan to code-explorer repo

**Files:**
- Already created: `docs/superpowers/specs/2026-04-20-research-skills-design.md`
- Already created: `docs/superpowers/plans/2026-04-20-research-skills.md`

- [ ] **Step 1: Stage spec and plan**

Run:
```bash
cd /home/marius/work/claude/code-explorer
git add docs/superpowers/specs/2026-04-20-research-skills-design.md \
        docs/superpowers/plans/2026-04-20-research-skills.md
```

- [ ] **Step 2: Verify diff is only the two files**

Run:
```bash
git diff --cached --name-only
```

Expected output:
```
docs/superpowers/plans/2026-04-20-research-skills.md
docs/superpowers/specs/2026-04-20-research-skills-design.md
```

- [ ] **Step 3: Commit**

Run:
```bash
git commit -m "docs: spec + plan for /research and /research-subagent skills"
```

Do not push — `experiments` branch accumulates commits locally until the feature lands.

---

## Rollback

Skills live in `~/.claude/skills/`. To roll back:

```bash
rm -rf ~/.claude/skills/researcher-mcp
rm -rf ~/.claude/skills/research
rm -rf ~/.claude/skills/research-subagent
```

Spec and plan commits in the code-explorer repo can be reverted with `git revert <sha>` if needed.

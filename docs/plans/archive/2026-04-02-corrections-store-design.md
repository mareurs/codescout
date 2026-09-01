---
status: archived
---
# Corrections Store — Design Spec

**Date:** 2026-04-02
**Status:** Draft
**Inspired by:** [rohitg00/pro-workflow](https://github.com/rohitg00/pro-workflow) — self-correcting memory system

> **NOT BUILT — archived 2026-09-02 as overtaken, not as rejected.**
>
> Nothing in this spec shipped. Verified at the bytes on 2026-09-02: there is no
> `src/corrections/`, no `learn` tool, and no `CorrectionsStore` — the only `corrections`
> in the tree is the librarian's unrelated filter-repair response field.
>
> **What happened instead.** The need this spec identified — durable, searchable
> corrections that survive a session — was met twice over by work that arrived on other
> paths:
>
> - **Semantic memory.** `memory(action="remember" | "recall" | "forget")` with
>   `bucket` classification and embedding search covers §*Search: FTS5 + Semantic Hybrid*
>   and §*Session Start Loading* directly. Backed by `src/memory/`, including a
>   sqlite-vec store for the daemon-free stack.
> - **`src/operator_rules/`** covers the standing-instruction half — the "always run tests
>   before committing" class this spec's §*Open Questions* asked whether to scope globally.
>
> **The design thinking is why this is archived rather than deleted.** §*Application
> Tracking* (`times_applied`, decay on unused corrections) and §*Deduplication* have no
> equivalent in what shipped — semantic memory records no usage counter, so nothing today
> answers "is this memory earning its context budget?" If that question comes back, it
> comes back to this file.
>
> **Its file pointers are stale** and were never updated: `src/agent.rs` is now
> `src/agent/mod.rs`, `src/prompts/server_instructions.md` is now a slice of
> `src/prompts/source.md`, and `src/tools/workflow.rs` was decomposed by the 2026-04-22
> refactoring plan. Read the paths as historical.

## Problem

codescout has a general-purpose `memory` tool that stores project knowledge as markdown
sections. But there's no structured mechanism for capturing *corrections* — the pattern
where a user corrects the agent's behavior and that correction should persist across
sessions.

Today, corrections live in CLAUDE.md or memory files as free-form text. There's no way
to track how often a correction is applied, whether it's still relevant, or to search
corrections by category.

## Prior Art: pro-workflow

pro-workflow solves this with a `[LEARN]` tag convention embedded in assistant responses,
parsed by a `Stop` hook, and stored in SQLite with FTS5. It tracks:

```
category | rule | mistake | correction | times_applied | created_at
```

Learnings are loaded at session start and surfaced via FTS5 search.

**What works well:** Structured schema, application tracking, FTS5 search.

**What we'd do differently:**
- Explicit tool (`learn`) instead of parsing tags from assistant output — more reliable
- Leverage codescout's existing SQLite infrastructure (sqlite-vec is already active)
- Integrate with the existing `memory` tool rather than creating a parallel system
- Add semantic search via embeddings (pro-workflow only has keyword FTS5)

## Design

### New Tool: `learn`

```
learn(action, category?, rule?, mistake?, correction?, query?)
```

| Action    | Parameters                              | Description                              |
|-----------|-----------------------------------------|------------------------------------------|
| `capture` | category, rule, mistake, correction     | Store a new correction                   |
| `search`  | query                                   | FTS5 + semantic search across corrections |
| `list`    | category? (optional filter)             | List corrections, grouped by category    |
| `remove`  | id                                      | Delete a correction that's no longer relevant |
| `stats`   | —                                       | Category counts, most/least applied      |

### Schema

New table in `usage.db` (already exists for tool usage tracking):

```sql
CREATE TABLE IF NOT EXISTS corrections (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project       TEXT NOT NULL,
    category      TEXT NOT NULL,
    rule          TEXT NOT NULL,
    mistake       TEXT,
    correction    TEXT,
    times_applied INTEGER DEFAULT 0,
    created_at    TEXT DEFAULT (datetime('now')),
    updated_at    TEXT DEFAULT (datetime('now'))
);

CREATE VIRTUAL TABLE IF NOT EXISTS corrections_fts USING fts5(
    category, rule, mistake, correction,
    content='corrections',
    content_rowid='id'
);
```

Embedding column added when semantic search is active:

```sql
ALTER TABLE corrections ADD COLUMN embedding BLOB;
```

### Categories

Free-form text, but the tool description suggests common categories:

- `testing` — test patterns, what to run when
- `style` — code style preferences the linter doesn't catch
- `architecture` — structural decisions, where things go
- `tools` — codescout/MCP tool usage patterns
- `workflow` — git workflow, PR conventions, deployment
- `domain` — project-specific business logic rules

### Search: FTS5 + Semantic Hybrid

`learn(action="search", query="...")` performs:

1. FTS5 keyword search with BM25 ranking
2. If embeddings are available, cosine similarity search via sqlite-vec
3. Results merged with reciprocal rank fusion (same pattern as `semantic_search`)

This gives pro-workflow's FTS5 baseline plus codescout's existing embedding advantage.

### Application Tracking

When the agent references a correction during a session, it can call:

```
learn(action="apply", id=N)
```

This bumps `times_applied` and `updated_at`. Over time, this surfaces which corrections
are load-bearing (high apply count) vs. stale (never applied, old `updated_at`).

The `stats` action reports this distribution to help prune the store.

### Session Start Loading

At session start (via companion plugin `SessionStart` hook or codescout's `onboarding` tool),
the most relevant corrections are loaded:

1. All corrections for the current project with `times_applied > 0` (proven useful)
2. Recent corrections (last 30 days) regardless of apply count
3. Capped at 20 corrections to avoid context bloat
4. Formatted as a compact list: `[category] rule` with a hint to `learn(action="search")`
   for the full library

### Deduplication

Before inserting, check for existing corrections with the same `project + category + rule`
(case-insensitive). If found:
- Update `mistake` and `correction` if they differ (the user refined their guidance)
- Bump `updated_at`
- Return a note that the existing correction was updated, not duplicated

### Integration with Existing Memory

The `learn` tool is separate from `memory` — corrections have structured fields and
lifecycle tracking that don't fit the markdown-section model. But they complement each
other:

- `memory` = project knowledge (architecture, conventions, glossary)
- `learn` = behavioral corrections (what the agent got wrong and how to do it right)

The `onboarding` tool should mention both systems in the generated system prompt.

## Implementation

### New Files

- `src/corrections/mod.rs` — schema, CRUD operations, FTS5 sync triggers
- `src/corrections/search.rs` — hybrid FTS5 + embedding search
- `src/tools/learn.rs` — tool implementation

### Modified Files

- `src/server.rs` — register `Learn` tool
- `src/agent.rs` — add `CorrectionsStore` to `ActiveProject`
- `src/tools/workflow.rs` — load corrections summary in onboarding output
- `src/prompts/server_instructions.md` — document `learn` tool

### Prompt Surface Updates

All three surfaces need to reference the new tool:
- `server_instructions.md` — add `learn` to the tool selection guide
- `onboarding_prompt.md` — mention corrections system in generated prompt
- `build_system_prompt_draft()` — include corrections summary if any exist

Bump `ONBOARDING_VERSION`.

## Test Plan

### Unit tests (`src/corrections/mod.rs`)
- Insert and retrieve by project
- Deduplication: same project+category+rule updates instead of duplicating
- `times_applied` bumps correctly
- FTS5 search returns ranked results
- Category filter on `list`
- Remove deletes from both table and FTS5 index

### Integration tests
- `learn(capture)` → `learn(search)` round-trip
- `learn(stats)` with mixed apply counts
- Embedding search when index is active
- Session start loading respects cap and recency rules

## Open Questions

1. **Should corrections be project-scoped or global?** Pro-workflow uses project scope.
   Some corrections are universal ("always run tests before committing"). Consider a
   `global: bool` flag.

2. **Automatic capture vs. explicit tool?** Pro-workflow parses `[LEARN]` tags from
   output. We chose explicit `learn()` calls for reliability. Should we also support
   a tag convention as a convenience shorthand?

3. **Expiry policy?** Corrections with `times_applied = 0` and `updated_at` older than
   90 days could be auto-archived. Or leave pruning entirely manual via `learn(remove)`.

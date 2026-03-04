# Semantic Memories — Design

**Date:** 2026-03-03
**Status:** Approved

## Problem

Code-explorer's current memory system is topic-keyed markdown files — you must know the
exact topic name to retrieve a memory. There's no way to search memories by meaning, no
automatic classification, and no support for unstructured observations that don't fit a
clean topic hierarchy.

Meanwhile, superpowers' episodic memory demonstrates the value of embedded, searchable
memory — but it captures raw conversation logs (temporal), not distilled knowledge
(semantic). The two systems are complementary.

## Decision

Add semantic memories to code-explorer: embedded, classified, searchable knowledge stored
in the existing `embeddings.db`. Coexists with the current markdown memory system.

## Data Model

New tables in `embeddings.db` (created by `open_db`):

```sql
CREATE TABLE IF NOT EXISTS memories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket     TEXT NOT NULL DEFAULT 'unstructured',
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
    id        INTEGER PRIMARY KEY,
    embedding FLOAT[<dim>]
);

CREATE INDEX IF NOT EXISTS idx_memories_bucket ON memories(bucket);
```

Embedding dimension matches the project's configured model (same embedder instance as
code chunks — same vector space, directly comparable).

### Buckets

| Bucket | Purpose | Examples |
|---|---|---|
| `code` | Patterns, conventions, API behaviors | "this module uses builder pattern", "auth uses JWT" |
| `system` | Build/deploy/infra/config knowledge | "requires docker for tests", "CI uses GitHub Actions" |
| `preferences` | User preferences, coding style, conventions | "always use snake_case", "prefer composition over inheritance" |
| `unstructured` | Catch-all for anything else | Ephemeral observations, misc notes |
| `structured` | Implicit — auto-created when markdown memories are written | Mirrors topic-keyed .md files |

### Preferences Bucket — Special Behaviors

The `preferences` bucket has behaviors beyond simple storage:

1. **Auto-injection:** During `onboarding`, the top preferences (by recency) are
   automatically included in the system prompt draft. Agents start every session
   with awareness of user preferences without explicit `recall`.

2. **Explicit recall:** Agents in specific workflows can `recall(bucket="preferences")`
   to pull workflow-relevant preferences (e.g., "how does the user want tests structured?").

3. **Smart routing:** When a user says "remember this next time" or similar, the
   classifier detects preference-like content (style rules, tool choices, workflow
   preferences) and routes to the `preferences` bucket automatically.

4. **Deviation requires confirmation:** Preferences include an implicit contract —
   if an agent needs to deviate from a stored preference (e.g., for a critical fix
   that requires a different approach), it should note the deviation and confirm
   with the user rather than silently ignoring the preference.

## Classification Heuristic

When the caller omits `bucket`, the tool auto-classifies via keyword scoring:

**code triggers:** function, method, struct, class, trait, impl, pattern, API, endpoint,
convention, naming, import, module, crate, package, type, interface, refactor, abstraction,
file paths (contains `/` or language-specific extensions)

**system triggers:** build, deploy, CI, config, environment, docker, infra, database,
migration, permission, secret, credential, server, port, host, pipeline, test command,
cargo, npm, pip

**preferences triggers:** prefer, always, never, style, convention, habit, default to,
use X instead of Y, "next time", "remember to", "I like", "I want", "don't use",
snake_case, camelCase, tabs, spaces, indentation

**Scoring:** Simple `content.contains()` scan. Whichever bucket gets the most keyword hits
wins. Ties go to `unstructured`. The caller can always override with an explicit `bucket`.

## Tool Interface

The `memory` tool gains three new actions alongside the existing four:

### Existing (unchanged)

| Action | Storage | Purpose |
|---|---|---|
| `write` | Markdown files | Structured topic-keyed notes |
| `read` | Markdown files | Read by topic key |
| `list` | Markdown files | List all topics |
| `delete` | Markdown files | Delete by topic key |

### New

| Action | Storage | Purpose |
|---|---|---|
| `remember` | SQLite + vec0 | Store + embed a semantic memory |
| `recall` | SQLite + vec0 | Search memories by meaning |
| `forget` | SQLite + vec0 | Delete a semantic memory by ID |

**`remember`:**
```json
{
  "action": "remember",
  "content": "The embedding pipeline uses a three-stage process: chunk → embed → store",
  "title": "embedding pipeline stages",
  "bucket": "code"
}
```
- `title` optional — auto-extracted from first sentence if omitted
- `bucket` optional — auto-classified via heuristic if omitted
- Returns `"ok"`

**`recall`:**
```json
{
  "action": "recall",
  "query": "how does the embedding pipeline work",
  "bucket": "code",
  "limit": 5
}
```
- `bucket` optional — searches all buckets if omitted
- `limit` optional — default 5
- Returns ranked results with similarity scores

**`forget`:**
```json
{
  "action": "forget",
  "id": 3
}
```
- Deletes by ID. Returns `"ok"`.

## Integration with `semantic_search`

New optional parameter: `include_memories: bool` (default `false`).

When `true`, queries both `vec_chunks` and `vec_memories`, merges by similarity, tags
results with `"source": "code"` or `"source": "memory"`.

Default behavior is unchanged — code-only search. Opt-in prevents polluting code results
with memory fragments.

## Markdown Memory Cross-Embedding

When `memory(action: "write")` is called, the content is also embedded into `vec_memories`
with `bucket: "structured"`. This bridges the two systems: structured memories are
human-readable, git-tracked, AND searchable via `recall`.

When the topic is rewritten, the corresponding embedding is updated (keyed by
`title = topic`). When deleted, the embedding is removed.

## Coexistence with Superpowers Episodic Memory

| Dimension | Episodic Memory | Code-Explorer Semantic Memory |
|---|---|---|
| **What** | Conversation logs | Distilled knowledge |
| **When populated** | Automatically on session end | Explicitly by the agent |
| **Scope** | Cross-project, temporal | Per-project, topical |
| **Use case** | "What did I do last Tuesday?" | "What do I know about auth?" |

No integration needed. An agent can use episodic memory to recall a past conversation,
then use `remember` to distill the lesson into project memory.

## Agent Agnosticism

All intelligence (classification, title extraction) lives in the Rust tool, not in
Claude-specific skills. Any MCP client (Claude Code, Copilot, etc.) can use
`remember`/`recall`/`forget` without platform-specific plugins.

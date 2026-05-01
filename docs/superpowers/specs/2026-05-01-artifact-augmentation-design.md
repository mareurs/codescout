# Artifact Augmentation — Design Spec

**Date:** 2026-05-01
**Status:** draft
**Topic:** librarian-mcp — augmentation system for AI-maintained artifacts

---

## Overview

Any artifact in the librarian catalog can opt into **augmentation**: a persistent
prompt + AI-editable params that enable server-assisted refresh. On refresh, the
server gathers context from configured sources and returns a structured package;
the AI synthesizes new content and writes it back via `artifact_update`.

`kind: tracker` is the primary use case — a purpose-built artifact whose body is
pure live state. All other kinds (spec, plan, adr, memory, doc) can opt in
optionally, remaining their primary kind first and augmented second.

Row presence in `artifact_augmentation` = augmented. No flag on `artifact` table.

---

## Data Model

### New table: `artifact_augmentation`

```sql
CREATE TABLE artifact_augmentation (
    artifact_id       TEXT    NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
    prompt            TEXT    NOT NULL,
    params            JSON    NOT NULL DEFAULT '{}',
    last_refreshed_at TEXT,
    refresh_count     INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (artifact_id)
);
```

`artifact` table is **unchanged**. Cascade delete handles cleanup when the parent
artifact is removed.

### `params` schema

Free-form JSON blob. Server interprets known keys; unknown keys are preserved and
returned in refresh responses (forward compat — AI may invent new params the server
doesn't gather yet).

Known top-level keys:

```json
{
  "gather_from": [ ... ],
  "format": "table | bullets | sections | prose",
  "max_tokens": 3000
}
```

### `gather_from` source types

Each entry in `gather_from` is an object with a `source` key:

| Source | Required fields | Optional fields |
|--------|----------------|-----------------|
| `git_log` | — | `limit`, `since` (`"last_refresh"` or ISO timestamp), `branch`, `grep` |
| `artifacts` | — | `filter` (FilterNode JSON), `limit`, `scope` |
| `symbols` | `path` | `kind` (function/struct/…), `limit` |
| `grep` | `pattern` | `path`, `limit` |
| `file` | `path` | — |
| `observations` | — | `artifact_id`, `limit`, `since` (`"last_refresh"` or ISO timestamp) |

Unknown source values: skipped with a warning entry in the refresh response.

Example `params` for a feature-state tracker:

```json
{
  "gather_from": [
    { "source": "symbols", "path": "src/tools", "kind": "struct" },
    { "source": "git_log", "limit": 20, "since": "last_refresh" }
  ],
  "format": "table",
  "max_tokens": 3000
}
```

---

## Refresh Cycle

`artifact_refresh(id)` is the primary new tool. It never writes — it returns a
context package for the AI to synthesize from.

### Steps

1. **Validate** — augmentation row must exist; otherwise `RecoverableError`:
   `"no augmentation for this artifact — call artifact_augment first"`.

2. **Gather** — run each `gather_from` source in parallel; collect results into
   a keyed map. Unknown sources produce a warning entry, not an error.

3. **Return context package:**

```json
{
  "artifact_id": "...",
  "prompt": "Maintain a snapshot of all tools...",
  "params": { "gather_from": [...], "format": "table" },
  "current_body": "| Tool | Status | ... |",
  "context": {
    "git_log": [...],
    "symbols": [...],
    "warnings": ["unknown source 'changelog' skipped"]
  },
  "last_refreshed_at": "2026-04-30T12:00:00.000Z",
  "hints": ["17 symbols gathered from src/tools", "20 commits since last refresh"]
}
```

4. **AI synthesizes** new body from `prompt + context + current_body`, calls
   `artifact_update` to write back.

5. **AI calls `artifact_refresh_commit(id)`** after writing back. Signals the
   refresh cycle is complete: increments `refresh_count`, sets `last_refreshed_at`.
   Kept separate from `artifact_update` because the server has no way to distinguish
   a refresh write from a regular edit. TimeMachine event logged via `artifact_update`.

### Design boundary

Server gathers (git, SQLite, symbols). AI synthesizes (reasoning, formatting).
Neither crosses into the other's domain.

---

## Tool Surface

### New tools

#### `artifact_augment(id, prompt, params?)`

Create or replace the augmentation row for any artifact. Idempotent — safe to call
on already-augmented artifacts to update the prompt or reset params. `params`
defaults to `{}` if omitted.

#### `artifact_refresh(id)`

Gather context + return package. Does not write anything. AI calls `artifact_update`
after synthesis.

#### `artifact_update_params(id, params)`

Merge-patch the `params` JSON — not a replace. AI calls this mid-session to tune
gather sources without touching the prompt or body. Only `params` and `updated_at`
touched.

#### `artifact_refresh_commit(id)`

Signals that a refresh cycle is complete. Updates `last_refreshed_at` and increments
`refresh_count` in `artifact_augmentation`. Must be called after `artifact_update`
in every refresh cycle. No-ops gracefully if augmentation row was deleted between
refresh and commit.

#### `tracker_create(repo, rel_path, title, prompt, params?)`

Atomic shorthand: create `kind: tracker` artifact + augment in one call. Avoids
leaving an un-augmented tracker behind if the second call fails. Returns the
artifact id.

### Modified tools

#### `artifact_get`

If augmentation row exists, response includes:

```json
"augmentation": {
  "prompt": "...",
  "params": { ... },
  "last_refreshed_at": "...",
  "refresh_count": 12
}
```

No change if not augmented.

#### `artifact_find`

New optional filter param: `augmented: bool`. When `true`, restricts results to
artifacts that have an augmentation row. When `false`, restricts to non-augmented.
Omit to return all.

#### `librarian_context`

Augmented artifacts rendered with a `[LIVE]` header block:

```markdown
<!-- LIVE: feature-state-tracker | last refreshed: 2026-04-30 | refresh #12 -->
> Prompt: Maintain a snapshot of all tools — name, status, what it does, key file.

| Tool | Status | What it does | Key file |
...
```

The prompt surfaces as a blockquote directive. Signals to the AI: "you are
responsible for keeping this current."

**Token budget priority:** augmented artifacts ranked before non-augmented.
Within augmented, `kind: tracker` ranked first. Existing `max_tokens` cap applies.

### Not added

- `tracker_refresh` as a separate tool — `artifact_refresh` covers it.
- `artifact_augment_delete` — cascade handles it on artifact deletion; `artifact_update`
  can clear the body if needed before deletion.

---

## `[LIVE]` Semantic Contract

| Marker | Meaning |
|--------|---------|
| `[LIVE]` | AI-maintained; may be stale if `last_refreshed_at` is old |
| *(none)* | Static doc, indexed from disk, not AI-managed |

`last_refreshed_at` is always shown in the header so both AI and user can judge
staleness at a glance. The AI reads the prompt as a standing directive each time
the artifact appears in context.

---

## Concrete Tracker Catalog

### code-explorer project trackers

| Tracker | Prompt summary | Primary gather sources |
|---------|---------------|----------------------|
| Feature state | Snapshot of all tools: name, status, what it does, key file | `symbols("src/tools")`, `git_log(since=last_refresh)` |
| Active experiments | What's in-flight on `experiments`: features, test status, next steps | `git_log(branch=experiments)`, `artifacts(kind=plan, status=active)` |
| Tool misbehaviors | Running log of known tool bugs, silent failures, workarounds | `file("docs/TODO-tool-misbehaviors.md")`, `artifacts(topic=gotchas)` |
| API surface | Public MCP tool list with descriptions + schema signatures | `symbols("src/tools", kind=struct)`, `artifacts(kind=spec)` |

### Cross-project / shared trackers

| Tracker | Prompt summary | Primary gather sources |
|---------|---------------|----------------------|
| ML experiments | Chronological log: model, dataset, hyperparams, metrics, lessons | `artifacts(topic=ml-experiments)`, `grep("accuracy\|loss\|epoch")` |
| Decision log | Key technical decisions, rationale, alternatives, outcome | `artifacts(kind=adr)`, `git_log(since=last_refresh)` |
| Dependency health | Dep versions, last updated, known issues, upgrade blockers | `file("Cargo.toml")`, `grep("deprecated\|FIXME")` |
| Session learnings | Distilled cross-session learnings that don't fit elsewhere | `observations(recent=true)`, `artifacts(kind=memory)` |

### Augmented non-trackers (opt-in examples)

| Artifact | Kind | Augmentation purpose |
|----------|------|---------------------|
| Architecture spec | `spec` | After each refactor, verify spec matches implementation; note drift |
| Active sprint plan | `plan` | Scan git log since last refresh, mark completed items, surface blockers |
| Onboarding doc | `doc` | When tools added or removed, update the tool inventory section |

---

## Implementation Notes

- Schema migration: add `artifact_augmentation` table to `catalog/schema.sql`
- `gather_from` sources map 1:1 to existing infrastructure: `git2` for git_log,
  `catalog::find` for artifacts, LSP/tree-sitter for symbols, ripgrep for grep,
  `std::fs` for file
- `artifact_update_params` uses JSON merge-patch (RFC 7396) — keys set to `null`
  are deleted, present keys are merged
- Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` — new tools change the
  tool surface the LLM needs to know about
- Update all three prompt surfaces: `server_instructions.md`, `onboarding_prompt.md`,
  `build_system_prompt_draft()` in `src/prompts/builders.rs`
- `tracker_create` must be atomic: wrap artifact insert + augmentation insert in a
  single transaction

---

## Open Questions

None — all design decisions resolved in brainstorm session.

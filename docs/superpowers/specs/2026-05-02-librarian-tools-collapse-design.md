# Librarian Tools Collapse: 16 → 5

**Date:** 2026-05-02
**Status:** draft

## Problem

16 MCP tools for artifact/librarian operations is too many. Two failure modes:

1. **Token pressure** — the tool-list description budget fills before the LLM gets to code tools.
2. **Conceptual sprawl** — overlapping names cause wrong-tool selection (`update` vs `augment` vs `event_create`; `state_at` vs `workspace_state_at`; `refresh` vs `refresh_stale`).

## Target: 5 tools

| New tool | Absorbs | Actions |
|---|---|---|
| `artifact` | find, get, create, update, link, graph, state_at | find \| get \| create \| update \| link \| graph \| state_at |
| `artifact_event` | event_create, timeline | create \| list |
| `artifact_augment` | (unchanged) | — |
| `artifact_refresh` | refresh, refresh_stale | gather \| list_stale |
| `librarian` | librarian_context, librarian_reindex, tracker_design, workspace_state_at | context \| reindex \| tracker_design \| workspace_state_at |

## Tool Designs

### `artifact`

Unified CRUD + query tool. `action` is required; params are action-scoped.

**`find`** — filter/search artifacts  
Params: `filter`, `kind`, `status`, `semantic`, `scope`, `augmented`, `include_archived`, `limit`, `offset`

**`get`** — fetch single artifact by id  
Params: `id`*, `include_links`, `links_direction`, `links_rel`, `include_observations`, `full`, `heading`, `headings`, `start_line`, `end_line`

**`create`** — create new artifact  
Params: `rel_path`*, `kind`*, `title`*, `body`*, `repo`, `owners`, `tags`, `status`, `augment`

**`update`** — update frontmatter/body/status  
Params: `id`*, `patch` {body, title, status, topic, owners, tags}, `addBlocks`, `addBlockedBy`, `owner`, `commit_refresh`, `activeForm`

**`link`** — create directional link  
Params: `src_id`*, `dst_id`*, `rel`*

**`graph`** — BFS expansion from seed  
Params: `id`*, `depth` (1-3), `rels`, `include_events`

**`state_at`** — time-travel snapshot of single artifact  
Params: `artifact_id`*, `commit` OR `timestamp` (exactly one required)

### `artifact_event`

Immutable event log operations. Kept separate from `artifact` because events are a different concept from field patches — append-only, anchored to commits.

**`create`** — append event to timeline  
Params: `artifact_id`*, `kind`*, `payload`*, `anchor_commit`, `head_commit`, `parent_event_id`, `author`, `source`, `also_mutates`, `resolves_intent_event_id`

**`list`** — return events for artifact  
Params: `artifact_id`*, `kinds`, `limit`, `since`, `until`

### `artifact_augment`

Unchanged. Attach or replace persistent prompt + gather params on an artifact. `merge=true` for RFC 7396 param-only patch.

Kept standalone because:
- Semantically distinct from CRUD (sets a living-update contract, not content)
- Complex enough params to warrant its own schema
- The `augment → refresh → update(commit_refresh)` workflow needs clear named steps

### `artifact_refresh`

Augmentation lifecycle operations.

**`gather`** — gather context for augmented artifact, return refresh package  
Params: `id`*

**`list_stale`** — list augmented artifacts older than threshold  
Params: `threshold_hours` (default 24), `scope` (default project), `limit` (default 10)

### `librarian`

Workspace-level operations that don't act on a specific artifact.

**`context`** — pack topic/anchor neighbourhood into markdown bundle  
Params: `topic` OR `anchor_id`, `max_tokens`, `scope`, `include_archived`

**`reindex`** — re-scan and classify markdown artifacts  
Params: `scope`, `repo`, `force`

**`tracker_design`** — return teaching prompt + archetype library + existing-tracker landscape  
Params: `intent`

**`workspace_state_at`** — time-travel snapshot of all artifacts at a commit/timestamp  
Params: `commit` OR `timestamp` (exactly one), `scope`, `kinds`, `freshness_filter`, `include_archived`

## Implementation Notes

### Schema pattern

Follow the `memory`/`workspace`/`library` tools: flat JSON Schema, `action` as required enum, per-action params documented in descriptions. No `oneOf` enforcement — description is the contract.

### Dispatch pattern

```rust
pub async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
    let action = args["action"].as_str().ok_or_else(|| anyhow!("action required"))?;
    match action {
        "find"     => handle_find(ctx, args).await,
        "get"      => handle_get(ctx, args).await,
        "create"   => handle_create(ctx, args).await,
        "update"   => handle_update(ctx, args).await,
        "link"     => handle_link(ctx, args).await,
        "graph"    => handle_graph(ctx, args).await,
        "state_at" => handle_state_at(ctx, args).await,
        other      => Err(RecoverableError::new(format!("unknown action: {other}"))),
    }
}
```

Each handler is the existing `call()` body from the old tool, extracted into a free function. No logic changes — pure wiring.

### Files to update

- `crates/librarian-mcp/src/tools/mod.rs` — `all_tools()` returns 5 tools
- Old tool files become handler modules (renamed from `find.rs` → `handlers/find.rs` etc.) or inline private fns in the new tool files
- `src/prompts/server_instructions.md` — update tool name references
- `src/prompts/onboarding_prompt.md` — update tool name references
- `src/prompts/builders.rs` `build_system_prompt_draft()` — update references
- `src/tools/onboarding.rs` `ONBOARDING_VERSION` — bump (tool names changed)
- `src/util/path_security.rs` `check_tool_access` — update write-tool match arms
- All tests referencing old tool names

### Backward compatibility

None. Old names vanish. Clients using old names get "unknown tool". The server instructions + onboarding prompt are the only migration path — LLMs re-learn from them.

### Test surface

The `server_registers_all_tools` test must list 5 tool names (plus unchanged codescout tools). The `prompt_surfaces_reference_only_real_tools` test will catch any stale old names in the prompt surfaces.

## Migration map (old → new)

| Old | New |
|---|---|
| `artifact_find` | `artifact(find)` |
| `artifact_get` | `artifact(get)` |
| `artifact_create` | `artifact(create)` |
| `artifact_update` | `artifact(update)` |
| `artifact_link` | `artifact(link)` |
| `artifact_graph` | `artifact(graph)` |
| `artifact_state_at` | `artifact(state_at)` |
| `artifact_event_create` | `artifact_event(create)` |
| `artifact_timeline` | `artifact_event(list)` |
| `artifact_augment` | `artifact_augment` |
| `artifact_refresh` | `artifact_refresh(gather)` |
| `artifact_refresh_stale` | `artifact_refresh(list_stale)` |
| `librarian_context` | `librarian(context)` |
| `librarian_reindex` | `librarian(reindex)` |
| `tracker_design` | `librarian(tracker_design)` |
| `workspace_state_at` | `librarian(workspace_state_at)` |

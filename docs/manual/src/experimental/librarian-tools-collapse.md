# Librarian Tools Collapse (16 → 5)

> ⚠ Experimental — may change without notice.

The 16 individual librarian tools have been collapsed into 5 action-dispatched tools, reducing MCP surface area and improving discoverability.

## New tools

| Tool | Actions | Replaces |
|------|---------|----------|
| `artifact` | `find`, `get`, `create`, `update`, `link`, `graph`, `state_at` | `artifact_find`, `artifact_get`, `artifact_create`, `artifact_update`, `artifact_link`, `artifact_graph`, `artifact_state_at` |
| `artifact_event` | `create`, `list` | `artifact_event_create`, `artifact_timeline` |
| `artifact_augment` | (unchanged) | `artifact_augment` |
| `artifact_refresh` | `gather`, `list_stale` | `artifact_refresh`, `artifact_refresh_stale` |
| `librarian` | `context`, `reindex`, `tracker_design`, `workspace_state_at` | `librarian_context`, `librarian_reindex`, `tracker_design`, `workspace_state_at` |

## Usage

Every tool takes a required `action` parameter:

```json
// Find active trackers
{"action": "find", "kind": "tracker", "status": "active"}

// Get one artifact with links
{"action": "get", "id": "abc123", "include_links": true}

// Append a note event
{"action": "create", "artifact_id": "abc123", "kind": "note", "payload": {"text": "..."}}

// Gather refresh context
{"action": "gather", "id": "abc123"}

// Pack context bundle
{"action": "context", "topic": "authentication"}
```

## Migration

Old tool names are no longer registered. If you have saved prompts or scripts using the old names, update them:

- `artifact_find {...}` → `artifact {action: "find", ...}`
- `artifact_get {...}` → `artifact {action: "get", ...}`
- `artifact_create {...}` → `artifact {action: "create", ...}`
- `artifact_update {...}` → `artifact {action: "update", ...}`
- `artifact_link {...}` → `artifact {action: "link", ...}`
- `artifact_graph {...}` → `artifact {action: "graph", ...}`
- `artifact_state_at {...}` → `artifact {action: "state_at", ...}`
- `artifact_event_create {...}` → `artifact_event {action: "create", ...}`
- `artifact_timeline {...}` → `artifact_event {action: "list", ...}`
- `artifact_refresh {...}` → `artifact_refresh {action: "gather", ...}`
- `artifact_refresh_stale {...}` → `artifact_refresh {action: "list_stale", ...}`
- `librarian_context {...}` → `librarian {action: "context", ...}`
- `librarian_reindex {...}` → `librarian {action: "reindex", ...}`
- `tracker_design {...}` → `librarian {action: "tracker_design", ...}`
- `workspace_state_at {...}` → `librarian {action: "workspace_state_at", ...}`

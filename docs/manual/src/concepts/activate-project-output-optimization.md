# workspace(action: activate) Output Optimization


`workspace(action: activate)` now returns a slim **orientation card** instead of the full raw config dump.

## New response shape

```json
{
  "status": "ok",
  "project": "my-project",
  "project_root": "/home/user/my-project",
  "read_only": false,
  "languages": ["rust"],
  "index": { "status": "not_indexed", "hint": "Run index(action: build) to enable semantic_search." },
  "memories": ["architecture", "conventions"],
  "hint": "CWD: /home/user/my-project. Run workspace(action: status) for health checks and memory staleness.",

  // RW only, and only when non-default (omitted for the sandboxed "default"):
  "security_profile": "root",

  // Multi-project workspaces only:
  "workspace": [
    { "id": "root", "root": ".", "languages": ["rust"], "depends_on": [] },
    { "id": "web", "root": "packages/web", "languages": ["typescript"], "depends_on": ["root"] }
  ],

  // When dependencies were auto-registered:
  "auto_registered_libs": { "count": 12, "without_source": 3 }
}
```

## What changed

| Before | After |
|--------|-------|
| Full `config` object (all TOML fields) | Slim orientation card |
| Security fields always present | `security_profile` only in RW mode, and only when non-default (`root`); `shell_enabled` dropped (it defaults to true everywhere, so it carried no signal) |
| No workspace on activation | `workspace` array included when multi-project |
| No memory list | `memories` array (topic names) |
| No index status | `index.status` field |
| Focus-switch returned minimal `{activated: {project_root}}` | Focus-switch returns the same full card |
| `auto_registered_libs` was an array of objects | Now a summary `{count, without_source}` |

## Hint scenarios

| Scenario | Hint |
|----------|------|
| First activation (home project) | `"CWD: …. Run workspace(action: status) for health checks…"` |
| Returning to home | `"Returned to home project. CWD: …. Run workspace(action: status)…"` |
| Switching away (RO) | `"Browsing {name} (read-only). CWD: … — remember to workspace(action: activate, path: ...)"` |
| Switching away (RW) | `"Switched project (read-write). CWD: … — remember to workspace(action: activate, path: ...)"` |

## `workspace(action: status)` for full details

The orientation card is intentionally compact. For detailed health checks, memory staleness
scores, and drift detection, call `workspace(action: status)` after activation.

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

## Repo name fields

The card may carry one more field about the repo's name, which lives in the committed file
`.codescout/repo.toml` at the repo root.

| Field | When | Meaning |
|-------|------|---------|
| `repo_name_unconfirmed: {suggested, action, instruction}` | No `repo.toml` exists, the user has not declined the dialog, and the activation is read-write | Ask the user to confirm or edit `suggested`, then call `onboarding(repo_name=…)`. That call writes the committed file. |
| `repo_name_error: "<one line>"` | `repo.toml` exists and cannot be read, in either mode | The file is reported, not treated as unconfirmed, so no write is suggested over it. |

A read-only activation carries no `repo_name_unconfirmed`, because it only browses the repo. The compact
form shows each field as one banner line, `⚠ REPO NAME UNCONFIRMED` or `⚠ REPO NAME UNREADABLE`.

## `workspace(action: status)` for full details

The orientation card is intentionally compact. For detailed health checks, memory staleness
scores, and drift detection, call `workspace(action: status)` after activation.

# Read-Only Default for `workspace(action: activate)`

When you call `workspace(action: activate)` with a path different from the home project (the one
codescout started with), the project is now activated in **read-only mode** by default.
All write tools (`edit_file`, `create_file`, `edit_code`) are blocked until
you explicitly opt in.

## Why

When an LLM activates a second project to browse code for reference, it shouldn't be
able to accidentally write to that project. Read-only-by-default makes cross-project
navigation safe without any extra ceremony.

## Behavior

| Activation | Write tools |
|---|---|
| Home project (initial `--project` or CWD) | Always enabled |
| Non-home project — no `read_only` param | **Disabled** (default) |
| Non-home project — `read_only: false` | Enabled (explicit opt-in) |
| Return to home project | Restored automatically |

## Response

`workspace(action: activate)` now includes a `read_only` field in its response:

```json
{
  "status": "ok",
  "activated": { "project_root": "/path/to/other-project", "..." },
  "read_only": true,
  "hint": "Switched project. CWD: /path/to/other-project — ⚠ remember to call workspace(action: activate) to return when done. This project is activated in read-only mode. To enable writes, call workspace(action: activate) with read_only: false."
}
```
## Usage

```
// Browse another project (read-only, safe)
workspace(action: activate, path: "/path/to/other-project")

// Activate with write access explicitly enabled
workspace(action: activate, path: "/path/to/other-project", read_only: false)
```

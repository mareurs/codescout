# Design: `approve_write` — Session-Scoped Write Access Outside Project Root

**Date:** 2026-05-03
**Status:** Draft

## Problem

Write tools (`edit_file`, `create_file`, `edit_code`, `edit_markdown`) are hard-bounded to the
project root (plus temp dir and configured `extra_write_roots`). There is no way to
write outside the project root without either using the `Root` security profile (fully
unrestricted, no acknowledgement) or editing `project.toml` at rest (static, requires
restart).

The existing `acknowledge_risk: true` pattern for `run_command` shows the right model:
let the LLM unblock itself by making an explicit acknowledgement call. The equivalent
for file writes is a pre-approval tool that grants write access to a directory for the
duration of the current MCP server session.

## Goal

Allow an LLM to write to directories outside the project root by first calling
`approve_write('<dir>')`. Approval is session-scoped — it does not persist across
server restarts. The deny-list (`~/.ssh/`, etc.) remains unconditionally enforced.

## Non-Goals

- Persisting approved paths to `project.toml`
- Read access changes (reads are already permissive)
- Shell command security changes

---

## Data Model

### `ActiveProject` — new field

```rust
pub(crate) session_write_roots: Arc<std::sync::Mutex<Vec<PathBuf>>>,
```

Initialized to `Arc::new(Mutex::new(vec![]))` in the `activate` code path. Because
`ActiveProject` is cloned when accessed via `Agent::with_project`, wrapping in `Arc`
ensures all clones share the same list — mutations made by `approve_write` are
immediately visible to subsequent write tool calls in the same session.

Cleared automatically on project re-activation: `activate` constructs a new
`ActiveProject`, so a new empty `Arc` is created.

### `Agent` — two new methods

```rust
/// Append a session-approved write root for the current project.
pub fn add_session_write_root(&self, path: PathBuf);

/// Return a snapshot of the current session-approved write roots.
pub fn session_write_roots_snapshot(&self) -> Vec<PathBuf>;
```

Both are sync because `session_write_roots` uses `std::sync::Mutex` (same pattern as
`dirty_files`). The `Agent` accessor acquires the outer `AgentInner` lock only long enough
to clone the inner `Arc`, then drops it before locking the `Mutex`.

---

## New Tool: `approve_write`

**File:** `src/tools/approve_write.rs`

### Input Schema

```json
{
  "path": {
    "type": "string",
    "description": "Absolute or project-relative path to the directory to approve for writing."
  }
}
```

### Behavior

1. Require active project (needed for relative path resolution and for storing approval).
2. Expand `~` and resolve path against project root if relative.
3. Apply `best_effort_canonicalize`.
4. **Breadth guard:** reject `/` and `$HOME` — too broad (mirrors the CWD guard in
   `validate_write_path`). Return `RecoverableError`.
5. **Deny-list check:** run `is_denied` against `denied_read_paths(&config)`. If hit,
   return `RecoverableError` — deny-list paths can never be approved.
6. Check `file_write_enabled` and `read_only` — return `RecoverableError` if either blocks.
7. `ctx.agent.add_session_write_root(resolved)`.
8. Return:

```json
{ "approved": "/abs/resolved/path", "scope": "this session only" }
```

The response returns the canonicalized path (new information — not an echo of the
input) and the scope so the LLM knows the approval is ephemeral.

### Error Messages

| Condition | Message |
|---|---|
| Path is `/` or `$HOME` | `"approve_write: '/' is too broad — specify a subdirectory"` |
| On deny-list | `"approve_write: '<path>' is in a protected location and cannot be approved"` |
| No active project | `"approve_write: no active project — activate a project first"` |
| `file_write_enabled = false` | standard `check_tool_access` message |
| `read_only = true` | `"approve_write: project is read-only"` |

---

## `validate_write_path` Changes

### Signature

```rust
pub fn validate_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],      // new
) -> Result<PathBuf>
```

### Body change

After building `allowed = vec![project_root, temp_dir, ...]`:

```rust
allowed.extend_from_slice(session_roots);
```

Session roots are checked alongside static roots — no other logic changes.

### Updated error message

```
write denied: '<path>' is outside the project root.
Call approve_write('<dir>') first to grant write access for this session.
```

### Production callers (6)

Each caller adds one line before the `validate_write_path` call:

```rust
let session_roots = ctx.agent.session_write_roots_snapshot();
```

Then passes `&session_roots` as the fourth argument.

| File | Lines |
|---|---|
| `src/tools/create_file.rs` | 49 |
| `src/tools/edit_file/mod.rs` | 203, 268, 329 |
| `src/tools/symbol/edit_code.rs` | 148 |
| `src/tools/markdown/edit_markdown.rs` | 359 |
| `src/fs/mod.rs` | 76 |

---

## Tool Registration (6-Location Checklist)

1. **Implementation:** `src/tools/approve_write.rs` — `struct ApproveWrite` + `impl Tool`
2. **Server:** `Arc::new(ApproveWrite)` in `CodeScoutServer::from_parts`
3. **Test:** `"approve_write"` in `server_registers_all_tools`
4. **Security gate:** add `"approve_write"` to the `file_write_enabled` arm in
   `check_tool_access` (writes disabled ⟹ approvals disabled)
5. **Gate test:** `file_write_enabled_disabled_blocks_approve_write`
6. **Prompt surface:** one-line mention in `src/prompts/server_instructions.md`

---

## Security Invariants

| Invariant | Enforcement point |
|---|---|
| Deny-list unconditionally blocks | `approve_write` checks deny-list before adding to session roots; `validate_write_path` also checks deny-list before the `allowed` vec |
| Too-broad approvals rejected | `approve_write` rejects `/` and `$HOME` at approval time |
| `Root` profile unaffected | `approve_write` runs normally (Root skips the boundary check anyway — harmless) |
| Read-only project | `approve_write` returns error before touching session state |
| Session cleared on re-activation | New `ActiveProject` = new empty `Arc<Mutex<Vec<PathBuf>>>` |
| No persistence | `session_write_roots` never serialized; dies with server process |

---

## Testing

### Unit tests (in `path_security.rs`)

- **Helper:** `fn default_session_roots() -> Vec<PathBuf> { vec![] }` — pass `&default_session_roots()` to all existing `validate_write_path` test callsites (14 callsites, no behavior change)
- `validate_write_path_allows_session_approved_root` — approved root unlocks subpath write
- `validate_write_path_session_root_still_respects_deny_list` — session root on deny-list path is rejected (defend-in-depth: `approve_write` would have blocked it, but validate also checks)
- `approve_write_rejects_filesystem_root` — `/` and `$HOME` rejected
- `approve_write_rejects_denied_path` — `~/.ssh/` rejected
- `approve_write_cleared_on_reactivation` — new `ActiveProject` has empty session roots

### Integration: `approve_write_then_edit_outside_project`

End-to-end: approve a temp dir, then `edit_file` a file in that dir — succeeds. Without prior `approve_write`, `edit_file` returns the hint message.

---

## Prompt Surface

`server_instructions.md`: add to the file-write tools section:

> `approve_write(path)` — grant write access to a directory outside the project root for this session. Required before `edit_file`/`create_file`/`edit_code` on out-of-project paths.

Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` — adding a new tool to
`server_instructions.md` means existing projects will not know `approve_write` exists
until their system prompt refreshes. The version bump triggers that refresh.

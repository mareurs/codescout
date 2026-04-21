# Cross-Process Write Serialization — Design

**Date:** 2026-04-17
**Status:** Design

## Problem

Multiple codescout MCP server instances running against the same project (e.g., two Claude Code sessions in different terminal windows on the same repo) can race on writes. Each instance holds its own `Arc<Mutex>`, which only protects writes within that process. Two instances calling `replace_symbol` or `edit_file` on the same file at nearly the same time can produce torn writes, lost updates, or — combined with the rmcp 0.1.5 cancellation race documented as **BUG-021** — crash the server.

Writes originating inside a single stdio session already serialize correctly via the existing in-process guard and the LLM's sequential tool calls. The gap is cross-process.

## Non-Goals

This spec covers **write atomicity** only. Two related problems are explicitly deferred to `docs/trackers/mcp-integration-ideas-2026-04.md`:

- **LSP read coherence across instances.** Instance A's `didChange` does not reach instance B's LSP client. A subsequent `hover` / `goto_definition` from B sees stale state. Solved cleanly by generalizing the Kotlin mux pattern to all LSPs — a separate, larger initiative.
- **Write queuing with symbol re-resolution.** On contention, a sophisticated scheduler could enqueue semantically-safe writes (`replace_symbol`, `insert_code`, `remove_symbol`) and re-resolve their targets against the post-write state. Out of scope; fail-fast is the first version.

## Design

### Lock mechanism

Per-project OS-level advisory file lock at `.codescout/write.lock`. File descriptor is opened once during `activate_project` and held for the lifetime of the `ActiveProject`. Writes `flock(LOCK_EX)` the fd before mutating, release after.

Release is automatic:
- On successful write — explicit `LOCK_UN` via RAII guard drop.
- On process crash — kernel releases the lock when the fd is closed.

This avoids the classic stale-lock-file problem of delete-on-release schemes.

### Crate

`fd-lock = "4"` — maintained, tiny, cross-platform (unix `flock`, Windows `LockFileEx`). Already a transitive dep of several things in the tree; adding it directly is low churn.

### Acquire point

Centralized in `CodeScoutServer::call_tool_inner` (`src/server.rs`). The function already knows the tool name; the existing write tool list at lines 1066-1079 becomes the gate condition:

```rust
const WRITE_TOOLS: &[&str] = &[
    "create_file", "edit_file",
    "replace_symbol", "insert_code", "remove_symbol", "rename_symbol",
    "memory",  // only write actions; see below
];
```

For `memory`, the gate triggers only when `action` is `write`, `remember`, `forget`, `delete`, or `refresh_anchors`. Read actions (`read`, `list`, `recall`) bypass the lock.

No per-tool changes. Each write tool's `call()` method stays untouched — the gate is one layer up.

### In-process layer

`ActiveProject` also gets `write_lock: Arc<tokio::sync::Mutex<()>>` for the single-instance case. The gate in `call_tool_inner` acquires both: inner async mutex first (cheap, yields), then the cross-process `flock`. Two cooperating guards; simpler than trying to make `fd-lock` async-friendly.

### Failure modes

1. **Cross-process contention** — another instance holds the lock for longer than the timeout (5s).
   → Return `RecoverableError { error: "another codescout instance is writing to this project — retry in a moment", … }`. `isError: false`, so sibling tool calls survive.

2. **Lock file I/O error** (permission denied, disk full on first open) — return `anyhow::bail!` — genuine bug or environment problem, fatal for this call.

3. **Process killed mid-write** — kernel releases the `flock` when the fd closes. Disk content may be partially written; that's a filesystem-level concern outside this spec. Most write tools use atomic rename-into-place (`tempfile::persist`), which handles this already.

### Timeout

5 seconds on the cross-process `flock` acquire. In-process async mutex has no explicit timeout (acquisitions are fast by construction). 5s was chosen because:

- Well above realistic write durations (typical `edit_file`: <100ms; `rename_symbol` with LSP roundtrip: up to ~2s).
- Short enough that a stuck holder on the other side surfaces quickly as a recoverable error rather than freezing the agent.
- Aligns with the existing `tool_timeout_secs` defaults.

Configurable via `security.write_lock_timeout_secs` in `.codescout/project.toml`, defaulting to 5.

### LSP notification

After the write completes and before the lock is released, the writing tool sends `textDocument/didChange` to its own LSP client (unchanged from today). Other instances' LSP clients do not receive this notification — that's the read-coherence gap tracked as a follow-up exploration, not solved here.

### Observability

One new `tracing` span per write: `write_lock.acquired` (level info, with `tool`, `project_id`, `wait_ms`). Contention surfaces via log noise during development and is available for the doctor tool.

## Architecture Diagram

```
Claude Code Instance A                Claude Code Instance B
        │                                     │
        ▼                                     ▼
  codescout process A                   codescout process B
        │                                     │
        │   call_tool_inner("edit_file")     │   call_tool_inner("edit_file")
        │   ├─ acquire in-process mutex ✓    │   ├─ acquire in-process mutex ✓
        │   ├─ acquire flock(LOCK_EX) ✓      │   ├─ acquire flock(LOCK_EX) ⏳ waits
        │   │                                │   │
        │   ├─ mutate file                   │   │
        │   ├─ LSP didChange                 │   │
        │   └─ release flock ───────────────▶│   ├─ (acquired after A releases)
        │                                    │   ├─ mutate file
        │                                    │   ├─ LSP didChange (its own client)
        │                                    │   └─ release flock
        ▼                                    ▼
              .codescout/write.lock (shared, advisory)
```

## Components Touched

| File | Change |
|---|---|
| `Cargo.toml` | add `fd-lock = "4"` |
| `src/agent.rs` | `ActiveProject` gains `write_lock: Arc<tokio::sync::Mutex<()>>` and `file_lock: Arc<tokio::sync::Mutex<fd_lock::RwLock<File>>>`; `Agent::activate_project` opens the lock file |
| `src/server.rs` | `call_tool_inner` wraps write-tool dispatch in lock acquire/release; new `WRITE_TOOLS` const and `is_write_call(tool_name, input)` helper |
| `src/config/project.rs` | `SecuritySection` gains `write_lock_timeout_secs: Option<u64>` with default |
| `docs/trackers/mcp-integration-ideas-2026-04.md` | new `## Explorations` section noting the mux-for-all-LSPs and queuing algorithms |

## Testing Strategy

### Unit
- `write_lock_acquire_contention_fails_after_timeout` — open two instances of the lock on one process, assert second acquisition returns `RecoverableError`.
- `write_lock_released_on_guard_drop` — acquire, drop, acquire again succeeds.
- `is_write_call_classifies_memory_actions` — `memory` with `action: write` is a write; `action: read` is not.

### Integration
- `tests/cross_process_write_lock.rs` — spawn two codescout binaries against a shared temp project, have both call `edit_file` in parallel, assert one succeeds and one returns recoverable error, then assert the successful call retries cleanly.
- Add a smoke test in `tests/mcp-smoke-rust.sh` that exercises the lock contention path via `cargo run start --transport http`.

### Manual
- Two terminal windows, both running `claude` on this repo. Fire an `edit_file` on the same file from each. Observe clean error on the loser, successful write on the winner.

## Risks

1. **NFS / exotic filesystems.** `flock` on NFS is unreliable on some kernels. Out of scope — codescout is aimed at developer laptops, not NFS. Document the limitation.
2. **Forgotten write paths.** If a new write tool is added without being placed in `WRITE_TOOLS`, it silently bypasses the lock. Mitigation: a test that enumerates all tools implementing a future `Tool::is_mutating()` marker and asserts `WRITE_TOOLS` matches. Cheap insurance.
3. **Deadlock across the two mutexes.** Always acquire in the same order (in-process first, then `flock`). Documented in the helper's rustdoc.
4. **Lock file collides with existing project state.** `.codescout/write.lock` is a new path; grep confirms no existing use. Adding it to `.gitignore` keeps it out of commits.

## Rollout

Single PR. No feature flag — the lock is always on once merged. Rollback is a revert.

## Follow-up Explorations (tracked, not scoped here)

1. **Generalized LSP mux** — extend the current Kotlin-only mux pattern in `src/lsp/mux/` to all languages, giving cross-instance read coherence and eliminating the split-brain problem. Estimated: one week, rolled out per-language (Rust → Python → TS/JS).
2. **Symbol-safe write queuing** — on contention, enqueue semantic writes (`replace_symbol`, `insert_code`, `remove_symbol`) and re-resolve their targets against post-write state. Fail-fast line-based edits. Needs a per-file scheduler and a formal notion of "queue-safe."
3. **Per-file lock granularity** — today: one lock per project. Refinement would be a `DashMap<PathBuf, FileLock>` so writes to different files don't block. Low priority — contention is rare and the cost of over-serializing is a short wait.

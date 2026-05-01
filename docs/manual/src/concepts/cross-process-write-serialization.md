# Cross-Process Write Serialization
When multiple codescout MCP server instances run against the same project directory simultaneously (e.g. two IDE windows, two Claude Code sessions, or a background indexer alongside an active session), write-tool calls are now serialized so they cannot corrupt each other.

## How It Works

Two lock layers protect each write:

1. **In-process mutex** — `tokio::sync::Mutex<()>` per `ActiveProject`. Acquired first. Serializes concurrent write-tool calls within a single codescout process.
2. **Cross-process flock** — `fs4` advisory lock on `.codescout/write.lock`. Acquired second. Serializes writes across all codescout instances on the same project.

The lock is held for the full duration of the tool call and released on drop (`WriteGuard` RAII).

## Behavior on Contention

If a write-tool call cannot acquire the cross-process lock within the timeout, it returns a `RecoverableError` (MCP `isError: false`). The agent receives a message:

```
another codescout instance is writing to this project
Retry in a moment — the holder should release shortly.
```

Because `isError` is false, sibling tool calls in the same MCP batch continue normally.

## Configuration

Set `write_lock_timeout_secs` in `.codescout/config.toml` under `[security]`:

```toml
[security]
write_lock_timeout_secs = 10  # default: 5
```

## Covered Write Tools

The following tools acquire the write lock: `create_file`, `edit_file`, `edit_markdown`, `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol`, and `memory` write actions (`write`, `remember`, `forget`, `delete`, `refresh_anchors`).

Read tools (`read_file`, `symbols`, `symbols`, etc.) skip the lock entirely — no overhead on reads.

## Lock File

The lock file lives at `.codescout/write.lock` inside the project root. It is created automatically on `workspace(action: activate)` and is gitignored.

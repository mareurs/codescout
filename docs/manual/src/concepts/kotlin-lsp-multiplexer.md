# Kotlin LSP Multiplexer

## Problem

Multiple codescout instances targeting the same Kotlin project cause severe
degradation. JetBrains' kotlin-lsp allows only one LSP process per workspace,
and two instances compete for Gradle daemon locks, consuming 3-4GB RAM with
duplicate project models and causing 120s+ timeouts.

## Solution

codescout now runs a **detached multiplexer process** (`codescout mux`) that
manages a single kotlin-lsp instance and allows multiple codescout sessions to
share it via a Unix socket.

```
┌─────────────┐     ┌─────────────┐
│ codescout-A  │     │ codescout-B  │
└──────┬───────┘     └──────┬───────┘
       │ Unix socket        │
       └────────────┬───────┘
                    │
          ┌─────────▼──────────┐
          │  codescout mux     │
          └─────────┬──────────┘
                    │ stdio
          ┌─────────▼──────────┐
          │   kotlin-lsp       │
          │  (single JVM)      │
          └────────────────────┘
```


## Activation

**The multiplexer is automatic — no configuration required.** When codescout
detects that a project uses Kotlin, it starts or connects to a mux process
transparently. No flags, no `project.toml` changes.

The `codescout mux` sub-command runs the mux process directly (for debugging),
but in normal use it is spawned by codescout itself.


## JVM Pre-warming

When a project declares `java` or `kotlin` in its language list, codescout spawns background LSP `get_or_start` tasks immediately on **server startup** and on every **`workspace(action: activate)`** call.

```toml
# .codescout/project.toml
[project]
languages = ["kotlin"]  # also triggers for "java"
```

Pre-warming eliminates the 8–15 s cold-start penalty that would otherwise occur on the first symbol query after startup. The warm-up runs in the background — server startup and `workspace(action: activate)` return immediately without waiting for the LSP to be ready.

**Concurrency safety:** `LspManager`'s watch-channel serialises parallel starters. Calling `workspace(action: activate)` from concurrent sessions cannot trigger duplicate LSP processes.

The multiplexer handles the rest of the connection lifecycle — see [How It Works](#how-it-works) below.
## How It Works

1. **First codescout instance** needing Kotlin LSP acquires an exclusive file
   lock (`flock`), spawns `codescout mux`, and connects as a client.
2. **Subsequent instances** find the lock held, skip spawning, and connect
   directly to the existing mux socket.
3. The mux handles **ID remapping** (so two clients can both send request ID 1
   without collision), **document state dedup** (didOpen/didClose tracked per
   client), and **version rewriting** (monotonic per-URI versions for didChange).
4. When all clients disconnect, the mux stays alive for 5 minutes (idle timeout),
   then shuts down kotlin-lsp and exits.

## Ownership & Crash Recovery

The mux process holds an exclusive `flock` on a lock file for its entire
lifetime. If it dies — even via SIGKILL or OOM — the OS releases the lock.
The next codescout instance detects the stale lock, cleans up, and spawns a
fresh mux. No PID files, no heartbeats, no race conditions.

## Gradle Isolation

Independently of the mux, kotlin-lsp now runs with an isolated
`GRADLE_USER_HOME` to prevent Gradle daemon cache lock contention between
instances.

## Benefits

| Metric | Before (2 instances) | After (2 instances) |
|--------|---------------------|---------------------|
| kotlin-lsp JVMs | 2 (~3-4GB total) | 1 (~2GB) |
| Gradle daemons | 2 (competing) | 1 (shared) |
| Cold start on 2nd session | 8-15s | 0s (mux already warm) |
| Typical LSP response | 120s+ timeout | 30-270ms |

## Limitations

- **Unix only** — uses Unix domain sockets. Windows support (named pipes) is
  planned but not yet implemented.
- **Kotlin only** — other languages use direct LSP connections. The `mux` flag
  in `LspServerConfig` makes it easy to opt in additional languages (e.g., jdtls
  for Java) in the future.
- **Concurrent file edits** — if two clients edit the same file simultaneously,
  the document state in kotlin-lsp may desync. This is inherent to LSP's
  single-client design and is acceptable since two agents editing the same file
  is already a bug.
- **Rename serialization** — `workspace/applyEdit` (used by rename) is routed to
  the client that initiated the rename. Concurrent renames from different clients
  are serialized through an edit lock.

## Diagnostics

The mux spawn/connect is visible in codescout diagnostic logs:

```
INFO codescout::lsp::manager: mux process ready for kotlin at "/tmp/codescout-kotlin-mux-<hash>.sock"
INFO codescout::lsp::manager: mux already running for kotlin, connecting to "/tmp/codescout-kotlin-mux-<hash>.sock"
```

The mux process itself logs to `.codescout/mux-kotlin-<hash>.log` (or `/tmp/`
if `.codescout/` does not exist in the workspace).

## Design

Full design spec: `docs/superpowers/specs/2026-03-24-kotlin-lsp-multiplexer-design.md`

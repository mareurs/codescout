# `peer`

Delegate read-only exploration to another codescout instance that owns a
different project.

> **Opt-in, not registered by default.** Set `CODESCOUT_PEER_ENABLED=1`, or
> `[peer]\nenabled = true` in `<project>/.codescout/project.toml`, to register
> the tool. Same layered-default shape as `LIBRARIAN_ENABLED` (env var
> overrides config, either direction), opposite resting state: `peer` defaults
> to **off**. Measured 2026-08-26 across every `.codescout/usage.db` on this
> machine — 29 projects, every session that has ever run here — the tool was
> called twice, ever, one of those an error. An opt-out default was exposing
> its schema and description to every session for a feature almost nobody
> reaches for.

## Why

Cross-project questions ("how does the backend call this endpoint?") need a
symbol index for a project this instance does not own. The alternatives are
activating the foreign project — which swaps your own project state out from
under you — or duplicating its index here. `peer` instead asks the instance that
already owns that project, and returns only the answer.

## The registry

Peers are declared in `<project>/.codescout/peers.toml`:

```toml
[[peer]]
id = "backend"
target = "/home/u/projB"
description = "The payments backend — Rust, axum, sqlx"
default_access = "ro"

[[peer]]
id = "frontend"
target = "/home/u/projC"
description = "Web client — TypeScript, React"
default_access = "rw"
```

A missing file is an empty registry, not an error. `description` is what the
agent reads to decide which peer can answer a question, so write it for that
purpose — languages and responsibilities, not prose.

## Actions

```text
peer(action="status")
```

Lists configured peers with their `id`, `description`, and whether the grant is
read-only.

```text
peer(action="query", peer="backend", tool="symbols", args={name: "charge_card"})
```

Runs one of the peer's read tools and returns its result. `action="explore"` is
an alias for the same thing.

```text
peer(action="knowledge", peer="backend", handle="@tool_abc123")
```

Fetches a buffer handle from the peer — the peer's own progressive-disclosure
buffers are addressable across the boundary, so a large result does not have to
cross it inline.

## What a peer will run — deny-by-default

Phase 1 is read-only delegation, and the exposed set is an explicit allowlist:

`symbols` · `symbol_at` · `references` · `call_graph` · `tree` · `grep` ·
`semantic_search` · `read_file` · `get_guide`

Every other tool — all writes, `run_command`, `workspace`, and every librarian
mutation — is rejected by construction. That is deliberately independent of the
peer's own `read_only` grant: an allowlist stays safe even if some tool's
`Tool::is_write` classification is wrong, whereas a deny-list inherits every
such gap. `default_access = "rw"` therefore does **not** currently buy write
delegation; write delegation is a later phase with its own curated set.

## Process lifecycle

The first `query`/`knowledge` against a peer launches a `codescout serve`
process for its `target` if one is not already listening, connects over a unix
socket whose name includes `codescout-peer-`, and shuts it down after an idle
timeout (`PEER_IDLE_TIMEOUT_SECS`). No manual start or stop.

## Where this lives

`src/tools/peer.rs` (the tool), `src/peer/registry.rs` (`peers.toml` parsing),
`src/peer/server.rs` (`PEER_EXPOSED_TOOLS` — the allowlist above),
`src/peer/launch.rs` (process reuse and idle shutdown), `src/peer/protocol.rs`
(envelope and error codes), `src/server.rs` (`peer_enabled_at_runtime` — the
opt-in gate above).

# Peer Delegation — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a requesting agent run a *remote codescout instance's* read tools over a per-workspace Unix socket — `peer.call_tool("symbols", …)` against project B — with the read/write wall enforced by codescout's existing dispatch gate.

**Architecture:** A standalone `codescout peer-serve --workspace B` process (mirroring the existing hidden `codescout mux` process) boots a `CodeScoutServer` for project B, activates it read-only or read-write per the registry, binds a per-user Unix socket (`codescout-<hash>-peer.sock`), and serves a small JSON envelope: `hello`, `tool.call`, `buffer.read`, `buffer.grep`. The requester side is a `PeerClient` plus a new `peer` MCP tool. Every `tool.call` routes through the server's existing `call_tool_inner`, so access checks, the write-guard, usage recording, and error routing are inherited — not re-implemented.

**Tech Stack:** Rust, tokio (`UnixListener`/`UnixStream`), serde_json, the existing `lsp::transport` framing (`Content-Length`-delimited JSON), rmcp types (`CallToolRequestParams`/`CallToolResult`), `async_trait`.

---

## ⚠️ Standing constraint — existing-file edits are approval-gated

The repository owner requires explicit approval before **any** modification to existing `src/` files. This plan creates several new files (`src/peer/*.rs`, `src/socket_discovery.rs`, `src/tools/peer.rs`) and modifies four existing ones. Tasks that touch an existing file are marked **⚠️ APPROVAL** in their header and must pause for the owner's go-ahead before the edit step. New-file tasks still warrant a heads-up since they land in `src/`, but carry the lighter ⚠️-free header.

**Existing files modified (all ⚠️):**
- `src/lib.rs` — register two new modules
- `src/lsp/mux/mod.rs` — move discovery helpers to the shared module, re-export for back-compat
- `src/server.rs` — expose a `pub(crate)` dispatch wrapper; register the requester-side `peer` tool
- `src/main.rs` — add the hidden `peer-serve` subcommand

---

## Reuse map — what transfers from the LSP mux, corrected against live code

Reconnaissance (2026-06-01) verified each row against the current source. The design spec's Section-2 table over-claimed "reusable verbatim" on three rows; the truth is below.

| Asset | Live location | Phase-1 action |
|---|---|---|
| `Content-Length` JSON framing | `lsp::transport::{read_message,write_message}` — generic `<R: AsyncBufReadExt>` / `<W: AsyncWriteExt>`, return `Result<Value>` | **Import verbatim.** |
| Request-id tagging (`"tag:id"`) | `lsp::mux::protocol::tag_request_id` | **Not needed in Phase 1** — synchronous 1-req/1-resp per connection. Returns in Phase 2 (events/jobs). |
| `workspace_hash` | `lsp::mux::workspace_hash` — already `pub` | **Move** to `src/socket_discovery.rs`, re-export. |
| Per-user socket dir | `lsp::mux::per_user_mux_dir` — **private `fn`** | **Move + rename** to `socket_discovery::per_user_runtime_dir`, re-export as `per_user_mux_dir`. |
| `socket_path_for_workspace(language, root)` | `lsp::mux::mod.rs:20` | **Not callable for peers** — bakes in `language` + `-mux-`. Write a new `peer_socket_path_for_workspace`. |
| `retry_on_mux_disconnect` | `fs::mod.rs:308` — coupled to `&dyn LspProvider` | **Pattern only.** Phase 1 surfaces connect failure as `RecoverableError`; a generic retry waits for Phase 2. |
| Tool dispatch | `CodeScoutServer::call_tool_inner` (`src/server.rs:513`, **private**) over `tools: Vec<Arc<dyn Tool>>` | **Route through it** via a new `pub(crate)` wrapper. Inherits `check_tool_access` + write-guard. |
| `OutputBuffer` | `src/tools/output_buffer.rs` — `OutputBuffer::new(50)`, `get(id) -> Option<BufferEntry>` | **Proxy** `get`. (Spec said `new(20)` — stale; it is 50.) |
| RO/RW wall | `Agent::activate(root, Some(read_only))` (`src/agent/mod.rs:497`) | **Set the boolean** from the registry; the write-guard enforces it. |

---

## File structure

```
src/
  socket_discovery.rs        NEW  workspace_hash, per_user_runtime_dir, peer_socket_path_for_workspace
  peer/
    mod.rs                   NEW  module root, PROTOCOL_VERSION, re-exports
    protocol.rs              NEW  PeerEnvelope, EnvelopeKind, PeerError, Capabilities, error codes
    server.rs                NEW  peer-serve: boot server-for-B, bind socket, accept loop, handlers, audit
    client.rs                NEW  PeerClient: connect, hello, call_tool, read_buffer/grep_buffer, @peer handles
    registry.rs              NEW  registry file (id → {target, description, default_access}) parse
  tools/
    peer.rs                  NEW  the `peer` MCP tool (requester-facing)
  lib.rs                     MOD  pub mod socket_discovery; pub mod peer;
  lsp/mux/mod.rs             MOD  re-export moved discovery helpers
  server.rs                  MOD  pub(crate) call_tool_by_name; register PeerTool
  main.rs                    MOD  Commands::PeerServe { … }
```

One file = one responsibility: `protocol.rs` owns the wire shape, `server.rs` owns serving, `client.rs` owns requesting, `registry.rs` owns peer addressing. The serving and requesting halves never share state — only the envelope type.

---

### Task 1: Extract socket discovery into a shared module ⚠️ APPROVAL

Moves the two genuinely-reusable discovery helpers out of `lsp::mux` (where `per_user_mux_dir` is private and the module name implies "LSP only") into a transport-neutral module both `lsp::mux` and `peer` import. Two concrete consumers now exist (mux socket + peer socket), which is what justifies the extraction.

**Files:**
- Create: `src/socket_discovery.rs`
- Modify: `src/lib.rs:33` (add `pub mod socket_discovery;` before `pub mod lsp;`)
- Modify: `src/lsp/mux/mod.rs:14-26` (replace `workspace_hash` body + `per_user_mux_dir` usage with re-exports)
- Test: inline `#[cfg(test)] mod tests` in `src/socket_discovery.rs`

- [ ] **Step 1: Write the failing test**

```rust
// src/socket_discovery.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn peer_socket_differs_from_mux_and_shares_dir() {
        let root = Path::new("/home/u/projB");
        let peer = peer_socket_path_for_workspace(root);
        let name = peer.file_name().unwrap().to_str().unwrap();

        // Peer socket carries the -peer- infix and the workspace hash.
        assert!(name.starts_with("codescout-"), "got {name}");
        assert!(name.contains("-peer-"), "expected -peer- infix, got {name}");
        assert!(name.contains(&workspace_hash(root)), "must embed the hash");
        assert!(!name.contains("-mux-"), "must not collide with the mux name");

        // Lives in the same per-user runtime dir as mux sockets.
        assert_eq!(peer.parent().unwrap(), per_user_runtime_dir());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib socket_discovery::tests::peer_socket_differs_from_mux_and_shares_dir`
Expected: FAIL — `cannot find function peer_socket_path_for_workspace` (module does not exist yet).

- [ ] **Step 3: Write the module**

```rust
// src/socket_discovery.rs
//! Per-user socket-path discovery, shared by the LSP mux (`lsp::mux`) and the
//! peer-delegation server (`peer`). Transport-neutral: knows about per-user
//! runtime directories and workspace hashing, nothing about LSP or peers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Stable-within-a-build hash of a workspace root. Both halves of a socket
/// pair (caller and listener) must run the same binary, so `DefaultHasher`'s
/// lack of cross-version stability is irrelevant.
pub fn workspace_hash(workspace_root: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    workspace_root.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// A directory for socket/lock files private to the current user.
///
/// Unix: prefers `$XDG_RUNTIME_DIR` (typically `/run/user/$UID`, mode `0700`).
/// Falls back to a `0700` subdir under `$TMPDIR` keyed by UID. Windows: temp dir.
pub fn per_user_runtime_dir() -> PathBuf {
    #[cfg(unix)]
    {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            let p = PathBuf::from(dir);
            if p.exists() {
                return p;
            }
        }
        use std::os::unix::fs::DirBuilderExt;
        // SAFETY: getuid is always safe; returns the real UID.
        let uid = unsafe { libc::getuid() };
        let dir = std::env::temp_dir().join(format!("codescout-{uid}"));
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir);
        dir
    }
    #[cfg(not(unix))]
    {
        std::env::temp_dir()
    }
}

/// Socket a peer-serve process for `workspace_root` listens on. Distinct from
/// the mux socket (`-mux-`) so the two coexist in the same per-user dir.
pub fn peer_socket_path_for_workspace(workspace_root: &Path) -> PathBuf {
    per_user_runtime_dir().join(format!(
        "codescout-peer-{}.sock",
        workspace_hash(workspace_root)
    ))
}

/// Lock file guarding a single peer-serve instance per workspace.
pub fn peer_lock_path_for_workspace(workspace_root: &Path) -> PathBuf {
    per_user_runtime_dir().join(format!(
        "codescout-peer-{}.lock",
        workspace_hash(workspace_root)
    ))
}
```

> Note: the socket name is `codescout-peer-<hash>.sock`. The test asserts `-peer-` is present; `codescout-peer-<hash>` satisfies it.

- [ ] **Step 4: Re-export from `lsp::mux` for back-compat (⚠️ existing file)**

In `src/lsp/mux/mod.rs`, delete the local `workspace_hash` (lines 14-18) and `per_user_mux_dir` (lines 46-70) and replace with re-exports so existing callers (`socket_path_for_workspace`, `lock_path_for_workspace`) keep compiling:

```rust
// src/lsp/mux/mod.rs — near the top, replacing the two removed fns
pub use crate::socket_discovery::workspace_hash;
use crate::socket_discovery::per_user_runtime_dir as per_user_mux_dir;
```

Leave `socket_path_for_workspace` and `lock_path_for_workspace` unchanged — they call `per_user_mux_dir()` and `workspace_hash()`, now resolved via the re-exports.

- [ ] **Step 5: Register the module (⚠️ existing file)**

In `src/lib.rs`, add before `pub mod lsp;` (line 33):

```rust
pub mod socket_discovery;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib socket_discovery && cargo test --lib lsp::mux`
Expected: PASS — new test green, mux tests still green (re-export transparent).

- [ ] **Step 7: Commit**

```bash
git add src/socket_discovery.rs src/lib.rs src/lsp/mux/mod.rs
git commit -m "refactor(socket): extract per-user discovery into socket_discovery; add peer socket path"
```

---

### Task 2: Peer envelope protocol types

The wire vocabulary. Pure new code — no existing file touched.

**Files:**
- Create: `src/peer/protocol.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/protocol.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_envelope_round_trips() {
        let env = PeerEnvelope {
            v: PROTOCOL_VERSION,
            id: "a:1".into(),
            kind: EnvelopeKind::Request,
            method: Some("tool.call".into()),
            params: Some(json!({ "tool": "symbols", "args": { "path": "src" } })),
            result: None,
            error: None,
        };
        let wire = serde_json::to_value(&env).unwrap();
        assert_eq!(wire["kind"], "request");
        assert_eq!(wire["method"], "tool.call");
        // null fields are omitted, keeping the frame small.
        assert!(wire.get("result").is_none());
        assert!(wire.get("error").is_none());

        let back: PeerEnvelope = serde_json::from_value(wire).unwrap();
        assert_eq!(back.method.as_deref(), Some("tool.call"));
    }

    #[test]
    fn error_envelope_carries_code() {
        let env = PeerEnvelope::error("a:1", PeerError {
            code: ErrorCode::AccessDenied,
            message: "peer is read-only".into(),
            data: None,
        });
        let wire = serde_json::to_value(&env).unwrap();
        assert_eq!(wire["kind"], "error");
        assert_eq!(wire["error"]["code"], "access_denied");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::protocol`
Expected: FAIL — `src/peer/protocol.rs` and types do not exist.

- [ ] **Step 3: Write the protocol module**

```rust
// src/peer/protocol.rs
//! Wire envelope for the peer-delegation protocol. Networked-ready: pure JSON,
//! no Unix-socket assumptions. A future TCP/TLS transport swaps only the
//! connect/listen layer, not this type.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bumped on any breaking change to the envelope or method set.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnvelopeKind {
    Request,
    Response,
    Event,
    Error,
}

/// The single message type on the wire. `serde(skip_serializing_if)` keeps
/// frames minimal (null fields omitted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEnvelope {
    pub v: u32,
    pub id: String,
    pub kind: EnvelopeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PeerError>,
}

impl PeerEnvelope {
    pub fn request(id: impl Into<String>, method: &str, params: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Request,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    pub fn response(id: impl Into<String>, result: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Response,
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: impl Into<String>, error: PeerError) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Error,
            method: None,
            params: None,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Closed taxonomy. Maps onto codescout's `RecoverableError` on the requester
/// side (see Task 11) so a peer's bad-input failure does not abort the
/// requester's sibling tool calls.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Envelope `v` not understood by the peer.
    VersionMismatch,
    /// `method` not one this peer serves.
    UnknownMethod,
    /// Tool name not found in the peer's registry.
    UnknownTool,
    /// Write tool attempted against a read-only peer.
    AccessDenied,
    /// Tool ran but returned an `isError` result; `data` carries the body.
    ToolError,
    /// Buffer handle not present in the peer's OutputBuffer.
    UnknownHandle,
    /// Malformed params for the method.
    BadParams,
}

/// Advertised by `hello`. Lets the requester avoid sending a write tool to a
/// read-only peer (defence-in-depth on top of the peer-side wall).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub project: String,
    pub root: String,
    pub read_only: bool,
    /// Tool names this peer serves (the peer's own registry).
    pub tools: Vec<String>,
    /// Phase 2: whether an attached executor can run async jobs. Always false in Phase 1.
    pub executor_available: bool,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib peer::protocol`
Expected: FAIL — `peer` module not registered in `lib.rs` yet (Task 3). If run in isolation before Task 3, it fails to compile the module path. Proceed to Task 3, then re-run.

- [ ] **Step 5: Commit**

```bash
git add src/peer/protocol.rs
git commit -m "feat(peer): wire envelope + error taxonomy + capabilities"
```

---

### Task 3: Register the peer module ⚠️ APPROVAL

**Files:**
- Create: `src/peer/mod.rs`
- Modify: `src/lib.rs` (add `pub mod peer;` after `pub mod migrate;` line 36)

- [ ] **Step 1: Write `src/peer/mod.rs`**

```rust
// src/peer/mod.rs
//! Peer delegation: run another codescout instance's read tools over a
//! per-workspace Unix socket. Phase 1 = synchronous remote tools.
//!
//! - `protocol` — the wire envelope (shared by both halves)
//! - `server`   — the `codescout peer-serve` process serving a workspace
//! - `client`   — the requester side, used by the `peer` MCP tool
//! - `registry` — peer addressing (id → target/description/access)

pub mod client;
pub mod protocol;
pub mod registry;
pub mod server;

pub use protocol::{Capabilities, EnvelopeKind, ErrorCode, PeerEnvelope, PeerError, PROTOCOL_VERSION};

/// Re-exported for convenience; the canonical home is `socket_discovery`.
pub use crate::socket_discovery::{peer_lock_path_for_workspace, peer_socket_path_for_workspace};
```

- [ ] **Step 2: Register in `lib.rs` (⚠️ existing file)**

Add after line 36 (`pub mod migrate;`), before `pub mod platform;`:

```rust
pub mod peer;
```

- [ ] **Step 3: Verify the tree compiles (modules referenced exist as stubs)**

Because `mod.rs` declares `client`/`registry`/`server`, create minimal stubs so the crate compiles between tasks:

```rust
// src/peer/server.rs  (stub — fleshed out in Tasks 5-9)
// src/peer/client.rs  (stub — fleshed out in Task 11)
// src/peer/registry.rs (stub — fleshed out in Task 10)
```

Each stub is a single doc comment line until its task. Create the three empty files now.

Run: `cargo build --lib`
Expected: PASS (empty modules compile).

- [ ] **Step 4: Re-run Task 2's tests now that the module path exists**

Run: `cargo test --lib peer::protocol`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/peer/mod.rs src/peer/server.rs src/peer/client.rs src/peer/registry.rs src/lib.rs
git commit -m "feat(peer): register module tree"
```

---

### Task 4: Expose a dispatch wrapper on the server ⚠️ APPROVAL

`call_tool_inner` is private and takes rmcp-specific params. The peer server needs a name+args entry that carries no rmcp coupling but inherits all of `call_tool_inner`'s gates.

**Files:**
- Modify: `src/server.rs` (add a method inside `impl CodeScoutServer`, near `call_tool_inner` at line 513)
- Test: add to the existing `mod tests` in `src/server.rs` (the file already has `tool_by_name`/`shared_ctx` helpers and `make_server()`)

- [ ] **Step 1: Write the failing test**

```rust
// src/server.rs — inside #[cfg(test)] mod tests
#[tokio::test]
async fn call_tool_by_name_dispatches_a_read_tool() {
    let (_dir, server) = make_server().await;
    let result = server
        .call_tool_by_name("tree", serde_json::json!({ "path": "." }))
        .await
        .expect("dispatch ok");
    // tree returns content blocks; success path has is_error None/false.
    assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");
}

#[tokio::test]
async fn call_tool_by_name_rejects_unknown_tool() {
    let (_dir, server) = make_server().await;
    let err = server
        .call_tool_by_name("does_not_exist", serde_json::json!({}))
        .await;
    assert!(err.is_err(), "unknown tool must error");
}
```

> If `make_server()` returns a 2-tuple in some tests and a 3-tuple (`_env`) in others, match the arity used by the surrounding `mod tests` block — check the helper signature before writing.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib server::tests::call_tool_by_name_dispatches_a_read_tool`
Expected: FAIL — `no method named call_tool_by_name`.

- [ ] **Step 3: Write the wrapper**

```rust
// src/server.rs — inside impl CodeScoutServer, immediately above call_tool_inner
/// Dispatch a tool by name with raw JSON args, returning the full
/// `CallToolResult`. Routes through `call_tool_inner`, so access checks, the
/// write-guard, usage recording, and error routing all apply. Used by the
/// peer-serve endpoint; carries no rmcp request/progress/peer coupling.
pub(crate) async fn call_tool_by_name(
    &self,
    name: &str,
    args: Value,
) -> std::result::Result<CallToolResult, McpError> {
    let req = CallToolRequestParams {
        name: name.to_string().into(),
        arguments: args.as_object().cloned(),
    };
    self.call_tool_inner(
        req,
        None, // no progress reporter — peer calls are synchronous
        None, // no rmcp Peer — not driven by an MCP client
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}
```

> Confirm `CallToolRequestParams`'s field names/types at the `use` site (top of `src/server.rs`); the struct comes from `rmcp::model`. `name` is an `Arc<str>`-like type — `.into()` from `String` is the established pattern (see how tests build requests, or how `call_tool` forwards `req`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib server::tests::call_tool_by_name`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
git add src/server.rs
git commit -m "feat(server): pub(crate) call_tool_by_name wrapper for peer dispatch"
```

---

### Task 5: Peer-serve — boot a server for B, bind the socket, handle `hello`

The serve process: construct an `Agent` for workspace B, activate it (read-only per the `read_only` arg), build a `CodeScoutServer`, bind the peer socket, and answer `hello` with capabilities. `tool.call`/`buffer.*` arrive in Tasks 6-7.

**Files:**
- Modify: `src/peer/server.rs` (replace the stub)
- Test: inline `#[cfg(test)] mod tests` using a tempdir workspace + a raw client over the socket

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/server.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::transport::{read_message, write_message};
    use crate::peer::protocol::{PeerEnvelope, PROTOCOL_VERSION};
    use tokio::io::BufReader;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn hello_returns_capabilities_for_read_only_peer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let sock = root.join("peer.sock");

        // Spawn the serve loop in the background, read-only.
        let serve_sock = sock.clone();
        let serve_root = root.clone();
        let handle = tokio::spawn(async move {
            let server = build_server_for(&serve_root, true).await.unwrap();
            let listener = bind_peer_socket(&serve_sock).unwrap();
            // Accept exactly one connection for the test, then return.
            accept_one(&listener, &server).await.unwrap();
        });

        // Give the listener a moment, then connect and say hello.
        let stream = connect_with_retry(&sock).await;
        let (rd, mut wr) = stream.into_split();
        let mut rd = BufReader::new(rd);

        let hello = PeerEnvelope::request("a:1", "hello", serde_json::json!({}));
        write_message(&mut wr, &serde_json::to_value(&hello).unwrap())
            .await
            .unwrap();

        let resp: PeerEnvelope =
            serde_json::from_value(read_message(&mut rd).await.unwrap()).unwrap();
        let caps = resp.result.unwrap();
        assert_eq!(caps["read_only"], true);
        assert_eq!(resp.v, PROTOCOL_VERSION);
        assert!(caps["tools"].as_array().unwrap().iter().any(|t| t == "symbols"));

        handle.abort();
    }

    async fn connect_with_retry(sock: &std::path::Path) -> UnixStream {
        for _ in 0..50 {
            if let Ok(s) = UnixStream::connect(sock).await {
                return s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("peer socket never came up");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::server::tests::hello_returns_capabilities_for_read_only_peer`
Expected: FAIL — `build_server_for`, `bind_peer_socket`, `accept_one` undefined.

- [ ] **Step 3: Write the serve scaffolding**

```rust
// src/peer/server.rs
//! The `codescout peer-serve` process: serve one workspace's read tools over a
//! per-workspace Unix socket.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};

use crate::agent::Agent;
use crate::lsp::transport::{read_message, write_message};
use crate::peer::protocol::{Capabilities, EnvelopeKind, ErrorCode, PeerEnvelope, PeerError};
use crate::server::CodeScoutServer;

/// Construct a `CodeScoutServer` bound to `root`, activated read-only or rw.
/// The read-only flag is the RO/RW wall lever — the existing write-guard in
/// `call_tool_inner` enforces it from here on.
pub async fn build_server_for(root: &Path, read_only: bool) -> Result<Arc<CodeScoutServer>> {
    let agent = Agent::new(Some(root.to_path_buf()))
        .await
        .context("failed to construct agent for peer workspace")?;
    agent
        .activate(root.to_path_buf(), Some(read_only))
        .await
        .context("failed to activate peer workspace")?;
    Ok(Arc::new(CodeScoutServer::new(agent).await))
}

/// Bind the per-user peer socket, restricted to mode 0600.
pub fn bind_peer_socket(socket_path: &Path) -> Result<UnixListener> {
    if socket_path.exists() {
        std::fs::remove_file(socket_path).ok();
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind peer socket: {}", socket_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(listener)
}

/// Accept a single connection and serve it to completion. Used by tests and by
/// the (sequential, Phase-1) accept loop.
pub async fn accept_one(listener: &UnixListener, server: &Arc<CodeScoutServer>) -> Result<()> {
    let (stream, _addr) = listener.accept().await?;
    serve_connection(stream, server).await
}

/// Serve one client connection: read envelopes, dispatch, write responses,
/// until the client disconnects (EOF).
async fn serve_connection(stream: UnixStream, server: &Arc<CodeScoutServer>) -> Result<()> {
    let (rd, mut wr) = stream.into_split();
    let mut rd = BufReader::new(rd);

    loop {
        let msg = match read_message(&mut rd).await {
            Ok(m) => m,
            Err(_) => return Ok(()), // EOF / client gone
        };
        let env: PeerEnvelope = match serde_json::from_value(msg) {
            Ok(e) => e,
            Err(e) => {
                let err = PeerEnvelope::error(
                    "0",
                    PeerError { code: ErrorCode::BadParams, message: e.to_string(), data: None },
                );
                write_message(&mut wr, &serde_json::to_value(&err)?).await?;
                continue;
            }
        };

        let reply = dispatch_envelope(&env, server).await;
        write_message(&mut wr, &serde_json::to_value(&reply)?).await?;
    }
}

/// Route one request envelope to its handler. Tasks 6-7 extend the match.
async fn dispatch_envelope(env: &PeerEnvelope, server: &Arc<CodeScoutServer>) -> PeerEnvelope {
    if env.kind != EnvelopeKind::Request {
        return PeerEnvelope::error(
            &env.id,
            PeerError { code: ErrorCode::BadParams, message: "expected a request".into(), data: None },
        );
    }
    match env.method.as_deref() {
        Some("hello") => handle_hello(&env.id, server).await,
        Some(other) => PeerEnvelope::error(
            &env.id,
            PeerError {
                code: ErrorCode::UnknownMethod,
                message: format!("unknown method: {other}"),
                data: None,
            },
        ),
        None => PeerEnvelope::error(
            &env.id,
            PeerError { code: ErrorCode::BadParams, message: "missing method".into(), data: None },
        ),
    }
}

async fn handle_hello(id: &str, server: &Arc<CodeScoutServer>) -> PeerEnvelope {
    let caps = Capabilities {
        project: server.project_name().await,
        root: server.project_root_string().await,
        read_only: server.is_read_only().await,
        tools: server.tool_names(),
        executor_available: false, // Phase 2
    };
    PeerEnvelope::response(id, serde_json::to_value(caps).unwrap_or(json!({})))
}
```

> `project_name()`, `project_root_string()`, `is_read_only()`, and `tool_names()` are small read accessors that must be added to `CodeScoutServer` (Step 4). `tool_names()` maps `self.tools.iter().map(|t| t.name().to_string())` — the same iteration the existing `server_registers_all_tools` test uses.

- [ ] **Step 4: Add the read accessors to `CodeScoutServer` (⚠️ existing file)**

```rust
// src/server.rs — inside impl CodeScoutServer
pub(crate) fn tool_names(&self) -> Vec<String> {
    self.tools.iter().map(|t| t.name().to_string()).collect()
}
pub(crate) async fn project_name(&self) -> String {
    self.agent.with_project(|p| Ok(p.name.clone())).await.unwrap_or_default()
}
pub(crate) async fn project_root_string(&self) -> String {
    self.agent.project_root().await.map(|r| r.display().to_string()).unwrap_or_default()
}
pub(crate) async fn is_read_only(&self) -> bool {
    self.agent.is_read_only().await
}
```

> Confirm the field/method on the project struct for `name` and the existence of `Agent::is_read_only()` — scout `Agent` (`src/agent/mod.rs`) and `with_project`'s closure type before writing. If `is_read_only` is named differently (e.g. a field on the active project), adapt to the real accessor; do not invent one.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib peer::server::tests::hello_returns_capabilities_for_read_only_peer`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/peer/server.rs src/server.rs
git commit -m "feat(peer): serve loop + hello/capabilities; server read accessors"
```

---

### Task 6: Peer-serve — `tool.call` routing

**Files:**
- Modify: `src/peer/server.rs` (extend `dispatch_envelope`, add `handle_tool_call`)
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/server.rs — in mod tests
#[tokio::test]
async fn tool_call_runs_a_read_tool_on_the_peer_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    let sock = root.join("peer.sock");

    let (sr, ss) = (root.clone(), sock.clone());
    let handle = tokio::spawn(async move {
        let server = build_server_for(&sr, true).await.unwrap();
        let listener = bind_peer_socket(&ss).unwrap();
        accept_one(&listener, &server).await.unwrap();
    });

    let stream = {
        let mut s = None;
        for _ in 0..50 {
            if let Ok(c) = tokio::net::UnixStream::connect(&sock).await { s = Some(c); break; }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        s.unwrap()
    };
    let (rd, mut wr) = stream.into_split();
    let mut rd = tokio::io::BufReader::new(rd);

    let req = crate::peer::protocol::PeerEnvelope::request(
        "a:1", "tool.call",
        serde_json::json!({ "tool": "tree", "args": { "path": "." } }),
    );
    crate::lsp::transport::write_message(&mut wr, &serde_json::to_value(&req).unwrap()).await.unwrap();

    let resp: crate::peer::protocol::PeerEnvelope =
        serde_json::from_value(crate::lsp::transport::read_message(&mut rd).await.unwrap()).unwrap();
    assert!(resp.error.is_none(), "tree should not error: {:?}", resp.error);
    // tool.call result wraps the tool's content blocks.
    assert!(resp.result.is_some());
    handle.abort();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::server::tests::tool_call_runs_a_read_tool_on_the_peer_workspace`
Expected: FAIL — `tool.call` falls through to `UnknownMethod`.

- [ ] **Step 3: Implement `handle_tool_call` and wire it in**

```rust
// src/peer/server.rs — add the arm in dispatch_envelope's match:
        Some("tool.call") => handle_tool_call(&env.id, env.params.clone(), server).await,

// ... and the handler:
async fn handle_tool_call(
    id: &str,
    params: Option<serde_json::Value>,
    server: &Arc<CodeScoutServer>,
) -> PeerEnvelope {
    let params = match params {
        Some(p) => p,
        None => return bad_params(id, "tool.call requires params"),
    };
    let tool = match params.get("tool").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => return bad_params(id, "tool.call requires a 'tool' name"),
    };
    let args = params.get("args").cloned().unwrap_or(serde_json::json!({}));

    match server.call_tool_by_name(&tool, args).await {
        Ok(result) => {
            // Serialise the CallToolResult into the envelope result. is_error
            // maps to a ToolError envelope so the requester can branch.
            let body = serde_json::to_value(&result).unwrap_or(serde_json::json!(null));
            if result.is_error.unwrap_or(false) {
                PeerEnvelope::error(
                    id,
                    PeerError { code: ErrorCode::ToolError, message: "tool returned an error".into(), data: Some(body) },
                )
            } else {
                PeerEnvelope::response(id, body)
            }
        }
        Err(e) => PeerEnvelope::error(
            id,
            PeerError { code: ErrorCode::UnknownTool, message: e.to_string(), data: None },
        ),
    }
}

fn bad_params(id: &str, msg: &str) -> PeerEnvelope {
    PeerEnvelope::error(id, PeerError { code: ErrorCode::BadParams, message: msg.into(), data: None })
}
```

> `CallToolResult` is an rmcp type; confirm it implements `Serialize` (it does — it crosses the MCP wire). If a field resists `serde_json::to_value`, serialise the `content` vector explicitly instead.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib peer::server::tests::tool_call_runs_a_read_tool_on_the_peer_workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/peer/server.rs
git commit -m "feat(peer): route tool.call through call_tool_by_name"
```

---

### Task 7: Peer-serve — `buffer.read` / `buffer.grep` proxy

Tool output above the inline budget lands in B's `OutputBuffer` as an `@tool_*`/`@file_*`/`@cmd_*` handle. Those handles are process-local to the peer-serve process, so the requester reads them by proxy.

**Files:**
- Modify: `src/peer/server.rs` (two arms + handlers)
- Modify: `src/server.rs` (a `pub(crate)` accessor for the output buffer)
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/server.rs — in mod tests
#[tokio::test]
async fn buffer_read_returns_stored_content() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let sock = root.join("peer.sock");

    let (sr, ss) = (root.clone(), sock.clone());
    let handle = tokio::spawn(async move {
        let server = build_server_for(&sr, true).await.unwrap();
        // Pre-seed the buffer with a known handle.
        let id = server.output_buffer_ref().store_tool("probe", "LINE_ONE\nLINE_TWO".into());
        // Stash the handle where the test can read it via a second hello field.
        std::fs::write(sr.join(".handle"), &id).unwrap();
        let listener = bind_peer_socket(&ss).unwrap();
        accept_one(&listener, &server).await.unwrap();
    });

    // Wait for the handle file, then connect.
    let handle_id = {
        let mut h = None;
        for _ in 0..50 {
            if let Ok(s) = std::fs::read_to_string(root.join(".handle")) { h = Some(s); break; }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        h.unwrap()
    };
    let stream = {
        let mut s = None;
        for _ in 0..50 {
            if let Ok(c) = tokio::net::UnixStream::connect(&sock).await { s = Some(c); break; }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        s.unwrap()
    };
    let (rd, mut wr) = stream.into_split();
    let mut rd = tokio::io::BufReader::new(rd);

    let req = crate::peer::protocol::PeerEnvelope::request(
        "a:1", "buffer.read", serde_json::json!({ "handle": handle_id }),
    );
    crate::lsp::transport::write_message(&mut wr, &serde_json::to_value(&req).unwrap()).await.unwrap();
    let resp: crate::peer::protocol::PeerEnvelope =
        serde_json::from_value(crate::lsp::transport::read_message(&mut rd).await.unwrap()).unwrap();
    let text = resp.result.unwrap()["content"].as_str().unwrap().to_string();
    assert!(text.contains("LINE_ONE") && text.contains("LINE_TWO"));
    handle.abort();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::server::tests::buffer_read_returns_stored_content`
Expected: FAIL — `output_buffer_ref` and the `buffer.read` arm are missing.

- [ ] **Step 3: Add the buffer accessor (⚠️ existing file)**

```rust
// src/server.rs — inside impl CodeScoutServer
pub(crate) fn output_buffer_ref(&self) -> Arc<crate::tools::output_buffer::OutputBuffer> {
    self.output_buffer.clone()
}
```

- [ ] **Step 4: Implement the handlers and wire the arms**

```rust
// src/peer/server.rs — arms in dispatch_envelope:
        Some("buffer.read") => handle_buffer_read(&env.id, env.params.clone(), server).await,
        Some("buffer.grep") => handle_buffer_grep(&env.id, env.params.clone(), server).await,

// handlers:
async fn handle_buffer_read(
    id: &str,
    params: Option<serde_json::Value>,
    server: &Arc<CodeScoutServer>,
) -> PeerEnvelope {
    let handle = match params.as_ref().and_then(|p| p.get("handle")).and_then(|h| h.as_str()) {
        Some(h) => h.to_string(),
        None => return bad_params(id, "buffer.read requires a 'handle'"),
    };
    match server.output_buffer_ref().get(&handle) {
        Some(entry) => {
            // stdout holds tool/file content; stderr appended when present.
            let content = if entry.stderr.is_empty() {
                entry.stdout
            } else {
                format!("{}\n{}", entry.stdout, entry.stderr)
            };
            PeerEnvelope::response(id, serde_json::json!({ "content": content }))
        }
        None => PeerEnvelope::error(
            id,
            PeerError { code: ErrorCode::UnknownHandle, message: format!("no such handle: {handle}"), data: None },
        ),
    }
}

async fn handle_buffer_grep(
    id: &str,
    params: Option<serde_json::Value>,
    server: &Arc<CodeScoutServer>,
) -> PeerEnvelope {
    let p = params.unwrap_or(serde_json::json!({}));
    let handle = match p.get("handle").and_then(|h| h.as_str()) {
        Some(h) => h.to_string(),
        None => return bad_params(id, "buffer.grep requires a 'handle'"),
    };
    let pattern = match p.get("pattern").and_then(|h| h.as_str()) {
        Some(s) => s.to_string(),
        None => return bad_params(id, "buffer.grep requires a 'pattern'"),
    };
    let re = match regex::Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => return bad_params(id, &format!("invalid regex: {e}")),
    };
    match server.output_buffer_ref().get(&handle) {
        Some(entry) => {
            let matched: Vec<&str> = entry.stdout.lines().filter(|l| re.is_match(l)).collect();
            PeerEnvelope::response(id, serde_json::json!({ "matches": matched, "count": matched.len() }))
        }
        None => PeerEnvelope::error(
            id,
            PeerError { code: ErrorCode::UnknownHandle, message: format!("no such handle: {handle}"), data: None },
        ),
    }
}
```

> `regex` is already a dependency (used across `src/tools`). Confirm `store_tool(&str, String) -> String` and `get(&str) -> Option<BufferEntry>` signatures at `src/tools/output_buffer.rs:244,120` — both verified during planning.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib peer::server::tests::buffer_read_returns_stored_content`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/peer/server.rs src/server.rs
git commit -m "feat(peer): buffer.read/buffer.grep proxy over OutputBuffer"
```

---

### Task 8: Prove the RO/RW wall

No new wall to build — `Agent::activate(root, Some(true))` plus the write-guard in `call_tool_inner` already block writes. This task is the *regression proof*: a write tool against a read-only peer must be refused, and the same tool against a read-write peer must be allowed.

**Files:**
- Test only: `src/peer/server.rs` (`mod tests`)

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/server.rs — in mod tests
#[tokio::test]
async fn read_only_peer_refuses_a_write_tool() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // build_server_for(..., true) → read-only. A mutating tool must be denied
    // by the existing write-guard, surfaced here as an error envelope.
    let server = build_server_for(&root, true).await.unwrap();
    let reply = handle_tool_call(
        "a:1",
        Some(serde_json::json!({
            "tool": "create_file",
            "args": { "path": "new.txt", "content": "x" }
        })),
        &server,
    )
    .await;

    assert!(reply.error.is_some(), "write tool must be refused on a RO peer");
    // The file must not have been created.
    assert!(!root.join("new.txt").exists(), "RO peer must not have written");
}
```

- [ ] **Step 2: Run test to verify it fails or passes**

Run: `cargo test --lib peer::server::tests::read_only_peer_refuses_a_write_tool`
Expected: This may PASS immediately if the wall already holds end-to-end. If it FAILS (file created, or no error), the wall is NOT inherited as assumed — STOP and capture an F-N session-log entry; the design's D-Wall assumption is wrong and must be revisited before proceeding.

- [ ] **Step 3: If it failed — investigate, do not patch blindly**

The expected behaviour is that `create_file`'s write path checks the active project's read-only flag (via `acquire_write_guard_if_writing` / `check_tool_access`). If the write slipped through, the gap is that the read-only flag set by `Agent::activate(.., Some(true))` is not consulted on this dispatch path. Scout `acquire_write_guard_if_writing` and `check_tool_access` to find where read-only is read, and record the finding. Resolve with the owner before adding any new gate (it would touch `src/server.rs` / `src/agent`).

- [ ] **Step 4: Commit**

```bash
git add src/peer/server.rs
git commit -m "test(peer): prove read-only peer refuses write tools (D-Wall regression)"
```

---

### Task 9: Append-only audit log of served calls

Every served `tool.call` is logged to a per-workspace JSONL file so the workspace owner can see what a peer ran.

**Files:**
- Modify: `src/peer/server.rs` (add an audit write in `serve_connection` / `handle_tool_call`)
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/server.rs — in mod tests
#[tokio::test]
async fn served_tool_calls_are_audited() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let audit = root.join("peer-audit.jsonl");

    let server = build_server_for(&root, true).await.unwrap();
    let _ = handle_tool_call_audited(
        "a:1",
        Some(serde_json::json!({ "tool": "tree", "args": { "path": "." } })),
        &server,
        &audit,
    )
    .await;

    let logged = std::fs::read_to_string(&audit).unwrap();
    assert!(logged.contains("\"tool\":\"tree\""), "audit must record the tool name");
    assert!(logged.contains("\"id\":\"a:1\""), "audit must record the request id");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::server::tests::served_tool_calls_are_audited`
Expected: FAIL — `handle_tool_call_audited` undefined.

- [ ] **Step 3: Implement the audit wrapper**

```rust
// src/peer/server.rs
use std::io::Write as _;

/// Wrap handle_tool_call with an append-only JSONL audit record. The serve loop
/// calls this variant, threading the audit path resolved at startup.
async fn handle_tool_call_audited(
    id: &str,
    params: Option<serde_json::Value>,
    server: &Arc<CodeScoutServer>,
    audit_path: &Path,
) -> PeerEnvelope {
    let tool = params
        .as_ref()
        .and_then(|p| p.get("tool"))
        .and_then(|t| t.as_str())
        .unwrap_or("?")
        .to_string();
    let reply = handle_tool_call(id, params, server).await;
    let record = serde_json::json!({
        "id": id,
        "tool": tool,
        "ok": reply.error.is_none(),
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(audit_path) {
        let _ = writeln!(f, "{record}");
    }
    reply
}
```

Wire `serve_connection` to call `handle_tool_call_audited` for the `tool.call` arm, with `audit_path` resolved at serve start (`<workspace>/.codescout/peer-audit.jsonl`, created if absent). Pass it through `serve_connection` / `dispatch_envelope` (add the parameter).

> No timestamp field: `Date`/`Instant` wall-clock is fine in the live server, but keep the *test* deterministic by asserting only on `tool`/`id`/`ok`. If a timestamp is added for the live path, exclude it from test assertions.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib peer::server::tests::served_tool_calls_are_audited`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/peer/server.rs
git commit -m "feat(peer): append-only JSONL audit of served tool calls"
```

---

### Task 10: Registry file — peer addressing

Maps a human/agent-facing `id` to a target workspace, a description (the agentic selection surface), and a default access level. Manual TOML now; a future LLM-manager writes the same file.

**Files:**
- Modify: `src/peer/registry.rs` (replace stub)
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/registry.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_and_resolves_socket() {
        let toml = r#"
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
        "#;
        let reg = Registry::from_toml_str(toml).unwrap();
        let backend = reg.get("backend").unwrap();
        assert_eq!(backend.default_access, Access::ReadOnly);
        assert!(backend.description.contains("payments"));
        // Socket path is derived from target, not stored.
        let sock = backend.socket_path();
        assert!(sock.to_str().unwrap().contains("codescout-peer-"));
        assert!(reg.get("frontend").unwrap().default_access == Access::ReadWrite);
        assert!(reg.get("missing").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::registry`
Expected: FAIL — `Registry`/`Access` undefined.

- [ ] **Step 3: Write the registry**

```rust
// src/peer/registry.rs
//! Peer registry: id → target workspace + description + default access. The
//! serve socket is *derived* from the target via socket_discovery, never stored.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::socket_discovery::peer_socket_path_for_workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

impl Access {
    pub fn is_read_only(self) -> bool {
        matches!(self, Access::ReadOnly)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerEntry {
    pub id: String,
    pub target: PathBuf,
    /// Agentic selection surface — a manager picks a peer by reading this.
    pub description: String,
    pub default_access: Access,
}

impl PeerEntry {
    pub fn socket_path(&self) -> PathBuf {
        peer_socket_path_for_workspace(&self.target)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Registry {
    #[serde(default, rename = "peer")]
    peers: Vec<PeerEntry>,
}

impl Registry {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("failed to parse peer registry TOML")
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Registry::default());
        }
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read peer registry: {}", path.display()))?;
        Self::from_toml_str(&s)
    }

    pub fn get(&self, id: &str) -> Option<&PeerEntry> {
        self.peers.iter().find(|p| p.id == id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.peers.iter().map(|p| p.id.as_str())
    }
}
```

> `toml` is already a workspace dependency (used for `Cargo.toml`/config parsing). Confirm it is in `Cargo.toml`'s `[dependencies]`; if it is dev-only, add it (a `Cargo.toml` edit — ⚠️ approval).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib peer::registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/peer/registry.rs
git commit -m "feat(peer): TOML peer registry with derived socket paths"
```

---

### Task 11: PeerClient — the requester side

Connect to a peer's socket, negotiate `hello`, call tools, read buffers. Maps peer error codes onto `RecoverableError` so a peer's bad-input failure does not abort the requester's sibling tool calls.

**Files:**
- Modify: `src/peer/client.rs` (replace stub)
- Test: inline (drives a real `serve_connection` over a tmp socket)

- [ ] **Step 1: Write the failing test**

```rust
// src/peer/client.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::server::{accept_one, bind_peer_socket, build_server_for};

    #[tokio::test]
    async fn client_hello_then_tool_call() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let sock = root.join("peer.sock");

        let (sr, ss) = (root.clone(), sock.clone());
        let handle = tokio::spawn(async move {
            let server = build_server_for(&sr, true).await.unwrap();
            let listener = bind_peer_socket(&ss).unwrap();
            // Serve two requests: hello + one tool.call.
            accept_one(&listener, &server).await.unwrap();
        });

        let mut client = PeerClient::connect(&sock).await.unwrap();
        let caps = client.hello().await.unwrap();
        assert!(caps.read_only);

        let blocks = client.call_tool("tree", serde_json::json!({ "path": "." })).await.unwrap();
        assert!(blocks.is_object() || blocks.is_array() || blocks.get("content").is_some());
        handle.abort();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib peer::client::tests::client_hello_then_tool_call`
Expected: FAIL — `PeerClient` undefined.

> Note: `accept_one` serves a single connection but loops over multiple requests within it (`serve_connection`'s loop), so one `accept_one` handles both `hello` and `call_tool` on the same connection. Good — the client holds one connection open.

- [ ] **Step 3: Write the client**

```rust
// src/peer/client.rs
//! Requester side of peer delegation. Holds one connection to a peer's socket.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;
use tokio::io::BufReader;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

use crate::lsp::transport::{read_message, write_message};
use crate::peer::protocol::{Capabilities, EnvelopeKind, PeerEnvelope};
use crate::tools::RecoverableError;

pub struct PeerClient {
    rd: BufReader<OwnedReadHalf>,
    wr: OwnedWriteHalf,
    next_id: u64,
}

impl PeerClient {
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| anyhow!("failed to connect to peer socket {}: {e}", socket_path.display()))?;
        let (rd, wr) = stream.into_split();
        Ok(Self { rd: BufReader::new(rd), wr, next_id: 0 })
    }

    fn next_id(&mut self) -> String {
        self.next_id += 1;
        format!("c:{}", self.next_id)
    }

    /// Send a request envelope, await the single correlated reply (Phase 1 is
    /// synchronous: one in-flight request per connection, so no tag mux needed).
    async fn round_trip(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id();
        let req = PeerEnvelope::request(&id, method, params);
        write_message(&mut self.wr, &serde_json::to_value(&req)?).await?;
        let resp: PeerEnvelope = serde_json::from_value(read_message(&mut self.rd).await?)?;

        match resp.kind {
            EnvelopeKind::Response => resp.result.ok_or_else(|| anyhow!("response missing result")),
            EnvelopeKind::Error => {
                let err = resp.error.ok_or_else(|| anyhow!("error envelope missing error"))?;
                // A peer's input-driven failure is recoverable on our side — do
                // not abort sibling tool calls. Genuine transport faults above
                // already returned Err(anyhow).
                Err(RecoverableError {
                    message: format!("peer error [{:?}]: {}", err.code, err.message),
                    guidance: None,
                    extra: Box::new(serde_json::Map::new()),
                }
                .into())
            }
            other => Err(anyhow!("unexpected envelope kind from peer: {other:?}")),
        }
    }

    pub async fn hello(&mut self) -> Result<Capabilities> {
        let v = self.round_trip("hello", serde_json::json!({})).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn call_tool(&mut self, tool: &str, args: Value) -> Result<Value> {
        self.round_trip("tool.call", serde_json::json!({ "tool": tool, "args": args })).await
    }

    pub async fn read_buffer(&mut self, handle: &str) -> Result<Value> {
        self.round_trip("buffer.read", serde_json::json!({ "handle": handle })).await
    }

    pub async fn grep_buffer(&mut self, handle: &str, pattern: &str) -> Result<Value> {
        self.round_trip("buffer.grep", serde_json::json!({ "handle": handle, "pattern": pattern })).await
    }
}
```

> Confirm `RecoverableError`'s fields at construction: `message: String`, `guidance: Option<Guidance>`, `extra: Box<serde_json::Map<String, Value>>` (verified at `src/tools/core/types.rs:179`). The `.into()` relies on the existing `From<RecoverableError> for anyhow::Error` (it exists — `route_tool_error` downcasts it). Confirm `crate::tools::RecoverableError` is the re-export path (it is — used in `src/server.rs`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib peer::client::tests::client_hello_then_tool_call`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/peer/client.rs
git commit -m "feat(peer): PeerClient — hello/call_tool/buffer proxy, recoverable peer errors"
```

---

### Task 12: CLI `peer-serve` subcommand ⚠️ + the requester-side `peer` MCP tool ⚠️

The last mile: a hidden subcommand to launch the serve process (mirroring `codescout mux`), and the `peer` MCP tool the local agent calls to delegate. Both touch existing files.

**Files:**
- Create: `src/tools/peer.rs` (the `peer` MCP tool)
- Modify: `src/main.rs` (`Commands::PeerServe` variant + dispatch arm, mirroring `Commands::Mux`)
- Modify: `src/peer/server.rs` (a public `run(socket, workspace, read_only, idle_timeout)` entry the CLI calls)
- Modify: `src/server.rs` (register `Arc::new(crate::tools::peer::PeerTool)` in the `tools` vec, ~line 105)

- [ ] **Step 1: Write the failing test (the MCP tool)**

```rust
// src/tools/peer.rs — at the bottom
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_tool_advertises_name_and_actions() {
        let t = PeerTool;
        assert_eq!(t.name(), "peer");
        let schema = t.input_schema();
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        for a in ["query", "knowledge", "explore", "status"] {
            assert!(actions.iter().any(|v| v == a), "missing action {a}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib tools::peer`
Expected: FAIL — `PeerTool` undefined.

- [ ] **Step 3: Write the `peer` MCP tool**

```rust
// src/tools/peer.rs
//! The requester-facing `peer` MCP tool. Resolves a peer id from the registry,
//! connects, and runs a remote read tool. `work` (async jobs) is Phase 2.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::peer::client::PeerClient;
use crate::peer::registry::Registry;
use crate::tools::core::types::{Content, Tool, ToolContext};

pub struct PeerTool;

#[async_trait]
impl Tool for PeerTool {
    fn name(&self) -> &str {
        "peer"
    }

    fn description(&self) -> &str {
        "Delegate read-only exploration to a peer codescout instance that owns another \
         project. action=query runs one of the peer's read tools (symbols/search/etc); \
         action=knowledge fetches a peer buffer handle; action=status lists configured peers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["query", "knowledge", "explore", "status"] },
                "peer": { "type": "string", "description": "Registry id of the target peer" },
                "tool": { "type": "string", "description": "For action=query: the peer tool name" },
                "args": { "type": "object", "description": "For action=query: the peer tool args" },
                "handle": { "type": "string", "description": "For action=knowledge: a peer buffer handle" }
            },
            "required": ["action"]
        })
    }

    async fn call_content(&self, args: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("status");
        let registry_path = ctx
            .agent
            .project_root()
            .await
            .map(|r| r.join(".codescout/peers.toml"))
            .ok_or_else(|| anyhow!("no active project to resolve peer registry"))?;
        let registry = Registry::load(&registry_path)?;

        match action {
            "status" => {
                let ids: Vec<&str> = registry.ids().collect();
                Ok(vec![Content::text(json!({ "peers": ids }).to_string())])
            }
            "query" | "explore" => {
                let id = args.get("peer").and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow!("action={action} requires 'peer'"))?;
                let entry = registry.get(id).ok_or_else(|| anyhow!("unknown peer: {id}"))?;
                let tool = args.get("tool").and_then(|t| t.as_str())
                    .ok_or_else(|| anyhow!("action={action} requires 'tool'"))?;
                let tool_args = args.get("args").cloned().unwrap_or(json!({}));

                let mut client = PeerClient::connect(&entry.socket_path()).await?;
                let _caps = client.hello().await?;
                let result = client.call_tool(tool, tool_args).await?;
                Ok(vec![Content::text(result.to_string())])
            }
            "knowledge" => {
                let id = args.get("peer").and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow!("action=knowledge requires 'peer'"))?;
                let entry = registry.get(id).ok_or_else(|| anyhow!("unknown peer: {id}"))?;
                let handle = args.get("handle").and_then(|h| h.as_str())
                    .ok_or_else(|| anyhow!("action=knowledge requires 'handle'"))?;
                let mut client = PeerClient::connect(&entry.socket_path()).await?;
                let result = client.read_buffer(handle).await?;
                Ok(vec![Content::text(result.to_string())])
            }
            other => Err(anyhow!("unknown peer action: {other}")),
        }
    }
}
```

> Confirm the exact import paths and constructors: `Content::text(...)` (the established content constructor — scout how other tools build `Vec<Content>`, e.g. `src/tools/tree.rs`), the `Tool` trait path (`src/tools/core/types.rs:340`), and `ToolContext`'s `agent` field. Match the real `Tool` trait method set — it has more required methods than shown (`is_write`, `input_schema`, etc.); implement them following a simple read-only tool like `Tree` as the template.

- [ ] **Step 4: Add the serve `run` entry and the CLI subcommand (⚠️ existing files)**

```rust
// src/peer/server.rs — public entry the CLI calls
/// Run the peer-serve process for `workspace`. Acquires a per-workspace lock,
/// binds the socket, and serves connections until idle-timeout. Mirrors
/// lsp::mux::process::run.
pub async fn run(socket_path: &Path, workspace: &Path, read_only: bool, _idle_timeout_secs: u64) -> Result<()> {
    let server = build_server_for(workspace, read_only).await?;
    let listener = bind_peer_socket(socket_path)?;
    loop {
        // Phase 1: serve connections sequentially (one requester at a time).
        // Phase 2 spawns per-connection tasks + the tag mux for concurrency.
        if accept_one(&listener, &server).await.is_err() {
            break;
        }
    }
    std::fs::remove_file(socket_path).ok();
    Ok(())
}
```

```rust
// src/main.rs — new variant in enum Commands, mirroring Commands::Mux (line 84)
#[command(hide = true)]
PeerServe {
    /// Path to the Unix socket to listen on
    #[arg(long)]
    socket: PathBuf,
    /// Workspace root to serve
    #[arg(long)]
    workspace: PathBuf,
    /// Serve read-only (default true)
    #[arg(long, default_value_t = true)]
    read_only: bool,
    /// Idle timeout in seconds
    #[arg(long, default_value_t = 300)]
    idle_timeout: u64,
},
```

```rust
// src/main.rs — new dispatch arm, mirroring the Commands::Mux arm (line 345)
#[cfg(unix)]
Commands::PeerServe { socket, workspace, read_only, idle_timeout } => {
    codescout::peer::server::run(&socket, &workspace, read_only, idle_timeout).await?;
}
```

- [ ] **Step 5: Register the `peer` tool (⚠️ existing file)**

In `src/server.rs`, add to the `tools: Vec<Arc<dyn Tool>>` vec (~line 132, near `Arc::new(Library)`):

```rust
            Arc::new(crate::tools::peer::PeerTool),
```

Also add `pub mod peer;` to `src/tools/mod.rs`'s module list (⚠️ existing file).

- [ ] **Step 6: Run the full suite**

Run: `cargo test --lib peer && cargo test --lib tools::peer && cargo build --release`
Expected: PASS. The release build is what the live MCP server loads (`~/.cargo/bin/codescout` → `target/release/codescout`).

- [ ] **Step 7: Update prompt surfaces (⚠️ existing files)**

Adding the `peer` tool means three prompt surfaces may need it (per CLAUDE.md "Prompt Surface Consistency"): `src/prompts/source.md` (`server_instructions` slice — no version bump needed), and the test `server::tests::prompt_surfaces_reference_only_real_tools` will now require `peer` to be a real tool (it is) — but if `peer` is mentioned in any surface it must match. Run:

Run: `cargo test --lib prompt`
Expected: PASS. If `source_md_under_cap` fires, move any peer guidance to a `get_guide` topic rather than inflating the slice.

- [ ] **Step 8: Commit**

```bash
git add src/tools/peer.rs src/tools/mod.rs src/main.rs src/peer/server.rs src/server.rs
git commit -m "feat(peer): peer-serve CLI subcommand + requester-side peer MCP tool"
```

---

## Manual end-to-end verification (after Task 12, before any cherry-pick)

Phase 1 is not "done" until verified against the live binary (CLAUDE.md ship discipline):

1. `cargo build --release`
2. Create `<projB>/.codescout/peers.toml` is not needed on B; create `<projA>/.codescout/peers.toml` with a `backend` entry pointing `target` at project B.
3. In a terminal: `codescout peer-serve --socket "$(codescout-peer-path B)" --workspace /abs/path/B --read-only true` — or rely on the requester auto-spawning it (Phase 1: launch manually; auto-spawn is a follow-up).
4. From project A's Claude Code session, after `/mcp` restart: `peer(action="status")` lists `backend`; `peer(action="query", peer="backend", tool="symbols", args={"path":"src"})` returns B's symbols.
5. Confirm the wall: `peer(action="query", peer="backend", tool="create_file", args={...})` returns a peer error (read-only).

---

## Self-review

**Spec coverage** (against `2026-06-01-peer-delegation-protocol-design.md`):
- §4 `src/peer/{mod,server,client,registry}.rs` + `peer` MCP tool → Tasks 2,3,5-12 ✓
- §5 envelope `hello`/`tool.call`/`buffer.read`/`buffer.grep` → Tasks 2,5,6,7 ✓; `job.*` + events → **Phase 2 (out of scope)** ✓
- §6 registry-as-file, derived socket, RO/RW default → Task 10 ✓; idle-timeout/lock → partially (Task 12 `run` has the loop; lock file is a noted follow-up) — **gap flagged below**
- §8 trust/wall → Task 8 ✓; audit → Task 9 ✓
- §9 error taxonomy → `ErrorCode` (Task 2) + `RecoverableError` mapping (Task 11) ✓
- §10 testing (fake-client fixture, prove-the-wall) → Tasks 5-8,11 ✓

**Known gaps (intentional, Phase-1 scope cuts — not placeholders):**
- **Lock file / idle-timeout / disconnect-retry** — the design's lifecycle row. Task 12's `run` serves sequentially without a flock or idle exit. This is safe for single-requester Phase 1 but must land before multi-requester use. Tracked as the first Phase-1.5 follow-up.
- **Auto-spawn of the serve process** — Phase 1 launches `peer-serve` manually; the requester auto-spawning it (as the LSP client auto-spawns the mux) is a follow-up.
- **Request-id tag multiplexing** — deliberately omitted (synchronous Phase 1); returns with Phase 2 events/jobs.

**Placeholder scan:** No "TBD"/"implement later". Every code step shows code. The `>` notes flag *real APIs to confirm at the seam* (field names on rmcp/Agent types I did not exhaustively read) — these are verification reminders, not missing logic.

**Type consistency:** `PeerEnvelope`/`EnvelopeKind`/`ErrorCode`/`PeerError`/`Capabilities` defined in Task 2, used consistently in 5-11. `build_server_for`/`bind_peer_socket`/`accept_one`/`serve_connection`/`dispatch_envelope`/`handle_tool_call` defined in Task 5-6, reused in 7-9,11-12. `call_tool_by_name` (Task 4) used in Task 6. `Registry`/`PeerEntry`/`Access` (Task 10) used in Task 12.

---
id: '0d301e2dd1b0f510'
kind: spec
status: draft
title: Peer Delegation Protocol — codescout↔codescout deep-exploration & work delegation
owners: []
tags:
- peer-delegation
- protocol
- architecture
- ipc
- design
topic: null
time_scope: null
---

# Peer Delegation Protocol — Design Spec

> **Status:** draft · **Date:** 2026-06-01 · co-designed with the Architecture Snow Lion.
> All code citations verified against the working tree this session (Principle 2: cite the import, not the diagram).

## 1. Problem & Goal

codescout instances are isolated. An agent on project **A** that needs deep knowledge or changes in project **B** must either bloat its own context exploring B, or lacks B's index / LSP / memory entirely.

**Goal:** a peer-to-peer channel so codescout-A can, against a peer that owns project B:
1. **Q&A (mode 2)** — call B's navigation/search tools remotely.
2. **Federated knowledge (mode 3)** — read B's accumulated librarian artifacts + memories.
3. **Deep exploration (mode 1)** — delegate an open-ended exploration goal; B's *agent* runs it and returns a cited report.
4. **Work delegation (mode 4)** — delegate code-writing; B's *agent* returns a reviewable diff.

**Scope:** same-machine, same-user, trusted, over unix sockets **now**; the message envelope is transport-agnostic so cross-machine (TCP/TLS + auth) can be added later without redesign.

## 2. Context — the existing mechanism (verified this session)

codescout's only inter-instance IPC today is the **LSP multiplexer** (`src/lsp/mux/`). What it is, and what transfers to this design:

| Layer | Code | Reusable for peer protocol? |
|---|---|---|
| Transport: `Content-Length`-framed JSON, 16 MiB cap | `src/lsp/transport.rs::{write_message,read_message}` | **Yes, verbatim** — carries any JSON |
| Multiplex: request-id tagging (`"tag:id"`) | `src/lsp/mux/protocol.rs::{tag_request_id,untag_response_id}` | **Yes** — general primitive |
| Discovery: per-user socket path | `src/lsp/mux/mod.rs::{workspace_hash,socket_path_for_workspace,per_user_mux_dir}` | **Yes** |
| Lifecycle: lock file, 300s idle-timeout, disconnect-retry | `src/lsp/mux/process.rs`, `src/fs/mod.rs::retry_on_mux_disconnect` | **Yes** |
| `DocumentState` (textDocument sync), cached LSP `initialize` | `src/lsp/mux/protocol.rs`, `process.rs` | **No** — LSP-specific |

**Topology is inverted.** The mux is *N clients → 1 shared backend*; peer delegation is *1 requester → N peers, each owning its own project*. We reuse the plumbing, swap the LSP payload for a codescout-native vocabulary, and flip the topology so a peer runs a *serve-peers* socket.

## 3. Decisions (ADR-style)

- **D-Model — Approach C (hybrid).** codescout stays an **LLM-free tool-provider + message bus**; heavy `job`s are routed to the *agent* that owns B (attach-first), not executed by codescout. *Absorbs:* all four modes + true cognitive offload without making codescout an agent. *Rejected:* (A) remote-tools-only (no offload), (B) embedded LLM in codescout (keys/cost/autonomous-writer surface). *Confidence: high.*
- **D-Topology — same-machine unix sockets, networked-ready envelope.** *Absorbs:* future cross-machine without protocol redesign. *Cost:* a `v`/`hello` negotiation seam carried from day one. *Confidence: high.*
- **D-Executor — attach-first; spawn deferred.** A `job` waits in B's queue until an agent attached to B claims it. *Rejected for now:* spawn-on-demand launcher (Principle 4 — no mechanism before the second concrete). *Confidence: medium* (rests on "a window is usually open on B"; revisit-when that breaks).
- **D-Wall — RO/RW enforced by a peer-layer `is_write_call` gate (revised 2026-06-01).** Read methods always available; write methods (`tool.call`→`edit_code`/`create_file`, `job{mode:work}`) require the peer to be RW **and** an explicit registry write-grant. *Absorbs:* networked/untrusted peers default RO. *Revised:* the original plan to reuse `workspace(read_only=true)` does **not** hold — a single-workspace `peer-serve` process makes its served workspace the Agent **home**, which is always read-write (`is_home` is true when `home_root` is unset on the first `activate`, forcing `effective_read_only=false`). So Phase 1 gates at the peer boundary: `PeerServer` holds `read_only`; `handle_tool_call` rejects writes via the existing unit-tested `CodeScoutServer::is_write_call(name,args)` *before* dispatch. *Convergence (Phase 1.5):* the workspace-pinning route (neutral home + `ensure_resident(peer_root, ro)` + per-call `workspace_override`) lets the Agent write-guard engage too — belt-and-suspenders. *Confidence: high.*
- **D-Buffer — (a) proxy + lease.** A remote `tool.call` that overflows buffers on B; A reads it via a proxied `buffer.read`. *Cost:* B must lease/pin peer-served buffers (the `OutputBuffer` is a 50-entry LRU) and cannot idle-shutdown while leases are open. *Revisit-when:* leasing proves fiddly → fall back to (c) re-buffer-on-A (no lifetime coupling). *Confidence: medium.*
- **D-Audit — serving-side audit line in v1.** `{requester_id, method, target, ts}` per write/job on B. *Confidence: medium* (cheap, pairs with D-Wall/D-Executor).
- **D-Networked-hardening — deferred.** Per-request auth + unguessable handle tokens are out of scope until the transport goes TCP. Same-machine same-user → unix-socket filesystem perms are the boundary. *Confidence: high it's deferrable.*

## 4. Architecture & components

New module **`src/peer/`** (sibling to `src/lsp/mux/`). Three layers; codescout stays LLM-free.

1. **Transport (reused).** `transport.rs` framing + tag multiplexer + per-user socket discovery.
2. **Peer-serve endpoint (new, opt-in).** A unix socket speaking a codescout-native JSON envelope. Two request classes: **`tool.call`** (Phase 1, synchronous, maps onto existing tool dispatch) and **`job`** (Phase 2, asynchronous, enqueued + handed to the executor).
3. **Job executor = an agent, not codescout.** Attach-first: a Claude Code/Hermes session attached to B claims the job and works using B's local codescout tools.

| Unit | Responsibility |
|---|---|
| `src/peer/mod.rs` | socket discovery, envelope types, version negotiation |
| `src/peer/server.rs` | peer-serve listener (reuses tag mux), `tool.call` dispatch, job queue, audit log |
| `src/peer/client.rs` | requester side: connect to peer, send envelope, await/poll, `@peerB:` handle namespacing |
| `src/peer/job.rs` | job model (id/mode/goal/status/result), executor handoff |
| `src/peer/registry.rs` | the peer registry file (id → target/description/default_access) |
| MCP surface | a new `peer` tool for the requesting agent (actions: `query`/`knowledge`/`explore`/`work`/`status`) |

## 5. Wire protocol / envelope

Built on `transport.rs` (`Content-Length`-framed JSON). Common envelope:

```jsonc
{ "v": 1, "id": "a:7", "kind": "request|response|event|error",
  "method": "...", "params": {...}, "result": {...},
  "error": { "code": "...", "message": "...", "data": {...} } }
```

**Methods:**
- `hello` → version negotiation + peer identity (`project`, `root`) + **capabilities** (served tools, `read_only`, executor-available). Cached + replayed like the mux's `cached_init`.
- `tool.call{ tool, args }` → thin pass-through to codescout's existing dispatch (`call_content`, `src/tools/core/types.rs:485`); `result` is the tool's normal JSON output. Covers modes 2 + 3; auto-covers future tools.
- `buffer.read{ handle, json_path?|start_line?|end_line? }` / `buffer.grep{ handle, pattern }` → proxy reads of B's `OutputBuffer` (decision D-Buffer).
- `job.submit{ mode: explore|work, goal, context?, constraints? }` → `{ job_id }`; `job.status{ job_id }`; `job.result{ job_id }`; `job.cancel{ job_id }`.
- `kind:"event"`: `job.progress`, `job.completed` — pushed peer→requester via the notification fan-out pattern.

Networked-ready: pure JSON, no unix-socket assumptions; a future TCP/TLS transport swaps only connect/listen.

## 6. Discovery, lifecycle, registry

- **Registry is a file, not a service.** `src/peer/registry.rs` reads a registry file mapping `id → { target: <peer project root>, description, default_access: ro|rw }`. The serve socket is *derived* from `target` via the per-user `workspace_hash` scheme (a new `codescout-<hash>-peer.sock`, distinct from the mux's `-mux-` socket) — a peer is addressed by its project; discovery resolves the socket. Registry file format (TOML vs JSON) and location are an implementation detail for the plan. **Manual editing now; a future LLM-manager process writes the same file** — no registration daemon (Principle 4: one registrar today). The **`description` is agentic surface** (a manager picks peers by reading it) — documented + first-class, weighted like a tool hint.
- **Lifecycle reused from the mux:** deterministic per-user socket path, lock file, 300s idle-timeout (extended while buffer leases / jobs are live), `retry_on_peer_disconnect` modeled on `retry_on_mux_disconnect`.
- **Executor handoff:** a `job` sits in B's queue until an attached agent claims it (`queued→claimed→running→done`).

## 7. Modes → messages

| Mode | Method(s) | Sync/Async | Gate |
|---|---|---|---|
| 2 — Q&A | `tool.call{ tool: symbols\|references\|semantic_search\|read_file }` | sync | read |
| 3 — Knowledge | `tool.call{ tool: librarian\|memory }` | sync | read |
| 1 — Explore | `job.submit{mode:explore}` → status/result/progress | async | read |
| 4 — Work | `job.submit{mode:work, constraints{branch}}` → result{diff,branch} | async | **write (RW + grant)** |

**Buffer-proxy flow (D-Buffer):** B's `tool.call` overflows → `call_content` buffers on B → B returns `{output_id:"@tool_X", summary, hint}` → A's client namespaces to `@peerB:tool_X` → A's `read_file("@peerB:tool_X", …)` routes a `buffer.read` to B → B's `OutputBuffer.get()` (`src/tools/output_buffer.rs:42`, in-memory LRU) returns the slice. `@ref` grammar preserved verbatim; transfer is lazy.

## 8. Trust & safety

- **D-Wall enforcement:** peer-side = `PeerServer.read_only` + the `is_write_call` gate rejects write tools at the peer boundary *before* dispatch (revised 2026-06-01 — the Agent write-guard alone never engages because the served workspace is the always-rw home); `hello.read_only` mirrors the peer grant. Requester-side = registry write-grant, default `ro`, elevation explicit. Phase-1.5 adds the workspace-pinning route so the Agent write-guard engages too. Defense in depth.
- **Work isolation:** `work` jobs run **on a branch**, return a **diff**, **never auto-merge**. Requester's agent + human review and land via the Standard Ship Sequence. Blast radius of a buggy/hijacked executor = a reviewable branch.
- **Executor inherits host sandbox:** the executor is an agent (Claude Code/Hermes), so its writes flow through *that* harness's approval/hooks/container — codescout adds no new sandbox and no new autonomous writer. *Footgun named:* B's safety = B's harness's safety; don't grant write to a peer whose executor you don't trust.
- **Audit (D-Audit):** `{requester_id, method, target, ts}` per write/job on B.
- **Deferred (D-Networked-hardening):** auth + unguessable handles when transport goes TCP.

## 9. Error handling

Every peer failure → `RecoverableError` (`isError:false`, sibling calls survive, actionable hint) — never a bare crash. The LLM-facing error surface is the project's moat.

| Envelope error | Requester sees | Hint |
|---|---|---|
| `PEER_UNREACHABLE` | RecoverableError | "peer not running/registered — start or register it" |
| `WRITE_DENIED` | RecoverableError | "peer read-only / no grant — elevate the registry grant" |
| `TOOL_ERROR` | pass-through | the remote tool's own hint, verbatim (no double-wrap) |
| `JOB_TIMEOUT` | RecoverableError | "no attached agent claimed the job — attach one or enable spawn" |
| `JOB_FAILED` | RecoverableError | executor's failure reason |
| `BUFFER_GONE` | RecoverableError | "peer buffer expired — re-run the tool.call" |
| `VERSION_MISMATCH` | `bail!` | fatal — incompatible protocol |

- `retry_on_peer_disconnect` modeled on `retry_on_mux_disconnect` for transient socket drops.
- **Durability asymmetry:** a `job` survives requester disconnect (owned by B, keyed by `job_id`; reconnect → re-poll). A sync `tool.call` does not. *This answers Heuristic 5* — the async job bus is justified precisely because the requester must be able to detach and reattach across a minutes-long exploration.

## 10. Testing

- **Fake-peer fixture** modeled on `tests/fixtures/fake_lsp_cancelled.py` (already speaks `Content-Length` JSON) — a stub peer; requester-side tests need no second real codescout.
- **Prove-the-wall (D-Wall):** write `tool.call` + `work` job vs a `read_only` peer → assert `WRITE_DENIED`; assert read `tool.call`s still succeed.
- **Three-query sandwich for the proxy lease (D-Buffer):** read `@peerB:tool_X` → force-evict on B → assert `BUFFER_GONE` (stale) → re-lease → assert fresh.
- **Job lifecycle:** submit → status transitions → result; cancel; attach-first timeout (no claimant → `JOB_TIMEOUT`).
- **Transport/multiplex:** envelope round-trip + tag namespacing (reuse mux protocol tests).
- **Env isolation:** any test building an `Agent`/workspace carries `EnvGuard` + `#[serial_test::serial]`.

## 11. Phasing

- **Phase 1 — remote tools (modes 2 + 3).** `hello`, `tool.call`, `buffer.read`/`grep` + lease, registry, RO/RW wall, audit. Near-pure reuse of existing dispatch + transport; low risk; immediately useful.
- **Phase 2 — jobs (modes 1 + 4).** `job.*` + attach-first executor + `job.progress` events + work-on-branch isolation. Adds a queue + handoff seam; cognition borrowed from the executor agent.

## 12. Deferred / open

- Networked transport (TCP/TLS) + per-request auth + unguessable handle tokens.
- Spawn-on-demand executor (`peer.executor_command`).
- Cross-machine peers.
- **Workspace-pinning RO convergence (Phase 1.5):** make the peer-serve Agent's home neutral and pin the served workspace read-only (`ensure_resident(peer_root, ro)` + per-call `workspace_override`) so the Agent write-guard engages alongside the peer-layer `is_write_call` gate. See the revised D-Wall (§3).

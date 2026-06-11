# ADR-2026-06-11 — Enforce the Mux Single-Owner Invariant Causally

## Status

Proposed — design recorded on `experiments` 2026-06-11. Implementation plan:
[[2026-06-11-mux-single-owner-invariant-impl]]. Reap mechanism (S1) is the
self-heal "option B" already scoped in bug
[[2026-06-11-mux-failure-masks-rocksdb-lock-collision]].

## Context

The kotlin (and rust) language server is fronted by a per-workspace
**multiplexer (mux)** so that N codescout processes share one LSP:

- Socket + lock paths are keyed by `(language, workspace_hash)` under
  `per_user_runtime_dir()` — `src/lsp/mux/mod.rs:15`. Every codescout process
  for the same user + workspace + language computes the *same* socket path, so
  sharing is by construction.
- `LspManager::get_or_start_via_mux` (`src/lsp/manager.rs:563`) `flock`s the
  ownership lock: lock free → spawn a detached `codescout mux` child, wait for
  its `ready` line, connect; lock held → connect to the already-running socket.
- The mux process `run` (`src/lsp/mux/process.rs:66`) holds the ownership lock,
  spawns **one** real LSP child with `kill_on_drop(true)`
  (`src/lsp/mux/process.rs:93`), and fans it out over the socket with idle-TTL
  shutdown, a memory watchdog, and document-state coherence.

The genuinely exclusive resource is **not** "the LSP" — it is the kotlin-lsp
**RocksDB index** under the analyzer home (`src/lsp/servers/mod.rs:73`). On the
RocksDB-migrated build (kotlin-lsp ≥ v262.4739.0; we directly observed
`…/rocks/v492/LOCK`) the index takes an **exclusive per-directory lock**.
Upstream's pre-RocksDB multi-instance index sharing
(v261.13587.0: *"Indicies are now stored in a dedicated folder and are properly
shared between multiple projects and LS instances"*) **predates** the RocksDB
migration and does not describe our build — see
[[2026-06-11-kotlin-lsp-upgrade-decoupled-from-sharing]].

The architecture therefore has one load-bearing invariant:

> **At most one process holds a given workspace's RocksDB index, and that
> process is the live mux.**

Three failure seams violate it — all observed on `backend-kotlin` 2026-06-11:

- **S1 — two locks, no causal link.** The mux-ownership lock
  (`per_user_runtime_dir`, held by the codescout/mux process) and the RocksDB
  index lock (analyzer home, held by the kotlin-lsp JVM) never check each other.
  Holding the ownership lock does not prove the index is free.
- **S2 — SIGKILL orphans the JVM.** `kill_on_drop(true)` rides `Child::drop`,
  which never runs under SIGKILL/OOM-kill. The spawn path (`process.rs:66-135`)
  has **no** `setsid`, process-group, `pre_exec`, or signal handler — read-
  confirmed (reconnaissance-patterns R-26). A SIGKILLed mux leaves its JVM
  reparented to `systemd`, squatting the index forever.
- **S3 — poison fallback.** `get_or_start`'s mux-failure handler has three
  branches: two correctly `return Err` (the index-contention signature check
  and the `kotlin_index_lock_held` probe), but the third sets `config.mux =
  false` and spawns a **competing direct LSP** on the same index.

## Decision

**Keep the mux as the single-owner boundary. Do not introduce a new transport
or a per-user daemon.** Make the invariant *causally enforced* at the mux
chokepoint via three mechanisms (one per seam):

1. **(S2) The mux owns its LSP as a process group and reaps on signalled exit.**
   `setsid` the mux into its own group; spawn the LSP child into it; install
   SIGTERM/SIGINT handlers that kill the group. `kill_on_drop(true)` stays as
   the in-process net but is no longer the only net.

2. **(S1) Ownership-lock acquisition verifies — and reaps — the index lock.**
   At mux startup, after taking the ownership lock, probe the RocksDB `LOCK`
   (reusing `kotlin_index_lock_held`, `src/lsp/manager.rs:236`, and
   `posix_write_lock_is_held`, `src/lsp/manager.rs:198`). If the index is held
   by a PID whose mux socket is dead (an orphan), reap it, then proceed.
   Ownership of the mux lock must *imply* the index is free.

3. **(S3) Remove the silent direct-LSP fallback for mux languages.** For
   `mux: true` languages (`src/lsp/servers/mod.rs` — kotlin, rust), never
   silently set `config.mux = false`. Make the fallback an explicit, default-off
   opt-in retained only for the test-runner case (`current_exe()` is not the
   codescout binary).

## Consequences

### Now easier

- One place reasons about index ownership. The deadlock class
  (orphan squatter starves every future mux) and the poison-fallback class
  (a second index-holder that is not the mux) both close.

### Now harder

- **Signal handling in the mux event loop.** The SIGKILL path *cannot* be
  caught, so its regression test must assert the **reap-before-spawn** net
  (mechanism 2), not the handler. How to test a path whose signal is
  uncatchable is the **Testing Snow Leopard's** craft — referral, not folded in
  here.
- **Orphan-vs-live classification must read process/lock state**, not issue a
  probe call from a second client — a second-client probe *recreates* the
  contention. This is the recorded miss R-23 in
  `docs/trackers/reconnaissance-patterns.md`; do not repeat it.
- **The no-fallback error text is agentic surface.** Per the project's
  agentic-surface-as-moat discipline, the error on the removed-fallback path
  must name the recovery (`fuser … LOCK`, retry), as the existing
  index-contention branch already does. A bare failure is a regression even if
  the diff looks clean.

### Change scenarios absorbed

- A second Claude Code window, a `codescout start --debug` server, or a parallel
  subagent targets the same kotlin workspace → it **joins** the mux, never
  spawns a competitor.
- The mux is SIGKILLed (OOM-killer, `kill -9`, crash) → its JVM dies with it; no
  immortal index squatter.
- A transient mux startup failure → an actionable retry error, not a silent
  degrade to a competing direct LSP.

### Revisit-when

- A two-instance test on an upgraded kotlin-lsp build proves concurrent index
  access works (RocksDB secondary/read-only mode) → the mux can demote to pure
  connection-pooling and mechanism 2 becomes a rare safety net rather than the
  primary defense. Tracked in
  [[2026-06-11-kotlin-lsp-upgrade-decoupled-from-sharing]].

**Confidence: high** — grounded in the observed exclusive `rocks/v492/LOCK` and
a read of the spawn path (`process.rs:66-135`).

## Alternatives considered

1. **Per-user daemon owning all LSPs; MCP servers become thin RPC clients.**
   Rejected — rule-of-three (`tool-registration-rule-of-three`). One workspace's
   pain does not justify a third always-on process type with its own crash /
   upgrade / lifecycle story. The mux *already is* a per-workspace daemon;
   promoting it to a global fabric is abstraction on a sample of one. Revisit if
   we want a single global LSP fabric across all workspaces, not just
   per-workspace sharing.

2. **Collapse the two locks into one — use RocksDB's `LOCK` as the ownership
   lock.** Rejected — its path carries a version segment (`…/rocks/v492/LOCK`)
   that changes across builds; coupling our ownership protocol to JetBrains's
   internal lock-file path is the fragile coupling the boundary exists to avoid.
   Keep our own lock; make it *verify* theirs (probe by directory glob, not the
   hardcoded `v492`).

3. **Each instance opens RocksDB read-only / as a secondary instance.**
   Rejected — unverified upstream capability on our build, and writes
   (`didChange`) need a primary. Betting the architecture on an unconfirmed
   RocksDB mode forks the design on hope; our observed exclusive lock says it is
   not what this build does.

4. **Abandon sharing — one LSP per instance.** Rejected — that is the contention
   we are escaping.

## Related

- [[2026-06-11-mux-failure-masks-rocksdb-lock-collision]] — bug; mechanism 2 is
  its deferred self-heal "option B".
- [[2026-06-11-kotlin-lsp-upgrade-decoupled-from-sharing]] — the decoupled
  upgrade decision and the ADR-1 revisit trigger.
- [[2026-06-11-mux-single-owner-invariant-impl]] — the implementation plan.
- `docs/trackers/reconnaissance-patterns.md` — R-23 (read-state-not-2nd-client),
  R-26 (grep locates, read confirms).
- Code: `src/lsp/manager.rs:563` (`get_or_start_via_mux`),
  `src/lsp/mux/process.rs:66` (`run`), `src/lsp/manager.rs:236`
  (`kotlin_index_lock_held`), `src/lsp/servers/mod.rs:73` (analyzer home).

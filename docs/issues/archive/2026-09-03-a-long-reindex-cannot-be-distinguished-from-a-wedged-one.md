---
kind: bug
status: fixed
tags:
- librarian
- observability
- reindex
- codescout-tool
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-06
opened: 2026-09-03
owner: marius
related:
- docs/issues/archive/2026-08-24-index-status-lock-contention-reads-as-failed.md
- docs/trackers/bug-fix-session-log.md
severity: medium
---

# BUG: a running `librarian(action="reindex")` emits nothing, so a working 12-minute run and a wedged one are the same observation

## Summary

`librarian(action="reindex", reembed=true)` can run for many minutes emitting no
progress, and the `librarian` tool exposes no action that answers *"is a reindex
running, and how far along is it?"*. Neither the caller nor a peer session can
distinguish healthy work from a hang, so both fall back to process-level proxies
(CPU%, run state, lock intuition) that are **non-discriminating**: a sleeping,
low-CPU process is the signature of a leaked lock guard *and* of an I/O-bound
embed loop. The `index` family closed exactly this gap on 2026-08-24 by
disclosing `running_elsewhere` + `holder_pid`
(`docs/issues/archive/2026-08-24-index-status-lock-contention-reads-as-failed.md`,
`05a0548d57664984`). `librarian` never received the equivalent.

## Symptom (Effect)

2026-09-03, during a full re-embed of this repo's artifact corpus after
`6f032dbd`:

- The reindex ran **~12m10s** and wrote **27,762** vectors (~38/sec).
- Total output visible to anyone, at any point before it returned: **none**.
- A peer session (`codescout-7e`, addressed by socket) observing the process
  reported the diagnosis **"leaked lock guard"** and asked whether the write lock
  needed releasing.
- The only way the peer's question could be answered was for me to hand-query the
  catalog and send a progress delta by `SendMessage`: `9760 → 10518` rows in
  `artifact_chunk` with a vector. That is a human relay standing in for a missing
  status surface.

The peer's inference was reasonable and wrong, and nothing in the system could
have corrected it. There is no error string to quote here — **the absence of
output is the symptom.**

## Reproduction

```
git rev-parse HEAD   # 596a8d7ae3f67c901ee6d6c6c977ec4cc723cd46, branch experiments
cargo rb && /mcp     # reconnect

# in .codescout/project.toml raise tool_timeout_secs (see the sibling bug below,
# or this call is killed at 60s before the symptom is even reachable)
librarian(action="reindex", reembed=true)

# from any other session, while it runs, try to answer
#   "is that reindex progressing, or wedged?"
# using only the librarian tool surface.
```

The `librarian` tool's own schema enumerates its actions: `context`, `reindex`,
`tracker_design`, `workspace_state_at`, `audit_doc_refs`, `legibility_scan`,
`link_scan`, `doctor`, `merge_worktree`, `audit_log`. **There is no `status`.**

## Environment

- Linux 7.1.9-zen1-2-zen, `experiments` @ `596a8d7a`
- MCP stdio transport; ~15 codescout server processes across 3 config profiles
  share one catalog (`~/.local/share/librarian/catalog.db`)
- Qdrant + sqlite-vec artifact stores both live; remote embedder over HTTP

## Root cause

Two independent facts, neither of which is a lock problem.

**1. The tool is a single request/response with no intermediate emission.**
`reindex`'s embed loop (`src/librarian/tools/reindex.rs:368-390`) iterates
`embed_queue` awaiting `svc.embed_artifact(...)` per item and accumulates counters
in locals (`total_embedded`, `embed_errors`); nothing is published until the
response is built. Durable state is written **after** the loop
(`:424-435` sets `last_reindex_embed_error_count`), so even a catalog query
mid-run cannot read the run's own bookkeeping — only the side effect of rows
appearing in `artifact_chunk`, which is what I hand-queried.

**2. There is no status action, and no lock an observer could read either.**
- `ToolContext.catalog` is `Arc<parking_lot::Mutex<Catalog>>`
  (`src/librarian/tools/mod.rs:85`) — **in-process only**, invisible outside the
  process that holds it.
- There is no catalog lock *file*. `lock_path` / `LOCK_FILE` appear only in the
  LSP mux (`src/lsp/mux/process.rs:68`) and `src/retrieval/index_lock.rs`, which
  belongs to the **code** index — a different subsystem, and the one that already
  has the disclosure.
- Cross-process safety is `PRAGMA busy_timeout = 5000` over WAL
  (`src/librarian/catalog/mod.rs:481`, `:524`) — a retry budget, not a queryable
  holder.

**So there is nothing to disclose today even if a caller asked.** The archived
`index` bug had a lock file whose first line is the holder's PID, which is what
made its fix cheap; `librarian` has no such artifact and would need to publish
one (or a progress row in `meta`).

*Measured 2026-09-03:* `wc`-free wall clock over the live reindex (~12m10s,
27,762 vectors); catalog counts read directly at 9760 and 10518. *Read at the
bytes this session:* every `path:line` above. *Not measured:* whether any peer's
librarian write actually exceeded the 5000 ms `busy_timeout` during the run. A
27k-upsert loop is the workload that could, but no `database is locked` error was
observed and the peer reported a process symptom, not a SQLite one. Left as a
thing to check, not a thing concluded.

## Evidence

### The exempt-list sibling, which makes this reachable at all

```rust
// src/server.rs:1264-1273
/// - `index` / `index_library`: embedding loops that run for many minutes.
fn tool_skips_server_timeout(name: &str) -> bool {
    matches!(name, "index" | "index_library" | "run_command")
}
```

`librarian` is an embedding loop that runs for many minutes and is absent. Filed
separately — see References; the two are siblings, not duplicates: this bug is
about *disclosure*, that one is about *survival*.

### The mutex is not held across the loop

```rust
// src/librarian/tools/reindex.rs:346-390 (abridged)
let (report, embed_queue) = {
    let cat = ctx.catalog.lock();
    indexer::index_repo_sync(...)?
};                                   // <- guard dropped here

if let (Some(svc), Some(store)) = (...) {
    for item in &embed_queue {
        match svc.embed_artifact(...).await {          // no catalog lock held
            Ok(vec) => match store.upsert(...).await { // re-takes it briefly
```

`SqliteVecArtifactStore` holds the same `Arc<Mutex<Catalog>>`
(`src/librarian/artifact_store.rs:351`), so across the run the mutex is acquired
and released on the order of 27,762 times and never held long.

### The prior art in the neighbouring subsystem

`index(action="status")` reports `{"status": "running_elsewhere", "holder_pid":
49334}`, live-verified against a rival CLI process on 2026-08-24
(`05a0548d57664984` § Summary). That fix is the template for this one.

## Hypotheses tried

1. **Hypothesis:** the reindex holds the catalog write lock for its duration, and
   that is what the peer observed.
   **Test:** read `src/librarian/tools/reindex.rs:346-390`,
   `src/librarian/tools/mod.rs:85`, `src/librarian/catalog/mod.rs:481`; grep the
   tree for `lock_path` / `LOCK_FILE`.
   **Verdict:** **rejected.** The guard is dropped before the loop, the mutex is
   in-process only, and no catalog lock file exists.
   **Evidence:** § *The mutex is not held across the loop*.
   **Note:** this was my own framing, adopted from the peer's premise while
   correcting their conclusion. Recorded as
   `bug-fix-session-log:F-109`, because the reasoning error is the reusable part.

2. **Hypothesis:** a `librarian` status/progress action exists and I missed it.
   **Test:** enumerate the served `librarian` action enum.
   **Verdict:** **rejected** — `context | reindex | tracker_design |
   workspace_state_at | audit_doc_refs | legibility_scan | link_scan | doctor |
   merge_worktree | audit_log`.

3. **Hypothesis:** a peer's librarian write was blocked past `busy_timeout`
   during the run, which would make this a contention bug rather than a
   disclosure one.
   **Verdict:** **deferred** — plausible from the workload, unobserved. Do not
   promote it to a cause without a `database is locked` sighting.

## Fix

*Implemented 2026-09-05.* `src/librarian/reindex_progress.rs` (new),
`src/librarian/tools/reindex.rs`, `src/librarian/tools/status.rs` (new),
`src/librarian/tools/librarian.rs`, `src/librarian/adapter.rs`.

The shape is the one planned above, plus three things the plan did not have.

**Publishing.** A running reindex writes a `catalog_meta` row keyed
`reindex_in_progress:<pid>` holding `{pid, started_ms, heartbeat_ms, done,
total, scope}`. Published once before the first embed — so a run whose first
item is slow is still visible — then refreshed on a **2 s** throttle, and
cleared when the loop ends.

**Reading.** `librarian(action="status")` partitions rows into `running` and
`stale` by resolving each holder's pid, and reports unparseable rows rather
than dropping them. Both halves shipped together: a published counter nothing
can read is the *"alarm nothing reaches"* defect in `CLAUDE.md` § *Testing
Discipline*.

**Answers to the two design questions this bug deliberately left open:**

- *Frequency* — throttled by **time**, not item count, because per-item cost
  varies by orders of magnitude between a one-line memory and a 25 KB tracker
  section. At 2 s a 12-minute run writes ~360 rows instead of 27,762.
- *Staleness* — resolved by **holder liveness**, never by age. Writing that
  down surfaced a live defect: `platform::process_alive` cast `u32 as i32`, so
  `kill`'s addressing mode flipped and a nonexistent process read as **alive**.
  Filed and fixed as
  `docs/issues/archive/2026-09-05-process-alive-reports-a-nonexistent-process-as-alive.md`;
  this fix's stale-row pruning does not work without it.

**A third question the plan did not contain, and the one that would have
shipped a broken fix.** `LibrarianAdapter::is_write` classifies unlisted
actions as writes (`adapter.rs`, `_ => true`), and a write takes the
cross-process write lock — which a running `reindex` holds. Left to the
default, `status` would have **blocked until the run it was asking about
finished, then truthfully reported that nothing was running**: a brand-new
instrument reproducing this bug exactly. `status` is in the unconditional-read
arm, pinned by
`librarian_status_is_a_read_or_it_cannot_observe_a_running_reindex`.

**Not done, deliberately:** the fields are not folded into `doctor`. `doctor`
is a full-catalog scan and would contend with the very run an observer is
asking about; the point of `status` is that it is cheap and lock-free.

SHA: `90336870` (**`experiments`**)
patch-id: `a4b40a8eec5b72daa2d97683935bdaff36aaba2a`

Depends on `01b185d6` / patch-id `5ee25dcd470221a49b7da116dcc1f796da18c908`
(`platform::process_alive`), without which the stale-row pruning here is inert:
a dead holder's pid read as **alive**, so its row was never collected.
## Tests added

The guard demanded above — *"a second process reading progress while a first is
mid-run"* — is
`librarian::tools::reindex::tests::a_running_reindex_publishes_progress_a_separate_connection_can_read`.
It uses a **file-backed** catalog and an embedder that opens its own
`Catalog::open` on each call, because a read through the writer's handle passes
even if the value never leaves the process, and an in-memory catalog cannot
express the failure at all.

**Observed RED under three production-path mutations** (not test-input
mutations), 2026-09-05:

| mutation | failure |
|---|---|
| publish `0` instead of `embed_done` | `the counter never advanced past its initial publish — [(pid, 0, 4) ×4]` |
| delete the post-loop `clear` | `the progress row must be cleared when the loop ends: [ReindexProgress { done: 1, total: 4, … }]` |
| drop `status` from `is_write`'s read arm | `librarian(action="status") — must answer while a reindex holds the lock: left: true, right: false` |

Supporting units: 11 in `librarian::reindex_progress::tests` (cross-connection
visibility, per-pid keying against clobber, LIKE-escape against a decoy key,
unparseable rows surfaced not dropped, prune-dead-keep-live), 5 in
`librarian::tools::status::tests`, 3 in `platform::unix::tests`.

**Read out of the DEFAULT lane, never the lean one.** `--no-default-features`
switches the librarian off, so it compiles none of this code and returns
`exit=0` whether it is right or broken. Measured on the gate run that shipped
this: **0** `librarian::` tests in the lean lane against **1714** in the
default one.

**Live-verified 2026-09-06**, after `cargo rb` and an MCP reconnect:
`librarian(action="status")` dispatches and returns its negative-result note
verbatim. The pre-reconnect server refused the same call with `unknown action
'status'`, which is the discriminating pair — one binary without the code, one
with it, same call.

That check was nearly misread, and the misreading is worth keeping. The
rebuilt binary was on disk and a server process was live, yet the call failed;
the available conclusion was "the rebuild missed my changes". It had not.
`/proc/<pid>/exe` on the serving process read **`(deleted)`** and did not
contain `reindex_in_progress:`, while the on-disk file at the same path did —
two objects behind one name. Same shape as the misdiagnosis this bug is about:
several signals consistent with both stories, and exactly one that separates
them.
## Workarounds

> **Corrected 2026-09-05 — the workaround below was blind on the run that needs it most.**
> Both halves were validated against a **first** embed and silently do not transfer to a
> **re-embed**. Second instance of this bug's own mechanism, made by its author. See
> `reconnaissance-patterns:R-182`.

The original text prescribed:

```sql
SELECT COUNT(*) FROM artifact_chunk;   -- rows grow as the walk queues them
```

That is true of a first embed — the 2026-09-03 relay above moved `9760 → 10518` on
exactly this query. It is **flat for the entire embed loop of a `reembed=true` run over
unchanged content**: no new chunk rows are created, so the count sits still while tens of
thousands of vectors are written. Measured 2026-09-04 on a 28,379-vector run that
reported `unchanged: 1483`.

**During a re-embed there is no monotone durable observable at all**, which is the
strongest form of this bug and the reason no better instrument exists:

| candidate | why it is flat or absent |
|---|---|
| `artifact_chunk` row count | no new rows for unchanged content |
| vector-store point count | `upsert` is idempotent on `chunk_id` (`artifact_store.rs:137`) — overwrites in place |
| `artifact.embedded_sha256` | the stamp block runs **after** the loop (`reindex.rs:447`) |
| `catalog.db` mtime | same reason |
| `embed_done` counter | a local `u32`, published only via `ctx.progress`, which is `None` unless the client sent `_meta.progressToken` — Claude Code sends `meta: None` |
| CPU% of the codescout process | the compute is in the embedder process (`llama-server`); codescout is IO-bound on HTTP |

So the honest workaround today is: **ask the running session**, and have it report the
counter it holds in memory. Nothing else discriminates. Inferring from `ps` is worse than
useless — a sleeping, low-CPU codescout process is the signature of a leaked lock guard
**and** of a healthy IO-bound embed loop, and a scout that stacks six such proxies gets
six copies of one blind spot, not corroboration.

Which is the argument for the fix rather than a note beside it: the counter exists
(`reindex.rs:437`) and reaches nobody durable.
## Resume

Start at `src/librarian/tools/reindex.rs:368-390` (the emitting-nothing loop) and
`src/librarian/tools/reindex.rs:424-435` (the `set_meta` call that shows the
mechanism already exists). Read `05a0548d57664984` first for the shape of the
`index` fix and the reasons its `holder_pid` disclosure was designed the way it
was.

## References

- `docs/issues/archive/2026-08-24-index-status-lock-contention-reads-as-failed.md`
  (`05a0548d57664984`) — the same gap in the code-index subsystem, fixed.
- `docs/trackers/bug-fix-session-log.md` § `F-109` — the reasoning error that
  nearly filed this bug with a false mechanism.
- `CLAUDE.md` § *Observer Blindness* — the three-question form this instantiates:
  who structurally cannot see it (every observer, including the caller), who can
  (the running loop, which holds the counter), and the check that runs when
  nobody is worried (a published progress row).

### Cluster adjudication

Tagged `cluster/gate-keyed-on-unobservable-event` (`IC-2`), matching the archived
`index` sibling, on the **remedy test**: both are fixed by emitting the real
state instead of leaving readers to a proxy.

One respect in which it differs, stated so a later reader can withdraw the tag
rather than re-derive the doubt: in `IC-2`'s canonical form the **system**
substitutes the proxy (`index(status)` returned a stale `"failed"`). Here the
system emits nothing and the **observer** substitutes one (CPU%, run state). If
that distinction is judged to matter, this belongs in a new class, not in
`cluster/unclassified`.

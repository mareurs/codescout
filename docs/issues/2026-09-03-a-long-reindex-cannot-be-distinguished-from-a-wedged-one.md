---
kind: bug
status: open
tags:
- librarian
- observability
- reindex
- codescout-tool
- cluster/gate-keyed-on-unobservable-event
closed: null
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

*Plan only — not implemented.*

The cheap shape, matching the `index` precedent: have `reindex` publish progress
to a durable, queryable place **during** the loop rather than only after it. The
catalog already has the mechanism — `catalog::gc::set_meta` is what `:424-435`
uses at the end. Writing `reindex_in_progress` (pid, started_at, done, total)
every N items, and clearing it in a guard on the way out, makes the run readable
by any process sharing the catalog file.

Then either add a `librarian(action="status")`, or fold the fields into
`doctor`'s report so an observer's existing habit surfaces it.

**Two design questions this bug does not decide:**

- *Frequency.* A `set_meta` per item is 27,762 extra write transactions competing
  with the upserts for the same 5000 ms `busy_timeout` budget — the fix could
  cause the contention this bug currently only speculates about. Batch it.
- *Staleness.* A progress row whose writer died is exactly the class of defect
  the `index` bug was (`cluster/gate-keyed-on-unobservable-event`): a stale
  record read as current. It needs a pid whose liveness a reader can check, which
  is why the `index` lock file stores one on its first line.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None yet — nothing is fixed. When it is, the guard must assert on the
**observable**: a second process reading progress while a first is mid-run. A
test that only checks `set_meta` was called is monotone under the writer dying,
which is the failure that matters.

## Workarounds

Query the side effect directly while a reindex runs:

```sql
SELECT COUNT(*) FROM artifact_chunk;   -- rows grow as the walk queues them
```

and for a peer, ask the running session rather than inferring from `ps` — a
sleeping low-CPU codescout process carries no information about which of the two
states it is in.

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

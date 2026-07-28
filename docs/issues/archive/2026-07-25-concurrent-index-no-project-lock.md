---
id: '87a8cb14f9b1eb84'
kind: bug
status: fixed
title: Concurrent `codescout index` on one project has no mutual exclusion — duplicate runs double the embedding workload
tags:
- retrieval
- indexing
- concurrency
- resource-exhaustion
closed: 2026-07-28
last_observed: 2026-07-27
opened: 2026-07-25
owner: marius
related:
- docs/issues/2026-06-19-mcp-server-oom-68gb.md
- docs/issues/2026-07-25-reindex-reembed-noop-without-force.md
severity: high
---

# BUG: Concurrent `codescout index` on one project has no mutual exclusion — duplicate runs double the embedding workload

## Summary

`RetrievalClient::sync_project` takes no per-project lock. Two concurrent
`codescout index --project <same-path>` invocations both run the full
`stream_index` pipeline against the same Qdrant collection and `project_id`,
duplicating the entire embedding workload for zero added value. Observed today
as two identical `backend-kotlin` index jobs pinning ~12 of 16 cores for 56
minutes with no client-side progress.

## Symptom (Effect)

Three `codescout index` processes alive simultaneously, two of them on the
**same** project, spawned 8 seconds apart:

```
    PID     ELAPSED     TIME STAT WCHAN                  COMMAND
  10661       56:36 00:00:00 Ssl  futex_do_wait          codescout index --project /home/marius/work/mirela/backend-kotlin
  11819       56:27 00:00:00 Ssl  futex_do_wait          codescout index --project /home/marius/work/mirela/backend-kotlin
  62748       39:39 00:00:00 Ssl  futex_do_wait          codescout index --project /home/marius/work/stefanini/southpole/MRV-poc
```

Each holds ESTABLISHED sockets to both embedder endpoints with empty queues —
i.e. blocked awaiting server responses, having consumed `00:00:00` CPU
themselves:

```
ESTAB 0 0 127.0.0.1:50560 127.0.0.1:48084 users:(("codescout",pid=10661,fd=17))
ESTAB 0 0 127.0.0.1:37068 127.0.0.1:48084 users:(("codescout",pid=11819,fd=17))
ESTAB 0 0 127.0.0.1:34352 127.0.0.1:48081 users:(("codescout",pid=11819,fd=16))
ESTAB 0 0 127.0.0.1:37398 127.0.0.1:48084 users:(("codescout",pid=62748,fd=16))
ESTAB 0 0 127.0.0.1:56480 127.0.0.1:48081 users:(("codescout",pid=62748,fd=18))
```

Host saturation while this ran (`top`, instantaneous):

```
load average: 16.59, 15.88, 15.08          # 16-core box
%Cpu(s): 82.0 us, 12.3 sy, 0.0 ni,  5.7 id
   1514 root  S 799.4  2.7   7,23 text-embeddings   # SPLADE sparse
   1379 root  R 404.2  2.5 197:57 llama-server      # CodeRankEmbed dense
MiB Mem : 31499.5 total, 2080.2 free, 19244.9 used
MiB Swap: 15751.0 total, 5452.0 free, 10298.9 used
```

After killing the three jobs and stopping the CPU embedders: `82.6 id`,
7589.8 MiB free, swap used 10298.9 → 6503.8 MiB.

No error, no warning, no "already indexing" message is emitted at any point.

## Reproduction

```bash
git rev-parse HEAD    # 52fcaf0118d9a6388a8c5828f1447b818d05f360 (branch: experiments)

# Terminal 1
codescout index --project /path/to/repo
# Terminal 2, immediately after
codescout index --project /path/to/repo      # same project
```

Both proceed. Expected: the second detects the first and either refuses with a
`RecoverableError` naming the holder, or waits.

Observe duplicated embedder load with `ss -tnp | grep 4808` (two client
connections per endpoint) and `top` (embedder CPU roughly doubles).

## Environment

- Linux 7.1.4-arch1-1, 16 cores, 31.5 GiB RAM
- codescout `experiments` @ `52fcaf01`, binary `~/.cargo/bin/codescout`
- Retrieval stack: `docker-compose.yml` profile `cpu` — `codescout-dense-cpu`
  (llama.cpp server, CodeRankEmbed-Q4_K_M), `codescout-sparse-cpu`
  (TEI `cpu-1.6`, Splade_PP_en_v1, `--dtype float32`), `codescout-qdrant`
- Six `codescout start --debug` MCP servers live across Claude Code sessions

## Root cause

`RetrievalClient::sync_project` (`src/retrieval/sync.rs:196-273`) has no
mutual-exclusion step. Its body runs, in order:

1. `self.code_store.ensure_collection(&collection, model_dim)`
2. `self.code_store.chunk_refs(&collection, project_id)` — the drift baseline
3. `stream_index(root, project_id, &collection, &server, &self.embedder, …)`

There is no lock acquisition anywhere in that path, and no caller-side guard.
The concurrency hazard is structural, not incidental: `chunk_refs` reads the
drift baseline *before* `stream_index` mutates it, so two overlapping runs each
compute their diff against a snapshot the other is invalidating.

**codescout already has the idiom this needs** — the gap is that it was never
applied to indexing:

- `src/agent/write_guard.rs:6` — `flock` (via `fs4`) on `.codescout/write.lock`,
  documented as serializing *writes*. It does **not** cover indexing; nothing on
  the `sync_project` path acquires it.
- `src/lsp/mux/process.rs:75-79` — `try_lock_exclusive()` on a per-workspace lock
  file, the ownership-arbiter pattern.
- `src/lsp/manager.rs:236` — `kotlin_index_lock_held()`, a per-language index
  lock. Precedent that "index work needs a lock" is already accepted in-repo.

**Not established:** what spawned the two duplicate jobs. Both are CLI processes
(`codescout index --project …`), not in-server background ops, so this is not
directly `agent/mod.rs:1581`'s `auto_index` path. With six MCP servers live, two
sessions each launching an index for the same project is the leading hypothesis
but is unverified — see Hypotheses tried #3. **The absence of the lock is
verified independently of what triggered the duplication**, and is the reason
the duplication was costly rather than merely redundant.

## Evidence

### `sync_project` body, read this session

`src/retrieval/sync.rs:196-273` via `symbols(symbol="RetrievalClient/sync_project",
include_body=true)`. No `flock`, no `try_lock`, no guard type, no
already-running probe. Sequence is `ensure_collection` → `chunk_refs` →
`stream_index` → `write_index_state`.

### Lock inventory across `src/**/*.rs`

`grep(pattern="flock|try_lock|LockFile|lockfile|index_lock|already_indexing|IndexGuard",
glob="src/**/*.rs")` → 40 matches in 7 files: `src/lsp/manager.rs` (21),
`src/agent/write_guard.rs` (10), `src/lsp/mux/process.rs` (4),
`src/librarian/catalog/gc.rs` (2), `src/agent/mod.rs` (1),
`src/librarian/adapter.rs` (1), `src/library/versions.rs` (1). **Zero in
`src/retrieval/`.**

### Cost measurement

See Symptom. Two identical jobs on `backend-kotlin` → the sparse embedder
averaged 764% and peaked at 799% CPU. SPLADE at `--dtype float32` on CPU is the
dominant cost, and it scales with client count because TEI runs
`--max-concurrent-requests 32`.


### 2026-07-27 recurrence — four concurrent jobs, and the cost was mis-attributed

Escalation of the original two-job observation. Four `codescout index` processes were
running against `backend-kotlin` simultaneously, all reparented to `systemd --user`
(PPID 2160) because their launching shells had exited:

```
PID       STARTED                     ELAPSED
1283296   ~05:50                      03:24:11
1436085   Mon Jul 27 07:20:52 2026    02:02:35
1545877   Mon Jul 27 08:15:14 2026    01:08:12
1551582   Mon Jul 27 08:18:12 2026    01:05:14
```

`fuser` on `.codescout/write.lock` showed three of them holding it open concurrently with
mode `F` — consistent with the root-cause finding that nothing on the `sync_project` path
takes an exclusive `flock`.

**What makes this recurrence worth recording is the mis-attribution it caused.** The
user-visible symptom was "the indexer has been running for three hours." That was
investigated at length and attributed to two real but secondary causes — corpus size from
over-fine chunking
([[2026-07-27-ast-chunker-no-minimum-chunk-size]]) and an 8-input request batch
([[../superpowers/specs/2026-07-27-embedder-batch-concurrency-design]]). Both are genuine.
Neither was the whole story: four processes were duplicating the same embedding work
through the same two servers.

It also explains a measurement that was written off at the time. A batch sweep against the
sparse server showed `queue_time` of 1.5-2.3 s out of 3.4 s total per request, recorded as
"normal GPU saturation." With a single client issuing one request at a time, TEI's internal
queue should have been empty. The queue depth was the other three indexers.

After killing all four, the GPU went from `100% util, 85 °C, 60 W` to `0% util, 70 °C,
5.4 W` with only the resident model servers left.

**Diagnostic lesson:** a per-process view (`ps -o etime -p <pid>`) answers "how long has
*this* one run" and silently hides duplication. `pgrep -af 'codescout index'` should be the
first step whenever indexing looks slow — before any throughput analysis, because
duplication invalidates every latency measurement taken under it.
## Hypotheses tried

1. **Hypothesis:** the index jobs were the CPU hogs.
   **Test:** `ps -o pcpu` then instantaneous `top -bn2`.
   **Verdict:** rejected — all three at `TIME 00:00:00` after 39–56 min. The
   *servers* burn the CPU; the clients are blocked. Note `ps %CPU` is a
   lifetime average, not instantaneous — both agreed here, but only `top`
   settles it.
   **Evidence:** Symptom § process table.

2. **Hypothesis:** `write_guard`'s `.codescout/write.lock` already serializes
   this.
   **Test:** read `src/agent/write_guard.rs:1-20` header docs; grep the
   `sync_project` path for any guard acquisition.
   **Verdict:** rejected — `write.lock` is scoped to write operations and is
   never acquired on the retrieval sync path.
   **Evidence:** Evidence § lock inventory.

3. **Hypothesis:** duplicates came from two MCP sessions each auto-indexing.
   **Test:** not yet run — would need session JSONL correlation against the
   spawn timestamps (8 s apart) and the six live `codescout start --debug` PIDs.
   **Verdict:** deferred. Does not gate the fix; the missing lock is the defect
   regardless of trigger.


4. **Hypothesis (2026-07-27):** the long index runtime was caused by corpus size and
   request batching alone.
   **Test:** killed the process that had been running longest, then re-checked for
   remaining indexers with `pgrep -af`.
   **Verdict:** rejected as a complete explanation. Three more indexers were still running.
   Corpus size and batch size are real contributors with their own bugs, but concurrent
   duplication was an unmeasured multiplier on top of both.

5. **Hypothesis (2026-07-27):** the 1.27x mean line-coverage overlap in the collection was
   an artifact of measuring mid-run, with concurrent writers laying down overlapping
   generations.
   **Test:** re-ran the coverage probe after all four indexers were killed and the GPU was
   idle.
   **Verdict:** rejected. Clean measurement is identical to the mid-run one — mean 1.27x,
   4.1% of `(file, start_line)` keys carrying 2+ variants, same worst-case files at 8.01x /
   5.53x / 5.41x. The overlap is structural to the chunker, not a concurrency artifact.
   Concurrency inflated *duration*, not coverage.
## Fix

**IMPLEMENTED** on `experiments` 2026-07-28 (`de9b1d34..50842163`, the index-lock +
embedder-batching work stream), and verified live the same day. This section
previously read "Plan, not yet implemented" — caught by a verify-open pass, which
is the zombie-open failure mode CLAUDE.md's cadence exists for.

Shipped as `src/retrieval/index_lock.rs`, acquired at `src/retrieval/sync.rs:234`
before the `chunk_refs` drift-baseline read (ordering is load-bearing: that read
establishes the baseline `stream_index` then mutates, so two overlapping runs would
each diff against a snapshot the other is invalidating).

**Deviations from the plan above, both deliberate:**

1. Plan said `.codescout/index.lock` per project root. Shipped keys on
   `sha256(project_id)` in `per_user_runtime_dir()` instead. Two reasons: the
   contended resource is the `(collection, project_id)` pair in Qdrant, not a
   filesystem root; and library syncs pass a third-party checkout as `root`, which
   must not gain a `.codescout/` directory.
2. Plan items 3 and 4 hold as written — `write.lock` is deliberately not reused, and
   the CLI refuses rather than queues.

**Plan item 2 was NOT implemented, and remains a real gap.** Contention surfaces via
`anyhow::Context` (`index_lock.rs`, `try_lock_exclusive().with_context(...)`), not
`RecoverableError`. The message is actionable — it names the lock path, explains the
first line is the holder's PID, and warns the holder may be an in-process background
index rather than a CLI run — but it carries no structured `Guidance`, so the MCP
layer renders it as a generic error. Per `get_guide("error-handling")` a
user-actionable contention condition should be `RecoverableError`. Tracked in Resume
below rather than silently marked done.

**Live verification, 2026-07-28** — the original symptom was four concurrent runs on
one project (3h24m / 2h02m / 1h08m / 1h05m, all orphaned to `systemd --user`).
After the fix, two separate index runs were observed on this host, each holding
exactly one lock and each completing alone: codescout at 18 minutes, MRV-poc at ~28
minutes. Useful throughput went from ~1.45 chunks/s (5.8 split four ways) to 4.45
— roughly 3x, entirely from eliminating the duplication.

Per CLAUDE.md the **master-side** SHAs go here after cherry-pick; the
`experiments`-side originals orphan on rebase and are deliberately not recorded.
## Tests added

In `src/retrieval/index_lock.rs`:

- `acquire_succeeds_for_fresh_project`
- `second_acquire_fails_while_first_is_held` — the core mutual-exclusion assertion,
  including that the error text carries the "already running" wording
- `different_projects_do_not_contend`
- `lock_is_released_on_drop`
- `preexisting_lock_file_does_not_block` — planted content shaped to kill two
  mutations at once: it starts with our own LIVE pid (so a PID-liveness check would
  refuse and fail) and is LONGER than what `acquire` writes (so deleting
  `set_len(0)` leaves a visible tail). A dead pid like 999999 pins neither — it is
  above `pid_max` and reads as dead anyway.
- `lock_path_is_deterministic_and_filename_safe`
- `lock_path_is_not_sited_in_bare_temp_dir` — guards the symlink-preemption hazard
- `acquire_in_does_not_touch_the_real_runtime_dir` (added 2026-07-28)

In `src/retrieval/sync.rs`:

- `sync_project_holds_index_lock_for_its_full_duration` — the guard that matters
  most, and the one whose design took two attempts. A single "acquire externally,
  then call `sync_project`, assert Err" test **cannot** distinguish `_index_lock`
  from `_`: if the lock is already held, `sync_project`'s own `acquire` fails
  identically under either binding, because the failure happens at
  `try_lock_exclusive()` before the binding pattern is reached. So it spawns
  `sync_project` against a `SlowEnsureStore` (provably still in flight) and probes
  from OUTSIDE at 100ms; that probe must fail iff the guard is still alive. Mutating
  the binding to `let _ =` passes 42/42 other tests with clippy clean, so this test
  is the only thing standing between the fix and a silent regression.
## Workarounds

Before launching an index, check for an existing one:

```bash
pgrep -af 'codescout index --project'
```

If the host is already saturated, identify the real consumers with
`top -bn2 -d 1` (not `ps %CPU`, which averages over process lifetime) and stop
the embedder containers by name rather than `docker compose down` — `down`
removes project-wide containers including `codescout-qdrant`, which has no
`profiles:` key:

```bash
docker stop codescout-dense-cpu codescout-sparse-cpu codescout-reranker-cpu
```

## Resume

N/A for the reported bug — fixed and verified. Two follow-ups:

1. **Plan item 2 outstanding:** convert the contention error from `anyhow::Context`
   to `RecoverableError` with structured `Guidance`, per
   `get_guide("error-handling")`. Low priority — the message is already actionable
   — but it is the difference between a generic MCP error and a guided one.
2. Archive this file only after the fix reaches `master`
   (`git branch --contains <sha>`), not on this status flip.
## References

- `src/retrieval/sync.rs:196-273` — `sync_project`, the unguarded path
- `src/agent/write_guard.rs:6-10,43-78` — existing flock idiom + ordering rule
- `src/lsp/mux/process.rs:75-79` — ownership-arbiter pattern to mirror
- `src/lsp/manager.rs:236` — `kotlin_index_lock_held`, per-language precedent
- `docs/issues/2026-06-19-mcp-server-oom-68gb.md` — prior resource-exhaustion
  bug on the same pipeline (cited at `src/retrieval/sync.rs:214`)
- `docs/trackers/dependency-review-session-log.md` — session context

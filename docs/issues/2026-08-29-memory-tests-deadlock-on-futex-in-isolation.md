---
status: open
opened: 2026-08-29
closed:
severity: high
owner: marius
related: []
tags: [cargo-test, memory, deadlock, flaky, embedder]
kind: bug
---

# `tools::memory::tests::*` deadlock on a futex, reproducibly, even in total isolation

## Summary

A fixed subset of ~15-16 tests under `src/tools/memory/tests.rs` (starting at
`delete_removes_entry` and continuing alphabetically: `list_after_writes`,
`memory_delete_removes_anchor_sidecar`, `memory_delete_still_works_without_embedder`,
`memory_delete_via_dispatch`, `memory_large_read_buffers_as_file_ref`,
`memory_list_via_dispatch`, `memory_read_accepts_name_alias_for_topic`,
`memory_read_missing_topic_embeds_available_and_suggestions`,
`memory_read_sections_filter_integration`, `memory_read_sections_string_coerced`,
`memory_write_accepts_project_alias_for_project_id`,
`memory_write_and_read_via_dispatch`, `memory_write_routes_to_project_dir`,
`memory_write_still_works_without_embedder`, `nested_topic_works`) hangs
indefinitely under `cargo test`. This is **not** contention with a concurrently
running sibling `cargo test` process, and **not** the previously-resolved
"wrong embedder port" misconfiguration (`docs/issues/archive/2026-08-27-cargo-test-fails-from-bash-passes-via-run-command.md`).
It reproduces in complete isolation: a single-threaded run of only this module
(`cargo test tools::memory::tests:: -- --test-threads=1`), with no other
`cargo test` process alive on the machine, still hangs on `delete_removes_entry`.

## Symptom

Running `cargo test` (via `mcp__codescout__run_command`, the tool this project's
own CLAUDE.md/memory `gotchas` documents as the one that reliably resolves
`CODESCOUT_EMBEDDER_URL` correctly) hangs indefinitely partway through the suite.
`cargo test`'s own "has been running for over 60 seconds" watchdog repeats the
same ~15-16 test names, unchanged, across multiple wait cycles (observed up to
13+ hours elapsed in one occurrence, per `ps -o etime`). The hang is silent —
no panic, no error, no output — the process simply never returns.

Observed 3 times this session, non-deterministically as to *when* it triggers
but deterministically as to *which tests*:

1. Full suite, first attempt: hung after ~2463 tests passed, embedder daemon
   (`codescout-dense-amd`, port 48081) confirmed unresponsive to `curl` at the
   time (`--max-time 5` timeout, exit 28). Killed after 13h18m elapsed
   (`ps -o etime`), embedder still unresponsive immediately after the kill.
2. Full suite, second attempt (after the embedder daemon recovered on its own —
   `curl http://127.0.0.1:48081/health` → `HTTP:200`): the exact same
   `tools::memory::tests::*` subset ran clean the *first* time through this
   region of the suite (all printed `... ok`), but then a **fresh** `cargo test`
   invocation immediately after hung on the identical subset again, this time
   with no sibling `cargo test` process running concurrently (verified via
   `ps -eo pid,etime,cmd`) and the embedder daemon still healthy per `curl`.
3. Isolated reproduction: `cargo test tools::memory::tests:: -- --test-threads=1`,
   nothing else running. Printed 7 tests `... ok`, then hung on
   `delete_removes_entry` with no further output for 150+ seconds.
4. **Decisive: full suite, fully serial** — `cargo test -- --test-threads=1`
   (every test in the binary, one at a time, zero concurrency possible). An
   intermediate attempt with a `--skip` list covering occurrence-1/2's 16 test
   names first hung on a *different* subset instead
   (`tools::memory::tests::refresh_anchors_clears_staleness`,
   `write_and_read_roundtrip`, `write_creates_anchor_sidecar`, and — notably —
   `tools::semantic::tests::concurrent_semantic_search_does_not_deadlock` and
   `semantic_search_emits_progress_text`), which had looked like evidence for a
   bounded shared resource (a semaphore/connection-pool permit exhausted by
   concurrent access). The fully-serial rerun refutes that as the *sole*
   mechanism: with `--test-threads=1` there is no concurrency at all, yet it
   reached and hung on `delete_removes_entry` again — the exact same test as
   occurrence 3 — after running only `content_within_budget_is_never_segmented`,
   `cross_embed_memory_stores_under_pinned_project_not_session_default`,
   `delete_private_does_not_affect_shared_store`, and
   `delete_private_removes_from_private_store` first, all `... ok`.

**Not confined to `src/tools/memory/tests.rs`.** With both `tools::memory::tests::`
and `tools::semantic::tests::` skipped, the unit-test binary (`codescout-...`) ran
clean: `4510 passed; 0 failed; 8 ignored; 115 filtered out`, as did every other
unit-test suite in the workspace (`feature_lanes`, `e2e::nav_eval`, etc.). But the
`tests/integration.rs` binary then hung on `workflow_project_memory_config` — a
different test, in a different binary, whose name again names "memory" — after a
concurrent sibling `cargo test` (the other checkout's own gate run) had started up
moments earlier. This is consistent with the resource being contended, not merely
leaked: a healthy run can still hang under load from a second simultaneous
`cargo test` process hitting the same local embedder daemon / shared runtime
state, on top of whatever single-process leak occurrences 3-4 demonstrate. Both
mechanisms likely coexist. Not chased further (see Resume) — timeboxed to protect
the fix-wave this bug was incidentally found during.
## Reproduction

```
cargo test tools::memory::tests:: -- --test-threads=1
```
Not yet reduced further (e.g. to a two-test minimal repro isolating which
earlier test poisons state for `delete_removes_entry`) — see Resume.

## Environment

- Branch: `sdd/operator-rules-phase-2`, worktree at
  `/home/marius/work/claude/codescout/.claude/worktrees/operator-rules-phase-2`.
- `cargo test` invoked via `mcp__codescout__run_command` (not native `Bash`).
- Local embedder daemon `codescout-dense-amd` on `127.0.0.1:48081`, confirmed
  listening and, at time of the 2nd and 3rd occurrences, responding `HTTP:200`
  to `/health` in under 1ms.
- A sibling checkout (`/home/marius/work/claude/codescout`, branch `experiments`)
  was running its own concurrent `cargo test` during occurrence 1 only; gone by
  occurrence 2 and not running at all during occurrence 3.

## Root cause

**Not fully diagnosed — process-level evidence only, no source-level fix
attempted (out of scope for the session that filed this).** What is confirmed:

- The hung process is genuinely blocked, not busy or slow. `cat
  /proc/<pid>/status` during occurrence 1 showed all 32 threads in state `S`
  (sleeping). During occurrence 3 (2-thread, `--test-threads=1` run),
  `/proc/<pid>/wchan` for the main thread read `futex_do_wait` — i.e. the
  process is parked on a `Mutex`/`Condvar`/channel primitive, not on socket
  I/O. This is evidence *against* "the embedder is just slow to answer an HTTP
  request" as the mechanism, and evidence *for* an in-process lock that is
  never released (a deadlock) or a channel whose other end never sends/closes.
- `delete_removes_entry` (`src/tools/memory/tests.rs:239-257`) itself is a
  plain three-call `Memory::call` sequence (write, delete, read-expect-err)
  against a `test_ctx_with_project()` context — nothing in the test body itself
  holds a lock across an await point.
- `test_ctx_with_project()` (`src/tools/memory/tests.rs:77-95`) pre-installs a
  `FixedEmbedder` and an `InMemorySemanticMemoryStore` via
  `Agent::set_memory_embedder_for_test` / `set_semantic_memory_store_for_test`
  specifically to avoid the live embedder/Qdrant path (per its own doc comment,
  citing `docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`).
  If that isolation seam is genuinely airtight, this hang should not be able to
  reach the real network embedder at all — which would point toward a
  same-process resource (e.g. a lock inside `Agent`, or in the anchor/sidecar
  write path) rather than the daemon. This is a hypothesis, not confirmed by
  reading `Agent::set_memory_embedder_for_test`'s implementation or the memory
  delete/anchor code path in this session.

## Evidence

- `ps -eo pid,etime,stat,cmd` and `/proc/<pid>/status` / `/proc/<pid>/wchan`
  output captured live during all three occurrences (not preserved to a file;
  see session transcript if needed).
- `delete_removes_entry` full body, read via `symbols(path="src/tools/memory/tests.rs", name="delete_removes_entry", include_body=true)`:
  ```rust
  #[tokio::test]
  async fn delete_removes_entry() {
      let (_dir, ctx) = test_ctx_with_project().await;
      Memory
          .call(json!({ "action": "write", "topic": "to-delete", "content": "bye" }), &ctx)
          .await
          .unwrap();
      Memory
          .call(json!({ "action": "delete", "topic": "to-delete" }), &ctx)
          .await
          .unwrap();

      let err = Memory
          .call(json!({ "action": "read", "topic": "to-delete" }), &ctx)
          .await;
      assert!(err.is_err());
  }
  ```

## Hypotheses tried

1. **"It's the embedder daemon being globally unresponsive."** Contradicted by
   occurrence 2 (daemon confirmed healthy via `curl` throughout) and occurrence
   3 (isolated run, still healthy, still hangs) — and by the `futex_do_wait`
   wchan, which is a lock/channel primitive, not a socket read.
2. **"It's contention with the sibling checkout's concurrent `cargo test`."**
   Contradicted by occurrence 3, where nothing else was running at all.
3. **"It's a bounded shared resource (semaphore/connection-pool) exhausted by
   *concurrent* test threads."** Suggested by the `--skip`-list run hanging on
   a *different* subset than occurrences 1-2 (including, ironically,
   `concurrent_semantic_search_does_not_deadlock`) — consistent with whichever
   tests land in the unlucky scheduler slots when a limited pool of permits
   runs out. **Refuted as the sole mechanism** by occurrence 4: a fully serial
   run (`--test-threads=1`, no concurrency possible) still hangs on
   `delete_removes_entry`, the same test occurrence 3 isolated. This narrows
   the shape of the bug: it looks like **state (a lock, a permit, or a
   channel sender) acquired by an earlier test and never released/dropped**,
   which occurrence 4 shows can starve a *specific* later test even with
   sequential, single-threaded execution — not solely a concurrency race.
   Prime suspect given the name and doc comment:
   `cross_embed_memory_stores_under_pinned_project_not_session_default`, the
   test immediately preceding the two `delete_private_*` tests and
   `delete_removes_entry` in occurrence 4's run, and one of only a few tests
   in this file whose subject (cross-project embedding) plausibly touches a
   shared/pooled resource rather than the fully-isolated per-test
   `InMemorySemanticMemoryStore` most tests here use. **Not confirmed** —
   next step for whoever resumes this is exactly the bisection in `## Resume`
   item 2, now with a concrete first suspect to test by running it immediately
   before `delete_removes_entry` in a 2-test filter.

No hypothesis is fully confirmed with a source-level fix. Occurrence 4 is the
strongest, most decisive evidence gathered so far and should be the starting
point for further work, not occurrences 1-3.

## Fix

Not implemented — out of scope for the session that found this (a documentation
+ small-fix wave on an unrelated branch, `sdd/operator-rules-phase-2`, that
happened to hit this while running the mandatory `cargo test` gate). No commit,
no SHA, no patch-id.

## Tests added

None. This bug *is* about existing tests; no new regression test was written
(there is nothing yet to assert against — the fix mechanism is unknown).

## Workarounds

- Retrying `cargo test` sometimes succeeds (occurrence 2's first pass got
  through the whole `tools::memory` region clean) — the hang is intermittent
  in *when* it triggers, not in *which tests* it would hang on if it triggers.
  This is not a reliable workaround, just an observed escape hatch: if a run
  hangs, killing it (`kill -9 <pid>`, itself gated behind an explicit
  dangerous-command acknowledgment under `run_command`) and retrying can
  produce a clean pass on the next attempt.
- Not evaluated: whether `cargo test -- --test-threads=N` for some `N>1` but
  `<`default avoids the hang, or whether a `--skip` list covering exactly this
  subset lets the rest of the suite gate reliably while this subset is run
  separately/serially/retried.

## Resume

Next concrete steps for whoever picks this up:
1. Reduce to a minimal repro: run `cargo test
   tools::memory::tests::delete_removes_entry -- --test-threads=1` **alone**
   (no other test name matching) to check whether it hangs standing entirely by
   itself, or only after other tests in the same binary have run first (the
   three prior occurrences never isolated this — the isolated run in this
   session still executed 7 other tests first).
2. If it only hangs after prior tests, bisect which earlier test(s) leave
   state behind — the `futex_do_wait` evidence suggests a lock/semaphore/
   channel that some earlier test acquires or holds a sender for and never
   releases/drops/closes.
3. Read `Agent::set_memory_embedder_for_test` and
   `Agent::set_semantic_memory_store_for_test` (not read this session) to
   check whether they involve a `OnceCell`/`OnceLock`/global registry that
   could be process-wide rather than per-`Agent`-instance despite each test
   constructing a fresh `Agent`.
4. Check the memory delete/anchor-sidecar write path
   (`src/memory/anchors.rs`, `src/memory/mod.rs`) for any lock held across an
   `.await` — a classic async-deadlock shape, and consistent with `delete`
   being the first-alphabetically test in the hanging subset.

## References

- `src/tools/memory/tests.rs:239-257` — `delete_removes_entry`
- `src/tools/memory/tests.rs:47-67` — `test_ctx_with_project_raw`
- `src/tools/memory/tests.rs:77-95` — `test_ctx_with_project`
- `docs/issues/archive/2026-08-27-cargo-test-fails-from-bash-passes-via-run-command.md` — the prior, unrelated, resolved embedder-port issue this bug is distinct from
- `docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md` — the isolation seam `test_ctx_with_project()` exists to provide
- codescout memory `gotchas`, section "`cargo test` Fails From Native `Bash` But Passes Via `run_command` — Export `CODESCOUT_EMBEDDER_URL`" — ruled out as the cause (see Hypotheses tried)

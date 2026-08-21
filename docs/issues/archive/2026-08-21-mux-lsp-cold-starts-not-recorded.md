---
status: fixed
opened: 2026-08-21
closed: 2026-08-21
severity: medium
owner: marius
related: [2026-06-11-mux-failure-masks-rocksdb-lock-collision]
tags: [lsp, mux, telemetry, usage-db, observability]
kind: bug
unverified: the three get_or_start Err-arm branches (index contention, kotlin lock held, refuse-fallback) have no automated test driving a genuine failure through the full mux dispatch -- see Tests added
---

# BUG: mux-managed LSP cold starts (kotlin, rust) are never recorded to `lsp_events`

## Summary

`get_or_start_via_mux` — the sole startup path for "mux languages" (kotlin, rust) since
the 2026-06-11 mux-single-owner-invariant refactor — never calls `write_lsp_event` /
`write_lsp_failure`. Every kotlin/rust LSP cold start since ~2026-06-11 is invisible to
`usage.db`'s `lsp_events` table, even though the mux processes themselves are running
fine right now. This was discovered while producing a cross-project latency report: the
report silently showed zero recent rust/kotlin rows, which read as "these languages
aren't used" when actually they're used constantly — just unmeasured.

## Symptom (Effect)

Across 4 independently-checked `.codescout/usage.db` files (codescout, backend-kotlin,
MRV-poc, eduplanner-ui), every `rust`/`kotlin` row in `lsp_events` has a `started_at`
no later than 2026-06-11, while every other language (java, python, javascript,
typescript, tsx, html, css, bash) keeps generating fresh rows through 2026-08-21 in the
same databases. Three of the four projects' *last-ever* rust/kotlin event lands within
an 11-hour window on the same calendar day:

```
codescout        rust    last seen 2026-06-11 08:09:22
codescout        kotlin  last seen 2026-06-11 12:01:34
backend-kotlin   rust    last seen 2026-06-11 19:03:01
backend-kotlin   kotlin  last seen 2026-06-11 15:39:57
eduplanner-ui    rust    last seen 2026-06-11 10:54:34
eduplanner-ui    kotlin  last seen 2026-06-11 10:54:19
```

Meanwhile `symbols`/`edit_code` calls against rust-path inputs in codescout's own
`tool_calls` table ran 1,178 / 1,090 times respectively in the 7 days before this bug
was filed — i.e. rust navigation is working and heavily used; only its cold-start
telemetry is silent.

## Reproduction

```
git rev-parse HEAD   # 9afb51f6308513a3e75571f69e8d91df83a72af9, branch experiments

sqlite3 /home/marius/work/claude/codescout/.codescout/usage.db \
  "SELECT language, COUNT(*), MIN(started_at), MAX(started_at) FROM lsp_events GROUP BY language;"
# rust/kotlin/go all stop cold, everything else continues to today

pgrep -af 'kotlin-lsp|rust-analyzer'
# live mux-owned kotlin-lsp / rust-analyzer processes ARE running right now for this
# project, with short elapsed times — proving cold starts still happen, unrecorded
```

## Environment

codescout MCP server, Rust, `src/lsp/manager.rs`. Reproduced on this machine across 4
separate project checkouts (codescout, backend-kotlin, MRV-poc, eduplanner-ui), so it's
not project-specific — it's a property of the shared codescout binary.

## Root cause

`write_lsp_event` and `write_lsp_failure` — the only two call sites that insert into
`lsp_events` anywhere in the codebase — live exclusively inside `do_start`
(`src/lsp/manager.rs:1081-1225`). `do_start` is called from exactly two places: the
non-mux branch of `get_or_start` (`:812`, `:816`) and the test-only
`get_or_start_for_test` (`:1422`, `:1425`).

`get_or_start` dispatches mux languages away from that path entirely:

```rust
// src/lsp/manager.rs:696-700
if config.mux {
    match self.get_or_start_via_mux(language, workspace_root, config.clone()).await
    ...
```

`get_or_start_via_mux` (`:881-1074`) contains zero calls to `do_start`, `write_lsp_event`,
or `write_lsp_failure` (confirmed by grep across the whole file — both symbols appear
only inside `do_start`'s body and its unit tests). Since kotlin and rust are mux
languages, **100% of their cold starts bypass the only code that records `lsp_events`.**

*Measured 2026-08-21:* `grep -n 'do_start\|write_lsp_event\|write_lsp_failure' src/lsp/manager.rs`
— all non-test hits fall inside `do_start`'s own line range or its two call sites; none
fall inside `get_or_start_via_mux`'s `881-1074` range.

**When this became total, and why:** kotlin was already a mux language before
2026-06-11 (see the March 2026 circuit-breaker doc), but a same-day series of
`fix(lsp)` commits on 2026-06-11 (`c5fb3979`, `5ea61fa8`, `96e25325`, `a538ff95`,
`df4a1737`, `249a5c56`, `4e579fc1` — implementing
`docs/adrs/2026-06-11-mux-single-owner-invariant.md`) made **rust** a strict mux
language too, and removed the direct-LSP fallback that used to run when the mux was
unavailable:

> "Mux languages (kotlin, rust) must never spawn a competing direct LSP on the shared
> index. The fallback is retained only for the test-runner exe case."
> — `96e25325`, `fix(lsp): refuse silent direct-LSP fallback for mux languages (S3)`

Before that fallback was removed, a failing/absent mux for kotlin or rust would silently
retry through the **direct** LSP path — which does call `do_start` and did get recorded.
That's incidental telemetry coverage, not intentional instrumentation of the mux path
itself; removing the fallback (correctly — it caused a documented RocksDB index-lock
deadlock, see `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`)
made the pre-existing telemetry gap in `get_or_start_via_mux` total instead of partial.

Neither the ADR nor either commit message mentions observability — this reads as an
unintentional side effect of the mux refactor (the new ownership path was never wired
to the usage-tracking calls the old path had), not a deliberate decision.

*inferred from `src/lsp/manager.rs:696-1225` + `git show c5fb3979 96e25325` — mechanism
confirmed by reading the code and the commit series; the "still happening today, just
silent" half is measured (see Evidence), the exact minute mux-only became total for
every historical build in between is not reconstructed and isn't needed to fix this.*

## Evidence

### `usage.db` timeline, all four checked projects

```
=== codescout ===
kotlin|11|2026-06-01 04:38:17|2026-06-11 12:01:34
rust|18|2026-06-01 04:42:56|2026-06-11 08:09:22

=== backend-kotlin ===
kotlin|52|2026-05-02 06:12:11|2026-06-11 15:39:57
rust|29|2026-05-02 06:12:08|2026-06-11 19:03:01

=== MRV-poc ===
rust|1|2026-05-16 10:27:14|2026-05-16 10:27:14
(kotlin: 0 rows, ever)

=== eduplanner-ui ===
kotlin|6|2026-06-03 13:00:15|2026-06-11 10:54:19
rust|5|2026-06-03 13:04:17|2026-06-11 10:54:34
```

### codescout's own last-6 kotlin events before going silent

```
rust|2026-06-10 06:46:05|new_session|success|
kotlin|2026-06-11 06:01:22|new_session|failed|LSP server disconnected
kotlin|2026-06-11 06:01:39|new_session|failed|LSP server disconnected
kotlin|2026-06-11 06:01:46|new_session|failed|LSP server disconnected
kotlin|2026-06-11 06:01:53|new_session|failed|LSP server disconnected
kotlin|2026-06-11 06:02:00|new_session|failed|LSP server disconnected
kotlin|2026-06-11 06:02:36|new_session|failed|LSP server disconnected
kotlin|2026-06-11 07:48:40|new_session|success|
rust|2026-06-11 08:09:22|lru_evicted|success|
kotlin|2026-06-11 09:49:53|new_session|failed|LSP server disconnected
kotlin|2026-06-11 12:01:34|new_session|failed|LSP server disconnected
(nothing after, ever)
```

### Live processes right now (2026-08-21), proving cold starts still occur

```
$ pgrep -af 'kotlin-lsp|rust-analyzer'
... codescout mux --socket .../codescout-kotlin-mux-7e868829c00fa9b2.sock ... -- kotlin-lsp --stdio ...
... kotlin-lsp --stdio ...
... codescout mux --socket .../codescout-rust-mux-7e868829c00fa9b2.sock ... -- rust-analyzer
... rust-analyzer
... rust-analyzer-proc-macro-srv
(elapsed 42:09 / 41:44 / 41:43 — started recently, well inside a single session)
```

### Call-site audit

```
$ grep -n 'do_start\|write_lsp_event\|write_lsp_failure' src/lsp/manager.rs
```
returns hits only inside `do_start` (`:1081-1225`, both DB writes at `:1152`/`:1208`),
its two production callers (`:812`, `:816`, both inside the non-mux branch of
`get_or_start`), its test-only twin `get_or_start_for_test` (`:1422`, `:1425`), and its
unit tests (`do_start_records_lsp_event_to_db`, `do_start_records_failure_event_when_start_fails`).
Zero hits inside `get_or_start_via_mux` (`:881-1074`).

## Hypotheses tried

1. **Hypothesis:** rust/kotlin LSP sessions have simply stayed alive, unevicted, since
   June — no new cold start ever needed.
   **Test:** checked live process elapsed time (`ps -o etime`) for the mux + LSP
   processes actually running against this project right now.
   **Verdict:** rejected. Elapsed times are 42 minutes and 2.5 hours — these are recent
   (re)starts, not two-month-old processes. A cold start clearly happened recently;
   nothing recorded it.
   **Evidence link:** "Live processes right now" above.

2. **Hypothesis:** the Kotlin LSP circuit-breaker (`gotchas` memory, "trips when two
   codescout instances target the same Kotlin project concurrently") permanently
   disables kotlin/rust starts after tripping.
   **Test:** read the circuit-breaker doc (`docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`
   reference) and the `startup_failures`/circuit-breaker fields in `LspManager`.
   **Verdict:** rejected as the mechanism here — the circuit-breaker gates whether a
   *call* is allowed to proceed (`symbols(include_body=true)` fails loudly with
   "circuit-breaker open"), it doesn't explain a silent, permanent absence of both
   success *and* failure telemetry rows across three independent projects with no
   corresponding `outcome='error'` spike in `tool_calls`.
   **Evidence link:** n/a (ruled out by code inspection, not by a data query).

3. **Hypothesis:** `get_or_start_via_mux` never calls the telemetry-recording function
   (`do_start`), so mux-language cold starts are architecturally invisible to
   `lsp_events` regardless of whether they succeed or fail.
   **Test:** `grep -n 'do_start\|write_lsp_event\|write_lsp_failure' src/lsp/manager.rs`
   and confirmed with `symbols(name=...)` line ranges that both DB-write call sites and
   both `do_start` callers fall outside `get_or_start_via_mux`'s `881-1074` span.
   **Verdict:** confirmed. This is the root cause.
   **Evidence link:** "Call-site audit" above.

## Fix

Extracted the recording logic `do_start` already had (`src/lsp/manager.rs`) into two shared
private methods on `LspManager`:

- `record_lsp_success(&self, key: &LspKey, handshake_ms: i64, reason: &str)` — writes the
  `lsp_events` success row + `pending_first_response` bookkeeping, unchanged from what
  `do_start`'s success arm did inline.
- `record_lsp_failure(&self, key: &LspKey, handshake_ms: i64, reason: &str, error: &str)` —
  writes the `outcome='failed'` row, unchanged from what `do_start`'s error arm did inline.

`do_start` now calls these instead of inlining the writes (behavior-preserving refactor —
covered by the pre-existing `do_start_records_lsp_event_to_db` /
`do_start_records_failure_event_when_start_fails` tests, both still green).

`get_or_start`'s mux dispatch (`src/lsp/manager.rs`, the `if config.mux { ... }` block) now
calls the same two helpers at every exit point:

- The single `Ok(client) =>` arm calls `record_lsp_success(&key, handshake_ms, "new_session")`
  before returning. Mux connects don't go through the direct-pool's eviction-reason
  bookkeeping (`pending_reason`), so the reason is always `"new_session"` — including a
  connect to an already-running mux, which just reports a much lower `handshake_ms` than a
  fresh spawn.
- Each of the three production `return Err(...)` branches (index contention, kotlin index
  lock held, and the "refuse silent fallback" branch) calls `record_lsp_failure` first, using
  the *original* `get_or_start_via_mux` error text (not the wrapped/enhanced message returned
  to the caller), matching what `do_start` records for a direct-path failure.
- The fourth branch (test-runner exe, falls through to `config.mux = false` and continues to
  `do_start`) is intentionally left alone — that path already gets recorded via `do_start`,
  unchanged.

- **SHA:** `0994309af8ca4be1e7751748e57b3f90125ee8eb` (`experiments`)
- **patch-id:** `642d74a28b4bd64d640cf8023b4368ff825fad0a`
## Tests added

`get_or_start_mux_success_records_lsp_event` (`src/lsp/manager.rs`, `#[ignore]`-gated —
spawns a real `kotlin-lsp` process, requires it installed). Verified sensitive to the fix by
temporarily removing the new `record_lsp_success` call: the test then failed with
`QueryReturnedNoRows` (no `lsp_events` row at all) — exactly the reported symptom. Restored
and re-confirmed green.

Getting this test to actually exercise `get_or_start`'s mux `Ok` arm (rather than the
test-runner direct-LSP fallback, which was already covered) took a wrong turn worth
recording: `get_or_start_via_mux` resolves the mux binary via `current_exe()`, which under
`cargo test` is the test binary itself — it can't speak the mux CLI protocol, so a naive
`get_or_start(..., Some(true))` call silently takes the `is_test_runner_exe` fallback to
`do_start` instead, which was *already* recording correctly. A first version of this test
asserted success without checking which code path produced it — passed for the wrong
reason, and would have kept passing after reverting the fix, since do_start's own test
already covers that path. Fixed by pre-starting a REAL mux with the actual compiled
`target/debug/codescout` binary (not `current_exe()`) and waiting for its "ready" line, so
`get_or_start` finds the mux ownership lock already held, skips spawning, and connects
straight through — the real `Ok` arm.

**Gap, stated plainly:** the three failure branches (index contention, kotlin lock held,
refuse-fallback) are NOT covered by an automated test. `kotlin_index_lock_held`'s own
detector is unit-tested directly (`kotlin_index_lock_held_false_for_non_kotlin_and_missing_home`),
but driving a *genuine* failure through the full mux dispatch turned out to be
architecturally hard to fake: an attempt to plant a held lock file to force the
index-contention path was defeated by `reap_orphan_index_holder`, which correctly identifies
and kills exactly that kind of orphaned lock-holder before spawning — legitimate self-healing
behavior, not a test bug, but it means a hermetic simulation doesn't survive contact with the
real code. Reproducing genuine RocksDB contention would require two real kotlin-lsp
processes racing the same index, which the mux exists specifically to prevent. The shared
`record_lsp_failure` helper itself IS covered (via `do_start_records_failure_event_when_start_fails`);
only the three new call sites in `get_or_start`'s Err arm lack a direct test.
## Workarounds

None needed for correctness — LSP itself works fine; this is an observability gap only.
Anyone measuring kotlin/rust cold-start latency from `lsp_events` should know the data
stops in June 2026 and query live process state (`pgrep -af 'kotlin-lsp|rust-analyzer'`,
`ps -o etime`) instead, or read mux logs directly, until this is fixed.

## Resume

N/A — fixed. If the `unverified:` gap ever matters (someone wants real coverage of the
three failure branches), the honest path is a real second-mux-instance integration test
outside `cargo test`, not another hermetic simulation — see "Tests added" for why the
in-process attempts don't survive contact with `reap_orphan_index_holder`.
## References

- `docs/adrs/2026-06-11-mux-single-owner-invariant.md`
- `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`
- codescout memory `gotchas` § Kotlin LSP Circuit-Breaker
- Latency Census artifact (this session) — the report that surfaced the gap

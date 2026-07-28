---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- lsp
- mux
- kotlin
- rocksdb
- observability
- multi-instance
- circuit-breaker
topic: null
time_scope: null
---

# BUG: Kotlin mux startup failure is unobservable, and the direct-LSP fallback collides on a held RocksDB lock that masquerades as "LSP server disconnected"

## Summary
When a stale/orphaned `intellij-server` holds the shared kotlin-lsp RocksDB index lock,
every fresh Kotlin LSP start dies with a generic `LSP server disconnected`. The LSP mux —
whose whole job is to be the *single* shared owner so this can't happen — fails to start
**silently** (its child is spawned with `stderr → /dev/null`, so the real cause is
discarded and the user sees `mux process failed to start:` with a blank reason). codescout
then falls back to a per-process **direct** LSP that hits the *same* held RocksDB lock and
disconnects, tripping the circuit breaker after 5 failures. The true root cause
(`org.rocksdb.RocksDBException: … LOCK: Resource temporarily unavailable`) is visible only
by digging the debug log. Affects any multi-instance Kotlin workspace; observed live in
`~/work/mirela/backend-kotlin` on 2026-06-11.

## Symptom (Effect)
Every Kotlin-LSP-backed tool call (`edit_code`, name-based `symbols`, `references`) returns:

```
LSP server disconnected
```

The debug log shows the mux failing with an **empty** reason, then a direct-LSP fallback that
also disconnects:

```
WARN codescout::lsp::manager: Mux startup failed for kotlin, falling back to direct LSP:
     mux process failed to start:  — hint: Check that another codescout mux isn't already
     running for this workspace, and that the lock file directory is writable.
WARN lsp_stderr: org.rocksdb.RocksDBException: While lock file:
     …/kotlin-lsp-home/26a9e85d58931839/…/analyzer/workspaces/…/rocks/v492/LOCK:
     Resource temporarily unavailable
     at org.rocksdb.RocksDB.open(Native Method)
     at com.jetbrains.ls.snapshot.api.impl.core.rocks.RocksDbStorageBackend$Companion.open(RocksDbStorageBackend.kt:86)
WARN codescout::lsp::manager: LSP circuit-breaker tripped for kotlin@… (5 failures in 56s)
```

`path`-based `symbols` still "succeeds" (tree-sitter AST fallback, no LSP needed), which makes
the failure look write-path-specific when it is not. `edit_code` requires the LSP's
`document_symbols`, so it can never fall back — it always fails. This asymmetry was previously
**misdiagnosed** as "`edit_code` reliably crashes the Kotlin LSP" (it does not — see Evidence).

## Reproduction
codescout `4ec75485` on `experiments`, Linux, MCP stdio transport, kotlin-lsp `262.4739.0`
(under `faketime` for the build-expiry workaround).

1. Have two codescout server processes (e.g. two Claude Code sessions, or worktrees) that both
   resolve the **same** Kotlin workspace hash, so they share one kotlin-lsp home
   (`~/.cache/codescout/kotlin-lsp-home/<hash>`).
2. Let one of them spawn a **direct-fallback** kotlin-lsp (i.e. when the mux path fails). It
   acquires the RocksDB index `LOCK` and keeps holding it (it can outlive its owning session as
   an orphan).
3. From the other session, trigger any LSP-backed Kotlin tool (`edit_code` / `references`).
   → `LSP server disconnected`; debug log shows `mux process failed to start:` (blank) +
   `RocksDBException … LOCK: Resource temporarily unavailable`; circuit breaker trips.

Confirm the squatter: `fuser <kotlin-lsp-home>/<hash>/…/rocks/v492/LOCK` names the holding PID.

## Environment
codescout `4ec75485` (`experiments`); kotlin-lsp `262.4739.0`; Arch Linux; MCP stdio.
Project: `/home/marius/work/mirela/backend-kotlin` (workspace hash `26a9e85d58931839`).

## Root cause
Three independent codescout-side weaknesses turn a recoverable lock contention into an opaque,
self-perpetuating failure. All in `LspManager::get_or_start_via_mux`
(`src/lsp/manager.rs:432-539`) and the fallback at `src/lsp/manager.rs:318`:

1. **Mux liveness is inferred from the advisory `flock` alone, never from socket
   connectability** — `src/lsp/manager.rs:456`:
   ```rust
   let need_spawn = match lock_file.try_lock_exclusive() {
       Ok(())  => { drop(lock_file); true }          // got lock → spawn
       Err(_)  => { /* "mux already running" */ false } // lock held → assume alive → connect
   };
   ```
   A stale `.lock` whose flock is still held (by a wedged/zombie process) but with **no live
   `.sock`** routes to the connect path, which then fails all 5 retries. Observed live: the
   backend-kotlin kotlin mux had `codescout-kotlin-mux-26a9e85d58931839.lock` present but
   **no** `.sock` (`ss -xlp` showed no listener for that hash).

2. **The mux child's stderr is discarded** — `src/lsp/manager.rs:485`
   (`.stderr(std::process::Stdio::null())`). When the child mux dies during its own LSP
   startup (because *its* kotlin-lsp also hits the held RocksDB lock), codescout reads only an
   empty stdout line and reports `mux process failed to start: {line.trim()}` with `line`
   empty (`src/lsp/manager.rs:502-508`). The real cause is thrown away.

3. **The direct-LSP fallback opens the shared RocksDB home unconditionally** — on mux failure,
   `src/lsp/manager.rs:318` falls back to a direct kotlin-lsp pointed at the *same* shared home,
   so it collides on the same held lock and the user sees a generic `LSP server disconnected`
   with no mention of lock contention or which PID holds it.

Underlying trigger (not a codescout code bug, but what the above fail to surface gracefully):
RocksDB allows exactly one process to hold the index lock; multiple codescout servers each
spawning a *direct* kotlin-lsp against one shared home contend on it. This is the
multi-instance hazard family (`docs/issues/archive/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`).

## Evidence

### lsp_events — every Kotlin start failed; other languages fine
`/home/marius/work/mirela/backend-kotlin/.codescout/usage.db`:
```
353 | 2026-06-11 07:35:27 | kotlin | new_session | failed | 4572 | LSP server disconnected
352 | 2026-06-11 07:35:11 | kotlin | new_session | failed | 4972 | LSP server disconnected
348 | 2026-06-11 07:34:58 | kotlin | new_session | failed | 4905 | LSP server disconnected
…  (bash/python/javascript/rust new_session → success in 90–272 ms)
```

### tool_calls — disconnect hits READS too, not just edit_code (falsifies the misdiagnosis)
```
16934 | 07:30:36 | symbols   | error | 4983 | LSP server disconnected
16935 | 07:30:47 | symbols   | error | 4926 | LSP server disconnected
16944 | 07:34:58 | symbols   | error | 6495 | LSP server disconnected
16941 | 07:33:25 | edit_code | error | 4971 | LSP server disconnected
16940 | 07:33:06 | edit_file | error |    0 | edit contains a symbol definition ("fun ") …  ← by-design gate, not a bug
16946 | 07:35:13 | symbols   | success | 9301 |                                              ← AST fallback, no LSP
```

### The squatter held the RocksDB lock
```
$ fuser …/kotlin-lsp-home/26a9e85d58931839/…/rocks/v492/LOCK
…/LOCK: 2699281
$ ps -o pid,ppid,lstart,cmd -p 2699281 2699279 1737255
2699281  2699279  …  intellij-server --stdio --system-path=/tmp/codescout-mux-kotlin-lsp-26a9e85d58931839
2699279  1737255  …  faketime … kotlin-lsp.sh --stdio --system-path=…26a9e85d…   (direct-fallback wrapper)
1737255  1737206  Wed Jun 10 14:07  /home/marius/.cargo/bin/codescout start --debug   (20h-old session's server)
```
No live `codescout mux` existed for hash `26a9e85d…` (kotlin); only a *rust*-analyzer mux did.

### Recovery confirmed the diagnosis
`kill 2699281 2699279` → `fuser LOCK` empty → a `references` call spawned a fresh
`intellij-server` (PID 3029079) that **survived** (prior attempts died at ~5s) and `fuser`
confirmed it now **holds** the lock. (Note: the clean recovery start still wrote **no**
`lsp_events` success row — a recurrence of `2026-06-05-lsp-failed-starts-not-recorded`'s
under-recording theme, this time on the success path.)

### Captured mux failure cause (the discarded stderr), 2026-06-11
Ran codescout's exact mux invocation manually (`build_mux_args` shape) for the backend-kotlin
hash with **stderr captured** — surfacing the line `src/lsp/manager.rs:485` throws away:
```
Error: another mux instance holds the lock

Caused by:
    Resource temporarily unavailable (os error 11)
```
`os error 11` = `EAGAIN` on the mux's *own* `.lock` flock — i.e. defect #1's symptom, named.
codescout never shows this; it only ever reports the blank `mux process failed to start:`.

### Deadlock ordering (why no mux could ever start)
The failures were self-perpetuating because of **acquisition order**, not just contention:
a *direct-fallback* kotlin-lsp grabbed the **RocksDB index lock** before any mux established.
Once a direct LSP holds that lock, a freshly-spawned mux's *own* kotlin-lsp can never open
RocksDB → the mux child dies before binding its socket → blank error → caller falls back to
direct → which also collides. Only the original direct lock-holder works; every other session
(and every mux attempt) is starved. The lock-holder rotates (orphan → my verification squatter
→ …) but the deadlock shape is invariant.

### Resolution confirmed, 2026-06-11
Clearing the full slate broke the deadlock: `kill` all kotlin-lsp for the hash + `rm` the stale
mux `.lock`/`.sock`, then a **fresh `codescout start`** on backend-kotlin spawned a *healthy*
mux (PID 3071881 ← `codescout start --debug` 3071812 ← `claude --resume` 2699173) that bound
its socket (`ss -xlp` LISTEN, `pid=3071881,fd=13`) and owns one LSP (3071949) holding the
RocksDB lock cleanly. Ground truth: `tool_calls` 16949 + 16950 (08:09 UTC) → `edit_code` →
**success** (vs 16948 `LSP server disconnected` pre-cleanup). NOTE: the healthy mux start wrote
**no** `lsp_events` row (latest remains 355, a failure) — the mux start path under-records on
success, a recurrence of `2026-06-05-lsp-failed-starts-not-recorded`'s theme.

### Self-inflicted recurrence (verification anti-pattern)
The first recovery attempt killed the orphan, then "verified" by issuing a `references` call
from a *different* codescout server (the debugging session, workspace-pinned). That call spawned
a direct-fallback LSP **owned by the debugging session** which grabbed the freed RocksDB lock —
making the debugging session the new squatter and breaking the user's *first* MCP restart
(`tool_calls` 16948, `lsp_events` 354/355 at 07:55). Lesson: a shared-resource recovery cannot
be verified by issuing a call from a second client — the call re-creates the contention.

### Failure-path repro run live — Fix 4 detection gap (2026-06-11)

Ran the pre-master gate (Resume step 1) on an isolated throwaway worktree (cs-fix4,
hash `35337242d11ee704`): held its RocksDB `LOCK` with an external `fcntl.lockf` holder
(POSIX `F_SETLK`, the same lock RocksDB uses), then fired `references` pinned to cs-fix4.
**Fix 4's contention guard did NOT engage.**

Sequence from `.codescout/debug.log` (12:01:28–34):
1. cs-fix4 mux spawned; its kotlin-lsp could not open RocksDB (lock held) and the mux failed
   with `Error: initialize response missing 'result'` — a *generic* handshake error. The
   drained mux stderr held the IntelliJ startup banner only; the `RocksDBException` went to the
   LSP's own `intellij-server.log`, **not** the drained stderr.
2. `mux_failure_is_index_contention("…initialize response missing 'result'…")` → **false** (no
   `Resource temporarily unavailable` / `RocksDBException` in the error string) → guard bypassed;
   `get_or_start` logged `Mux startup failed for kotlin, falling back to direct LSP`.
3. The direct fallback LSP then hit the lock: `org.rocksdb.RocksDBException: While lock file:
   …/rocks/v492/LOCK: Resource temporarily unavailable` (`lsp_stderr`) → `LSP server
   disconnected` returned to the tool.

**Conclusion:** Fix 4's stderr-signature detection is timing/log-routing fragile. When the
kotlin-lsp fails RocksDB *during initialize*, the signature is absent from the drained stderr,
so contention is misclassified as a generic failure, the no-fallback guard is bypassed, and the
direct fallback re-hits the lock. No squatter resulted here only because the external holder
kept the lock held throughout; had the lock been transiently free, the direct fallback would
have grabbed it and re-created the original deadlock. **Fix 4 does not pass its own pre-master
gate — do NOT ship as-is.**

**Proposed robust fix:** detect contention by *probing lock state*, not by parsing stderr.
Before the direct fallback, attempt a non-blocking `flock`/`lockf` (or scan `/proc` holders) on
the workspace's RocksDB `LOCK`; if held, return the actionable index-lock error and do not fall
back. Detection-by-state is immune to the stderr timing/log-routing race. The unit tests pass
because they feed `mux_failure_report` a stderr tail that already contains the signature — they
cannot catch this gap; only the live repro did.
## Hypotheses tried
1. **Hypothesis:** `edit_code`'s write path crashes the Kotlin LSP (prior session's claim).
   **Test:** read `tool_calls` for the failing window. **Verdict:** REJECTED — `symbols`
   (read path) disconnects identically (rows 16934/16935/16944); failures correlate with LSP
   *start latency*, not tool. Evidence: tool_calls subsection.
2. **Hypothesis:** kotlin-lsp build expired (the `2026-06-05` cause). **Test:** process list
   shows `faketime 2026-06-04` wrapper in place; stderr shows RocksDB, not "build expired".
   **Verdict:** REJECTED — different root cause; the faketime workaround is present.
3. **Hypothesis:** stale mux `.lock` without a live `.sock` wedges startup. **Test:** listed
   `/run/user/1000` — `.lock` present, `.sock` absent, `ss -xlp` no listener for the hash;
   read `get_or_start_via_mux`. **Verdict:** CONFIRMED as a contributing path (defect #1).
4. **Hypothesis:** a process holds the RocksDB index lock and starves all other starts.
   **Test:** `fuser LOCK` → PID 2699281; killing it freed the lock and a fresh LSP started and
   held it. **Verdict:** CONFIRMED — primary trigger.

## Fix
*Plan (not yet implemented).* Three changes, in `src/lsp/manager.rs`:

1. **Capture the mux child's failure cause.** Replace `.stderr(Stdio::null())` (`:485`) with a
   piped/captured stderr (or have the mux print its failure reason to stdout before exiting),
   so `mux process failed to start: {reason}` (`:502-508`) carries the real cause instead of a
   blank string. Lowest-effort, highest-diagnostic-value change.
2. **Verify socket connectability, not just the flock**, when deciding `need_spawn` (`:456`).
   If the flock is held but `LspClient::connect(&socket_path)` fails (stale lock, no live
   socket), treat the mux as dead: reclaim/respawn rather than retry-connect-then-fall-back.
   Pairs with cleaning the orphaned `.lock`/`.sock` artifacts.
3. **Surface RocksDB lock contention explicitly.** When the LSP stderr contains
   `RocksDBException … LOCK: Resource temporarily unavailable`, return an actionable error
   ("another kotlin-lsp holds this workspace's index; PID <holder> via `fuser <LOCK>`") instead
   of generic `LSP server disconnected`. Consider detecting/cleaning orphaned direct-fallback
   LSPs whose owning server has exited.

Also fold in: record an `lsp_events` row on the **successful** mux/direct recovery start (the
clean start above wrote none) — extends `2026-06-05-lsp-failed-starts-not-recorded`.

4. **Break the deadlock ordering.** The root failure is that a *direct-fallback* LSP can grab
   the shared RocksDB lock before any mux establishes, after which no mux can ever start (its own
   LSP can't open RocksDB). Options: prefer a (bounded) wait-and-retry for the mux over an
   immediate direct fallback that squats the shared lock; or make the direct fallback use an
   isolated, non-shared LSP home so it can't poison the mux's index lock; or have a failed/transient
   direct LSP release the RocksDB lock promptly instead of lingering as a squatter. Confirmed live:
   clearing all kotlin-lsp + the stale mux lock let a fresh `codescout start` spawn a healthy mux
   and `edit_code` succeeded immediately (Evidence § Resolution).

**Implemented 2026-06-11 on `experiments`** (unit-tested, clippy clean, full suite green;
pending live `/mcp` verify + master ship — see Resume). Steps **1, 3, 4** below are done in
`src/lsp/manager.rs`:
- **1 (capture stderr):** `get_or_start_via_mux` now spawns the mux with `stderr` **piped** and
  drains it into a bounded ring buffer (`MUX_STDERR_TAIL_LINES`), so the failure error carries
  the mux child's real cause instead of a blank string.
- **3 (actionable hint):** new pure `mux_failure_report()` + `mux_failure_is_index_contention()`
  build an index-lock-aware `(message, hint)` (detects RocksDB `LOCK`/`EAGAIN` + the mux flock).
- **4 (no poisoning fallback):** `get_or_start`'s mux-fallback arm now returns the error instead
  of falling back to a direct LSP when the failure is index contention — so no squatter is
  created and the caller sees the actionable error rather than `LSP server disconnected`.
- **2 (flock/socket liveness reclaim): dropped.** Reading the code showed the mux `flock` is
  released on process death (a dead mux never blocks the next), so the stale-lock-without-socket
  case self-heals; it was not the recurring root cause. The real cycle is the RocksDB-index
  contention addressed by 1/3/4.

The original plan (now historical) follows.
### Robust contention detection (lock-probe) — implemented 2026-06-11

The live repro (Evidence § "Failure-path repro run live") proved the stderr-signature detection
misses the `initialize response missing 'result'` failure mode. Fixed by adding
**detection-by-state**: `kotlin_index_lock_held(language, workspace_root)` +
`posix_write_lock_is_held(path)` in `src/lsp/manager.rs` probe the workspace's RocksDB `LOCK`
with a non-blocking `fcntl(F_SETLK, F_WRLCK)` — the SAME lock family RocksDB uses (`fs4`'s
`flock` API is blind to it). `get_or_start`'s fallback arm now returns the actionable index-lock
error when EITHER the stderr signature OR the live lock-probe detects contention, so the
direct-fallback collision can no longer occur regardless of where the failing LSP logged its
exception. Tests: `posix_write_lock_is_held_false_on_unlocked_file`,
`kotlin_index_lock_held_false_for_non_kotlin_and_missing_home`, and `#[ignore]`
`posix_write_lock_is_held_true_when_another_process_holds_it` (spawns a python3 fcntl holder;
passes under `--ignored`). `clippy --all-targets -D warnings` clean. Pending: live re-run of the
repro to confirm the guard now engages end-to-end.
### Update 2026-06-14 — shared spawn-failure root cause fixed (Kotlin core still open)

The `Failed to spawn mux process` symptom this bug shares with the rust mux had a **confirmed cross-cutting cause**: `std::env::current_exe()` resolves to a *deleted inode* after a mid-session `cargo build` rename-replaces the running binary, so `spawn()` fails with ENOENT. Fixed in `b2115c4f` — `resolve_mux_binary()` in `src/lsp/manager.rs` (strip the ` (deleted)` marker → stable install path → actionable error). See `docs/issues/2026-06-13-rust-lsp-mux-spawn-fail-deadlocks-source-editing.md`.

This does **not** address this bug's Kotlin-specific core. **Still open:**
- **Defect #1** — mux liveness inferred from the advisory `flock` alone, never socket connectability (`src/lsp/manager.rs:456`): a stale `.lock` with a held flock but no live `.sock` routes to the connect path and fails all retries.
- **Defect #3** — the direct-LSP fallback collides on the held RocksDB index lock and reports a generic `LSP server disconnected` instead of naming the lock holder.

Both deferred — verification requires a live multi-instance Kotlin workspace (not reproducible from this session). Already shipped earlier: stderr capture (defect #2) + the lock-probe orphan reap (2026-06-11). Status stays `open` to keep defects #1/#3 visible as actionable work.

### Update 2026-06-14 (later) — defect #1 fixed in code; defect #3 confirmed already-fixed

Picked the two residual defects back up with a **live multi-instance Kotlin setup actually
present** (backend-kotlin + its `solver-db-logs` worktree — two live muxes, the substrate the
earlier deferral was waiting on).

**Defect #1 — flock-only liveness — FIXED in code (`src/lsp/manager.rs`).**
`get_or_start_via_mux` no longer infers liveness from the flock alone. Liveness is now
**flock AND socket reachability**, in a bounded 2-round loop:
- Round 0 is unchanged on the happy path — flock held + healthy socket connects on the first
  attempt and returns (zero added latency to the hot path).
- When the flock is held but the socket is unreachable across the 5 connect retries, round 1
  **re-arbitrates the flock**: a now-free lock ⇒ the holder died and left a stale `.sock`
  (which `process::run` unlinks before re-binding, `src/lsp/mux/process.rs:222`) ⇒ respawn; a
  still-held lock ⇒ a live but **wedged** mux ⇒ `mux_socket_unreachable_error` — an actionable
  `RecoverableError` naming the lock + socket and routing to `fuser`, instead of the bare
  `ECONNREFUSED`/`ENOENT` the old code returned via `last_err.unwrap()`.
- New free helper `claim_mux_lock(lock_path) -> io::Result<Option<File>>` is the testable
  liveness arbiter (`Some` = flock free, `None` = held). The flock stays the *spawn arbiter*
  (only one process may spawn); the socket becomes the *liveness signal* — the actual fix.

**Defect #3 — direct-LSP fallback collides + generic error — was ALREADY fixed in code;
"still open" only ever meant *unverified*, not *unfixed*.** Current `get_or_start` gates the
fallback behind three guards before it can run: `mux_failure_is_index_contention`, the
`kotlin_index_lock_held` lock-probe (Fix 4 / `c5fb3979`), and the single-owner invariant
(ADR `2026-06-11-mux-single-owner-invariant`) which **refuses the direct fallback outright in
production** (`!exe_is_test`; the silent `config.mux=false` fallback runs only inside a test
runner). The generic-`LSP server disconnected`-from-a-colliding-fallback path can no longer
execute outside a test binary — no code change needed for #3.

**Tests added** (`src/lsp/manager.rs`, `mod tests`):
- `claim_mux_lock_some_when_free_none_when_held` — the flock arbiter (free⇒Some, held⇒None).
  Holds the flock on a second in-process fd; `flock(2)` conflicts across open file descriptions
  even within one process, so this needs **no subprocess and no `#[ignore]`** — distinct from
  the fcntl-lock `posix_write_lock_*` tests, which DO need an external holder (fcntl locks are
  per-process and don't self-conflict).
- `mux_socket_unreachable_error_names_situation_and_routes_to_holder` — the wedged-mux error
  names the situation, cites both paths, and routes to `fuser`.

Full lib suite green (**2744 passed, 9 ignored**); `clippy --all-targets -- -D warnings` clean;
`cargo fmt` applied.

**Verification status (honest):** the liveness *logic* is unit-covered. The full live
end-to-end (construct a held-flock + dead-`.sock`, fire a pinned kotlin op, observe the
actionable error) is **still pending a deliberate `cargo build --release` + `/mcp` reconnect** —
not run this session to avoid disrupting the user's live backend-kotlin mux (R-23), and because
the fix is not yet in the running binary. **Status stays `open`** until that live e2e lands
(Resume step 1). Both #1 (code) and #3 (pre-existing) now have fixes; the only residue is live
verification.

### Verified live 2026-06-14 — defect #1 e2e PASSED (all four states)

Re-ran after the fix went live (release binary rebuilt post-`c8746b70`, fresh `/mcp`). Verified
end-to-end against the running server on a **throwaway rust workspace** (`/tmp/cs-wedge-test`) —
the user's live kotlin muxes were never touched. Rust routes through the same
`get_or_start_via_mux`, so it exercises the identical code path.

| State | How induced | Result |
|---|---|---|
| Happy (flock held + healthy socket) | `references` on codescout's own live rust mux | resolved correctly |
| Fresh spawn (flock free) | `references` on a brand-new workspace | mux spawned (hash `de91bfca…`), resolved |
| **Wedged (flock held + dead socket)** | killed the mux, held its flock from an external `flock(1)`, removed the `.sock` | **`mux startup failed for rust: mux for rust holds its lock (…) but its socket (…) is unreachable — the mux process is wedged or mid-restart … locate the holder with` `fuser` `…`** — the exact actionable error; pre-fix this returned a bare `ECONNREFUSED`/`ENOENT` |
| Recovery (holder cleared) | `fuser -k` the holder → `references` | flock freed → round-0 respawn → resolved |

This also live-confirmed **defect #3's production guard**: the wedged error was wrapped by
`get_or_start`'s single-owner refusal (`codescout will not fall back to a direct LSP for a
multiplexed language`), proving the fallback is gated off in production exactly as the ADR intends.

Bystander note (test-harness only, NOT a codescout bug): the `flock(1)` CLI leaks the lock fd to
its `sleep` child (no `O_CLOEXEC`), so killing `flock` didn't free the lock until the child was
reaped (`fuser -k` cleared it). codescout's own lock fd is `O_CLOEXEC` (Rust `std::fs::File`
default), so its spawned LSP never inherits it.

All four states pass → Resume step 1 (live failure-path repro) is **done**. Defects #1/#2/#3 are
now fixed **and** verified; the only remaining step is the master ship (Standard Ship Sequence).

### Update 2026-06-15 — wedged-path e2e is now an automated regression test

The manual live verification above is now encoded so the failure path can't silently regress:
`lsp::manager::tests::get_or_start_via_mux_surfaces_wedged_error_when_flock_held_socket_absent`
(`#[ignore]`-gated, like the sibling python-holder tests). It holds the mux ownership **BSD
`flock`** from a separate `python3` (`fcntl.flock`, blocking acquire — the same lock family
`fs4::try_lock_exclusive` uses, distinct from the `fcntl.lockf` POSIX locks the
`posix_write_lock` tests hold), removes any socket, then drives the real
`get_or_start_via_mux("rust", …)` and asserts the actionable `wedged or mid-restart` + `fuser`
error — **no Kotlin, no Gradle, no multi-instance needed.** Runs green under
`cargo test --lib lsp::manager -- --include-ignored` (45/45). This **retires the
"verification-limited" caveat** the bug carried across prior sessions — the failure mode
reproduces deterministically from a single external flock holder.
## Tests added

Implemented on `experiments` (`src/lsp/manager.rs`, in `mod tests`):

- `lsp::manager::tests::mux_failure_is_index_contention_detects_lock_signatures` — the
  RocksDB `LOCK`/`EAGAIN` and "another mux instance holds the lock" signatures classify as
  contention; a genuine spawn failure (`failed to spawn LSP server`) and empty string do not.
- `lsp::manager::tests::mux_failure_report_surfaces_stderr_cause_with_index_hint` — an empty
  stdout "ready" line + the real cause on stderr yields a message that **surfaces the cause**
  (regression for the blank `mux process failed to start:`) and the index-lock hint.
- `lsp::manager::tests::mux_failure_report_handles_silent_exit_with_generic_hint` — no stdout,
  no stderr → "(no diagnostic output …)" + the generic (non-index) hint.

Full lib suite green (2675 passed, 0 failed, 7 ignored); `cargo clippy --all-targets -- -D
warnings` clean; `cargo fmt` applied. The stderr-drain + fallback wiring is covered by
compilation + the manager suite; end-to-end behavior is pending a live `/mcp` repro (see Resume).
## Workarounds
- **Free the lock:** `fuser <kotlin-lsp-home>/<hash>/…/rocks/v492/LOCK` to find the holding
  PID, then `kill <pid>` (and its `faketime`/`kotlin-lsp.sh` wrapper). The next navigation call
  starts a clean LSP. Verified 2026-06-11.
- Avoid running multiple codescout servers against the same Kotlin workspace hash
  simultaneously (separate windows on the *same* repo share the home).
- `path`-based `symbols` still works (AST fallback) for read-only navigation while the LSP is
  down; `edit_code`/`references` do not.

## Resume
**UPDATE 2026-06-11 — pre-master gate RUN, FAILED. Do NOT ship Fix 4 to master yet.** The
failure-path repro (step 1 below) was executed live on an isolated worktree and exposed a
detection gap: when the kotlin-lsp fails RocksDB during `initialize`, the mux fails with
`initialize response missing 'result'` (no RocksDB signature in drained stderr), so
`mux_failure_is_index_contention` returns false, the no-fallback guard is bypassed, and the
direct fallback re-hits the lock → `LSP server disconnected`. See Evidence § "Failure-path repro
run live". **Next action (DONE 2026-06-11):** lock-state probing implemented — see Fix § "Robust
contention detection (lock-probe)". **VERIFIED live 2026-06-11:** re-ran the failure-path repro after the lock-probe fix — held
cs-fix4's RocksDB `LOCK` (external `fcntl.lockf`), fired `references` pinned to cs-fix4 → got the
actionable index-lock error (`kotlin LSP index is locked by another process …` + `fuser` hint)
in **1.6s**, with **no `falling back to direct LSP`** and **no squatter** (`debug.log` id=8). Same
failure mode that returned `LSP server disconnected` pre-fix; the lock-probe catches it where the
stderr signature can't. Fix 4 + lock-probe complete and end-to-end verified; full `cargo test`
green (2683 passed, 8 ignored). Step 1 below is now DONE. Next gate: Standard Ship Sequence to
master. The pin fix
(separate bug `2026-06-11-lsp-tools-ignore-workspace-pin-path`, defects #1/#2) can ship
independently; this Fix-4 work cannot.

**Committed + pushed to `experiments`** (Fix steps 1 + 3 + 4; step 2 dropped — see Fix):
`c5fb3979` (fix) + `3bc1009d` (trackers). Unit-tested (3 new tests), `clippy -D warnings` clean,
full lib suite green (2675). Rebuilt + `/mcp`-reconnected; **happy path live-verified** (a
`references` on backend-kotlin connects to the healthy mux through the rewritten path). **Not
yet on master.**

Next:
1. (pre-master gate) Live-reproduce the *failure* path: a 2nd kotlin-lsp holding the RocksDB
   lock → trigger `edit_code` from another server → confirm (a) the error reads `mux process
   failed to start: … RocksDBException … Resource temporarily unavailable` + index-lock hint,
   and (b) no poisoning direct-LSP squatter. (Couldn't run live this session without disrupting
   the user's working backend-kotlin mux — R-23.)
2. Standard Ship Sequence to master (frog audit first); cite the **master-side** SHA in Fix;
   `git mv` to `docs/issues/archive/`; flip frontmatter `status: fixed`.

### Self-heal decision (Architecture Snow Lion, 2026-06-11)

Asked to auto-heal the deadlock. Outcome: **the load-bearing self-heal already shipped at Fix
4** — every session is routed onto the single shared mux, whose 300s idle-timeout + `kill_on_drop`
(`process.rs:93,376`) free the lock on abandon/exit; concurrent sessions *connect* rather than
contend. Two candidate additions were evaluated against the change-scenario test:

- **A — isolate the direct-fallback index home: DROPPED.** Wall in an empty field. Fix 4 already
  prevents the direct fallback from running on contention, so a shared-home direct LSP is
  near-impossible in production; A absorbs no scenario Fix 4 doesn't. *Revisit-when:* a
  shared-home direct LSP is observed holding the lock after Fix 4 is live. Confidence: high.
- **B — reap a provably-orphaned lock-holder + retry: DEFERRED.** The one real residual scenario
  (a `SIGKILL`'d/OOM-killed mux orphans its LSP → immortal lock-holder, no idle-timeout, no live
  owner) stands alone, and B's design (scan `/proc` for our `--system-path`, reap only holders
  with no live `codescout` ancestor, retry once, at the `get_or_start_via_mux` chokepoint) is
  *correct*. But it adds a cross-process scan+kill surface (Linux-bound) against the project's
  structural grain, for a rare event the shipped **actionable error** already lets a human
  recover (today's manual `fuser` + `kill`). Two-strike discipline. *Revisit-when:* a second
  observed SIGKILL-orphan deadlock — then build B as specced. Confidence the design is right:
  high; that it's worth building now: medium → defer.
## References
- `src/lsp/manager.rs:432-539` (`get_or_start_via_mux`), `:318` (direct-LSP fallback),
  `:456` (flock-only liveness), `:485` (stderr→null), `:502-508` (blank error).
- `src/lsp/mux/mod.rs:15-29` (`socket_path_for_workspace` / `lock_path_for_workspace`).
- Live triage: `/home/marius/work/mirela/backend-kotlin/.codescout/{usage.db,debug.log}`.
- Related: `docs/issues/2026-06-05-lsp-failed-starts-not-recorded.md` (lsp_events under-recording),
  `docs/issues/archive/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md` (multi-instance shared home).
- NOTE: CLAUDE.md cites `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`, which does
  not exist at that path (doc-ref drift — flag for `audit_doc_refs`).

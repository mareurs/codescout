---
id: ee9d8d80ad5ecdc8
kind: bug
status: open
title: peer idle-timeout test is the third load-sensitive step in a class fixed twice per-instance
tags:
- cluster/repro-env-diverges-from-gate-env
closed: ''
last_observed: 2026-09-01
opened: 2026-09-01
owner: marius
related:
- '64efc41ac6686afb'
severity: low
unverified: Pre-existence is NOT established — no bisect against 0ed6cb18 was run. The no-call-path argument makes causation by the T-7 diff implausible, but 2/2 clean full runs at a low flake rate cannot distinguish pre-existing from aggravated.
---

## Summary
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections` failed once in a full
parallel `cargo test --workspace` during the T-7 SDD run. This is the **third** step of the
class first filed as `docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md`
(archived `fixed`), and the second recurrence — that file already records one, headed
*"the HOLDER step flakes too; fix covered only the reclaim step."*

The pattern is the finding. Each fix raised the real-time budget of **the step that had
flaked**, and the class then produced a different step. `2026-07-03` raised connect budgets
(`connect_with_retry` 50×20ms → 250×20ms, two inline loops, plus `peer/client.rs`);
`2026-07-05` added the mux holder step. Today's test measures neither — it asserts the server
**exits** after an idle period, so no connect budget covers it.

## Symptom (Effect)
One failure in one `cargo test --workspace` run. Not reproduced since.

## Reproduction
Not reliably reproducible. Measured 2026-09-01 on the fix commit `37f45079`:

| probe | result |
|---|---|
| the test in isolation, ×5 | 5/5 pass (1.13–1.42s each) |
| full `cargo test --workspace`, ×2 | 2/2 clean, exit 0, every result line `ok` |
| full workspace `--test-threads=1` (implementer) | 5030/5030 |
| full `cargo test --workspace` (implementer, parallel) | 4809 pass, this one fails |

So: real, load-sensitive, and low-rate.

## Environment
codescout worktree `audit-shards-t7` @ `37f45079`, forked from `experiments` `0ed6cb18`.
Linux, `nproc`-wide default parallelism, concurrent with a release build.

## Root cause
Not established, and deliberately not asserted. The 2026-07-03 file's own root cause reads
*"Unknown — not investigated beyond isolation"*, and nothing since has improved on that.

The working hypothesis that fits all three steps: **a test asserting a real-time property
under `nproc`-wide parallelism is asserting about the scheduler, not about the code.** Connect
readiness, flock reclaim and idle-timeout expiry are three different observations of the same
underlying quantity — whether this process got CPU inside a fixed wall-clock window.

## Evidence
### Why this is a class instance, not a new bug
The prior fix's own text names the shape: *"the original fix had covered only the reclaim
step; the holder step flaked under a full parallel run."* That is fix-per-observed-instance
against a class with several members — CLAUDE.md § Testing Discipline's *"Mutate once per
guarded SITE, not once per feature"*, in its remediation form. Three sites, three separate
discoveries, no pass that enumerated them.

### Why it is not attributable to the T-7 change
Stated as an argument, not a proof. `host.rs` is a new module whose every item is
`#[expect(dead_code)]` because **nothing outside its own tests calls it** — Tasks 2 and 3 wire
it in. There is no call path from it to `peer::server`. What the change does do is add ~5 tests
to the binary, marginally increasing parallel load, which could aggravate a load-sensitive
timer without causing it.

**What was NOT done:** a bisect against `0ed6cb18`. At an unmeasured rate that did not
reproduce in two full runs, establishing pre-existence would need many runs. So "pre-existing"
is the hypothesis the mechanism supports and is *not* something this file asserts.


### Fourth observation — independent session, 2026-09-01, with load DIRECTLY observed

Added by `codescout-17` (not this file's author) because it bears on the `unverified:` field's
exact gap: *"2/2 clean full runs at a low flake rate cannot distinguish pre-existing from
aggravated."*

- **Where:** main checkout, `HEAD 72484f8d5817e4675191d84caaaad869abf78f71`, working tree
  carrying an unrelated `src/util/librarian_guard.rs` change and a peer's uncommitted
  `doctor.rs` edits. **No T-7 audit-shard code in the tree** — that work is in
  `.worktrees/audit-shards-t7`. Same failure, same message.
- **Result:** `cargo test --workspace` → 4804 passed / **1 failed**, the failure being this
  test: `run() did not exit within 10s of a 1s idle timeout`. Re-run in isolation
  (`cargo test --workspace run_exits_after_idle_timeout_with_no_connections`) → **ok in
  1.19s**.
- **The load was observed, not inferred.** Both cargo invocations printed
  `Blocking waiting for file lock on build directory`, and `ListAgents` reported two other
  interactive sessions **both `busy`** at the time. So this run has direct evidence for the
  scheduler hypothesis in *Root cause* rather than an appeal to plausibility — the process
  demonstrably was not getting the CPU it wanted.

**What this does and does not settle.** It does **not** establish pre-existence: no bisect was
run here either, so the `unverified:` field stands as written. What it adds is that the failure
reproduces in a tree **without** the T-7 diff, which strengthens the no-call-path argument from
implausible-by-reading to not-observed-to-need-it. Attribution to load is now backed by a
measured contention signal in at least one instance.

**Method note for whoever bisects.** This instance was nearly re-derived from scratch: the
isolation re-run had already been launched before a peer pointed out this file existed. The
file was **unreadable by `read_markdown`** — it is a tool-created bug file and therefore
stamped, so the librarian guard refused it, which is
`docs/issues/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md` firing on
the corpus's newest file. Reaching it needed `artifact(action="find")` then
`artifact(action="get")`. Worth knowing if a future session looks for this record by path and
concludes it is absent.
## Hypotheses tried
- *Named in a prior flake file?* No — `2026-08-26-wine-lane-flakes-under-load-on-three-tests`
  narrowed itself to one unrelated test (`run_migrations_is_safe_under_concurrent_connections`).
- *Deterministic break from the T-7 diff?* Ruled out: 2/2 clean full runs and 5/5 isolated on
  the same commit.

## Fix
Two options, and the second is the one worth taking.

1. **Raise this test's budget**, as 2026-07-03 and 2026-07-05 each did for their step. Cheap,
   and it is the move that produced two recurrences — it treats the instance.
2. **Enumerate the class once.** Grep `src/peer/` and `src/lsp/` for every test asserting a
   real-time property (a sleep, a deadline, a bounded retry loop, an "exits within N"
   assertion) and decide per site: generous budget, or an event-driven wait that does not
   assert about the scheduler at all. An idle-timeout test in particular can usually observe
   the exit rather than time it.

Explicitly rejected again, for the reason the prior file gives: `#[serial]` does not help,
because the contention is CPU starvation from the *other* ~4800 tests, which serializing these
among themselves does not relieve.

## Tests added
N/A — not started. Note the prior file recorded *"Tests added: None — the regression signal is
the existing suite staying green"*, and that signal is precisely what a low-rate flake defeats.

## Workarounds
`--test-threads=1` for a decisive full-suite answer (5030/5030 today). Do not read a single
green parallel run as evidence the class is gone.

## Resume
Start with option 2's enumeration — the count is the thing nobody has. If it is three sites,
fix all three now; if it is fifteen, that is a different conversation and worth knowing before
raising a fourth budget.

## References
- `docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md` (fixed,
  with its own 2026-07-05 recurrence note)
- `src/peer/server.rs` `connect_with_retry`; `src/lsp/manager.rs`
  `claim_mux_lock_some_when_free_none_when_held`
- `docs/trackers/catalog-audit-trail-session-log.md` — observed during the T-7 SDD run

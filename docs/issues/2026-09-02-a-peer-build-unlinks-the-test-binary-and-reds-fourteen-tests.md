---
id: dcbd9ee0aa40274c
kind: bug
status: fixed
title: 'BUG: a peer''s cargo build unlinks the running test binary, so the stale-binary guard reds 13 tests for a session that changed nothing'
tags:
- cluster/shared-resource-carries-no-owner
- retrieval
- shared-checkout
- concurrency
- test-isolation
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: med
---

## Summary

On a shared checkout, `cargo build` in **any** session unlinks
`target/debug/deps/codescout-<hash>` while another session's suite is running.
`guard_stale_binary` then correctly observes that its own executable is gone and
declines to index. It is right to — **for a server**. It was answering for a test
process, which has neither property the guard protects against: no longevity, and
a tempdir instead of shared per-project state.

13 tests go red. Nothing in the tree changed, no test is flaky in the timing
sense, and the failure is attributed to whoever happens to be running the suite.

## Symptom (Effect)

12 `retrieval::sync::tests::*` fail with one message:

```
an oversized chunk must not abort the sync: this codescout server is running a
binary that has been deleted from disk, so re-indexing would write vectors
produced by code that no longer exists...
```

and `retrieval::index_state::tests::a_live_binary_does_not_report_itself_deleted`
fails with the tell in plain sight:

```
"the test runner's own executable exists, so this must be a definite false"
left: Some(true)   right: Some(false)
```

**Reproducible, not flaky.** Same 13, same order, across four observed runs in two
sessions. Under `--workspace` there are ~68s in which a peer build can land; in
isolation 0.63s, which is why the same set passes alone and fails together. It
tracks *who else is building*, not what the tests do — which is why it appears and
vanishes with nobody touching test code.

## Root cause

Two causes, one trigger. **The trigger is `target/`**: a directory shared by every
session in the checkout, which records what changed and never who changed it.

**Cause 1 — the ambient signal (12 tests).** `SyncOpts.writer` already exists as a
test seam, and its doc comment says so: *"Injecting the provenance keeps the policy
testable while leaving detection where it belongs."* The policy test uses it,
injecting `exe_deleted: Some(true)`. Every other test writes
`..SyncOpts::default()` → `writer: None` → production's *snapshot the live
process*. So they read `/proc/self/exe` by default and inherited whatever a peer
was doing. The seam existed; the default pointed the wrong way for tests.

**Cause 2 — a premise the test did not own (1 test).**
`a_live_binary_does_not_report_itself_deleted` asserted `Some(false)`
unconditionally, on the stated premise that *"the test binary is running from a
file that exists"*. On a shared checkout that is not the test's premise to make.
When a peer's build falsified it, the test failed reporting **an inverted
predicate** — a true statement about the environment, misattributed to the code.

## Evidence

### The guard is correct and stays correct

`guard_stale_binary` (`src/retrieval/sync.rs`) is unchanged, as are its three unit
tests and the call-site wiring proof. The hazard it exists for is real and filed:
`docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`.
Nothing here weakens it. What changed is *who is asked* — a test process is not a
server, and was never the party the question was about.

### Verified by reproduction, not by the tests passing

Forcing `ambient_exe_deleted()` to `Some(true)` under `cfg(test)` — a faithful
simulation of the concurrent-build condition — fails **exactly the 12 sync tests
observed in the wild, by name**. Restoring `Some(false)` passes them. The
simulation is what makes this a diagnosis rather than a correlation: "the tests
pass now" is also what they did five minutes before a peer ran `cargo build`.

### Why "enumerate the peer" is not the remedy

This is `IC-17`'s claim holding exactly. Every session in this checkout was
positively identified by socket enumeration this morning — 7 sessions across 3
profiles — and that changes nothing: a peer running `cargo build` is doing
something entirely correct, in a resource with no owner field, and the damaged
party cannot tell whose build it was or ask them to stop. `OB-2` is this same
`target/` clobber seen from the arming side.

## Hypotheses tried

- **Index-lock contention.** Plausible and wrong: two of the failures are
  `*_holds_index_lock_for_its_full_duration`, and a pair of lock tests failing
  together reads as contention. `sync_worktree_holds_index_lock_for_its_full_duration`
  was the one failure in the set with a different message, which is what made the
  hypothesis survive. The other twelve name the binary. Treating the shared
  signature as membership would have folded a `target/` concurrency bug into a
  timeout class.
- **Load-sensitive timeouts.** Ruled out by reproducibility: same 13, same order,
  across runs.

## Fix

**Fixed on `experiments` at `50b1605f`**
(`50b1605fb0d63adfe9f084a2c4b8d91d2df68b34`), patch-id
`70c8ef6bae0c40471b06dde862c12cbab2b17cd8`.

**Cause 1 — at the fallback, not at 13 call sites.** A new
`ambient_exe_deleted()` reads the live process in production and is `Some(false)`
under `cfg(test)`. One site, and it cannot be forgotten by the next test the way a
per-call-site convention can — a fix that every future test must remember is a
policy, not a mechanism.

**Deliberate coverage loss, named rather than left implied:** the production
branch now has no test exercising it. It never had a deliberate one — those 13
reached it incidentally, and that incidental coverage is precisely what broke. The
policy is covered directly and does not route through here.

**Cause 2 — split the predicate from the process.** `path_reports_deleted` is
extracted as a pure function; `exe_is_deleted` becomes `read_link` + delegate. The
inversion coverage moves to a deterministic fixture test, and the live test now
compares against the filesystem either way, becoming a wiring test.

## Tests added

`path_reports_deleted_discriminates_all_three_real_cases` — the three cases
`exe_is_deleted`'s doc comment described in prose, now fixtures: a live path, a
`" (deleted)"`-suffixed path that does not exist, and a real file **named**
`"x (deleted)"` that does. The third is annotated load-bearing: delete it and the
`!p.exists()` conjunct becomes untestable, so a predicate that dropped it would
pass.

`a_live_binary_does_not_report_itself_deleted` rewritten to compare against ground
truth rather than a constant. It is now a wiring test and says so.

**Mutation-verified by simulating the field condition** rather than by editing an
assertion: `ambient_exe_deleted() → Some(true)` reproduces the 12 sync failures by
name.

## Workarounds

Run the suite when no peer is building, or `cargo test -p codescout --lib
retrieval::sync` in isolation (0.63s window instead of ~68s). Neither is
actionable by the party who hits it, which is the point.

## Resume

**Closed.** Gate green both lanes at the fix commit — lean 3482 passed / 0 failed,
default 5203 passed / 0 failed, clippy `--workspace --all-targets --features
local-embed -D warnings` exit 0.

**Not addressed, and deliberately:** `target/` still has no owner. This fix makes
one consumer stop asking a question it had no business asking; it does not stop a
peer's build from unlinking a running binary. Any other code that consults
`/proc/self/exe`, or any future one, is exposed the same way. That is `IC-17`'s
*Mechanism status: partial*, unchanged.

## References

- Diagnosis: `codescout-05`, which reproduced the cluster twice with stdout
  retained and identified the deleted-binary guard over the index-lock hypothesis.
- The hazard the guard exists for:
  `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
- Same `target/` clobber from the arming side: `OB-2`.


---
id: d78bb9d7d7acfb0b
kind: bug
status: fixed
title: 'BUG: process_alive reports a nonexistent process as alive, and terminate_process would signal a whole process group'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-06
opened: 2026-09-05
owner: marius
related: []
severity: medium
---

# BUG: `process_alive` reports a nonexistent process as alive, and `terminate_process` would signal a whole process group

## Summary

`crate::platform::process_alive(pid)` and `terminate_process(pid)` take a `u32`
and cast it with `pid as i32` before calling `libc::kill`. The sign of a `pid_t`
is not a formality — it selects the *addressing mode* — so any `u32` at or above
`2^31` wraps negative and silently changes what the call means. `process_alive`
then returns **`true` for a process that does not exist**, and `terminate_process`
would send SIGTERM to a process **group** rather than to one process.

## Symptom (Effect)

```
assertion failed: !is_alive(&sample(u32::MAX, 0))
```

`process_alive(u32::MAX)` → `true`. `u32::MAX as i32` is `-1`, and
`kill(-1, 0)` asks "may I signal *every* process I have permission to signal?",
which succeeds. The same holds for `0`: `kill(0, 0)` addresses the caller's own
process group, so `process_alive(0)` is true for any caller in a live group.

The pid-0 half has been known since **2026-08-18** and was worked around at one
call site (`src/tools/rendezvous.rs`, an explicit `if pid == 0` collection arm)
rather than fixed at the platform layer. The `≥ 2^31` half was not known.

## Reproduction

```
git rev-parse HEAD        # 92fad220 (+ working tree), branch experiments
cargo test --workspace --lib platform::unix
```

Before the fix, with a test asserting `!process_alive(u32::MAX)`:

```rust
assert!(!crate::platform::process_alive(u32::MAX));   // FAILS: returns true
assert!(!crate::platform::process_alive(0));          // FAILS: returns true
```

## Environment

- Linux 7.1.11-arch1-1, `experiments`, codescout 0.15.0
- Unix path only. **Windows is unaffected**: `OpenProcess` takes a `u32`
  natively and returns a null handle for a pid that does not exist
  (`src/platform/windows.rs:299`), so no cast happens and no addressing mode
  changes.

## Root cause

`src/platform/unix.rs` (pre-fix):

```rust
pub fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    ...
}
```

`kill(2)`'s first argument is an addressing scheme, not an identifier:

| value | what `kill` addresses |
|---|---|
| `> 0` | that one process |
| `0` | every process in the **caller's own process group** |
| `-1` | every process the caller has permission to signal |
| `< -1` | every process in process group `-pid` |

So `pid as i32` is a **reinterpretation**, not a widening. The `u32` domain has
2³¹ values that name no single process, and all of them silently become group or
broadcast addresses.

*Measured 2026-09-05:* `process_alive(u32::MAX)` returned `true` on Linux
7.1.11, observed as a red test in `librarian::reindex_progress::tests`. The pid-0
case was measured 2026-08-18 (`kill -0 0` exits 0), recorded in
`src/tools/rendezvous.rs`. *Not measured:* whether `terminate_process` has ever
been reached with an out-of-range pid — see § Hypotheses tried, item 2.

## Evidence

### The comment that was right about its own caller and wrong as a general claim

```rust
// src/lsp/client.rs:1614-1615
// The `u32 as i32` cast is safe because Linux PIDs are assigned from a range
// that fits in i32 (maximum 4,194,304 on 64-bit kernels).
let _ = crate::platform::terminate_process(*pid);
```

True — of **that** caller, which passes a kernel-assigned `Child::id()`. The
claim is about the *provenance* of the pid, not about the function, and the
function is shared. `librarian::reindex_progress` now reads pids back out of
`catalog_meta`, where a corrupt or hand-edited row can hold any `u32`, so the
premise no longer holds tree-wide.

### The workaround that localised the pid-0 half

```rust
// src/tools/rendezvous.rs — pre-fix comment
// A stray `0.json` can only be garbage: `std::process::id()` is never 0, and
// `process_alive(0)` is unconditionally true on unix because `kill(0, 0)`
// addresses the caller's process GROUP, not process number zero — so the
// liveness check below would never collect it.
```

One call site diagnosed the mechanism correctly and repaired itself. Nothing
propagated the finding to the shared function, so every other caller kept the
defect. This is the cheap tell for the class: **a call-site workaround whose
comment explains a platform-level bug**.

## Hypotheses tried

1. **Hypothesis:** the failing assertion was a bad fixture — `u32::MAX` is simply
   an unrealistic pid.
   **Test:** read `src/platform/unix.rs:103-105`; check `kill(2)`'s contract for
   negative and zero arguments.
   **Verdict:** **rejected.** The fixture was unrealistic *and* the function was
   wrong; the unrealistic input is what exposed it. The fixture was separately
   replaced with a spawned-and-reaped pid, because a constant only reaches the
   new range guard and would leave a `process_alive` that never calls `kill` green.

2. **Hypothesis:** some live caller already passes an out-of-range pid, making
   this an active rather than latent defect.
   **Verdict:** **deferred — no evidence either way.** Every current
   `terminate_process` caller passes `Child::id()`; every `process_alive` caller
   except `reindex_progress` does too. Do not promote this to "exploited" without
   an observed out-of-range value.

## Fix

`src/platform/unix.rs` — a single validating helper, `addressable_pid`, applied
by both functions:

```rust
fn addressable_pid(pid: u32) -> Option<libc::pid_t> {
    match libc::pid_t::try_from(pid) {
        Ok(p) if p > 0 => Some(p),
        _ => None,
    }
}
```

`process_alive` returns `false` for a pid that names no single process;
`terminate_process` returns `InvalidInput` rather than signalling. Validating in
the platform layer rather than at each call site is the difference between one
check and N — and the pid-0 history is the evidence that N does not happen.

The `rendezvous.rs` pid-0 arm is **kept**, deliberately: "a `0.json` is garbage"
is a fact about that directory's naming scheme, not about signal semantics, and
it should not silently start depending on a platform detail. Its comments were
corrected in the same change, since they asserted behaviour that is now false.

SHA: `01b185d6` (**`experiments`**)
patch-id: `5ee25dcd470221a49b7da116dcc1f796da18c908`

## Tests added

`src/platform/unix.rs`:

- `liveness_is_true_for_this_process_and_false_for_a_reaped_one` — the real
  case, using a spawned-and-reaped pid so the test reaches `kill` itself.
- `a_pid_that_does_not_address_one_process_is_not_alive` — `0`, `0x8000_0000`,
  `u32::MAX`. **Every row was `true` before this fix.**
- `terminating_a_pid_that_does_not_address_one_process_is_refused` — the same
  boundary on the signalling path, where the consequence is a wrong *action*
  rather than a wrong answer.

`src/librarian/reindex_progress.rs`:

- `is_alive_is_true_for_this_process_and_false_for_an_impossible_pid` — pinned
  from the consumer that made the input untrusted.

The two liveness tests are deliberately paired: the reaped-pid row alone stays
green against a function that only range-checks, and the constant rows alone stay
green against one hard-wired to `false`.

## Workarounds

Validate before calling, as `rendezvous.rs` did for pid 0:

```rust
if pid > 0 && i32::try_from(pid).is_ok() { /* safe to ask */ }
```

## Resume

N/A — fixed in this change. If reopening: start at
`src/platform/unix.rs` § `addressable_pid` and check whether any caller wants
*group* addressing deliberately (none does today); such a caller needs its own
explicitly-named function, not a relaxation of this guard.

## References

- `src/tools/rendezvous.rs` — the 2026-08-18 pid-0 workaround this generalises.
- `src/lsp/client.rs:1614` — the safety comment that is correct about its own
  caller and does not transfer.
- `docs/issues/archive/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md`
  — the fix whose liveness check surfaced this.
- `docs/trackers/reconnaissance-patterns.md` § `R-182`.

### Cluster adjudication

Tagged `cluster/addressing-without-an-escape-hatch` (`IC-6`) on both halves of
that claim. **No disambiguator:** "process N" and "process group N" share one
signed-integer namespace, separated only by sign, so one representation names two
different targets. **No escape:** a process whose id is ≥ 2³¹ cannot be named as
a single-process target at all — the input is unrepresentable, which is exactly
why no ordinary test reached it.

Stated so a later reader can withdraw the tag rather than re-derive the doubt:
the addressing scheme here is POSIX's, not codescout's. Codescout's defect is
*consuming* such a namespace without validating that its token lies in the
addressable subset. If that distinction is judged to matter, this belongs in a
new class rather than in `cluster/unclassified`. The rival candidate considered
and rejected was `cluster/guard-narrower-than-its-name` (`IC-14`) — rejected on
the remedy test: nothing here was guarded too narrowly, a function returned a
wrong answer, and the repair is a disambiguator rather than a wider guard.

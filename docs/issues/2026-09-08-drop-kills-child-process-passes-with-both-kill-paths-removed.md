---
kind: bug
status: open
tags:
- cluster/assertion-that-cannot-fail
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: medium
---

# BUG: `drop_kills_child_process` passes with BOTH of codescout's kill paths removed

## Summary

`lsp::client::tests::drop_kills_child_process` is named for, and universally read as,
the regression guard on `LspClient`'s child-reaping. It covers **neither** of the two
mechanisms that do the reaping. Deleting `Drop`'s `terminate_process`, deleting
`.kill_on_drop(true)`, or deleting **both together** each leave the test green in under
0.1s, because a third path nobody wrote gets there first: dropping the client closes the
child's stdio pipes, and `rust-analyzer` is built with `unix_sigpipe = "sig_dfl"`, so it
dies of `SIGPIPE`. The test asserts *something* reaps the child, which is true of any LSP
server that dies when its client goes away.

## Symptom (Effect)

The test passes under every mutation of the code it names. Three runs, each with the
mutation verified applied at the bytes before the run:

```
MUTATION-1  Drop's terminate_process removed        (grep -c terminate_process -> 0)
            test lsp::client::tests::drop_kills_child_process ... ok    0.05s

MUTATION-2a .kill_on_drop(true) -> (false)          (grep -c 'kill_on_drop(true)' -> 0)
            test lsp::client::tests::drop_kills_child_process ... ok    0.05s

MUTATION-2b BOTH of the above, simultaneously
            test lsp::client::tests::drop_kills_child_process ... ok    0.08s
```

There is no failing input inside the assertion's claimed scope.

## Reproduction

At `5c634a37209e076ab0785d69bc94aa92d85fa15f` on `experiments`, with `rust-analyzer`
installed (the test self-skips without it):

```bash
sed -i 's/\.kill_on_drop(true);/.kill_on_drop(false);/' src/lsp/client.rs
sed -i 's|let _ = crate::platform::terminate_process(\*pid);|let _ = *pid;|' src/lsp/client.rs
# verify applied -- an unapplied mutation prints green for the wrong reason
grep -c 'kill_on_drop(true)' src/lsp/client.rs    # expect 0
grep -c terminate_process src/lsp/client.rs       # expect 0
cargo test --lib lsp::client::tests::drop_kills_child_process -- --exact
# -> test result: ok. 1 passed
```

## Environment

Linux 7.2.3-zen1-3-zen, `rust-analyzer 1.97.1 (8bab26f 2026-07-14)`, branch
`experiments`, `cargo test --lib` (default features).

## Root cause

Three mechanisms can reap the child, and the assertion `!process_alive(pid)`
(`src/lsp/client.rs`, final assert of the test) is satisfied by any one of them. Ordered
by which actually wins:

1. **`Drop for LspClient` -> `crate::platform::terminate_process(*pid)`
   (`src/lsp/client.rs:1616`)** — sends `SIGTERM` synchronously inside `drop`. This is
   the path that fires in practice.
2. **`.kill_on_drop(true)` (`src/lsp/client.rs:465`)** — the `Child` handle is moved into
   the stdout reader task (`child.wait()`, `src/lsp/client.rs:581`), never stored on
   `LspClient`, which holds only `child_pid`. `Drop` aborts that task, the future is
   dropped at the next runtime poll, the `Child` drops, and tokio sends `SIGKILL`. It
   arrives **after** the process is already dead.
3. **`SIGPIPE`** — `stdin` was moved into `LspClient.writer` and `stdout` into the reader
   task, so dropping the client closes both pipe ends. `rust-analyzer` builds with
   `unix_sigpipe = "sig_dfl"` (rather than the Rust default of `SIG_IGN`), so its next
   write to the closed stdout kills it. This is invisible until 1 and 2 are both removed.

measured 2026-09-08: `strace -f -e trace=kill,tgkill` on the test binary, unmutated and
under MUTATION-2b — see Evidence. Not inferred from the source; both signal orderings
were observed.

The comment at `src/lsp/client.rs:1602-1605` calls `terminate_process` a "safety net" that
fires "even if the graceful path was skipped". The strace shows it is the **primary**
killer on this path, and `kill_on_drop` is the one whose signal is always redundant. The
comment has the two backwards.

## Evidence

### strace, unmutated — SIGTERM wins, SIGKILL is a no-op

```
336887 kill(336898, SIGTERM)            = 0
336910 +++ killed by SIGTERM +++
336909 +++ killed by SIGTERM +++
336908 +++ killed by SIGTERM +++
336907 +++ killed by SIGTERM +++
336887 kill(336898, SIGKILL)            = 0
336898 +++ killed by SIGTERM +++
```

### strace, MUTATION-2b — no `kill()` at all; the child dies of SIGPIPE

```
331548 +++ killed by SIGPIPE +++
331549 +++ killed by SIGPIPE +++
331547 +++ killed by SIGPIPE +++
331538 +++ killed by SIGPIPE +++
```

### The child is genuinely live before the drop, and fully gone 40ms after

Instrumented poll loop under MUTATION-2b:

```
PROBE pre-drop:  pid=249966 state=[S] comm=rust-analyzer
PROBE post-drop: iters=2 proc_exists=false state=[] alive=false
```

`state=[S]` rules out the alternative reading that the "alive" assertion was passing on a
zombie — `process_alive` is `kill(pid, 0)`, which succeeds for a zombie
(`src/platform/unix.rs:148-157`).

## Hypotheses tried

1. **Hypothesis:** the test body never runs (the `rust_analyzer_available()` guard returns
   early, which `return`s and passes). **Test:** `panic!("PROBE: body reached")` inserted
   after the guard. **Verdict:** rejected — `FAILED ... PROBE: body reached`.
2. **Hypothesis:** `rust-analyzer` exits on stdin EOF. **Test:** `timeout 10
   rust-analyzer < /dev/null`. **Verdict:** rejected — exit code 124, survived the full
   10s.
3. **Hypothesis:** an *initialized* `rust-analyzer` exits on stdin EOF, and the control
   above was wrong because it gave EOF before `initialize`. **Test:** python LSP harness,
   real `initialize` handshake, response read, then `stdin.close()`. **Verdict:**
   rejected — survived 10s.
4. **Hypothesis:** closing the stdout **read** end is what kills it. **Test:** same
   harness, three arms — close stdin, close stdout, close both. **Verdict:** rejected as
   run, all three survived 5s — **but this control was under-specified**: it never sent
   `initialized`, so the server had no indexing work and never wrote to the broken pipe.
   The mechanism is real (hypothesis 5); this arm failed to trigger it.
5. **Hypothesis:** `SIGPIPE`. **Test:** `strace -f -e trace=kill,tgkill` under
   MUTATION-2b. **Verdict:** confirmed — `+++ killed by SIGPIPE +++`.

Hypotheses 2-4 are the reason this is filed with an strace rather than a reading: three
plausible mechanisms were falsified by controls before the observed one was found, and
hypothesis 4 was falsified by a control that was itself wrong.

## Fix

Not fixed. The test is annotated as inert at `src/lsp/client.rs` (the `INERT for BOTH
deliberate kill paths` block at the head of the test body) so nobody credits it with
coverage it does not provide — `CLAUDE.md` § *Testing Discipline*, "annotate an inert
fixture as inert".

Sketch for whoever takes it: the child has to be one that **survives SIGPIPE**, or the
assertion has to name the signal rather than the liveness. Two shapes, neither costed:

- Spawn a child that ignores `SIGPIPE` and does not exit on EOF (`sh -c 'trap "" PIPE;
  while :; do sleep 1; done'`) and assert on it. Needs `LspClient::start` to tolerate a
  non-LSP child, which it may not.
- Assert on the **signal**, not on liveness: reap the child in the test and check
  `WTERMSIG == SIGTERM`. This discriminates path 1 from path 3 directly, and would red on
  MUTATION-1. It cannot separate path 1 from path 2 (both would be a signal), so
  `kill_on_drop` still needs its own site — `CLAUDE.md`, "mutate once per guarded SITE".

Note that path 2 is **not** dead code: `.kill_on_drop(true)` is the only mechanism in the
WIN-5 spawn-timeout path (`src/lsp/client.rs:468-472`), where no `LspClient` is ever
constructed and so `Drop` never runs. That path has no test at all.

## Tests added

None — this file records the absence. Adding an assertion to the existing test does not
fix it; the child is the problem, not the assertion count.

## Workarounds

N/A — no user-visible defect. The production code reaps correctly by three independent
routes. What is missing is coverage, so the risk is a future refactor removing all three
without any test reddening.

## Resume

Decide between the two Fix shapes above. Start by checking whether `LspClient::start`
completes against a non-LSP child (`sh -c ...`) — if it requires a real `initialize`
response, the signal-assertion shape is the only option. Run
`cargo test --lib lsp::client::tests::drop_kills_child_process -- --exact` to anchor, then
apply MUTATION-1 from Reproduction and require an observed RED before crediting any new
assertion.

## References

- `src/lsp/client.rs` — the test, `Drop for LspClient`, and the spawn site
- `src/platform/unix.rs:148-157` — `process_alive`, and why a zombie reads alive
- `docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md` —
  the flake-class work this was found under
- `CLAUDE.md` § *Testing Discipline* — "demand an observed RED, never an assertion's
  existence"; "mutate the PRODUCTION path, not the test's inputs"

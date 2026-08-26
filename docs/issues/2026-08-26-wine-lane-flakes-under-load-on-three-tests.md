---
status: open
opened: 2026-08-26
closed:
severity: low
owner: marius
related: []
tags: [windows, wine, ci, flake, concurrency]
unverified: 'Observed exactly ONCE, and the mechanism is inferred rather than measured. Contention is the hypothesis that fits all three symptoms and the recovery, but nothing was instrumented to prove it — no load was measured, no retry was scripted, and the run that failed is not reproducible on demand. Treat the mechanism as a lead, not a finding. Do NOT add retries or raise timeouts on this evidence.'
kind: bug
---

# BUG: three unrelated tests failed together on the wine lane under load, and passed on the next run

## Summary

A local `windows-gnu` run failed three tests that share no code. The next run of the
identical command, with no source change, passed 4293/0. They are load-sensitive flakes,
not defects — recorded so the next person to see one does not spend the afternoon
diagnosing a platform bug that is not there.

## Symptom (Effect)

One run, three failures:

```
librarian::catalog::tests::run_migrations_is_safe_under_concurrent_connections
  panicked: run_migrations must be safe under concurrent connections sharing a
  catalog: database is locked / Error code 5: database is locked

tools::run_command::tests::unfiltered_output_carries_a_line_count_and_explicit_empty_stdout
  left: Null   right: ""        (the `stdout` key was absent)

tools::run_command::tests::unfiltered_output_line_count_survives_inline_truncation
  left: Null   right: 20000     (the `unfiltered_output_lines` key was absent)
```

Both `run_command` failures showed a response missing keys that
`src/tools/run_command/output.rs` sets **in a single block** with the keys that WERE
present — so the response could not have come from that block at all. That impossibility
is the tell that the run, not the code, was wrong.

## Reproduction

Not reproducible on demand. Three runs of the same command on the same tree:

| run | conditions | result |
|---|---|---|
| 1 | immediately after a full recompile — compiler, 4303 tests and wine contending | **3 failed** |
| 2 | warm build, same command, no source change | 4293 passed, 0 failed |
| 3 | warm build, at a later HEAD | 4293 passed, 0 failed |

The two `run_command` tests were also run **in isolation** under wine and passed in 0.52s.

## Environment

Local wine 11.16 loop, `x86_64-pc-windows-gnu`, PortableGit via `CODESCOUT_BASH` +
`WINEPATH`. Not seen on CI — GitHub Actions was in a `major_outage` throughout, so the
lane has had no CI run to compare against.

## Root cause

**Not established.** Resource contention is the hypothesis that fits, and it fits all
three symptoms at once, which is the only reason it is worth writing down:

- `database is locked` is SQLite's ordinary response to a busy timeout expiring — a
  contention symptom by construction, not a logic error.
- Both `run_command` failures are consistent with the tee capture read at
  `src/tools/run_command/output.rs:129-131` returning `None`
  (`std::fs::read_to_string(...).ok()`), which collapses `unfiltered_ref` to `None` and
  drops the whole key group. A `.ok()` that swallows the error is exactly the shape that
  turns a transient I/O failure into a silently absent field.
- Run 1 was the only one that paid a full recompile concurrently with 4303 wine-hosted
  tests.

**Explicitly ruled out**, so nobody re-walks them:

1. *A platform defect in the new `unfiltered_output` size-signal feature* (`c172fe10`) —
   rejected: both tests pass under wine in isolation and in runs 2 and 3.
2. *Two producers of the envelope, only one updated* — rejected: `grep` over `src/**/*.rs`
   finds exactly one site setting `unfiltered_output`, and it sets all four keys together.
3. *A tee tmpfile name collision between parallel tests* — rejected:
   `inject_tee` uses `tempfile::Builder::new().prefix("codescout-unfiltered-").tempfile()`,
   which is unique per call by construction.

## Evidence

The impossible-looking payload, which is what redirected the investigation from "platform
defect" to "bad run":

```
{"exit_code":1,"stderr":"Cygwin WARNING:…","unfiltered_output":"@cmd_3ee4bbf5",
 "unfiltered_truncated":true}
```

`unfiltered_output` and `unfiltered_truncated` present, `unfiltered_output_lines` and
`stdout` absent — from a block that sets all four. The `stderr` is the wine Cygwin
FAST_CWD warning, which is expected on every `bash.exe` here and is a red herring.

## Hypotheses tried

1. **A real Windows defect in the peer's new size-signal feature.** *Test:* run both tests
   alone under wine. *Verdict:* rejected — 2 passed in 0.52s.
2. **Deterministic at this HEAD.** *Test:* re-run the identical full command. *Verdict:*
   rejected — 4293/0.
3. **Load-sensitive contention.** *Test:* none performed. *Verdict:* consistent with all
   evidence, **not confirmed**.

## Fix

None, deliberately. A flake seen once, whose mechanism is inferred, does not justify a
change — and the two changes it would invite are both actively harmful:

- **Retries** would convert a real future regression into a green run.
- **Raising the SQLite busy timeout** would be tuning against a number nobody has measured.

The cost of leaving it is bounded and known: at worst one spurious red on a lane whose
failures are read by hand.

## Tests added

None; see Fix.

## Workarounds

Re-run the lane. If the same three fail together again, that is the datapoint this file is
waiting for — and *then* instrument before changing anything.

## Resume

If a second occurrence appears: capture load at failure time (`uptime`, concurrent wine
processes) and add a temporary probe at
`src/tools/run_command/output.rs:129-131` logging the `read_to_string` error that `.ok()`
currently discards. That single `.ok()` is the highest-value instrumentation point — it is
the one place a transient failure becomes an absent field with no trace.

## References

- `docs/issues/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md` — the
  other wine-lane caveat; that one is a version divergence, this one is load, and they are
  not the same thing
- `src/tools/run_command/output.rs:129-131` — the `.ok()` that would hide the I/O error
- `src/librarian/catalog/mod.rs:820` — the concurrent-migration assertion

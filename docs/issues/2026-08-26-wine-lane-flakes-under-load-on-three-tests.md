---
kind: bug
status: open
tags:
- cluster/repro-env-diverges-from-gate-env
- windows
- wine
- ci
- flake
- concurrency
closed: null
opened: 2026-08-26
owner: marius
related: []
severity: low
unverified: 'NARROWED 2026-08-26 by CI run 32997878934: two of the three tests failed there with the wine-9.0 `timed_out` signature, NOT this file''''s partial-key payload, so they belong to the lane''''s already-classified wine-9 hang group and are skipped there. Only run_migrations_is_safe_under_concurrent_connections remains in scope for this file — observed ONCE, locally, never on CI, mechanism inferred (contention) and never instrumented. Treat it as a lead, not a finding, and do NOT add retries or raise the SQLite busy timeout on this evidence.'
---

# BUG: three unrelated tests failed together on the wine lane under load, and passed on the next run

## Summary

A local `windows-gnu` run failed three tests that share no code. The next run of the
identical command, with no source change, passed 4293/0. They are load-sensitive flakes,
not defects — recorded so the next person to see one does not spend the afternoon
diagnosing a platform bug that is not there.

> **UPDATE 2026-08-26, and it narrows this file to ONE test.** CI run `32997878934`
> (`ae7db407`, wine 9.0) failed two of the three — but with a **different payload**:
> `{"timed_out":true,…}`, the wine-9.0 hang signature, not the partial-key payload
> described below. Those two (`unfiltered_output_carries_…` and
> `unfiltered_output_line_count_…`) are therefore **group 6 of the lane's skip list**, the
> already-classified wine-9 hang class, and are now skipped there.
>
> **Same two test names, two unrelated causes.** Only the payload separates them: a
> `timed_out` envelope is the wine-9 hang; a partial key set is the flake this file is
> about. That is a genuinely nasty trap — a local reproduction of one looks like a
> reproduction of the other, and the test name is the same in both logs.
>
> What remains in scope here is **one** test:
> `librarian::catalog::tests::run_migrations_is_safe_under_concurrent_connections`
> (`database is locked`), seen once, locally, never on CI.
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


### 2026-09-02 — two more datapoints, and the first measurement separating this from the MSVC lanes

Status unchanged: already `open`, correctly. What is new is job-grain data across four consecutive
`experiments` runs, read from `gh run view <id> --json jobs` rather than the run-level conclusion:

| run | sha | wine | the three MSVC lanes |
|---|---|---|---|
| 33570342471 | `a82026d7` | fail | all fail |
| 33574961971 | `6d89a69b` | fail | all fail |
| 33577436407 | `2d04c6ad` | **success** | all fail |
| 33600281053 | `62d7fa4b` | success | all pass |

**Two of four for this lane — and `2d04c6ad` is the row that earns its keep.** The wine lane passed
while all three `windows-latest` MSVC lanes failed, so the two populations move independently. Until
today the split between this file and
`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md` rested on the *reasoning* that
wine-under-load and MSVC wall-clock are different mechanisms; it now rests on an observation where
one fired and the other did not. Cross-referenced from that file's Evidence section.

**Not a fix and not evidence of one.** Nothing in these lanes changed across the four runs. The
2026-09-02 green is the same output a repaired lane would produce, which is precisely why it settles
nothing — the load conditions this file names as the trigger are a property of the runner, not of
the commit.

**No failure log was read from these runs.** The claim is *this lane still fails intermittently*,
not *it fails on the three tests § Symptom names*. Whether the same three tests are involved is
unverified here, and confirming it would strengthen or split this file — which is the more useful
next step than another pass/fail tally.
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

## Instrumented 2026-08-28 — the `.ok()` § Resume named, and the mechanism pinned

§ *Resume* named one action and ruled out the two that would have needed a judgement call
(retries, raising the SQLite busy timeout — both "actively harmful" on one unreproduced
occurrence). That one action is done, and a second, cheaper thing turned out to be available
alongside it.

**1. The `.ok()` no longer swallows the error.** `src/tools/run_command/output.rs` read the
tee capture with `std::fs::read_to_string(&tmpfile.0).ok()` — the single place where a
transient I/O failure becomes an absent field with no trace. It is now a `match` that logs
the error at `warn` and still returns `None`.

**Behaviour is unchanged, deliberately.** Same `None`, same degradation, no retry, no timeout
change. The only difference is that a second occurrence leaves a record instead of a silence.

**2. The inferred mechanism is now a test** —
`an_unreadable_tee_capture_drops_the_whole_key_group_without_panicking`
(`src/tools/run_command/tests.rs`). This file reasoned about the mechanism from an
impossibility — *"the response could not have come from that block at all"* — but the path
was never exercised. It now is, with an unreadable tee path, asserting the two properties
the reasoning depended on:

- the degradation is **total** — all four of `unfiltered_output`, `unfiltered_output_lines`,
  `unfiltered_truncated`, `unfiltered_buffered_lines` absent. A *partial* group is precisely
  what made the flake read as impossible.
- it does **not** panic, and the rest of the response is untouched.

**Mutation-verified.** Leaking one key outside the group
(`result["unfiltered_output_lines"] = json!(0)` before `Ok(result)`) fails the test naming
`unfiltered_output_lines` as the survivor. Probe reverted.

> **A process note worth keeping, because it nearly produced a false pass.** The first
> mutation attempt used a blind `sed` whose anchor did not match, and a `git checkout` to
> revert — which silently reverted the *fix* instead of the probe. The test then "passed
> against the mutation" while running on unmutated code with the fix removed: a green that
> meant nothing, of exactly the kind this repo's own guidance warns about. The second attempt
> asserted the probe was present *and* the fix was still present before believing the result.
> Verify the mutation landed before trusting that a test survived it.

### What this does and does not change

**Does not close the bug.** Status stays `open`. The root cause is still *not established* —
contention remains a hypothesis that fits, and one local occurrence is not a finding. Nothing
here makes the flake reproducible.

**Does change what the next occurrence costs.** Previously it would have produced the same
unreadable partial response and no evidence. Now the `warn` line names the path and the OS
error, and the test pins the degradation contract so a future refactor cannot quietly turn a
total drop into a partial one.

The `unverified:` note still governs the rest: only
`run_migrations_is_safe_under_concurrent_connections` remains in scope for this file, seen
once, locally, mechanism inferred and still uninstrumented. **Do not add retries or raise the
SQLite busy timeout on this evidence.**

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

- `docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md` — the
  other wine-lane caveat; that one is a version divergence, this one is load, and they are
  not the same thing
- `src/tools/run_command/output.rs:129-131` — the `.ok()` that would hide the I/O error
- `src/librarian/catalog/mod.rs:820` — the concurrent-migration assertion

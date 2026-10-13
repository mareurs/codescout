---
id: '9cfe45615630c0f4'
kind: bug
status: open
title: 'BUG: under dash the own-stderr route the marker recommends reads 0 B when the query ends in a subshell with a redirect, and the test that guards it is red in CI on every run'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
topic: run_command own-stderr marker route
opened: 2026-10-13
severity: medium
unverified: 'Root cause inside dash is not established: only the trigger shape is measured (see Reproduction). The test stops at its first failing row under dash, so which of the other rows also fail there was not examined. No fix attempted, so no regression test exists yet.'
---

# BUG: under dash the own-stderr route the marker recommends reads 0 B when the query ends in a subshell with a redirect, and the test that guards it is red in CI on every run

## Summary

`run_command` tells the caller, when a query's own stderr was cut, to read the rest with `{ QUERY; } 2>&1 >/dev/null | cut -c1-4000`. Production runs every command with `sh -c`. Under dash, if `QUERY` ends in a subshell that carries a redirect, such as `(grep -n "" F) >&2`, that route prints nothing, while the stream holds 63786 B. The test `the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts` asserts exactly this and fails under dash. It passes on a machine whose `sh` is bash, so the failure was invisible locally.

Found 2026-10-13 while answering an external audit report (CI-01). The report listed the public CI as red; this is the single test behind the Linux failures.

## Symptom (Effect)

CI run `38223100655` (2026-10-12), jobs "Feature check (opt-in build configs)" and "Test (server-stack — the build cargo rb ships)":

```
thread 'tools::run_command::tests::the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts' panicked at src/tools/run_command/tests.rs:11120:9:
"(grep -n \"\" {A}) >&2": the wrapped route read 0 B, the stream is 63786 B
```

The same log shows the test skipping rows its `sh` rejects: `"grep -n \"\" {A} |& cat": this sh rejects the syntax, skipped` (also `<(`). That is a shell that is not bash.

## Reproduction

Measured 2026-10-13 with a private Debian dash 0.5.12 (a Codex pilot run kept it; `T-10` of `docs/trackers/codex-pilot-checks-2026-10-13.md`). Same compiled test binary, only `PATH` differs:

- `sh` is bash (`/bin/sh -> bash` on this machine): `87 rows compared, 42 write to stderr`, `1 passed`.
- dash first on `PATH`: `"(grep -n \"\" {A}) >&2": the wrapped route read 0 B, the stream is 63786 B`, `1 failed`, exit 101. The byte count equals CI's.

Trigger narrowed by hand, 2026-10-13, `{ Q; } 2>&1 >/dev/null | wc -c`:

| Q | bash | dash 0.5.12 |
|---|---|---|
| `(echo hi) >&2` | 3 | **0** |
| `(echo hi) 1>&2` | 3 | **0** |
| `(echo hi \| cat) >&2` | 3 | **0** |
| `(echo hi >&2)` | 3 | 3 |
| `echo hi \| (cat) >&2` | 3 | 3 |
| `(echo hi); echo yo >&2` | 3 | 3 |
| `(echo hi >&2) >&2` | 3 | 3 |

Without the parentheses, `{ grep -n "" F >&2; } 2>&1 >/dev/null | wc -c` prints 12 under both. The trigger is a subshell that is the last command of the brace group and has its stdout redirected to fd 2.

## Environment

Linux. CI uses `ubuntu-latest` (`.github/workflows/ci.yml`). Local machine: `/bin/sh -> bash`. The 7 rows above were measured with dash 0.5.12-2 and with this machine's bash.

## Root cause

Established:

- Production spawns `sh -c <cmd>` (`src/platform/unix.rs`, `shell_command_configured`).
- The marker recommends the brace-group route (`OWN_STDERR_STAGE` in `src/tools/run_command/output.rs`).
- Under dash that route loses the stream for the shape above (table).

Not established: why dash drops it. A guess, unverified: the redirect on a subshell that is the group's last command is resolved against different descriptors than in bash. Nobody has read dash's source for this.

Why no local run caught it: the machine's `sh` is bash, and the test's own comment says it expects `sh` to differ for syntax (`|&`, `<(`) but treats the remaining rows as shell-independent.

## Evidence

CI log excerpts above; Codex run files `t10-bash-release.log` and `t10-dash.log` under the pilot directory (a temporary `/tmp` path, so cite the tracker's T-10 result instead).

## Hypotheses tried

1. **Hypothesis:** the failure is flaky or timing-dependent. **Test:** ran the same compiled binary with only `PATH` changed. **Verdict:** rejected. **Evidence:** deterministic pass with bash, deterministic fail with dash.
2. **Hypothesis:** it is the `{A}` buffer substitution, not the shell. **Test:** the `echo hi` rows above use no substitution. **Verdict:** rejected. **Evidence:** the table.

## Fix

Not done. Two directions, not chosen:

- If the product must work where `sh` is dash, the marker must not recommend the route for queries of this shape, or must recommend a form that dash handles (for example without the trailing `>&2` on a subshell).
- If the project only supports `sh` = bash, the test must say so and skip when `sh` is not bash, and `shell_command_configured` should name the shell it needs.

The first is the one the product evidence supports, because the shell that `sh -c` finds is the host's.

## Tests added

None.

## Workarounds

On a host where `sh` is dash, drop the parentheses from a query that ends in `(…) >&2` before using the marker's route.

## Resume

Open. First step: run the failing test with dash first on `PATH`, then collect every failing row instead of stopping at the first. Then choose a direction in Fix. Related: `docs/trackers/codex-pilot-checks-2026-10-13.md` (T-10).

## References

- `src/tools/run_command/tests.rs` (`the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts`)
- `src/tools/run_command/output.rs` (`OWN_STDERR_STAGE`)
- `src/platform/unix.rs` (`shell_command_configured`)
- CI run `38223100655`

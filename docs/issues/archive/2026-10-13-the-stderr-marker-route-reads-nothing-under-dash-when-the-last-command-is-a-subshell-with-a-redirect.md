---
id: e6e7cdc81cf3cba0
kind: bug
status: fixed
title: 'BUG: under dash the own-stderr route the marker recommends reads 0 B when the query ends in a subshell with a redirect, and the test that guards it is red in CI on every run'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
topic: run_command own-stderr marker route
closed: 2026-10-13
opened: 2026-10-13
severity: medium
unverified: 'CLEARED 2026-10-13. Was: the verified fix remained uncommitted with no fix SHA or stable patch-id. It is now committed on experiments as 77d8f40e55a4ffe33a828fc0778543dd9cc6d57b, patch-id 3a155423c92d078651c935054d52578718cd03ca, matching the reviewed patch; the fresh full gate passed. Unmeasured shell scope remains explicit in Tests added and Resume.'
---

# BUG: under dash the own-stderr route the marker recommends reads 0 B when the query ends in a subshell with a redirect, and the test that guards it is red in CI on every run

## Summary

`run_command` tells the caller, when a query's own stderr was cut, to read the rest with `{ QUERY; } 2>&1 >/dev/null | cut -c1-4000`. Production runs every command with `sh -c`. Under dash, if `QUERY` is a lone subshell that carries a redirect, such as `(grep -n "" F) >&2`, that route prints nothing, while the stream holds 63786 B. The test `the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts` asserts exactly this and fails under dash. It passes on a machine whose `sh` is bash, so the failure was invisible locally.

Found 2026-10-13 while answering an external audit report (CI-01). The report listed the public CI as red; this is the single test behind the Linux failures.

## Symptom (Effect)

CI run `38223100655` (2026-10-12), jobs "Feature check (opt-in build configs)" and "Test (server-stack — the build cargo rb ships)":

```
thread 'tools::run_command::tests::the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts' panicked at src/tools/run_command/tests.rs:11120:9:
"(grep -n \"\" {A}) >&2": the wrapped route read 0 B, the stream is 63786 B
```

The same log shows the test skipping rows its `sh` rejects: `"grep -n \"\" {A} |& cat": this sh rejects the syntax, skipped` (also `<(`). That is a shell that is not bash.

## Reproduction
Minimal standalone probe with null stdin and captured stdout/stderr: `(printf 'sample\n') >&2` writes `sample\n` to stderr in both shells. `{ (printf 'sample\n') >&2; } 2>&1 >/dev/null | cut -c1-4000` recovers it under bash and returns no bytes under dash. `( (printf 'sample\n') >&2; ) 2>&1 >/dev/null | cut -c1-4000` recovers it in both.

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

Without the parentheses, `{ grep -n "" F >&2; } 2>&1 >/dev/null | wc -c` prints 12 under both. The trigger is a subshell that is the sole command of the brace group and carries its own redirect. (Corrected 2026-10-14: this sentence first said "last command", but a subshell that is last and not sole is unaffected. See Root cause.)

## Environment

Linux. CI uses `ubuntu-latest` (`.github/workflows/ci.yml`). Local machine: `/bin/sh -> bash`. The 7 rows above were measured with dash 0.5.12-2 and with this machine's bash.

## Root cause

Established:

- Production spawns `sh -c <cmd>` (`src/platform/unix.rs`, `shell_command_configured`).
- Before the fix, the marker recommended the brace-group route (`OWN_STDERR_STAGE` in `src/tools/run_command/output.rs`).
- Under dash that route loses the stream for the shape above (table).

Established 2026-10-14, which replaces the earlier "not established": dash's parser discards the subshell's own redirects. In dash 0.5.12-2 `src/parser.c`, `command()`, `case TBEGIN` returns the inner list node itself (`n1 = list(0)`). After the closing `}`, the `if (redir)` block wraps `n1` in a new `NREDIR` node only when `n1->type != NSUBSHELL`. Otherwise it runs `n1->nredir.redirect = redir`. So when the sole command of a redirected brace group is a subshell, the group's redirects replace the subshell's own list. `{ (cmd) >&2; } 2>&1 >/dev/null` runs as `(cmd) 2>&1 >/dev/null`, and the bytes go to `/dev/null`. `case TLP` allocates a new `NSUBSHELL` node with `redirect = NULL`, so `( QUERY; ) <redirects>` never replaces a list that `QUERY` wrote.

Measured 2026-10-14 with a dash binary that reports no version (behavior consistent with the 0.5.12-2 source) and this machine's bash. Each probe writes to named files: `E` is the subshell's own target and `O` is the group's target.

| Q | bash | dash |
|---|---|---|
| `{ (echo hi) >E; } >O` | E | **O** |
| `{ (echo hi) >&2; } 2>E` | E | **stdout** |
| `{ (echo hi) >&2; } 2>E >O; :` (group not the shell's last command) | E | **O** |
| `{ :; (echo hi) >E; } >O` (subshell last, not sole) | E | E |
| `{ (echo hi) >E && :; } >O` | E | E |
| `{ { (echo hi) >E; }; } >O` (plain inner braces collapse to the subshell node) | E | **O** |
| `( (echo hi) >E; ) >O` | E | E |
| `( { (echo hi) >E; } ) >O` | E | E |

The third row shows that the no-fork exit path is not the cause. The fourth row shows that "last command" was too broad.

Why no local run caught it: the machine's `sh` is bash, and the test's own comment says it expects `sh` to differ for syntax (`|&`, `<(`) but treats the remaining rows as shell-independent.

## Evidence

The CI and baseline pilot observations above are preserved in T-10 of `docs/trackers/codex-pilot-checks-2026-10-13.md`. The disposable pilot directory has been retired. Follow-up literal probes, RED/GREEN logs, applied-mutation review logs, the reviewed source patch and final gate proof are preserved under `.codescout/resident-private/codex-pilot-fix-20261013/`, with `evidence-manifest.json` recording file hashes.

## Hypotheses tried

1. **Hypothesis:** the failure is flaky or timing-dependent. **Test:** ran the same compiled binary with only `PATH` changed. **Verdict:** rejected. **Evidence:** deterministic pass with bash, deterministic fail with dash.
2. **Hypothesis:** it is the `{A}` buffer substitution, not the shell. **Test:** the `echo hi` rows above use no substitution. **Verdict:** rejected. **Evidence:** the table.

## Fix

Emit `( QUERY; ) 2>&1 >/dev/null | cut -c1-4000` rather than the brace group. An outer subshell recovered the literal stream under both bash and dash in standalone probes. Keep the existing accepted-query gate and response budgets. `wrapped_route_in` now substitutes the query into the complete emitted template, so tests execute the marker's actual wrapper and stage.

The independently reviewed source was transferred to the main checkout and both files compared byte-identically with the worktree. The fresh `./scripts/gate.sh --check` passed: Python light=0; FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0 FMT_STATE=passed, including integrations and doc-tests. Root library results: 4476 passed/0 failed/7 ignored under lean; 6757 passed/0 failed/8 ignored under default. **Fix SHA (experiments):** `77d8f40e55a4ffe33a828fc0778543dd9cc6d57b`. **patch-id:** `3a155423c92d078651c935054d52578718cd03ca`, observed with `git show <sha> | git patch-id --stable` and equal to the reviewed source patch's patch-id. A fresh pre-commit full gate also passed all lanes (`gate-commit.log`, `gate-commit-verification.json`).

## Fix provenance

- **SHA:** `77d8f40e55a4ffe33a828fc0778543dd9cc6d57b` (`experiments`)
- **patch-id:** `3a155423c92d078651c935054d52578718cd03ca`

## Tests added

`an_own_stderr_recovery_route_preserves_redirected_subshell_output` executes the real marker for line-only and byte truncation, asserting exact fixture bytes via both the shell oracle and run_command. Its pre-fix dash run failed: empty output versus 30 `payload` lines. After the production fix, all 7 `own_stderr` tests pass under bash and dash; the adversarial comparisons cover 87 rows/42 stderr writers under bash and 83 rows/40 stderr writers under dash, retaining syntax skips.

Independent review applied three production mutations under dash: restoring braces and reversing redirects each failed on missing payload lines; cutting the recovery width from 4000 to 40 failed the 3000-character byte case after the line case passed. Observed 3 killed, 0 survived, 0 inconclusive; the exact reviewed diff was restored after each. The restored dash suite passed 7/7. Durable selected evidence is under `.codescout/resident-private/codex-pilot-fix-20261013/`; the final full gate passed (`gate-final.log`, `gate-final-verification.json`).

## Workarounds

Use `( QUERY; ) 2>&1 >/dev/null | cut -c1-4000` as the outer wrapper. This preserves the measured redirected-subshell stream without changing the query or global shell configuration.

## Resume

Complete. The source fix is committed on experiments as `77d8f40e55a4ffe33a828fc0778543dd9cc6d57b` (stable patch-id `3a155423c92d078651c935054d52578718cd03ca`), matches the reviewed patch and passed the fresh full development gate. The pilot record is done and retained; its disposable environment is removed, with selected hashed evidence preserved. The operator-approved recipe correction remains in the archiving tracker's author's pending file. Windows shell behavior, dash-rejected bash-only syntax and the full dash suite remain outside the measured scope; the normal gate also excludes heavy Python, server-stack and local ONNX weight execution.

## References

- `src/tools/run_command/tests.rs` (`the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts`)
- `src/tools/run_command/output.rs` (`OWN_STDERR_STAGE`)
- `src/platform/unix.rs` (`shell_command_configured`)
- CI run `38223100655`

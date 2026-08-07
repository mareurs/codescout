---
status: mitigated
opened: 2026-08-08
closed: 2026-08-08
severity: high
owner: marius
related: [docs/issues/2026-08-07-msys-pathconv-optout-breaks-native-exe-paths.md]
tags: [windows, process-spawn, ci, wine]
kind: bug
---

# BUG: `run_command` is dead on a Windows host with no Git Bash, and says only "program not found"

## Summary

Since WIN-32 (`d564c9bb`) every Windows command spawns through Git Bash. On a host
without Git for Windows the resolver falls through to the bare name `bash`, so
`CreateProcessW` answers `program not found` — for every single call. The message names
neither the requirement nor the fix, and 22 lib tests in the wine CI job said nothing
more than that.

## Symptom (Effect)

`Windows-gnu cross (MinGW + wine)` on PR #10, run `31220855460`, job `93004908153`:

```
test result: FAILED. 3313 passed; 22 failed; 10 ignored; 0 measured; 12 filtered out

---- platform::windows::tests::msys_form_path_resolves_for_native_binaries stdout ----
git should be spawnable — Git Bash implies a git install: Error { kind: NotFound, message: "program not found" }

---- tools::run_command::tests::execute_shell_command_echo_cross_platform stdout ----
called `Result::unwrap()` on an `Err` value: program not found
```

All 22 share that one message. The other three Windows cells (`windows-latest`,
default / no-features / local-embed) pass — the GitHub Windows runner ships Git.

## Reproduction

Any Windows host without Git for Windows, or the wine CI image as-is:

```
scripts/build-windows.sh test --lib -- --skip <existing wine skips>
```

Through the live MCP surface: unset `CODESCOUT_BASH` / `CLAUDE_CODE_GIT_BASH_PATH`,
uninstall Git for Windows, call `run_command("echo hi")`.

## Environment

`fix/windows-paths-and-doctor` at `dbfd9dbd`, rebased on `origin/experiments` `f244ad17`.
CI: ubuntu-latest running `x86_64-pc-windows-gnu` under wine, MinGW cross-build. Wine has
no Git for Windows installed.

## Root cause

`git_bash_path()` ended its resolution chain with `PathBuf::from("bash")`
(`src/platform/windows.rs`, pre-fix) — a deliberate choice, documented as "so an
unresolvable install surfaces as a spawn error naming the program, rather than a panic at
startup". The reasoning holds for *not panicking*; it does not hold for what the operator
then sees. `program not found` is what the OS says about `bash`, and nothing in the
message connects it to Git for Windows, to `CODESCOUT_BASH`, or to the design decision
that made a POSIX shell mandatory on Windows in the first place.

Measured 2026-08-08: CI job `93004908153`, 22 failures, every one carrying that string and
no other diagnostic.

Second-order: the resolution chain was unreachable in a test. Everything ran through
`std::env::var_os` + `Path::is_file` against the real host, so the not-installed branch
could only be observed by uninstalling git — which is why it shipped unexercised.

## Evidence

### CI log, job 93004908153

```
+ exec cargo test --target x86_64-pc-windows-gnu --lib -- --skip ...
thread 'tools::run_command::tests::run_in_background_returns_bg_handle' panicked at src/tools/run_command/tests.rs:1532:6:
called `Result::unwrap()` on an `Err` value: program not found
```

### The three Windows MSVC cells on the same run

`Test (windows-latest / default)` 6m9s pass, `/ no-features` 4m53s pass,
`/ local-embed` 5m47s pass — same code, host with Git.

## Hypotheses tried

1. **Hypothesis:** a wine path-handling quirk, i.e. WIN-27's class.
   **Test:** read the failure string on all 22 — it is `program not found`, an
   image-resolution failure, not a path failure; and `msys_form_path_resolves_for_native_binaries`
   fails at *spawn*, before any path is passed.
   **Verdict:** rejected.
2. **Hypothesis:** Git Bash is present but shadowed by the System32/WSL exclusion.
   **Test:** the exclusion only skips `%SystemRoot%\System32`; wine has no Git install
   root at all, and the same code passes on `windows-latest`.
   **Verdict:** rejected.
3. **Hypothesis:** the resolver reaches its `bash` fallback and the OS message is the
   whole diagnostic.
   **Verdict:** confirmed — see Root cause.

## Fix

Mitigation, not a root-cause fix: Git Bash remains a hard requirement on Windows, because
the alternative (falling back to `cmd /C`) re-opens exactly what WIN-32 closed — the
security layer tokenizes POSIX while the shell executes something else.

- `resolve_git_bash(env, is_file)` in `src/platform/windows.rs` — the resolution chain,
  now a pure function over an injected environment + existence probe, returning
  `Option<PathBuf>`. `git_bash_path()` caches it; `shell_command_configured` keeps the
  bare-`bash` fallback so nothing panics at startup.
- `shell_unavailable_hint()` in `src/platform/windows.rs`, `src/platform/unix.rs`
  (always `None`), re-exported from `src/platform/mod.rs`.
- `RunCommand::call` in `src/tools/run_command/mod.rs` preflights it and returns a
  `RecoverableError` naming Git for Windows and `CODESCOUT_BASH` before any spawn.
- `.github/workflows/ci.yml` — the 22 wine failures are skipped as environmental, with
  the un-skip protocol (install Git in the image, drop the block wholesale) in the
  comment.

Fix SHA: `experiments` — recorded once the branch merges.

Root-cause close would be installing Git for Windows in the wine image, which retires the
skip block. Until then this stays `mitigated`: the tool now refuses legibly, but a
Git-less Windows host still cannot run commands.

## Tests added

`src/platform/windows.rs`, `mod tests`:

- `resolve_git_bash_is_none_when_nothing_is_installed` — the not-installed branch, via
  the injected probe. This is the branch that shipped unexercised.
- `resolve_git_bash_never_selects_the_wsl_launcher` — a `bash.exe` existing ONLY under
  System32 must still resolve to `None`. Previously undecidable in a test.
- `resolve_git_bash_honours_the_codescout_bash_override` — the escape hatch the new hint
  advertises actually works.

Windows-only (the module is `cfg(windows)`), so they run on the three `windows-latest`
cells and are compiled out elsewhere.

## Workarounds

Install Git for Windows, or point codescout at any bash.exe:

```
CODESCOUT_BASH=C:\path\to\bash.exe
```

`CLAUDE_CODE_GIT_BASH_PATH` is honoured too, so a Claude Code host that already sets it
needs no second variable.

## Resume

N/A for the mitigation. To close at the root: add Git for Windows to the
`Install MinGW + wine` step in `.github/workflows/ci.yml`, then delete the 22-skip block
added here and confirm the cross job goes green.

## References

- `docs/trackers/windows-platform-support.md` — WIN-32 (the Git Bash routing), WIN-36 (this)
- `docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md` — WIN-27, the other wine skip list
- PR https://github.com/mareurs/codescout/pull/10, run `31220855460`

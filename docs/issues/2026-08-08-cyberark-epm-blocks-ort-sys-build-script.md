---
status: open
opened: 2026-08-08
closed:
severity: high
owner: marius
related: [WIN-35]
tags: [windows, cyberark-epm, build, ort, fastembed]
kind: bug
---

# BUG: CyberArk EPM denies execution of the freshly-compiled `ort-sys` build script, blocking any `local-embed*` build

## Summary
Building the main crate with `--features local-embed-dynamic` (or presumably
`local-embed`) fails during compilation, before any test runs: cargo cannot
execute `ort-sys`'s build script. Every other build-script crate in the same
build (windows-sys, tokio, schemars, git2, rusqlite, …) compiles fine — this
is specific to `ort-sys`'s build-script binary.

## Symptom (Effect)
```
error: failed to run custom build command for `ort-sys v2.0.0-rc.11`

Caused by:
  could not execute process `C:\Users\MAILINCA.BRN.002\work\claude\codescout\target\debug\build\ort-sys-ba1f45cadd700188\build-script-main` (never executed)

Caused by:
  Access is denied. (os error 5)
```
Reproduced 3/3 times in a row (`cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx`), identical error each time. Direct invocation of the compiled exe confirms it:
```
$ ./target/debug/build/ort-sys-ba1f45cadd700188/build-script-main.exe
/usr/bin/bash: line 1: ./target/debug/build/ort-sys-ba1f45cadd700188/build-script-main.exe: Permission denied
```
Exit code 126 from bash (permission denied), not "file not found" — the file is present and intact.

## Reproduction
1. `git checkout feat/local-onnx-embedder` at `b3aaf820bfce68f3d2637b493c813298afc0edc3`
2. `cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx`
3. Observe the `ort-sys` build-script failure above.

## Environment
Windows 11 Enterprise 10.0.26200, this VDI, `stable-x86_64-pc-windows-gnu` toolchain, codescout repo, branch `feat/local-onnx-embedder`. `fastembed = "5"` pulling `ort-sys v2.0.0-rc.11`.

## Root cause
CyberArk EPM application control denying execution of a freshly-built,
unsigned `.exe` — the same mechanism as WIN-35 (pyright's uv trampoline) and
the misattributed-to-WIN-18 cases under WIN-32, per
`docs/trackers/windows-platform-support.md` WIN-32/WIN-35 rows: WIN-18
(CrowdStrike) *deletes* the file; CyberArk EPM *denies execution* and leaves
it intact. Here `os error 5` fires at build-script *launch* rather than at
DLL load (the scenario `src/retrieval/local_onnx.rs`'s own hint text
anticipates for `TextEmbedding::try_new_from_user_defined`) — one step
earlier in the pipeline, same policy.

Unconfirmed why `ort-sys`'s build script specifically trips this when other
build scripts in the same `cargo build` (windows-sys, tokio, schemars, git2,
rusqlite — all of which also emit fresh unsigned build-script binaries) do
not: hypothesis is that `ort-sys`'s build script performs an action EPM
flags (e.g. child-process spawn or network fetch, mirroring the WIN-35
trampoline pattern) rather than merely emitting `cargo:` directives, but
this is *not measured* — no process trace was taken.

*Measured 2026-08-08: 3 consecutive `cargo test --features local-embed-dynamic` runs, identical `os error 5` each time; direct `./…/build-script-main.exe` invocation confirms `Permission denied` (exit 126), file present on disk.*

## Evidence
### Consecutive build failures
```
error: failed to run custom build command for `ort-sys v2.0.0-rc.11`
Caused by:
  could not execute process `...\ort-sys-ba1f45cadd700188\build-script-main` (never executed)
Caused by:
  Access is denied. (os error 5)
```
(identical across 3 runs, session 2026-08-08, Task 3 of the local-onnx-embedder plan)

### Direct exe invocation
```
$ ./target/debug/build/ort-sys-ba1f45cadd700188/build-script-main.exe; echo "EXIT:$?"
/usr/bin/bash: line 1: ...build-script-main.exe: Permission denied
EXIT:126
```

## Hypotheses tried
1. **Hypothesis:** Transient AV scan lock on a freshly-written exe, clears on retry.
   **Test:** Re-ran the same `cargo test` command 3 times in a row (~2 min apart, deterministic hash so same binary each time).
   **Verdict:** rejected — identical failure every time, no self-clearing.
2. **Hypothesis:** CyberArk EPM application-control policy denies this exe execution outright (WIN-35 pattern).
   **Test:** Direct invocation of the built exe via `run_command` (Git Bash) — `Permission denied`, exit 126, file intact on disk (`ls -la` shows it present, non-zero size).
   **Verdict:** confirmed as the proximate symptom; the EPM-specific mechanism (vs. some other Windows-side deny, e.g. mismatched exec bit) is inferred from the WIN-32/WIN-35 precedent in `docs/trackers/windows-platform-support.md`, not independently re-verified here (no EPM console access from this session).

## Fix
Not fixable from codescout — this is a host security-policy block, matching the WIN-35 precedent ("Not fixable from codescout — needs a CyberArk policy exception"). Needs a CyberArk EPM policy exception for `ort-sys`'s build-script binary (or for `cargo`-emitted build scripts generally under `target/**/build/*/build-script-main.exe`), analogous to what WIN-35 asks for the uv trampoline.

Status kept `open`, not `mitigated`: no workaround was found in this session (see Workarounds — none available).

## Tests added
N/A — this is a host/tooling block, not a code defect; there is nothing in codescout's own source to regress-test. The `LocalOnnxEmbedder` unit tests (`src/retrieval/local_onnx.rs::tests`) exist and are believed correct per code review against the task-3 brief, but could not be executed on this host to confirm — see the task-3 report for that caveat.

## Workarounds
None found this session. Building on a host without the CyberArk EPM block (or with a policy exception for freshly-built `ort-sys` build scripts) is required to compile any `local-embed` / `local-embed-dynamic` feature build here.

## Resume
Ask for a CyberArk EPM exception (or investigate `ORT_LIB_LOCATION` / a pre-built `ort-sys` binding cache that would let `cargo` skip re-running the build script — check `ort-sys`'s build.rs for an env var that short-circuits execution, e.g. a way to point directly at a prebuilt bindings.rs, avoiding the need to execute the build script at all). Re-run `cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx` once either lands.

## References
- `docs/trackers/windows-platform-support.md` WIN-32, WIN-35 rows (the CyberArk EPM precedent + the WIN-18-vs-CyberArk distinction)
- `docs/superpowers/specs/2026-08-08-local-onnx-embedder-design.md:210` (the `os error 5` → CyberArk EPM lesson this bug reconfirms one build step earlier)
- `.superpowers/sdd/2026-08-08-local-onnx-embedder/task-3-report.md` (this session's full report)

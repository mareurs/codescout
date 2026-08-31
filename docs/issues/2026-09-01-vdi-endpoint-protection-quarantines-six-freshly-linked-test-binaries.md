---
kind: bug
status: mitigated
tags:
- windows
- vdi
- edr
- tests
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
unverified: the exact CrowdStrike detection rule/verdict name is not confirmed (no Falcon console access from this session) — the WIN-18 vs WIN-35 mechanism attribution IS confirmed (file-deletion signature plus direct process confirmation of both CSFalconService and CyberArk EPM's vf_agent running on this host)
---

# BUG: on this VDI, 6 specific freshly-linked `cargo test` binaries are deleted/quarantined before `cargo test` can execute them — reproducible across repeated runs, unrelated to their content passing or failing

## Summary

Running `cargo test --workspace --no-default-features --no-fail-fast` (GNU
toolchain; see
`docs/issues/2026-08-31-vdi-msvc-build-tools-missing-link-exe-shadowed-by-git-coreutils.md`
for why GNU) consistently fails to **execute** exactly 6 of the ~30 built
test binaries, every run, with `Access is denied. (os error 5)`. The other
~24 binaries — including some that are just as "fresh" (linked in the same
`cargo test` invocation, seconds apart) — run fine. This is not a test
logic failure: cargo reports `could not execute process ... (never
executed)`, and the named `.exe` files are **absent from disk** immediately
after, confirmed via `ls`.

## Symptom (Effect)

```
     Running tests\issue_clusters.rs (target-gnu-rebuild\debug\deps\issue_clusters-c2c637f0ad967fad.exe)
error: test failed, to rerun pass `-p codescout --test issue_clusters`

Caused by:
  could not execute process `...\issue_clusters-c2c637f0ad967fad.exe` (never executed)

Caused by:
  Access is denied. (os error 5)
```

Same shape for `link_scan`, `retrieval_e2e`, `retrieval_integration`,
`retrieval_overlap` (all in the `codescout` package), and
`ollama_probe_installs_its_own_crypto_provider` (in `codescout-embed`).

`ls target-gnu-rebuild/debug/deps/issue_clusters-*.exe` immediately after:
`No such file or directory` — same for all 5 other names. `Test-Path` from
native PowerShell confirms the same for `issue_clusters` specifically.
Running a sibling binary built moments apart (`feature_lanes-*.exe`) by
direct invocation succeeds normally, ruling out a blanket exec block.

## Reproduction

```
RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu CARGO_TARGET_DIR=target-gnu-rebuild cargo test --workspace --no-default-features --no-fail-fast
```

Reproduced identically across two full runs several minutes apart on
2026-09-01 — same 6 target names both times, same error shape. Retrying
`cargo build --test issue_clusters` alone confirms the `.exe` DOES exist
immediately after a successful link (`ls` shows it, 8.3 MB); executing it
directly moments later (`target-gnu-rebuild/debug/deps/issue_clusters-*.exe`)
gets `Permission denied` from bash (exit 126), and it is gone from disk on
the next `ls`.

## Environment

Windows 11 Enterprise VDI (`MAILINCA.BRN.002`), `1.97.1-x86_64-pc-windows-gnu`
toolchain, `codescout` repo, `experiments` branch at `4c8a210d` (post-rebase
onto `origin/experiments`).

## Root cause

**CrowdStrike Falcon (WIN-18), not CyberArk EPM.** This VDI runs both
security products — confirmed directly: `Get-Process` shows `CSFalconService`
+ 5 `CSFalconContainer` + `CsFalconUIHost` running, and `Get-Service` shows
`vf_agent` (CyberArk EPM Agent) running too (`pasagent` stopped). The project
already worked out how to tell the two apart by symptom
(`docs/trackers/windows-platform-support.md` WIN-18 vs WIN-35, and
`docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`):

- **CrowdStrike (WIN-18)** *deletes* the freshly-written binary — the file is
  gone afterward.
- **CyberArk EPM (WIN-35)** *denies execution but leaves the file intact* —
  `os error 5` fires at launch, `ls` still shows the file.

This session's evidence is unambiguously the first kind: `cargo build --test
issue_clusters` succeeds, `ls` confirms the `.exe` exists (8,317,568 bytes),
direct execution gets `Permission denied` (bash exit 126), and a subsequent
`ls` shows `No such file or directory`. The file was removed, not merely
blocked from executing — the deletion signature, i.e. WIN-18.

**This is the same class WIN-18 already tracks, not a new mechanism** — this
bug file exists to hold the fuller incident detail per this tracker's own
convention (`docs/trackers/windows-platform-support.md` § *Scope & boundary*:
"a one-line WIN-N row pointing at the bug file... that holds the detail").
See that row for the update reflecting this session's evidence.

**New data point that complicates WIN-18's existing theory.** WIN-18's summary
states the heuristic "targets tiny isolated PEs" (originally observed against
a 3-line hello-world exe) and that "large cargo outputs... survive." The 6
binaries this session lost were NOT tiny — `issue_clusters` alone was 8.3 MB,
comparable to `feature_lanes` (11.4 MB) which survived, built moments apart in
the same `cargo test` invocation. Raw file size alone does not predict which
binaries get quarantined; something about content/behavior (imports,
entropy, code shape) more likely drives the ML heuristic, but this is not
confirmed — no EDR console access from this session to check the actual
detection verdict/rule name.

No single behavioral property cleanly explains the affected set either:
`issue_clusters` and `link_scan` both spawn `git` as a subprocess from
within the test; `retrieval_e2e`/`retrieval_integration`/`retrieval_overlap`
exercise the SQLite-vector retrieval stack; `ollama_probe_installs_its_own_crypto_provider`
installs a rustls crypto provider. No shared mechanism across all 6 was
identified.
## Evidence

### Persistence across retries
Identical 6-target failure list on two separate `cargo test` invocations,
several minutes apart — rules out a transient scan-in-progress race, which
would be expected to clear given enough time.

### File genuinely removed, not merely locked
```
cargo build --test issue_clusters   # succeeds
ls target-gnu-rebuild/debug/deps/issue_clusters-*.exe   # 8317568 bytes, exists
target-gnu-rebuild/debug/deps/issue_clusters-c2c637f0ad967fad.exe   # bash: Permission denied, exit 126
# later:
ls target-gnu-rebuild/debug/deps/issue_clusters-*.exe   # No such file or directory
```

### Not a blanket exec block
```
target-gnu-rebuild/debug/deps/feature_lanes-40a5ed35f2a76877.exe   # runs fine, exit 0, built minutes before issue_clusters
```

## Hypotheses tried

1. **Hypothesis:** Transient AV real-time-scan lock on a freshly written
   PE file; retrying after a delay should succeed.
   **Test:** Re-ran the full `cargo test` suite a second time, several
   minutes later.
   **Verdict:** rejected — identical 6 targets failed identically both
   times.

2. **Hypothesis:** All newly-built binaries in this session are blocked
   (environment-wide lockdown, not per-file).
   **Test:** Ran `feature_lanes-*.exe` directly, built in the same
   `cargo test` invocation as the blocked binaries.
   **Verdict:** rejected — it ran normally.

## Fix

Not applicable — this is VDI-local endpoint-protection behavior, not a
codescout code defect. No code change.

## Tests added

N/A.

## Workarounds

- Use `--no-fail-fast` so the rest of the suite still reports results
  instead of stopping at the first blocked binary.
- Treat these 6 targets as untested on this VDI until IT allow-lists the
  build output directory (`target-gnu-rebuild/` or wherever
  `CARGO_TARGET_DIR` points) with the endpoint-protection product, or until
  run on a machine/CI without this restriction. GitHub Actions CI is
  unaffected (these are ordinary integration tests with no special CI
  handling needed).

## Resume

If this VDI's endpoint protection console is ever accessible, check its
quarantine/detection log for these 6 binary names around this bug's
timestamp to confirm the mechanism definitively (currently inferred, not
observed directly).

## References

- `docs/trackers/windows-platform-support.md` WIN-18 — the tracked mechanism
  this bug instantiates (CrowdStrike deletes freshly-built unsigned PEs);
  this file holds the fuller incident detail per that tracker's own
  convention. WIN-35 is the *different* mechanism (CyberArk EPM denies but
  keeps the file) this bug is explicitly NOT an instance of.
- `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
  — the CyberArk EPM case this bug was initially (incorrectly) compared to;
  kept as a reference for the distinguishing test (file present vs. absent
  after the failure) that separates the two mechanisms.
- `docs/issues/2026-08-31-vdi-msvc-build-tools-missing-link-exe-shadowed-by-git-coreutils.md`
  — the related but distinct build-time (not execution-time) VDI toolchain gap.

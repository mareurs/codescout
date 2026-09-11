---
kind: bug
status: fixed
tags:
- windows
- wine
- rendezvous
- tests
- cluster/repro-env-diverges-from-gate-env
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related: []
severity: low
unverified: whether CI's windows-gnu-cross-under-wine job actually needs ppid==0 (the reason the split was presumably introduced) was not re-verified against CI itself — no CI access from this session. See Resume.
---

# BUG: `tools::rendezvous::tests::publish_records_a_zero_parent_pid_on_windows_by_design` asserted `ppid == 0` as "Windows by design", contradicting the real `ToolHelp32Snapshot`-based implementation and an earlier fix's own regression test

## Summary

`src/tools/rendezvous.rs` had two PPID tests: a `#[cfg(unix)]` one asserting
`ppid != 0`, and a `#[cfg(windows)]` one asserting `ppid == 0` with a doc
comment claiming "Windows has no getppid here... 0 is deliberate." That claim
is false on real Windows: `parent_pid()`'s Windows branch does a real
`CreateToolhelp32Snapshot` walk and resolves a genuine PPID, as already
fixed and verified in
`docs/issues/archive/2026-08-19-rendezvous-parent-pid-stub-returns-zero-on-windows.md`.
Running the suite natively on this VDI reproduced the contradiction directly:
the "by design" test failed because the real implementation returned a real
PID (16516), not 0.

## Symptom (Effect)

```
thread 'tools::rendezvous::tests::publish_records_a_zero_parent_pid_on_windows_by_design' (26768) panicked at src\tools\rendezvous.rs:379:9:
assertion `left == right` failed: Windows has no getppid here; 0 is deliberate and means 'never matched', which is the safe direction versus a wrong match
  left: 16516
 right: 0
```

## Reproduction

```
git rev-parse HEAD   # aa242912, branch experiments (before this fix)
RUSTUP_TOOLCHAIN=1.97.1-x86_64-pc-windows-gnu CARGO_TARGET_DIR=target-gnu-rebuild cargo test --workspace --no-default-features
```

Any native-Windows run of `tools::rendezvous::tests` reproduces it.

## Environment

Windows 11 Enterprise VDI, rustc 1.97.1, `x86_64-pc-windows-gnu` host
toolchain (see
`docs/issues/2026-08-31-vdi-msvc-build-tools-missing-link-exe-shadowed-by-git-coreutils.md`
for why gnu instead of msvc on this machine). `codescout` repo, `experiments`
branch at `aa242912`.

## Root cause

`docs/issues/archive/2026-08-19-rendezvous-parent-pid-stub-returns-zero-on-windows.md`
fixed the original "Windows PPID is a hardcoded 0 stub" bug by implementing a
real `CreateToolhelp32Snapshot` + `Process32FirstW`/`Process32NextW` walk
(`src/tools/rendezvous.rs:185-226`), and its own regression test —
`publish_records_the_parent_pid_the_hook_matches_on`, asserting `ppid != 0` —
was verified passing **unconditionally on Windows** on 2026-08-19, on this
same `1.97.1-x86_64-pc-windows-gnu` toolchain.

Sometime after that fix, that test was narrowed to `#[cfg(unix)]` and a new
`#[cfg(windows)]` sibling (`publish_records_a_zero_parent_pid_on_windows_by_design`)
was added asserting `ppid == 0`, with a comment claiming this was "Windows by
design" and that the unconditional assertion "failed on all three lanes."
That is inconsistent with the 2026-08-19 fix's own verified result on the
identical target triple — the likely explanation (not confirmed from this
session, which has no CI access) is that CI's `windows-gnu` job
(`.github/workflows/ci.yml:260`, "Windows-gnu cross (MinGW + wine)") runs
under **wine on `ubuntu-latest`**, not on real Windows, and wine's
`CreateToolhelp32Snapshot` may not enumerate the host process tree the same
way real Windows does — falling into the `snapshot == INVALID_HANDLE_VALUE`
→ `return 0` branch that's still in `parent_pid()` for exactly this kind of
failure. Whoever added the split likely generalized a wine-CI-specific `0`
into a false "Windows has no getppid" claim, not realizing the two Windows
environments (real hardware vs. wine emulation) diverge here.

*Measured 2026-08-31: real Windows (this VDI, native — not wine) resolves a
genuine nonzero ppid via the existing `ToolHelp32Snapshot` implementation.
Wine's behavior was NOT independently re-verified this session — no CI
access — so whether the split was ever actually necessary for CI is still
open. See Resume.*

## Evidence

### `parent_pid()`, Windows branch (unchanged by this fix, already correct)
`src/tools/rendezvous.rs:185-226` — see the 2026-08-19 bug file's Fix section
for the full listing; confirmed unchanged and still present.

### The contradiction, side by side
- 2026-08-19 fix bug, `## Tests added`: *"Verified 2026-08-19 (Windows,
  `1.97.1-x86_64-pc-windows-gnu`...): `test
  tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on
  ... ok`"* — unconditional, passing on Windows.
- Pre-fix code (this bug, before today): that same test narrowed to
  `#[cfg(unix)]`, plus a `#[cfg(windows)]` sibling asserting the opposite
  value, with a comment asserting the exact stub behavior the 2026-08-19 fix
  replaced.

## Hypotheses tried

1. **Hypothesis:** The `16516` observed on this run is a process-nesting
   artifact of the `run_command` → bash → cargo → test-binary spawn chain
   (same hypothesis the original 2026-08-19 bug considered for its `0`).
   **Test:** Read `parent_pid()`'s Windows implementation directly — it walks
   `PROCESSENTRY32W` entries for the CURRENT test binary's own pid
   (`std::process::id()`), not any wrapper process, so nesting depth is
   irrelevant to what it resolves.
   **Verdict:** rejected — the value is a real parent pid of the test binary
   process itself, consistent with the already-fixed implementation working
   as designed.

## Fix

`src/tools/rendezvous.rs`:

- Removed `publish_records_a_zero_parent_pid_on_windows_by_design` (the
  test asserting the false "Windows is always 0" contract).
- Un-narrowed `publish_records_the_parent_pid_the_hook_matches_on` back to
  running on both platforms (`#[cfg(unix)]` and `#[cfg(windows)]` blocks
  inside one function, each asserting `ppid != 0` with its own message),
  matching what the 2026-08-19 fix originally verified. The comment now
  states the real reason wine may differ (untested from here) instead of
  the false "Windows has no getppid" claim.

Deliberately did NOT touch `parent_pid()`'s Windows implementation itself —
it was already correct; only the test's claim about it was wrong.

- **SHA (experiments):** `db1c038e41ab8b79ca36bf81c4dd411651d827c2`
- **patch-id:** `7fa93fc9b1010da50ccd012ab3b15438e3b89d51`

## Tests added

No new test — `publish_records_the_parent_pid_the_hook_matches_on` now
covers both platforms again, which is the regression coverage (a future
"fix" that reintroduces a Windows-always-0 stub would fail this test on any
native-Windows run, same as it did here).

## Workarounds

None needed once fixed.

## Resume

**Open question, not resolved by this fix:** was the unix/windows split ever
actually required to keep CI's `windows-gnu` (wine) job green? This session
has no CI access to check. Before considering this fully closed:

1. Push this fix and watch the `windows-gnu` CI job
   (`.github/workflows/ci.yml:260`) specifically.
2. If it fails there with `ppid == 0` under wine, that confirms the
   wine-specific-failure hypothesis in Root Cause — the right fix at that
   point is to detect wine at runtime (or gate on an env var CI sets) and
   accept 0 *only* in that specific case, with a comment saying so
   precisely, rather than re-broadening to "Windows" generally.
3. If it passes, the original split was unnecessary and this fix is fully
   correct as-is.

## References

- `docs/issues/archive/2026-08-19-rendezvous-parent-pid-stub-returns-zero-on-windows.md`
  — the original fix this bug's test contradicted.
- `.github/workflows/ci.yml:260` — the `windows-gnu` (wine) CI job whose
  behavior motivated (as best as can be inferred) the incorrect split.
- `docs/issues/2026-08-31-vdi-msvc-build-tools-missing-link-exe-shadowed-by-git-coreutils.md`
  — why this VDI is on the gnu toolchain, which is what let this run at all.


## Fix provenance

- **SHA:** `3a70166c` (`3a70166cf0ca29c4f416e4ac0327bd0614b87d03`, on `origin/experiments`) —
  positional; does not survive a rebase of `experiments`. **This one already did not.**
- **patch-id:** `7fa93fc9b1010da50ccd012ab3b15438e3b89d51` — content hash of the diff; survives
  rebase and cherry-pick.

**The SHA here was RECOVERED from the patch-id on 2026-09-11, and that is worth recording
because it is this corpus's rule paying off rather than predicting.** § *Fix* above declared
`db1c038e41ab8b79ca36bf81c4dd411651d827c2`, which no longer resolves — `git cat-file -e` fails
on it, orphaned by a rebase of `experiments` exactly as `CLAUDE.md` § *Bug Tracking* warns. The
patch-id beside it did not decay, and recovery took one pass over 6044 indexed patch-ids:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 7fa93fc9b1010da5 /tmp/patch-ids.txt
```

Redirects, not pipes — Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer.

**Verified, not merely resolved.** `3a70166c` is *fix(windows): correct a stale rendezvous PPID
test and a missing cfg arm found on native VDI runs*, and its diff removes
`publish_records_a_zero_parent_pid_on_windows_by_design` — which is precisely what § *Fix*
claims was done. The same commit also filed this bug file and its two VDI siblings, which is why
those two carry `no_fix_commit:` rather than a SHA.

**The prose hash `aa242912` is not a fix anchor and does not resolve either** — doctor's
`terminal_status_without_fix_anchor` flagged this file partly because that hash *reads* as
provenance to anyone scanning for one. It is left in place where it sits; this section is the
anchor.

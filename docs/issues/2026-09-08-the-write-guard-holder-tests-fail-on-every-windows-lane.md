---
kind: bug
status: fixed
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-08
opened: 2026-09-08
owner: marius
related:
- docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md
- docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md
severity: medium
---

# BUG: the three write-guard holder tests fail on every Windows lane, and they are now the only thing keeping `experiments` red

## Summary

`agent::write_guard::tests` fails three tests on `windows-latest`, in all three Windows
configurations (`default`, `local-embed`, `no-features`):

```
a_contended_acquire_names_the_holder_not_merely_that_someone_holds_it
releasing_the_lock_clears_the_holder_record
the_refusal_reports_elapsed_hold_and_promises_no_deadline
```

They pass on `ubuntu-latest` and `macos-latest`, and in both local gate lanes.

**These three are now the entire red.** At `2ad4490c` the run is **14 jobs green, 4 red**, and
every red is Windows-family: the three lanes above plus `Windows-gnu cross (MinGW + wine)`.
Every Linux and macOS lane — including `server-stack`, the build `cargo rb` ships — is green for
the first time since 09-06T17:35.

## Symptom (Effect)

`test result: FAILED. 5139 passed; 3 failed` on `windows-latest / default`.

**A second-order effect worth naming:** `cargo test` stops after a failing test binary, so the
lib failure means the Windows lanes **never reach the integration tests at all**. Verified at
`2ad4490c` — `tests/embedder_env_isolation.rs` appears in the ubuntu job log
(`Running tests/embedder_env_isolation.rs`) and **zero** times in the Windows one. So every
`tests/*.rs` guard in this repo is currently unexercised on Windows, and nothing says so.

## Environment

`windows-latest`. Not reproducible on this machine (Linux) — see *cluster*.

## Root cause

**The holder record lived INSIDE the locked file, and Windows' lock is mandatory.**

`acquire` wrote `<epoch_ms>\t<holder>` into `.codescout/write.lock` — the file it had just
flocked — and a contender read it back through **its own** handle. `flock(2)` is *advisory*, so
on Unix that second handle reads freely; `LockFileEx` is *mandatory*, so on Windows it fails with
`Os { code: 33 }`, *"another process has locked a portion of the file"*.

One fact, all three failures — which is why they always failed together:

| test | how it surfaced |
|---|---|
| `releasing_the_lock_clears_the_holder_record` | `read_to_string(&lock).unwrap()` panicked on error 33 at `:344` |
| `a_contended_acquire_names_the_holder…` | the read returned `Err` → `None` → the anonymous branch |
| `the_refusal_reports_elapsed_hold…` | same `None`, same branch |

**This was a PRODUCTION defect, not a test artifact, and that is the part worth stopping on.**
The record exists so a *refused* party can learn who holds the lock and message them — the whole
point of the bug that introduced it. On Windows that read could never succeed, so every refusal
reported *"The holder recorded no identity"* regardless of who held it. The feature was inert on
a third of the matrix from the day it shipped, and the three red tests were the only thing
saying so.

**How it shipped.** The tests came from
`docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md`
(`7f023c2ec0ae7856`, status `fixed`), whose *Tests added* section asserts they are *"all reached
in **both** gate lanes (the file is under `src/agent/`, so the lean lane is not vacuous for
it)"*. True, carefully derived, and insufficient: **both gate lanes are Linux.** The author
reasoned about the feature axis and the platform axis never entered the sentence — so this class
was entered by someone actively checking their own coverage, which is harder to catch than
forgetting to.

## Evidence

- `2ad4490c` job matrix: 14 success, 4 failure, all Windows-family.
- Identical three names at `999553cf` (before the embedder fix), where they sat alongside the
  embedder failures. **Pre-existing, not introduced by `ab33f4ff`** — that commit's diff touches
  nothing under `src/agent/`.
- `999553cf` windows/default failed 8+ tests; `2ad4490c` fails 3. The embedder fix cleared the
  Windows embedder failures too.

## Hypotheses tried

- *"The new `tests/embedder_env_isolation.rs` guard broke Windows."* **Refuted** — it does not
  appear in the Windows failure list, and per *Symptom* it never runs there at all.

## Fix

**Fixed 2026-09-08 on `experiments` — `d2900ecb`, patch-id
`071904e3879455b0a8aa7957f9db8240e4fa3b91`. The holder record moved to a sidecar file outside
the lock.**

**VERIFIED IN CI, which is the only instrument that could do it.** At `d2900ecb` all three
`Test (windows-latest / *)` lanes went **red → green** (17 of 18 jobs green; the only remaining
red was `Windows-gnu cross`, an unrelated `apt` failure at setup). The Fix section below was
written while this was still *reasoned rather than demonstrated*; it is now demonstrated.

`holder_record_path(root)` → `.codescout/write.lock.holder`. `write.lock` is now only ever
locked and never written; the record is read and written with plain `std::fs` calls on a file
nobody locks, so it behaves identically under advisory and mandatory locking.

**The correctness argument is unchanged, deliberately.** The record is still written *after* the
flock is taken and cleared *before* it is released, so the only process that can have written it
is the one holding the lock. The ordering was always the guarantee and the location never was,
which is why moving the bytes costs nothing.

Ripple: `acquire` takes a `holder_path: PathBuf`; `WriteGuard` carries it and clears it on drop;
`src/server.rs` passes `holder_record_path(&p.root)` from the same `with_project_at` closure that
already resolves the pinned project's lock — so the sidecar inherits the same regime-3 pinning as
the lock it accompanies, rather than acquiring a second, subtly different notion of "the project".

**`.gitignore` needed its own line.** The existing `.codescout/write.lock` rule is an exact match,
not a prefix, so `write.lock.holder` slipped past it and would have been committed.

**Degradation under a concurrent read is safe and unchanged:** a torn read fails to parse →
`None` → the anonymous branch, which reports having no identity rather than inventing one.

**Verification, with its limit stated.** All 7 `agent::write_guard::tests` pass locally, and the
three formerly-failing ones now exercise the cross-handle read on *every* platform rather than
only where it was already safe. **But this fix cannot be verified on Linux** — the failure it
removes is unreproducible here by construction, which is the same property that let the defect
ship. The only real verification is CI's `Test (windows-latest / *)`; until that lane is read,
the fix is reasoned rather than demonstrated.

## Tests added

None. What is owed alongside a fix is a signal for the second-order effect: nothing currently
reports that the Windows lanes stop before the integration tests, so a `tests/*.rs` guard can be
green in every readable lane and unexercised on Windows indefinitely.

## References

- `src/agent/write_guard.rs` — the three tests
- `docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` — added them
- `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md` — the prior
  instance of this shape, nine tests, archived `fixed`
- `.github/workflows/ci.yml:218` — `cargo test --workspace ${{ matrix.config.flags }}`

---
status: mitigated
opened: 2026-08-26
closed: 2026-08-26
severity: medium
owner: marius
related: []
tags: [ci, windows, wine, cross-compile, test-environment]
unverified: 'Mitigated, not fixed: one test is skipped on the wine lane rather than the version gap being closed. The durable fix (pin the lane to a WineHQ build matching the local loop) is designed and argued below but NOT implemented or measured. Also unverified: 19 of the 31 skip-list entries were measured stale under wine 11.16 and are deliberately LEFT IN PLACE, because that measurement does not transfer to wine 9.0 — the whole point of this bug. Nobody should remove them citing this file.'
kind: bug
---

# BUG: the windows-gnu lane runs wine 9.0 while the local loop runs wine 11.16, and the two disagree

## Summary

`scripts/build-windows.sh` exists so Windows failures can be reproduced locally instead of
through CI round-trips. That only works while the two wines behave the same. They do not:
ubuntu-latest packages **wine 9.0**, a current dev box runs **wine 11.16**, and in a single
day the gap produced two divergences — one that cost a CI cycle to find and one that is
still costing a skipped test.

## Symptom (Effect)

`heredoc_body_pipes_are_not_rewritten_into_the_written_file` fails only on the
`Windows-gnu cross (MinGW + wine)` lane. Run `32970579826`:

```
assertion `left == right` failed: the heredoc write must land before the read means
anything: {"timed_out":true,"stderr":"Command timed out after 30 seconds",
"exit_code":null,"hint":"Command exceeded 30s. If it launches background processes …"}
  left: Null
 right: 0
```

The same test passes on all three `windows-latest` lanes, and locally under wine 11.16 the
whole file completes in 0.58s.

**It did not look like this at first.** Before the test checked its write, the only output
was:

```
the heredoc body must land byte-for-byte:
```

— an empty string, which reads identically whether the write was refused, failed, timed
out, or wrote elsewhere. That message sent the first investigation toward heredoc
corruption. The timeout only became visible once the write's `exit_code` was asserted
(`7d8bfae7`).

## Reproduction

Deterministic on the CI runner; **not reproducible locally**, which is the bug.

```
# CI: any push touching this lane. Local, for contrast:
CODESCOUT_BASH='Z:\<portable-git>\bin\bash.exe' \
  scripts/build-windows.sh test --lib -- heredoc_body_pipes
# → ok. 1 passed; finished in 0.58s
```

## Environment

- CI: `ubuntu-latest`, `apt-get install wine wine64 wine32:i386` → `9.0~repack-4build3`
  (read from run `32961510592`'s apt log).
- Local: `wine --version` → `wine-11.16`.
- Both: PortableGit 2.55.0.5 extracted with `7z x`, `CODESCOUT_BASH` pointing at a `Z:`
  path. The Git side is pinned; only wine is not.

## Root cause

The lane installs whatever wine the base image's apt repository carries, and never records
or pins the version. Nothing in the workflow, the script header, or the failure output
names it, so a divergence presents as a codescout defect rather than as a version gap.

Measured 2026-08-26, two independent instances:

1. **The shell.** 8 tests died on `no POSIX shell available` because wine ships no bash.
   Fixed by supplying PortableGit (`ba046b9c`) — 7 of the 8 then passed.
2. **The heredoc hang.** The 8th still fails: the write times out after 30s on wine 9.0 and
   completes in under a second on wine 11.16. Same commit, same Git Bash, same
   `CODESCOUT_BASH` shape.

A third, smaller instance is visible in the same measurement: under wine, every `bash.exe`
invocation emits a 6-line `Cygwin WARNING: Couldn't compute FAST_CWD pointer` block on
stderr, which consumes 6 of `run_command`'s 100-line display budget and shifts
`buffer_query_truncation_hint_shows_next_page`'s expected next-page offset from 101 to 95.
That one is an emulator artifact at any wine version and is a separate, permanent skip.

## Evidence

With **zero** skips, the local wine 11.16 lane fails 12 tests, not 31. The current skip
list has 31 entries, so **19 name tests that now pass** — `symbols_overview_glob_marks_grammarless`
and `msys_form_path_resolves_for_native_binaries` among them, both in the block the
workflow attributes to a wine-broken glob walk rather than to the shell.

That number is recorded here and **deliberately not acted on.** It was measured under
wine 11.16, and this bug is precisely that wine 11.16 measurements do not transfer to the
CI lane. Removing 19 skips on local evidence would be the same mistake this file exists to
document, one lane wider.

## Hypotheses tried

1. **Heredoc pipe-rewrite corruption regressed** — the shape the original failure message
   implied. **Test:** assert the write's exit code before reading. **Verdict:** rejected;
   the write never completed, so nothing was corrupted.
2. **Test parallelism / shared cwd inside the binary.** **Test:** ran the full local wine
   suite with CI's exact skip list — 4249 passed, 0 failed. **Verdict:** rejected.
3. **An environment difference between the two wines.** **Test:** read the wine version
   from both. **Verdict:** confirmed — 9.0 vs 11.16.

## Fix

**Mitigation taken:** the test is skipped on the wine lane only, with the measured reason
and an un-skip protocol in `.github/workflows/ci.yml`. Its coverage is not lost — it runs
and passes on all three `windows-latest` lanes, which is where a Windows regression guard
belongs; the wine lane is a fast proxy, not the source of truth.

**The durable fix, designed and not taken:** pin this lane's wine to a WineHQ build
matching the local loop (add the WineHQ apt repository and install a pinned
`winehq-stable`), then drop the skip and re-measure the other 19. Not done here for two
reasons worth stating rather than leaving implicit. It swaps a distro-packaged wine for a
third-party repo on every run of this lane, which is a stability trade the lane's owner
should make deliberately rather than as a rider on a flake fix. And it wants its own CI
cycle to measure, because the honest expectation is that changing wine versions moves the
failure set in *both* directions.

- **SHA:** `906b8fe3` (branch `experiments`) — the mitigation
- **patch-id:** `fa06e8d8fac2acbc90a3b594541f2308f9cd45d9`

## Tests added

None, and none is possible from this repo: the defect is a property of the CI runner's
package set, not of any code path. The guard is the workflow comment, which now names the
version, the measurement, and the un-skip protocol — and warns against the wrong diagnosis
the original symptom invites.

## Workarounds

For anyone reproducing a Windows failure locally: a green local wine run is **not** a green
`windows-gnu` lane, and vice versa. `scripts/build-windows.sh`'s header already says a green
wine run is not a green `windows-latest`; this adds that it is not even a green wine lane.
Check `wine --version` before trusting a local result, and prefer `windows-latest` as the
arbiter.

## Resume

N/A — mitigated. Reopen by implementing the WineHQ pin above.

## References

- CI run `32961510592` — the shell fix; 8 → 1 on this lane
- CI run `32970579826` — the timeout, made visible by `7d8bfae7`
- `docs/issues/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md` — the
  Windows sweep this lane was built to serve
- `scripts/build-windows.sh` — the local loop whose fidelity this bug bounds

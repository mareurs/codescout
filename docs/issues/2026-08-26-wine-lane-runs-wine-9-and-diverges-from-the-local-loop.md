---
status: mitigated
opened: 2026-08-26
closed: 2026-08-26
severity: medium
owner: marius
related: []
tags: [ci, windows, wine, cross-compile, test-environment]
unverified: "Mitigated, not fixed: TWO tests (the heredoc write and the yes|head overflow) are skipped on the wine lane rather than the version gap being closed. Both hang identically under wine 9.0 and pass under 11.16, so they share a cause and a remedy. The durable fix — pin the lane to a WineHQ build matching the local loop — is designed and argued below but NOT implemented or measured; the honest expectation is that changing wine versions moves the failure set in BOTH directions. Skip list is now 8 entries, each classified, down from 32."
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
2. **The hangs — two of them, one cause.** Measured separately, from two CI logs, and they
   produce the *identical* envelope:

   ```
   {"timed_out":true,"stderr":"Command timed out after 30 seconds","exit_code":null}
   ```

   | test | command | run |
   |---|---|---|
   | `heredoc_..._written_file` | `cat > note.txt <<'EOF' … EOF` | `32970579826` |
   | `run_command_with_overflow_..._once` | `yes filler \| head -2000` | `32976889055` |

   What they share is a **forked pipeline or redirection under MSYS**, where a child has to
   be reaped and a handle closed before `run_command`'s `output()` can observe EOF — which
   is precisely what `run_command`'s own timeout hint describes ("leaves background
   processes holding the stdout pipe open, so `output()` never gets EOF"). Both finish in
   well under a second on wine 11.16, same commit, same Git Bash, same `CODESCOUT_BASH`.

   The second one is the load-bearing datapoint: it arrived as the **1 of 22** that did not
   transfer when the stale skips were dropped, so it was found by a measurement rather than
   by looking for it, and it independently reproduced the first one's mechanism.

A third, smaller instance is visible in the same measurement: under wine, every `bash.exe`
invocation emits a 6-line `Cygwin WARNING: Couldn't compute FAST_CWD pointer` block on
stderr, which consumes 6 of `run_command`'s 100-line display budget and shifts
`buffer_query_truncation_hint_shows_next_page`'s expected next-page offset from 101 to 95.
That one is an emulator artifact at any wine version and is a separate, permanent skip.

## Evidence

With **zero** skips, the local wine 11.16 lane fails 12 tests, not 31.

**Corrected 2026-08-26.** This section first said "19 name tests that now pass". That was
arithmetic done by hand and it was wrong; the measured figure is **22**. Determined by
running the lane with a candidate 10-entry list rather than by subtraction — 4280 passed,
0 failed — so 22 of the 32 entries then present named tests that pass. The discrepancy came
from `symbols_path_type`, a single entry that matches four test names by substring, which
makes entry-count and test-count arithmetic diverge exactly where it is easiest not to
notice.

The 10 that remain are classified per entry in `.github/workflows/ci.yml`; two of them were
never triaged before this pass. `format_compact_live_renders_claude_md_as_map_shape` turns
out to be permanent for any cross-compiled lane — the test reads the repo's real `CLAUDE.md`
via `env!("CARGO_MANIFEST_DIR")`, which bakes in the *build* machine's path, so on a
Linux-hosted cross-build `Path::join` under Windows semantics yields
`/home/…/codescout\CLAUDE.md`, a hybrid with no drive letter. And three others
(`activate_populates_head_sha`, `check_index_scope_respects_gitignore`,
`reindex_backfills_commits_table`) fail on `program not found` because they invoke `git`
directly rather than through Git Bash, so `CODESCOUT_BASH` never reaches them — a concrete,
cheap un-skip protocol that is now written down and not yet attempted.

**The stale entries were removed in this pass.** The reasoning that previously withheld
them — a wine-11 measurement does not transfer to a wine-9 lane — was re-costed once the
lane had a **green** baseline on CI (run `32972195661`): with a known-good tip, a red run
names exactly which entries wine 9.0 still needs, which is the CI-side measurement this
file said was missing. That is the whole cost, and it buys back the gnu-ABI coverage the
stale entries were silently withholding.
## Hypotheses tried

1. **Heredoc pipe-rewrite corruption regressed** — the shape the original failure message
   implied. **Test:** assert the write's exit code before reading. **Verdict:** rejected;
   the write never completed, so nothing was corrupted.
2. **Test parallelism / shared cwd inside the binary.** **Test:** ran the full local wine
   suite with CI's exact skip list — 4249 passed, 0 failed. **Verdict:** rejected.
3. **An environment difference between the two wines.** **Test:** read the wine version
   from both. **Verdict:** confirmed — 9.0 vs 11.16.

## Fix

**Mitigation taken:** both hanging tests are skipped on the wine lane only, with the
measured reason and a shared un-skip protocol in `.github/workflows/ci.yml`. Their coverage
is not lost — both run and pass on all three `windows-latest` lanes, which is where a
Windows regression guard belongs; the wine lane is a fast proxy, not the source of truth.

Two things were *not* done, and both would have been wrong. Neither test was edited to
accommodate the emulator — that would disarm it on the platform it exists for. And neither
got a raised `timeout_secs`, which is the obvious-looking knob and the wrong one: a hang is
not slowness, so a larger timeout buys nothing but wall-clock before the same failure.

**Coverage recovered in the same pass**, so the skip list shrank while this bug was being
written: 32 entries → 8. Twenty-two were stale, and three more (`activate_populates_head_sha`,
`check_index_scope_respects_gitignore`, `reindex_backfills_commits_table`) fell to one env
var — they invoke `git` directly rather than through Git Bash, so `CODESCOUT_BASH` never
reached them and `WINEPATH` pointing at PortableGit's `cmd/` does.

**The durable fix, designed and not taken:** pin this lane's wine to a WineHQ build
matching the local loop (add the WineHQ apt repository and install a pinned
`winehq-stable`), then drop the heredoc skip and re-measure. (The other 22 stale entries no
longer wait on that — they were dropped separately once a green baseline existed; see
Evidence.) Not done here for two reasons worth stating rather than leaving implicit. It swaps a distro-packaged wine for a
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
- `docs/issues/archive/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md` —
  the Windows sweep this lane was built to serve (archived 2026-08-26: all six groups
  fixed, verified on all three `windows-latest` lanes in run `32970579826`)
- `scripts/build-windows.sh` — the local loop whose fidelity this bug bounds

---
id: c318a9960bc52f54
kind: bug
status: archived
title: 'BUG: the wine lane dies at setup on winehq-devel''s unmet wine-devel dependency, with the pin published and the index stable'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-24
opened: 2026-09-24
severity: medium
---

# BUG: the wine lane dies at setup on `winehq-devel : Depends: wine-devel (= 11.17~noble-1)`, with the pin published and the index stable

## Summary

The `Windows-gnu cross (MinGW + wine)` CI job intermittently fails before building anything. `apt-get install winehq-devel=11.17~noble-1` exits 100 with `winehq-devel : Depends: wine-devel (= 11.17~noble-1)` / "held broken packages". The latest instance is today's run. Both causes the step's own comment names are ruled out for that run: the pinned version **is** published for amd64 and i386, and WineHQ's index had not changed for more than four days. The real cause, measured 2026-09-24: the lane pinned only the metapackage, and apt fills the other three exact-version levels with the NEWEST release. So the pin breaks whenever WineHQ has published past it. It is reproducible in any clean container and fixed in `87d90d8c`.

## Symptom (Effect)

Run `35950893889` (2026-09-24T03:18Z, `f918548c`), job `107478995627`, step `Install MinGW + wine`:

```
The following packages have unmet dependencies:
 winehq-devel : Depends: wine-devel (= 11.17~noble-1)
E: Unable to correct problems, you have held broken packages.
##[error]Process completed with exit code 100.
```

The lane never reaches a test, so it gives no coverage signal at all; a red here says nothing about codescout.

## Reproduction

**Reproduced 2026-09-24, first try, in a CLEAN `ubuntu:24.04` container**, with the same `dpkg --add-architecture i386`, the same WineHQ sources and the same pin. The lane's command exits 100 with the identical error. The `HEAD` step was also run verbatim (extracted from `ci.yml`) with a no-op `sudo` shim: `STEP_EXIT=100`, same lines. So this is **not** intermittent per runner image, and **not** runner state.

The earlier "51 of 82 failed wine jobs" pattern (09-06 ×26, 09-07 ×17, 09-08 ×6) and today's three-in-a-row are the same mechanism at two pins. See Root cause.

## Environment

`ubuntu-latest` (noble) GitHub runner. apt sources: the Azure Ubuntu mirror (`azure.archive.ubuntu.com`, with i386 enabled by `dpkg --add-architecture i386`), `packages.microsoft.com`, `dl.google.com` (chrome), and `dl.winehq.org`. `WINE_PIN: "11.17"` (`.github/workflows/ci.yml:546-607`).

## Root cause

**Measured 2026-09-24: the lane pinned only the metapackage, and apt's resolver fills every other level with the CANDIDATE (newest) version.**

The chain is exact at every level (`apt-cache show` at `11.17~noble-1`):

```
winehq-devel  Depends: wine-devel (= 11.17~noble-1)
wine-devel    Depends: libc6 (>= 2.38), wine-devel-i386 (= 11.17~noble-1), wine-devel-amd64 (= 11.17~noble-1)
```

WineHQ published 11.18 (index dated 19 Sep), so the candidate for every family member became `11.18~noble-1` (`apt-cache policy`). Pinning `winehq-devel=11.17~noble-1` alone therefore leaves the three lower levels at 11.18, and the exact `(= 11.17)` dependencies cannot be met. When `wine-devel` is pinned too, apt states it outright one level down: `wine-devel : Depends: wine-devel-amd64 (= 11.17~noble-1) but 11.18~noble-1 is to be installed`.

Measured in the clean container, one level at a time:

| explicitly pinned | result |
|---|---|
| `winehq-devel` (the lane) | fails: `winehq-devel : Depends: wine-devel (= …)` |
| + `wine-devel` | fails on `wine-devel-amd64` / `-i386`, "but 11.18 is to be installed" |
| + `wine-devel-amd64` | fails on `wine-devel-i386` |
| + `wine-devel-i386:i386` | **resolves** |
| preferences pin `wine-devel*` (bare glob) | fails on `wine-devel-i386`: the glob matches native-arch packages only |
| preferences pin `winehq-devel wine-devel* wine-devel*:i386` | **resolves**, all four at 11.17 |

So the step's comment, *"`winehq-devel` Depends on `wine-devel (= <exact>)`, so pinning the metapackage cascades and no sub-package needs naming"*, was false. The pin worked exactly while it was WineHQ's newest release: 11.16 went red once 11.17 was out (09-05..08), the bump to 11.17 "fixed" it only by making the pin newest again, and 11.17 went red once 11.18 was out (from 09-19).

**Why it read as runner-side.** Every earlier check queried the INDEX (is 11.17 published? did the index change?), and the index was fine. Nobody ran the RESOLVER outside the runner. That is also why the old comment sent the next reader to "runner apt state … instead of bumping again".

## Evidence

Clean-container runs on 2026-09-24, all by `docker run ubuntu:24.04`:

- the lane's `install` command alone, `apt-get install -s`: the CI error, verbatim;
- the table in Root cause, one row per invocation;
- `HEAD`'s `Install MinGW + wine` step, extracted from `ci.yml` and run verbatim (real install): `STEP_EXIT=100`;
- the fixed step, the same way (real install): `STEP_EXIT=0`, and the step's own check printed `>>> wine pinned to: wine-11.17`.

The CI side: runs `35950893889` (`f918548c`), `36041448098` (`ea972b40`) and `36046104949` (`16c7ed4f`) all failed at `Install MinGW + wine` with the same two lines.

## Hypotheses tried

1. **The pin is unavailable.** Rejected: all four packages are published at 11.17 for both architectures.
2. **WineHQ published half an update.** Rejected: the index was unchanged since 09-19.
3. **Runner multiarch or third-party-repo conflict.** **Rejected**: reproduced in a clean container with no Microsoft or Google repos and no preinstalled packages.
4. **The resolver fills unpinned levels with the candidate.** **Confirmed**: see Root cause.

## Fix

**FIXED in `87d90d8c` (2026-09-24).** The install step writes an apt preferences pin before `apt-get update`:

```
Package: winehq-devel wine-devel* wine-devel*:i386
Pin: version ${WINE_PIN}~${UBUNTU_CODENAME}-1
Pin-Priority: 1001
```

It keeps the explicit `winehq-devel=V` install and the existing post-install `wine --version` assertion. The `:i386` term is load-bearing (see the table). The step's comment now states the mechanism and drops the paragraph that pointed at runner state.

A glob rather than the four names: a sub-package WineHQ adds later is covered without an edit.

## Tests added

`tests/ci_wine_pin.rs`, two tests over the step's `run:` block, not its comments.

- `the_wine_pin_covers_the_whole_family_by_architecture`: the block writes to `/etc/apt/preferences.d/` and names `wine-devel*:i386`.
- `the_wine_pin_is_written_before_winehq_devel_is_installed`: the pin comes before the `winehq-devel=${WINE_PIN}` install.

**This is a SHAPE test and says so in its header.** It cannot show apt resolves; the clean-container red/green does. It guards the regressions that are easy to make. **Observed RED** by mutating `ci.yml` in an isolated worktree (`scripts/mutation-probe.sh`): removing the pin (the broken `HEAD` shape) KILLED both tests; narrowing the glob to `wine-devel*` KILLED only the family test; moving the pin after the install KILLED only the ordering test. 3/3, each by its own test.

## Workarounds

None needed after `87d90d8c`. On an older tree: re-running the job does not help while a release newer than the pin exists. Bumping `WINE_PIN` to WineHQ's newest release would turn it green only until the next release.

## Resume

**Verified on a GitHub runner, 2026-09-24.** Run `36054775540` (head `51edbf86`, which contains `87d90d8c`), job `107818878761`: `Install MinGW + wine` succeeded and logged `>>> wine pinned to: wine-11.17`. `Cross-build` and `Cross-clippy` then succeeded, and the job reached `Cross-test under wine (lib only)` for the first time since 09-19. That meets the pass condition this section named, so the file is archived.

**The job is still red, for reasons that are not this bug.** Its cross-test ran `5701 passed; 2 failed`. Those two failures are the first tests this lane has reached since it broke, so they are newly visible, not new regressions:
- `librarian::catalog::augmentation::tests::a_ledger_re_declaring_its_own_prefix_is_not_a_conflict` (from `45d49a10`, filed as `c6cff39df3eed0c6`);
- `tools::symbol::tests::references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare` (from `1ea1d36b`, filed as `868e689cccfe84b3`).

Both also fail on native `Test (windows-latest / default)`. The earlier note here that the native Windows jobs were failing in `cargo test` is these same two tests. The `Audit Doc Refs` red on `51edbf86` is raw eval transcripts from `50113f61`, owned by that session.

## References

- `.github/workflows/ci.yml:546-607`: the install step and its pin rationale, including the same symptom at 11.16 (`d2900ecb`).
- `docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md`: why the lane pins WineHQ at all.
- `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md`: closed 2026-09-24 as not recurred; its sweep note points here for the lane's current red.
- Found by the open-bug sweep: `deep-agent-workflow-observations:DWF-7`.

## Fix provenance

- **SHA:** `87d90d8c` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `0eaa6f1051709de40b815fd846d54f0b29522720` — content hash of the diff; survives rebase and cherry-pick.

`fix(ci): pin the whole wine-devel family, so the wine lane survives WineHQ publishing a newer release`

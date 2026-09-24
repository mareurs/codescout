---
id: '23e78edc1a6d1e46'
kind: bug
status: open
title: 'BUG: the wine lane dies at setup on winehq-devel''s unmet wine-devel dependency, with the pin published and the index stable'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
opened: 2026-09-24
severity: medium
---

# BUG: the wine lane dies at setup on `winehq-devel : Depends: wine-devel (= 11.17~noble-1)`, with the pin published and the index stable

## Summary

The `Windows-gnu cross (MinGW + wine)` CI job intermittently fails before building anything. `apt-get install winehq-devel=11.17~noble-1` exits 100 with `winehq-devel : Depends: wine-devel (= 11.17~noble-1)` / "held broken packages". The latest instance is today's run. Both causes the step's own comment names are ruled out for that run: the pinned version **is** published for amd64 and i386, and WineHQ's index had not changed for more than four days. The real cause is one dependency level deeper, and apt's output does not print it.

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

Not reproducible from outside the runner (see § Evidence). Open question: whether it is intermittent per runner image. By the open-bug sweep's count (verifier evidence, not re-derived by the collector), 51 of the 82 failed wine jobs on `experiments` between 2026-09-01 and 2026-09-24 died on this apt error: 09-05 ×1, 09-06 ×26, 09-07 ×17, 09-08 ×6, 09-24 ×1. The same window had 48 successful wine jobs.

## Environment

`ubuntu-latest` (noble) GitHub runner. apt sources: the Azure Ubuntu mirror (`azure.archive.ubuntu.com`, with i386 enabled by `dpkg --add-architecture i386`), `packages.microsoft.com`, `dl.google.com` (chrome), and `dl.winehq.org`. `WINE_PIN: "11.17"` (`.github/workflows/ci.yml:546-607`).

## Root cause

Not established. What is ruled out, for the 2026-09-24 run:

- **The pin.** Fetched directly on 2026-09-24, WineHQ's `noble/main` `binary-amd64` and `binary-i386` Packages both list `winehq-devel`, `wine-devel`, `wine-devel-amd64` and `wine-devel-i386` at `11.16~noble-1`, `11.17~noble-1` and `11.18~noble-1`.
- **A half-published WineHQ update.** `InRelease` is dated `Sat, 19 Sep 2026 19:40:16 UTC`, and both Packages files are `last-modified: Sat, 19 Sep 2026 19:41:18 GMT`. The index was stable for 4+ days before the failure.
- **Fetch failure.** The runner log shows `Get:52 https://dl.winehq.org/wine-builds/ubuntu noble/main i386 Packages [247 kB]` and `Get:53 … amd64 Packages [247 kB]`, both successful.

What remains is **the runner's own package state**. apt prints only the first unmet level (`winehq-devel` → `wine-devel`), not why `wine-devel` (→ `wine-devel-amd64`/`wine-devel-i386` → their i386 multiarch dependencies) cannot be satisfied. The most plausible class is multiarch version skew: an amd64 library at one version (preinstalled on the image or from `noble-updates`) and its i386 twin not co-installable at the same version, or a conflict with a package from the Microsoft or Google repos. This is a hypothesis, not a finding.

`ci.yml:571-580` already predicted this: *"if the lane is still red at 11.17 the cause was never the pin — look at runner apt state or a half-published WineHQ update instead of bumping again."* That prediction now holds, with the second option ruled out for this run.

## Evidence

Job log saved during the sweep, grep of the `Install MinGW + wine` step: the apt-get update lines above, then `Reading package lists...` and the unmet-dependency lines. The index checks were run 2026-09-24 against `https://dl.winehq.org/wine-builds/ubuntu/dists/noble/{InRelease,main/binary-{amd64,i386}/Packages}`.

## Hypotheses tried

1. **The pin is unavailable.** Rejected: all four packages are published at 11.17 for both architectures.
2. **WineHQ published half an update.** Rejected for the 09-24 run: the index was unchanged since 09-19.
3. **Runner multiarch or third-party-repo conflict.** Open; see § Fix.

## Fix

Not started. The next step is diagnostic, not a pin bump. Make the failure name its own cause by adding, on failure only, `apt-get install -s -o Debug::pkgProblemResolver=yes "wine-devel=${WINE_PIN}~${UBUNTU_CODENAME}-1" "wine-devel-i386=${WINE_PIN}~${UBUNTU_CODENAME}-1"`. Listing the sub-packages explicitly makes apt print the unmet dependency one or two levels further down. If it proves to be multiarch skew, the options are pinning the conflicting library's i386 twin, installing from a clean `apt-get update` restricted to the needed sources, or a container image with wine preinstalled. Choose once the diagnostic has named the package.

## Tests added

None.

## Workarounds

Re-run the job: roughly half the wine jobs in the window succeeded. Read a red wine lane as "setup failed" before reading it as a test result.

## Resume

Add the failure-only diagnostic above, wait for the next occurrence, then decide the remedy from the package it names.

## References

- `.github/workflows/ci.yml:546-607`: the install step and its pin rationale, including the same symptom at 11.16 (`d2900ecb`).
- `docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md`: why the lane pins WineHQ at all.
- `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md`: closed 2026-09-24 as not recurred; its sweep note points here for the lane's current red.
- Found by the open-bug sweep: `deep-agent-workflow-observations:DWF-7`.

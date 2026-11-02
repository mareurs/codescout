---
id: '6e78851b29cdb20b'
kind: bug
status: fixed
title: 'BUG: the windows-gnu lane dies at apt-get update because apt on Ubuntu 26.04 ignores a keyring named .key'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- ci
- windows
closed: 2026-11-01
opened: 2026-11-01
severity: high
---

## Summary

The `windows-gnu` job saves the WineHQ signing key as `/etc/apt/keyrings/winehq-archive.key` and downloads a `.sources` file that names that path in `Signed-By`. The apt on Ubuntu 26.04 (`resolute`) only trusts a keyring whose name ends in `.gpg` or `.asc`. It prints "ignored as the file has an unsupported filetype", counts the repository as unsigned, and `apt-get update` exits 100. The lane never reaches the build.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the step was written and last reproduced on Ubuntu 24.04 (the 2026-09-24 bug file in References), and the two CI runs read for this file (2026-10-30 and 2026-11-01) both ran it on the image `ubuntu-26.04` and failed the same way. It differs from the class's other members in that the step reads the codename from the runner (to survive image moves) but not the apt rules that came with the new image. When `ubuntu-latest` moved to 26.04 is not established.

## Symptom (Effect)

CI run 41322768679, job `Windows-gnu cross (MinGW + wine)`, step `Install MinGW + wine`: `W: ... The key(s) in the keyring /etc/apt/keyrings/winehq-archive.key are ignored as the file has an unsupported filetype.`, `E: The repository 'https://dl.winehq.org/wine-builds/ubuntu resolute InRelease' is not signed.`, then `Process completed with exit code 100.`

## Reproduction

`docker run --rm -v <script>:/r.sh:ro ubuntu:26.04 bash /r.sh current` where the script repeats the step's key and `.sources` download, writes the pin file and runs `apt-get update`. It prints the same warning and error and exits 100. With the key saved as `winehq-archive.asc` and the `Signed-By` line rewritten, `apt-get update` exits 0 and `apt-cache policy winehq-devel` shows the candidate `11.17~resolute-1`, the pinned version.

## Environment

Runner image `ubuntu-26.04`; container `ubuntu:26.04` (digest `sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c`).

## Root cause

The file extension is the format selector in apt's keyring handling. WineHQ publishes the key ASCII-armored, which apt accepts as `.asc`. The step named it `.key`.

## Evidence

The CI log lines above, and the container runs: before, `rc=100` with the same message; after, `rc=0` and a candidate of `11.17~resolute-1`.

## Hypotheses tried

That `resolute` has no WineHQ build at the pinned version: refuted, the candidate is present (versions 11.17 to 11.21 are listed).

## Fix

The key is saved as `winehq-archive.asc`, and a `sed` rewrites the `Signed-By` line of the downloaded `.sources` file to match. A comment in `.github/workflows/ci.yml` states the reason.

Fixed by `683c04eae` on `experiments` (patch-id `2cd79c86f3e5062b52bd0206d01ae7fbc61ad3cb`). Verified only up to `apt-get update` and the pinned candidate, in an ubuntu:26.04 container. Runner result: the `Windows-gnu cross (MinGW + wine)` job got past `apt-get update` and ran the whole lane (wine install, Windows build, tests under wine) in run 41342167539 (it failed at clippy) and run 41344511357 (it failed at tests, for other causes). It is green in run 41413376172.

## Tests added

None: the step runs only on CI. RED and GREEN are the container runs above.

## Workarounds

None.

## Resume

1. Read `Windows-gnu cross (MinGW + wine)` on the first CI run with the fix. If the step passes, later steps will run on Ubuntu 26.04 for the first time and may reveal more failures (wine 11.17 on this image, the MinGW toolchain version, the skipped-test list tied to wine 9.0 behaviour).
2. The fix does not change the pin: `WINE_PIN` is still 11.17 while 11.21 is published.

## References

- CI run 41322768679.
- `docs/issues/archive/2026-09-24-wine-lane-dies-at-apt-setup-with-the-pin-published.md` (an earlier apt-setup failure of the same step).

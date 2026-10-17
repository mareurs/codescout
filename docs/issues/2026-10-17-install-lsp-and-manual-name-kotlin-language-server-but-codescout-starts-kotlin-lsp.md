---
kind: bug
status: fixed
title: 'BUG: scripts/install-lsp.sh and the manual install and troubleshoot kotlin-language-server, but codescout starts kotlin-lsp'
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
closed: 2026-10-17
unverified: "The real download was not exercised: the URL and the archive's internal layout come from the release page and the installed kotlin-lsp-bin package, and the installer was run only against a stub curl and a fabricated archive. macOS is refused with a pointer to the manual, not installed."
---

# BUG: scripts/install-lsp.sh and the manual install and troubleshoot kotlin-language-server, but codescout starts kotlin-lsp

## Summary

The Kotlin language server codescout starts is JetBrains' `kotlin-lsp` (`src/lsp/servers/mod.rs:111`, `crate::platform::lsp_binary_name("kotlin-lsp")`, through the mux). `scripts/install-lsp.sh` still installs fwcd's `kotlin-language-server`, and three manual pages still tell the reader to check for it. A reader who follows them gets a server codescout never starts. Found 2026-10-17 while fixing the Kotlin smoke script, whose header named the wrong server for the same reason.

## Class fit

The install script and the troubleshooting page are documents of which binary codescout needs, and the code names a different one: the account in the document is contradicted by the code, which is the class's defining trait. It differs from the class's other members in that one page of the same manual (`docs/manual/src/language-support.md`) already states the right binary, so the manual contradicts itself too.

## Symptom (Effect)

Read 2026-10-17 from the checked-in tree:

- `scripts/install-lsp.sh:91` maps `kotlin` to `kotlin-language-server`; `install_kotlin` (`:199-232`) downloads `fwcd/kotlin-language-server` and symlinks `kotlin-language-server` into the install dir. It installs no `kotlin-lsp`.
- `docs/manual/src/troubleshooting.md:110` tells the reader to run `which kotlin-language-server`; `:125` and `:428` name it in the slow-start notes.
- `docs/manual/src/architecture.md:147` lists `kotlin-language-server` as the Kotlin server.
- `docs/manual/src/language-support.md:24` and `:133-137` name `kotlin-lsp` (JetBrains), invoked with `--stdio`, which is what the code starts.

## Reproduction

```
grep -n 'kotlin-language-server' scripts/install-lsp.sh docs/manual/src/troubleshooting.md docs/manual/src/architecture.md
grep -n 'lsp_binary_name("kotlin' src/lsp/servers/mod.rs
```

## Environment

2026-10-17, this machine: `command -v kotlin-lsp` prints `/usr/bin/kotlin-lsp`; no `kotlin-language-server` is installed.

## Root cause

Not investigated. The code comments in `src/symbol/query.rs:502`, `src/symbol/edit.rs:16` and `src/tools/symbol/tests.rs:3119` still say `kotlin-language-server` too; those describe behaviour measured against the old server and may be correct as history, which is a separate question from what the installer and the manual should say.

## Evidence

The grep output above, and the live run of `tests/mcp-smoke-kotlin.sh` on 2026-10-17, which started `kotlin-lsp` (the mux command line carries `-- kotlin-lsp --stdio --system-path=...`).

## Hypotheses tried

None.

## Fix

Decision taken from the code (2026-10-17): codescout starts the binary named in `lsp_binary_name("kotlin-lsp")` (`src/lsp/servers/mod.rs:111`; on Unix `lsp_binary_name` returns the name unchanged, `src/platform/unix.rs:163`) and finds it on `PATH`; the mux command line is `-- kotlin-lsp --stdio --system-path=...`. So the installer must put a `kotlin-lsp` on `PATH`, and `--check` must look for that name. Nothing is read from a configured path.

The installer's source is JetBrains' release, read on 2026-10-17 from `https://api.github.com/repos/Kotlin/kotlin-lsp/releases/latest` and the release page: the GitHub release has no assets (the tag is `kotlin-lsp/v263.6379.0`); the downloads are on `download.jetbrains.com/language-server/kotlin-server/<version>/kotlin-server-<version>[-aarch64].tar.gz` for Linux, `.sit` for macOS and `.win.zip` for Windows. The installed package on this machine (`/usr/share/kotlin/kotlin-lsp`, owned by `kotlin-lsp-bin`) shows the layout: `bin/intellij-server` is the launcher, and `/usr/bin/kotlin-lsp` is a symlink to it; `kotlin-lsp.sh` beside it prints that it is deprecated and execs `bin/intellij-server`.

Changed:

- `scripts/install-lsp.sh`: `binary_for kotlin` is `kotlin-lsp`; `install_kotlin` resolves the version from the `kotlin-lsp/v<version>` tag, downloads the Linux `.tar.gz` for the CPU (`-aarch64` on arm64), unpacks it under `~/.local/share/kotlin-lsp`, finds `bin/intellij-server` (at the root or one directory down), and links it as `~/.local/bin/kotlin-lsp`. On macOS it refuses with a message pointing at the manual, because the archive there is a `.sit` this script does not unpack.
- `docs/manual/src/troubleshooting.md`: `which kotlin-lsp` (`:110`), the slow-start note (`:125`) and the startup-time table row (`:428`).
- `docs/manual/src/architecture.md:147`: the Kotlin row names `kotlin-lsp`.
- `docs/manual/src/language-support.md` (Kotlin section): said the release is an archive on the GitHub releases page and to place "the `kotlin-lsp` script" on `PATH`; now says where the archives are, that the launcher to link is `bin/intellij-server`, that the install script does this on Linux and not on macOS.
- `contrib/systemd/README.md:115`: named `kotlin-language-server` as the JVM LSP codescout spawns with `-Xmx2g`; it is `kotlin-lsp`. Not in the original list; the same claim.

Not changed: the code comments in `src/symbol/query.rs`, `src/symbol/edit.rs` and `src/tools/symbol/tests.rs`, which record what fwcd's server did when BUG-027 was measured; they are history, not a statement of what codescout uses. `CHANGELOG.md` and archived documents likewise.

## Tests added

`tests/kotlin_server_name_in_installer_and_manual_matches_code.rs`, three tests that derive the started binary from the `"kotlin" =>` arm of `src/lsp/servers/mod.rs` and compare the documents to it:

- no line of `scripts/install-lsp.sh`, `troubleshooting.md`, `architecture.md`, `language-support.md` or `contrib/systemd/README.md` contains `kotlin-language-server` (these files each say which server codescout uses; the ban is deliberately not applied to the changelog, archived issues or code comments);
- `binary_for kotlin` equals the started binary, and `install_kotlin` has the line `ln -sf "${launcher}" "${INSTALL_DIR}/kotlin-lsp"`;
- the architecture table's Kotlin row and `troubleshooting.md`'s `which ...` line and startup-time row name the started binary.

RED before the fix, 3 of 3 failed: `binary_for kotlin names a binary codescout does not start (left: kotlin-language-server, right: kotlin-lsp)`, the architecture row, and a list of 17 offending lines in the five files. GREEN after: `3 passed; 0 failed`. Mutants observed RED: linking the launcher as `kotlin-lsp-x` (`install_kotlin does not link the launcher as kotlin-lsp`), and re-adding the old name to `architecture.md` (`architecture.md:148 ... names kotlin-language-server`; the ban is blunt and also rejects a "not kotlin-language-server" note, which these five files do not need).

Also run, not committed as a test: `scripts/install-lsp.sh kotlin` with a stub `curl` (it recorded the two URLs asked for: the `releases/latest` API and `https://download.jetbrains.com/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz`), `HOME` and `TMPDIR` in a scratch directory, `PATH` limited to a stub directory, and a fabricated archive with the launcher one directory down: it linked `~/.local/bin/kotlin-lsp` to `bin/intellij-server`, the link ran, and `--check` reported `kotlin  kotlin-lsp  <path>`. Ceiling: no real download was made, so the URL and the archive's internal layout are read from the release page and the installed package, not exercised.

## Workarounds

Install `kotlin-lsp` by hand as `docs/manual/src/language-support.md` describes.

## Resume

Done. Optional: run `./scripts/install-lsp.sh kotlin` on a machine without `kotlin-lsp` to exercise the real download (not done here: no network install on the fixing machine). Archive with the fix SHA (labelled experiments) and its patch-id once this reaches experiments.

## References

- `src/lsp/servers/mod.rs:111`
- `scripts/install-lsp.sh`
- `docs/manual/src/troubleshooting.md`
- `docs/manual/src/architecture.md`
- `docs/manual/src/language-support.md`

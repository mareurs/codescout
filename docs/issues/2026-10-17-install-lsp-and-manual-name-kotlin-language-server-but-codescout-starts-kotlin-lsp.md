---
kind: bug
status: open
title: 'BUG: scripts/install-lsp.sh and the manual install and troubleshoot kotlin-language-server, but codescout starts kotlin-lsp'
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
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

Decision needed: whether `install-lsp.sh` should install JetBrains' `kotlin-lsp` (and from where; `language-support.md:133-135` names its GitHub releases page), and then correct `troubleshooting.md`, `architecture.md` and the install script's header and server map. Not fixed here.

## Tests added

N/A: open.

## Workarounds

Install `kotlin-lsp` by hand as `docs/manual/src/language-support.md` describes.

## Resume

1. Decide the installer's source for `kotlin-lsp`. 2. Rewrite `install_kotlin` and the `:91` map entry. 3. Fix the three manual pages. 4. Decide separately whether the `kotlin-language-server` code comments should say which server each measurement was made against.

## References

- `src/lsp/servers/mod.rs:111`
- `scripts/install-lsp.sh`
- `docs/manual/src/troubleshooting.md`
- `docs/manual/src/architecture.md`
- `docs/manual/src/language-support.md`

---
id: c7bd57f261ca9463
kind: bug
status: open
title: 'BUG: imported Pi memory guard reencodes invalid UTF-8 before backup'
owners:
- marius
tags:
- cluster/unclassified
- pi
- albert-guard
- external-extension
- memory
- backup
closed: null
opened: 2026-10-14
severity: medium
unverified: Native SDK/adaptor acceptance of the proposed isolated correction and evidence JSON remain pending.
---

# BUG: imported Pi memory guard reencodes invalid UTF-8 before backup

## Summary

The Pi topic-write guard supplied by Albert, SHA256 9eaed6299b80e05dafc6d109a05ecac45b74894093ea6291bdc85ee27881d02f, reads an existing topic as a decoded UTF-8 string before creating its backup. Node replaces invalid byte sequences instead of raising. An override can therefore be permitted after a backup whose bytes differ from the original.

This record concerns the received, unintegrated client extension. It does not assert that the repository's tool-selection extension contains this backup implementation.

## Symptom (Effect)

For synthetic original bytes 61ff620a, the saved backup is 61efbfbd620a. The write hook returns undefined, permitting dispatch. The probe preserves the original fixture and runs without any client installation, model, network provider or daily memory access.

## Reproduction

Load the exact received TypeScript extension in a fresh Node process, register its tool_call hook, create an isolated public topic containing bytes 61ff620a, and call codescout_memory with action write, force true and the explicit override marker in content. Inspect the backup as a Buffer, not decoded text.

A fail-closed regression expects a block result before any backup or overwrite for invalid UTF-8. The original extension produced 13 passed / 1 failed out of 14 top-level tests. An isolated one-expression correction produced 14 passed / 0 failed.

## Environment

Local review: Node 26.10.0, Linux, real temporary files, hook registration shim. Albert reports Node 22.23.1, Pi SDK 0.84.1 and pi-mcp-adapter 2.27.0. The proposed correction has not been rerun in his native SDK/adaptor profile. His evidence JSON has not been received.

## Root cause

fs.readFileSync(file, "utf-8") decodes with replacement. saveBackup then writes that string as UTF-8, preserving the replacement characters rather than the original invalid bytes. The surrounding catch cannot catch a decode failure that was not thrown.

## Evidence

Guard source: the old-content read in its default export; saveBackup writes oldContent with UTF-8 encoding.
Local evidence is retained in /tmp/codescout-albert-pi-guard-review-20261014 and the Albert transition handoff package.
The 14-test harness includes genuine BOM/Unicode byte preservation. Three applied mutations were killed: HOME-only protection, removal of workspace pinning, and restoration of lossy UTF-8 decoding. No candidate survived. These are hook/filesystem checks, not native SDK execution.

## Hypotheses tried

Invalid UTF-8 was decoded with replacement rather than rejected: reproduced with exact original/backup bytes.
Concurrency and later-hook redirection remain separately documented limitations of the client hook; this correction does not close them.

## Fix

Proposed only, not installed or integrated: read a Buffer and decode with TextDecoder("utf-8", { fatal: true, ignoreBOM: true }). The existing catch then blocks malformed UTF-8 before backup. ignoreBOM true retains a valid leading BOM so valid UTF-8 backup bytes remain exact.

Original SHA256: 9eaed6299b80e05dafc6d109a05ecac45b74894093ea6291bdc85ee27881d02f.
Proposed SHA256: bb0987371d5445e50d67e341e8043df90462ee366b5a173501698c19f79927ff.
No fix commit or patch-id is assigned: the candidate is an isolated received-artifact correction, not a committed repository fix.

## Tests added

The handoff includes guard-contract.test.mjs. Expected behavior is tested through the actual imported extension and real temporary files. Original RED, corrected GREEN and lossy-decoder mutation RED were observed. Source SHA256 was verified unchanged afterward.

## Workarounds

Do not approve destructive writes of topics whose bytes cannot be decoded as UTF-8. Retain originals and fix encoding separately only after preserving them. Use the bounded client profile until a backend history contract is designed and verified.

## Resume

Receive Albert's evidence JSON; rerun the corrected extension's full native profile on his SDK/adaptor/Node versions; decide ownership of a shared backend backup contract. Keep this issue open until the supported integration is fixed and verified.

## References

Albert's report: raport-fix-protectie-memorii-pi-2026-10-14.md.
Received source: codescout-pi-memory-write-guard-2026-10-14.ts.

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


**Re-verified 2026-10-16** (Node 26.10.0, copies of both files in a scratch directory; the repository holds neither, they live in `~/Downloads/albert-codescout-transition-2026-10-14/`):

- SHA256 of the received guard `9eaed629...d02f` and of the proposed one `bb098737...27ff` match this record. `diff` of the two is exactly one line (123): `fs.readFileSync(file, "utf-8")` against `new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(fs.readFileSync(file))`.
- Direct probe: topic bytes `61ff620a`, override marker in content, `force: true`. Received guard: hook returns `undefined` (write permitted), backup bytes `61efbfbd620a`. Proposed guard: hook returns `{block: true, reason: "...the existing topic cannot be read safely; write refused."}`, no backup file written.
- `guard-contract.test.mjs` against the received guard: 14 tests, 13 pass, 1 fail (`invalid existing UTF-8 refuses before any lossy backup or overwrite`). Against the proposed guard: 14 pass, 0 fail.

Still not established: the native Pi SDK / adapter run, and where the guard is to live. No file in this repository contains `saveBackup` or the guard (grep over `*.ts *.mjs *.js *.sh *.py`, 0 hits), so there is nothing in-repo to patch until the guard is imported.


**Import review 2026-10-16: NOT imported into `contrib/pi/`.** The 14-test harness passes 14/14 against the proposed guard, but a wider review found defects the harness does not cover. Each was reproduced with a scratch probe (Node 26.10.0, copies of the files, nothing run from the received directory), against `codescout-pi-memory-write-guard-proposed.ts` (SHA256 `bb098737...27ff`). Line numbers are that file's.

1. Line 115-117, 123-138: the guard also checks and backs up `$HOME/.codescout/memories`, but the server writes only the project store (`MemoryReadDirs::primary`, `src/tools/memory/mod.rs`). A project with no `arch` topic and a HOME topic `arch` of 8 lines: a new project topic `arch` is refused ("preserves only ~0%") and the HOME file is backed up. The harness asserts this on purpose (`new topic allowed and HOME existing topic protected`), so the behaviour is designed, not accidental; it is a false refusal against the backend contract.
2. Line 123: `readFileSync` follows symlinks and no `lstat` precedes it. With `memories/topic.md` a symlink to a file outside the store, the guard copies that file's bytes into `.history` and permits the write. The server then refuses (`src/memory/write_transaction.rs:68`, "must be a regular file"), so nothing is overwritten, but the foreign bytes are already copied.
3. Line 56 (`saveBackup`): the guard creates `.history` but not `.history/.gitignore`; the server writes `*` there (`write_transaction.rs:63`), but only on a write that reaches it. When the guard blocks, the backup stays unignored: `git status --porcelain` in a probe repo printed `?? .codescout/memories/.history/topic.<stamp>.0.md.bak`. This repo tracks `.codescout/memories`, so `git add -A` would take it. An existing `.history` with mode 0755 also stays 0755 (the server resets it to 0700).
4. Line 76, 138, 142: a top-level `event.input` that is a JSON string is parsed into a copy, so the workspace pin is applied to the copy and the string is dispatched unpinned (probe: `has workspace pin: false`). Through the `mcp` gateway the same input makes line 142 assign a property on a string and throw `TypeError: Cannot create property 'args' on string`; what Pi does with a throwing hook was not measured.
5. Not defects of the code but limits to state in any README: `action: "delete"` is not guarded here and the server's delete keeps no backup (`write_transaction.rs:26`, `begin_delete` reads no existing content); the override marker is matched anywhere in `content`, including prose that only mentions it (probe: total replacement permitted); a `cwd` that is a subdirectory of the project finds no topic and permits the write; the guard does not take the server's `write.lock`.

The invalid-UTF-8 half of this bug is covered in the server as of `f5f1c6f7`: `WriteTransaction::prepare` refuses a non-UTF-8 topic (`src/memory/write_transaction.rs:77`) and `write` stores the exact bytes of a valid one, tested by `forced_writes_preserve_history_in_the_selected_store_and_refuse_invalid_bytes` (`src/tools/memory/tests.rs:1345-1356`). The received client guard would duplicate that backup in a second layout (`.history/<topic>.<stamp>.<n>.md.bak` beside the server's `.history/<topic>.md/<uuid>.md.bak`). Unresolved ownership question for the user: keep a client guard at all, or rely on the server contract and add the overlap policy there. Keep this issue open until that is decided; if the guard is imported later, fix 1 to 4 first and add a committed test for each.

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

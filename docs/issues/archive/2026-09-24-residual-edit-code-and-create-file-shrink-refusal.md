---
id: a7b44ebae482645a
kind: bug
status: fixed
title: 'RESIDUAL: Make edit_code(replace) refuse (not warn) on shrink, guard create_file overwrite, and make the class gate check the guard runs on the right operands'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-10-08
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make edit_code(replace) refuse (not warn) on shrink, guard create_file overwrite, and make the class gate check the guard runs on the right operands.


### Re-verified 2026-10-08 — both residual write paths reproduce live

Live server build `a2871af0` (`git_dirty=true`, `exe_deleted=false`), code inspected at HEAD `21237e7f2734d8757c6b9666e2f79a9cfd6a318c`.

On a disposable Python fixture, `edit_code(action=replace)` reduced a complete function from 37 lines to 2. The real response was `status: ok` with `shrink: ... 653 → 37 bytes (95%) ... 37 → 2 lines (95%)`. A subsequent symbol read confirmed that the shorter declaration was written. This is advisory after the write, not refusal before it.

On a separate disposable markdown fixture of about 5.6 KB, `create_file(overwrite=true, content="# Overwritten\n")` returned `ok`; `read_file` confirmed the file now contained the 14-byte replacement. No shrink warning or refusal was emitted.

`do_replace` computes shrink over the replaced symbol range correctly but uses it only in the response after writing. `CreateFile::call` checks existence when overwrite is false, then writes directly when true. These are confirmed current data-loss paths. Prioritise refusal before writing, with an explicit intentional-shrink escape, plus behavioural tests observing unchanged file bytes on refusal and the actual symbol-range operands. No production code changed; disposable fixtures were cleaned up.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md` (status `mitigated` when this residual was filed; caveat discharged on 2026-10-08):

> Advisory only - edit_code(action=replace) WARNS and does not refuse, so a caller who ignores the warning still loses the code. src/tools/create_file.rs (overwrite: true) remains unguarded, now recorded in the class gate's EXEMPT list with its reason rather than silently. The class gate proves each surface's module CONTAINS a guard call, not that the call runs on the right operands.

## Fix

Implemented and verified on experiments on 2026-10-08. `edit_code(action="replace")` refuses a >50% byte or line reduction of the actual replaced symbol range when that range is at least 200 bytes. `force=true` acknowledges an intentional reduction; syntax, name and sibling corruption checks still run. `create_file(overwrite=true)` now reads existing content and applies the same predicate before writing. `force` alone does not authorize overwrite.

The registry-wide runtime test calls every content-bearing tool, checks omitted/false force refusals and unchanged backing bytes, then checks proportionate edits and forced reductions. Its symbol fixture is surrounded by many siblings and asserts the exact 9→3 line operands; the create-file exemption was removed. A discovered percentage underflow was also corrected when one dimension grows while the other shrinks.

**SHA (experiments):** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34`.

**patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`.

## Tests added

`tests/symbol_lsp.rs` covers refusal with omitted/false force, valid forced reduction, proportionate edits, and forced syntax/name/body-only/sibling rollback. Create-file tests cover byte-only and line-only refusal, proportional writes, and independent overwrite intent. `every_content_bearing_tool_runs_its_shrink_guard_before_writing` exercises the actual registered population.

The full gate exited `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; both symbol-suite runs passed all 75 tests. Applied mutation review killed all eight selected logic mutations after two observed survivors were fixed. The class fixture separately killed whole-file operands. A fresh standalone debug binary over stdio MCP refused both residual paths without changing disk bytes and allowed explicit forced writes; the installed live MCP binary was not replaced.

## Resume

N/A — this residual is verified on experiments. No master promotion is required for archive.

## Fix provenance

- **SHA:** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34` (`experiments`)
- **patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`

## References

- `docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md` — parent

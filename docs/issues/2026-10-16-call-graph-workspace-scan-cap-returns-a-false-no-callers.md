---
kind: bug
status: open
title: call_graph's workspace definition scan stops at 5,000 files and a stopped scan reads as "no such symbol"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
---

## Summary

`CachedResolver::lookup_pos_via_ts_workspace` is call_graph's Phase B fallback: when the LSP cannot place a symbol, it walks the workspace parsing same-language files for a definition. It stops after `MAX_WORKSPACE_FILES_SCAN` (5,000) parsed files and returns `None`. `None` means both "no file defines it" and "the walk stopped before reaching the file that does", and the caller records either as `not_found` for the rest of the resolver's lifetime. Filed from the `PROBE_ROWS` classification of `call_graph.workspace_files_scan` as `Coverage::NoMarker`.

## Class fit

A zero from a capped scan read as "not present" rather than "not reached" — the class's own wording. It differs from `references.corroborate_files_scan` (`docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`), which loses a warning; this one loses a *position*, so a symbol that exists is treated as unresolvable and its edges are dropped from the graph.

## Symptom (Effect)

On a project with more than 5,000 same-language files, with the LSP unable to place the symbol, a definition reached after the 5,000th parsed file is not found and `call_graph` proceeds as though the symbol were unresolvable, with no note that the scan was cut short. Inferred from source — not reproduced (needs a >5,000-file fixture with the LSP down); how the dropped node is rendered to the caller was not traced.

## Reproduction

`src/tools/symbol/call_graph/mod.rs:158-198`: `scanned` counts parsed files (`:184`), the loop breaks at `scanned >= MAX_WORKSPACE_FILES_SCAN` (`:172`), the function falls through to `None`. The caller (`:94-102`) inserts the symbol into `not_found` on `None`. Read at `experiments` f5f1c6f7 — inferred, not measured at runtime.

## Root cause

An `Option` return that carries "found" and nothing about whether the search was exhaustive; the cap is documented in a comment ("a monorepo stops short instead of stalling") and nowhere in the data.

## Fix

Not designed. Candidate: distinguish `NotFound` from `ScanIncomplete` in the return and surface the latter in the call_graph result's existing warning channel. Note the file-walk bound is a performance guard against the MCP ceiling, so the fix is disclosure, not removing the cap.

## Tests added

None — filing only.

## Resume

1. Trace how an unresolved symbol is rendered, then decide where the incompleteness note belongs.
2. Move `call_graph.workspace_files_scan` to `Probed`.

## References

- `src/tools/symbol/call_graph/mod.rs:86-102,148-198`
- `src/tools/core/cap_probe.rs` row `call_graph.workspace_files_scan`
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`

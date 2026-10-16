---
kind: bug
status: open
title: references' zero-result corroboration stops scanning at 5,000 source files and a stopped scan reads as "no other references"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: medium
---

## Summary

When `references` finds no locations outside the definition file, `corroborate_zero_references` scans the project's same-language source files for the identifier as a safeguard against a stale LSP graph (a zero is "the signal an agent uses to delete supposedly-dead code"). The scan stops after `MAX_FILES_SCAN` (5,000) files and returns a bare `Vec<PathBuf>`. An empty vector is produced both by "scanned everything, found nothing" and by "stopped at the cap, never reached the file that uses it", and the caller treats the two identically: it adds no warning. Filed from the `PROBE_ROWS` classification of `references.corroborate_files_scan` as `Coverage::NoMarker`.

## Class fit

The class's own sentence — "a zero from a capped scan reads as 'not present' rather than 'not reached'" — is this bug's whole shape: the safeguard exists to refuse a false zero and itself manufactures one past the cap. It differs from the ceiling members that truncate a returned list: here nothing returned is partial; the *absence* of a warning is the partial answer, and it is a safety warning that goes missing.

## Symptom (Effect)

On a project with more than 5,000 same-language files, `references` on a symbol used only in files the walk reaches after the 5,000th returns the bare zero, with no `completeness_warning`, where a smaller project would have produced one. Inferred from source, not reproduced (needs a >5,000-file fixture).

## Reproduction

`src/tools/symbol/references.rs:162-200`: `scanned` is incremented per same-language file (`:193`) and the loop `break`s at `scanned >= MAX_FILES_SCAN` (`:180`); the function returns `hits`. The only caller (`:467-469`) sets `completeness_warning` only when `!others.is_empty()`. Read at `experiments` f5f1c6f7 — inferred from source, not measured at runtime.

## Root cause

Bound and result share one return value: the `Vec` carries hits and nothing about whether the walk completed. The doc comment says the bounds mirror call_graph's Phase B (`call_graph.workspace_files_scan` has the same defect — see its issue).

## Fix

Not designed. Candidate: return `(hits, scan_complete: bool)` and, when incomplete with no hits, set a `completeness_warning` saying the corroboration scan stopped at the cap. The warning path already exists at `:466-477`.

## Tests added

None — filing only. The existing test (`tools/symbol/tests.rs` ~`:5890`) covers hits within the cap.

## Resume

1. Make the return carry completeness; surface it through the existing `completeness_warning`.
2. Move `references.corroborate_files_scan` to `Probed` (a fixture can lower the cap via an injectable bound, or generate >5,000 files).

## References

- `src/tools/symbol/references.rs:162-200,457-477`
- `src/tools/core/cap_probe.rs` row `references.corroborate_files_scan`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`

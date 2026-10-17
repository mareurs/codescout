---
kind: bug
status: open
title: references' corroboration scan and call_graph's Phase B walk skip a file they cannot read and still report a clean miss
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
---

## Summary

Both fallback walks (`corroborate_with_cap` in `src/tools/symbol/references.rs` and `CachedResolver::lookup_pos_via_ts_workspace` in `src/tools/symbol/call_graph/mod.rs`) admit a file against the file budget and only then try to read and parse it. A read failure (not valid UTF-8, permissions), a parse failure, or a directory the walker cannot enter drops that file with nothing recorded. The walk then ends as `NotFound` / an empty hit list, which the callers treat as a trustworthy negative. Found while reviewing the fix for the two scan-cap bugs, which closed the *cap* path to this false negative and left this one.

## Class fit

The class's own shape: a search that did not cover its population reports "not present" rather than "not reached". The difference from the scan-cap members is that the cap is not involved at all, so `ScanBudget::cut_short_at` cannot see it: the files are admitted, then skipped one by one on an `else { continue }`.

## Symptom (Effect)

A definition (call_graph) or a use (references) that lives only in a file `std::fs::read_to_string` rejects is never seen. call_graph treats the symbol as unplaceable and drops its edges with no warning; references returns a bare zero with no `completeness_warning`.

Measured 2026-10-17 with throwaway probes (deleted after the run), a project containing `latin1.rs` = `// caf\xe9\nfn ghost() {}\n` and `b.rs`:

```
PROBE4 found=None not_found={"ghost"} cut_short={}
```

and for the references side, `latin1.rs` = `// caf\xe9\nfn caller() { target(); }\n` next to `def.rs`:

```
PROBE4R CorroborationScan { hits: [], cut_short_at: None }
```

Both are the bare negative; neither names the unread file.

## Reproduction

`src/tools/symbol/references.rs`, `corroborate_with_cap`: `let Ok(src) = std::fs::read_to_string(path) else { continue; };` after `budget.admit()`. `src/tools/symbol/call_graph/mod.rs`, `lookup_pos_via_ts_workspace`: the same `let Ok(source) = ... else { continue; }`, then `let Ok(syms) = extract_symbols_from_source(...) else { continue; }`. Both loops iterate `walker.flatten()`, which drops walker errors (an unreadable directory) silently as well.

## Root cause

The budget counts files *admitted*, not files *examined*. `ScanBudget` (`src/tools/symbol/scan_budget.rs`) records only a refusal by the cap, and its docs now say so; nothing records a skip after admission. Measured 2026-10-17 by the probes above.

## Fix

Not designed. Candidate: count skipped files in the same place `admit` is called (a `skipped` tally on `ScanBudget`, or `WorkspaceScan::NotFoundSkipping(n)` / a `skipped` field on `CorroborationScan`) and let the existing `completeness_warning` channels say "N files could not be read". Clause 2 of the ADR applies: a non-UTF-8 file in a tree is routine (vendored fixtures, legacy encodings), so the warning must name a *relevant* skip (a same-language file) and not fire on every walk.

## Tests added

None — filing only.

## Resume

1. Decide the tally's home (`ScanBudget` vs the two result types).
2. Decide whether a lossy read (`String::from_utf8_lossy`) is the better fix for the references text scan, since a whole-word match does not need valid UTF-8.

## References

- `src/tools/symbol/scan_budget.rs`
- `src/tools/symbol/references.rs` (`corroborate_with_cap`)
- `src/tools/symbol/call_graph/mod.rs` (`lookup_pos_via_ts_workspace`)
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`

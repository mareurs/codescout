---
kind: bug
status: open
title: audit_doc_refs stops its basename index at 50,000 files and reports the un-indexed tail's refs as Missing
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
---

## Summary

`audit_doc_refs` resolves a bare-basename citation through an index built by `build_basename_index`. The build stops after `MAX_INDEXED_FILES` (50,000) files. A ref to a file in the un-indexed tail then gets `Verdict::Missing` — a claim that the file does not exist — and nothing in the audit output says the index was incomplete. The function's own doc comment states this as intended behaviour ("basename misses just fall through to `Verdict::Missing` for the un-indexed tail"). Filed from the `PROBE_ROWS` classification of `audit_doc_refs.basename_index` as `Coverage::NoMarker`.

## Class fit

A capped scan read as "not present" rather than "not reached": a truncated index produces a false `Missing`, a statement of absence about a file that exists. It differs from the walk-based members in its blast radius: the cap is high (50,000 files, an order above the 5,000 of the references/call_graph scans), so only very large monorepos reach it, and the result is a lint finding rather than a deletion signal.

## Symptom (Effect)

On a repo with more than 50,000 non-ignored files, a doc citing a file beyond the cap by basename only is reported as missing. Inferred from source — not reproduced (needs a >50,000-file fixture, or the cap made injectable).

## Reproduction

`src/librarian/tools/audit_doc_refs/mod.rs:742-782`: `count` increments per file and the walk `break`s at `count >= MAX_INDEXED_FILES` (`:767`); the function returns the map with no completeness flag. The caller (`:579`, `:615`) stores it in the resolver context unchanged. Read at `experiments` f5f1c6f7 — inferred, not measured at runtime.

## Root cause

The index type is a bare `HashMap`; there is no slot for "built from a truncated walk". The doc comment at `:747-749` describes the false `Missing` as graceful degradation, which is the defect restated as a design.

## Fix

Not designed. Candidate: return `(index, complete: bool)` and, when incomplete, add a note to the report (and demote basename `Missing` to an "unresolved: index incomplete" verdict).

## Tests added

None — filing only.

## Resume

1. Decide the incomplete-index verdict and where the note lands in the audit report.
2. Make the cap injectable so a test can drive it, then move `audit_doc_refs.basename_index` to `Probed`.
3. `audit_doc_refs` is owned by the docs agent this round — coordinate before editing it.

## References

- `src/librarian/tools/audit_doc_refs/mod.rs:742-782`
- `src/tools/core/cap_probe.rs` row `audit_doc_refs.basename_index`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`

---
id: f6fc2372ed80c5a5
kind: bug
status: open
title: audit_doc_refs makes one blocking document_symbols LSP call per reference, with no per-file memo
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-30
severity: low
unverified: The share of audit_doc_refs time spent in document_symbols calls is not measured.
---

# BUG: audit_doc_refs makes one blocking document_symbols LSP call per reference, with no per-file memo

## Summary

Each file-and-symbol reference makes its own `document_symbols` call under `block_in_place`, with no per-file memo (`src/librarian/tools/audit_doc_refs/resolver.rs:634-660`). A grep of the docs found 629 such refs across 259 distinct files, so a per-file memo would remove about 59% of those calls.

## Class fit

`cluster/unclassified`. What the case shows: a lookup keyed on (file, symbol) whose expensive part depends only on the file.

## Symptom (Effect)

usage.db: `librarian action=audit_doc_refs` 23 calls, avg 15.4 s, max 129 s. A normal full scan is 54-57 s for ~2,600 files and ~82,700 refs. The 129 s call passed `path=` and ran project-wide — already filed in `docs/issues/2026-10-30-librarian-reindex-link-scan-and-audit-doc-refs-accept-a-path-argument-and-run-project-wide-without-saying-so.md`.

## Reproduction

`librarian(action="audit_doc_refs")` on codescout.

## Environment

Shared rust-analyzer behind the LSP mux.

## Root cause

inferred from `src/librarian/tools/audit_doc_refs/resolver.rs:634-660` — the share of the 55 s spent in these calls is not measured.

## Evidence

Code read 2026-10-30 by the librarian explorer of this session.

## Hypotheses tried

None.

## Fix

Memoize `document_symbols` per file for the length of one audit call.

## Tests added

None yet.

## Workarounds

Pass `paths` (plural) to scope the scan.

## Resume

Time the resolver's LSP calls with a span to size the saving.

## References

- `docs/issues/2026-10-16-audit-doc-refs-basename-index-cap-reports-existing-files-missing.md`

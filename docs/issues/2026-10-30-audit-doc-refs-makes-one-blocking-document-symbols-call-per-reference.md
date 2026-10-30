---
id: f6fc2372ed80c5a5
kind: bug
status: fixed
title: audit_doc_refs counted newlines from the top of the file for every markdown event (O(events x size)) and asked the server for a file's symbols once per reference
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: high
unverified: What remains of the 11-15 s after the fix was not profiled. The share of the 7.5 s saved by the symbols memo is one measurement, CLI and no tracker emission.
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

Two costs, found by sampling stacks (`gdb`, 4 of 4 samples at 5/9/13/17 s on a one-file audit), not by reading the code:

1. **`byte_offset_to_line` (`src/librarian/tools/audit_doc_refs/parser.rs`) counted every `\n` from the top of the file for every pulldown event**, not for every reference: O(events x file size). One tracker file (`docs/trackers/bug-fix-session-log.md`) took 20 s of pure CPU (user 19.8 s, sys 0.1 s) through the CLI. The issue as filed named the wrong cost.
2. `resolve_file_symbol` asked the language server for a file's symbols once per reference (`resolver.rs`), with no per-file memo. This is the cost the issue named. It was real but second: 7.5 s of the 22.5 s left after the first fix.

## Evidence

Code read 2026-10-30 by the librarian explorer of this session.

## Hypotheses tried

1. **Hypothesis (as filed):** the time is the per-reference `document_symbols` call. **Verdict:** partly. It was 7.5 s of about 22 s after the first fix, and none of the original 59 s: a single-file audit with no language-server wait took 20 s.
2. **Hypothesis:** the cost is CPU in the parser, not I/O. **Verdict:** confirmed, `byte_offset_to_line`, 4 of 4 stack samples.

## Fix

Done 2026-10-30:
- `parse_refs` computes the newline positions once and `byte_offset_to_line` binary-searches them (`partition_point`), which counts exactly the `\n` before the offset.
- `DocSymbolMemoScope` (`resolver.rs`) remembers a file's `document_symbols` answer for one audit loop; `call_with_index_cap` opens it around the synchronous loop and drops it right after. Only an ANSWER is remembered; a failed or timed-out call is asked again, as before.

Measured 2026-10-30, release builds, `emit_tracker: false`:

| | before | after |
|---|---|---|
| one file (`bug-fix-session-log.md`), CLI | 20.0 s | 0.36-0.40 s |
| full scan (2,915 files, 90,698 refs), CLI | 59.2 s | 14.7 / 15.1 s |
| full scan, over MCP | 59.0 / 55.1 / 60.4 s | 14.6 / 11.3 s |

The full-scan CLI JSON (230,416 bytes) is byte-identical before and after, for the parser fix alone and for both fixes.

## Tests added

`parser::line_lookup_tests` (the new function against the old counting formula at every char boundary, past the end, with CRLF and multibyte text; a newline belongs to its own line) and `resolver::tests::a_files_symbols_are_asked_once_inside_a_memo_scope_however_many_refs_cite_it` (counts mock `document_symbols` calls: 2 without a scope, +1 for three refs inside one, +1 after it ends; red when the memo lookup is disabled, verified by mutation). The mock gained `document_symbols_calls` and `MockLspProvider::with_shared_client`.

## Workarounds

Pass `paths` (plural) to scope the scan.

## Resume

Time the resolver's LSP calls with a span to size the saving.

## References

- `docs/issues/2026-10-16-audit-doc-refs-basename-index-cap-reports-existing-files-missing.md`

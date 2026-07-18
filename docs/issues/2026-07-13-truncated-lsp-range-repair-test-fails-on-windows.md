---
status: fixed
opened: '2026-07-13'
closed: '2026-07-19'
severity: medium
owner: marius
related:
- docs/issues/2026-07-10-edit-code-impl-method-selection-range-refusal.md
tags:
- lsp
- edit_code
- windows
- test-infra
kind: bug
---

# BUG: `edit_code_replace_repairs_truncated_lsp_range_from_ast` fails on Windows — "symbol not found" instead of exercising the range-repair path

## Summary
`docs/issues/2026-07-10-edit-code-impl-method-selection-range-refusal.md` was marked
`fixed` on 2026-07-13 (commit `1c04046b`), verified with "full `cargo test` = 3184
passed / 0 failed". On this Windows machine, the regression test added by that same
commit — `tools::symbol::tests::edit_code_replace_repairs_truncated_lsp_range_from_ast`
— fails, and fails for a *different* reason than the bug it's supposed to guard:

```
replace must repair the truncated range instead of refusing: symbol not found:
Point/distance — hint: Use symbols(path) to list symbols. Trait impl methods use
format 'impl Trait for Struct/method'.
```

This is a "symbol not found" (zero matches from `document_symbols`), not the
"suspicious range" error the fix targets — i.e. the mock's `document_symbols`
response for the test's synthetic file is never being returned at all on this
platform, so the range-repair code path under test is never reached.

## Reproduction
- Fails in isolation (`cargo test --lib tools::symbol::tests::edit_code_replace_repairs_truncated_lsp_range_from_ast -- --exact`), not a test-order/flake issue.
- Fails identically on pristine `origin/experiments` tip (`f886bc4c`, detached HEAD) — not caused by any local rebase/commit.
- Fails identically checked out directly at `1c04046b` (the commit that introduced the test and claimed it GREEN) — the "3184 passed / 0 failed" verification in the bug file was almost certainly run on a non-Windows machine (WSL/Linux), and the Windows case was never exercised.

## Hypothesis (untested, not yet confirmed by reading the code)
`src/lsp/mock.rs`'s `with_symbols(path, ...)` keys its canned response on exact path
equality ("The path must match exactly what the tool passes to `document_symbols`").
The test builds `file` from `tempfile::tempdir()` + `.join(...)`, while the actual
call in `src/symbol/query.rs` (`fetch_validated_symbol` → `client.document_symbols(path, lang)`)
resolves its own `path` via `Agent`/workspace root logic. On Windows, `tempfile::tempdir()`
paths and re-derived/canonicalized paths can differ in casing, short-name (8.3) vs
long-name form, or backslash/forward-slash normalization — any of which would break an
exact-match `HashMap`/`Vec` lookup keyed by `PathBuf`, causing `document_symbols` to
return empty for a path that "looks the same" to a human. This matches the broader
pattern of Windows path-normalization bugs already tracked elsewhere in this repo
(`fix: resolve remaining Windows path-normalization fallout from the forward-slash
series`, etc.) — not independently verified here, just the most likely candidate.

## Impact
- The regression test for the 2026-07-13 fix provides no coverage on Windows —
  Windows CI/local runs get a false "everything's fine" only because the failure
  reads as a *different* pre-existing test-infra issue, not a red flag on the fix
  itself. The actual production fix (`repair_symbol_range` in `src/symbol/query.rs`)
  is unverified on Windows.
- `cargo test --lib` on this machine reports 1 failure out of ~3005 total (not 0) for
  as long as this stands unfixed.

## Workaround
None applied — did not modify test infra or production code. Flagged only; a
release build (`cargo rb`) was still done via the documented `CARGO_TARGET_DIR`
workaround since this failure is unrelated to the code being shipped.

## Status
`fixed` (2026-07-19) — the path-mismatch hypothesis was confirmed. `Agent::new` canonicalizes
the project root (`std::fs::canonicalize`), which on Windows rewrites the path to the
extended-length `\\?\` verbatim form. The test built its `file`/`SymbolInfo.file`/mock key
from the RAW `tempfile::tempdir()` path, never canonicalized — so the mock's exact-`PathBuf`
lookup in `document_symbols` never matched what the tool actually resolved and passed at call
time, and the mock silently returned an empty symbol list ("symbol not found") instead of
exercising the range-repair path. Fixed by deriving `file` from `std::fs::canonicalize(dir.path())`
instead of the raw tempdir path, matching the established pattern already used elsewhere in
this test file (e.g. `references_honors_workspace_override_for_relative_path`). No production
code change was needed — `repair_symbol_range` itself was correct all along, just never
reached on Windows.

## Fix idea / Pointer
Add a debug assertion or `eprintln!` in the mock's `document_symbols` on a
miss (log the requested path vs the stored keys) to confirm/refute the
path-mismatch hypothesis on this machine, then normalize whichever side is
inconsistent (likely: canonicalize both the test's `file` and the mock's stored
key the same way `document_symbols`'s real caller does).

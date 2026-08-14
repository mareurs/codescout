---
id: '51d5a80b7fe2f125'
kind: bug
status: open
title: 'BUG: edit_code has no way to disambiguate two symbols with an identical name_path, so a duplicate impl block cannot be removed or replaced without reverting the file'
tags:
- edit-code
- name-resolution
- tool-friction
closed: null
opened: 2026-08-11
owner: marius
related:
- eac50663994f55de
severity: medium
---

## Summary

`edit_code(action="replace")` on a struct symbol whose replacement body also contained a full new `impl <Struct> { ... }` block did not consume or dedupe the pre-existing `impl` block that already followed the struct in the file — `replace` only swaps the targeted symbol's own span. The result was two syntactically identical `impl CodeEmbedderAdapter { ... }` blocks in one file (a genuine Rust duplicate-definition error: two inherent impls both defining `new`/`dimensions`/`check_dim`/`wrap`). Neither `edit_code(remove, symbol="impl CodeEmbedderAdapter")` nor `edit_code(replace, symbol="impl CodeEmbedderAdapter")` could recover, because `find_unique_symbol_by_name_path` (`src/symbol/query.rs:714`) resolves purely by name-path string equality — there is no line/position disambiguator in the current tool surface for two symbols that share the exact same name_path.


### Update 2026-08-14 — scope narrowed, and a separate `insert` defect found

Measured while adding a method to three identically-named `FixedEmbedder` impls in
`src/tools/memory/tests.rs`.

**The claim "no way to disambiguate two symbols with an identical name_path" is
true only for duplicates in the *same enclosing scope* — which is exactly this
bug's case, so the finding stands.** But it does not generalise. For duplicates in
*different* scopes the name path is already qualified by the enclosing item, and
`edit_code`'s ambiguity error hands the disambiguators back:

```
ambiguous name_path "impl DenseEmbedder for FixedEmbedder/embed" matches 3 symbols:
  memory_remember_then_recall_e2e_via_test_seams/impl DenseEmbedder for FixedEmbedder/embed,
  cross_embed_memory_stores_under_pinned_project_not_session_default/impl DenseEmbedder for FixedEmbedder/embed,
  memory_recall_signals_has_more_when_capped/impl DenseEmbedder for FixedEmbedder/embed
```

Those paths **resolve**, and `action="replace"` accepted all three. So the gap is
narrower than the title suggests: two `impl CodeEmbedderAdapter` blocks at *file*
scope share an enclosing scope and have no qualifier available, which is
genuinely unresolvable by name path — while same-name symbols in different scopes
are already addressable today. Any fix should target the same-scope case (a
line/position disambiguator) and need not touch name-path qualification.

**Separately — a worse adjacent defect.** Each of those three resolvable paths,
copied verbatim as the error instructed, failed on `action="insert"`:

```
cannot determine end of 'embed' for insert-after — AST parse failed
```

The hint reads *"The file likely has syntax errors that broke tree-sitter's
parse"*. **The file had none** — the only failure at that moment was semantic
(missing trait items after adding a trait method), which tree-sitter does not see.
So `edit_code` returns a disambiguator that its own `insert` path then refuses,
with a hint pointing at a cause that does not exist.

**Workaround (verified, used three times):** `action="replace"` on the whole impl
*object* rather than `insert`-after one of its methods. `replace` needs no
end-of-method bound, and it preserves the outer `#[async_trait::async_trait]`
attribute.

Taken together with `edit_file`'s keyword filter, the two tools deadlocked on "add
a method to a trait impl nested inside a test fn" until that workaround was found
— `edit_file` refused any content containing `fn `, and `edit_code` could not
bound the insertion point. The `edit_file` half is fixed in `138de7c5`; this half
is not.
## Symptom (Effect)

```
edit_code(action="replace", symbol="CodeEmbedderAdapter", path="src/retrieval/embedder.rs",
          body="struct CodeEmbedderAdapter { ... }\n\nimpl CodeEmbedderAdapter { /* new methods */ }")
  → succeeds, but the file now has TWO `impl CodeEmbedderAdapter` blocks (Rust: E0592 duplicate definitions)

edit_code(action="remove", symbol="impl CodeEmbedderAdapter", path="src/retrieval/embedder.rs")
  → fails: two symbols share the identical name_path "impl CodeEmbedderAdapter" — cannot address
    "the second one" or "the one at line N"
```

## Reproduction

1. Pick a struct with a following inherent `impl <Struct> { ... }` block.
2. `edit_code(action="replace", symbol="<Struct>", body=<new struct AND a restated impl block in one body>)`.
3. The file now has two `impl <Struct>` blocks.
4. `edit_code(action="remove"|"replace", symbol="impl <Struct>", ...)` → fails on ambiguous name_path; nothing in the tool surface can pick "the second one."

Branch: `feat/local-onnx-query-path`, Task 5 (`src/retrieval/embedder.rs`, `CodeEmbedderAdapter`).

## Environment

codescout `feat/local-onnx-query-path`, Linux, MCP stdio, rust-analyzer LSP.

## Root cause

`find_unique_symbol_by_name_path` (`src/symbol/query.rs:714`) matches by name-path string equality only; it has no secondary key (line number, byte offset) to break a tie when two symbols render to the identical name_path. Every `edit_code` action built on top of it (remove/replace/insert) inherits the same blind spot. *Inferred from two tool responses in one session — the resolver code was not read.*

## Evidence

Quoted from `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)":

> First attempt at the struct/impl edits used `edit_code(action="replace")` on the `CodeEmbedderAdapter` struct symbol with a body that also contained a full new `impl CodeEmbedderAdapter { ... }` block. `replace` only swaps the targeted symbol's own span — it does not consume or dedupe trailing content — so the pre-existing `impl CodeEmbedderAdapter { ... }` that immediately followed the struct was left in place, now duplicated... Neither `edit_code(remove/replace, symbol="impl CodeEmbedderAdapter")` nor a `new`-scoped name_path could disambiguate the two identical `impl CodeEmbedderAdapter` blocks (`find_unique_symbol_by_name_path`, `src/symbol/query.rs:714`, resolves purely by name-path string equality — no line/position disambiguator exists in the current tool surface).

## Hypotheses tried

None — filed on notice. The workaround (`git checkout` + redo as two separate calls) was faster than instrumenting the resolver in the moment.

## Fix

Not implemented. Candidate: when `find_unique_symbol_by_name_path` returns >1 candidate, extend the existing ambiguous-name_path error path (which already fires correctly for other duplicate-name cases) to accept an optional position hint (line number, or "nth match" ordinal) instead of only erroring.

## Tests added

None. A regression test would assert that `edit_code(remove, symbol=X, line=N)` (or equivalent) can select one of two symbols sharing name_path X, once such a parameter exists.

## Workarounds

Never combine "replace a container symbol" with "restate a sibling/child block that already exists elsewhere in the file" in one `edit_code(replace)` body — do the two edits as separate calls (container first, then the pre-existing block, addressed while it is still uniquely named). If a duplicate is already created and uncommitted, `git checkout -- <file>` is the fastest recovery.

## Resume

Two independent pieces now, and the second is the cheaper win:

**1. The same-scope disambiguator (the original bug).** Read
`find_unique_symbol_by_name_path` (`src/symbol/query.rs:714`) and its callers in
`edit_code`'s remove/replace/insert dispatch. Check whether the ambiguous-match
error already carries enough position data (line/byte range per candidate) to
expose a disambiguator parameter cheaply. Note the scope narrowing in § Summary →
*Update 2026-08-14*: only same-enclosing-scope duplicates need this.

**2. `insert`'s end-of-symbol bound for a nested trait-impl method**, plus its
misleading hint. Reproduce with three same-named local `impl` blocks in separate
test fns (`src/tools/memory/tests.rs` has exactly that shape today) and
`action="insert", position="after"` on a fully-qualified method path. Fix the hint
regardless of the bound: "the file likely has syntax errors" is wrong for a file
that only fails to type-check, and it sent one session hunting for syntax errors
that did not exist.
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)"
- `src/symbol/query.rs:714` — `find_unique_symbol_by_name_path`
- Related but a different mechanism (naming-form asymmetry between `symbols` and `edit_code` for trait-impl methods, not duplicate name_path resolution): `docs/issues/2026-08-08-symbols-and-edit-code-disagree-on-the-same-name-path.md`

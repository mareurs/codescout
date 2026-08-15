---
id: '51d5a80b7fe2f125'
kind: bug
status: fixed
title: 'BUG: edit_code has no way to disambiguate two symbols with an identical name_path, so a duplicate impl block cannot be removed or replaced without reverting the file'
tags:
- edit-code
- name-resolution
- tool-friction
closed: 2026-08-15
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

Shipped in `b2bc8edb`, taking the candidate this file proposed — a position hint
on the ambiguous path — and choosing **line over ordinal**.

An ordinal ("nth match") depends on traversal order, which is an implementation
detail the caller cannot see and which changes when the resolver does. A line is
already printed by `symbols`, stable, and verifiable by eye.

`edit_code` gains `at_line`, threaded through `fetch_validated_symbol` to the new
`find_unique_symbol_by_name_path_at`. Three details matter more than the parameter:

1. **The ambiguity error now carries each candidate's span.** With identical paths
   the span is the *only* thing distinguishing candidates — the old error printed
   the same string N times and offered no way forward. A disambiguator with no way
   to learn what to pass is not usable.
2. **`at_line` matches anywhere in the span**, not just the declaration line,
   because the number a caller has is whatever `symbols` printed — which may be a
   body line or the end.
3. **A line matching no candidate is its own error**, distinct from "symbol not
   found": the name *did* resolve. Conflating them would send the caller hunting
   for a wrong name when their name was right.

1-based on the surface, 0-based internally; converted once in `symbol_covers_line`
rather than asking callers to subtract.
## Tests added

`at_line_breaks_a_tie_no_name_can_break` — two blocks with byte-identical
`name_path`s at spans 11-21 and 31-41, covering:

- ambiguity error lists **both spans** (the fix's usability precondition);
- declaration line resolves;
- a **mid-body** line resolves — span match, not declaration match;
- both span ends are inclusive;
- a line in neither span errors *naming the line and listing the real spans*, and
  explicitly does **not** read as "not found";
- a line passed against an already-unique name still resolves — adding the
  parameter must never turn a working call into a failing one.

Gate: `cargo test --workspace` → 3810 passed / 0 failed / 50 ignored; clippy clean.
## Workarounds

Never combine "replace a container symbol" with "restate a sibling/child block that already exists elsewhere in the file" in one `edit_code(replace)` body — do the two edits as separate calls (container first, then the pre-existing block, addressed while it is still uniquely named). If a duplicate is already created and uncommitted, `git checkout -- <file>` is the fastest recovery.

## Resume

**Item 2 resolved 2026-08-15 in `cafa4b37`** — with a correction to how it was
framed here. Item 1 remains.

### 1. The same-scope disambiguator (the original bug) — fixed

Fixed in `b2bc8edb` — see § Fix. The investigation this section asked for
("check whether the ambiguous-match error already carries enough position data to
expose a disambiguator cheaply") had a clean answer: it did **not** carry position
data, and adding it was the larger half of the value. The parameter without the
spans in the error would have been unusable, because nothing told the caller which
line to pass.

The § Summary scope narrowing held: only same-enclosing-scope duplicates need
this, and different scopes still resolve by name alone.
### 2. The `insert` refusal — diagnosed, and it was not what this file said

This file described it as `insert`'s "end-of-symbol bound" failing. Measured, the
split is different and worth keeping straight:

- **The refusal is correct and stays.** BUG-051's residual closure refuses rather
  than trust an LSP end that can be short and would splice new code mid-body.
- **The message was the defect.** It asserted "AST parse failed" and hinted "the
  file likely has syntax errors" for every cause — false whenever the file parses,
  which is the usual case. It now checks `crate::ast::has_syntax_errors` and says
  which of the two actually applies, with the working workaround named
  (`action="replace"` on the enclosing symbol).
- **The underlying limitation is an LSP/AST asymmetry, not a bound bug.**
  `ast_does_not_expose_methods_of_an_impl_nested_in_a_function`
  (`src/tools/symbol/tests.rs`) measures it: tree-sitter's extractor surfaces
  `outer` but not the method of an impl nested inside it, on a fixture confirmed
  clean by `has_syntax_errors`. The LSP resolves that same symbol fine —
  `symbols(name="one")` returns it with a body. So there is nothing for the AST
  end-line resolver to match; it is not failing to bound a symbol it can see.

**If you want insert to actually work on this shape**, that is a third piece of
work and it belongs to the extractor, not to `edit_code`: teach
`crate::ast::extract_symbols` to descend into `impl` blocks inside function
bodies. The test above fails the moment it does, which is the intended signal to
come back and relax the refusal — the end would then be knowable. Until then
`action="replace"` on the enclosing symbol is the supported route, and the error
now says so.

### Related, and closed

The `edit_file` half of the deadlock (its keyword guard rejecting any content
containing `fn `/`trait `) is fixed in `138de7c5`. Together with the above, "add a
method to a trait impl nested inside a test fn" is no longer a deadlock: `replace`
on the impl object works, and `edit_file` no longer refuses identifiers that merely
contain a keyword.
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)"
- `src/symbol/query.rs:714` — `find_unique_symbol_by_name_path`
- Related but a different mechanism (naming-form asymmetry between `symbols` and `edit_code` for trait-impl methods, not duplicate name_path resolution): `docs/issues/2026-08-08-symbols-and-edit-code-disagree-on-the-same-name-path.md`

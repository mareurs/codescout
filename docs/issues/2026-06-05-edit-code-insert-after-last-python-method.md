---
status: fixed
opened: 2026-06-05
closed: 2026-06-05
severity: high
owner: marius
related: ["docs/issues/archive/2026-05-02-edit-code-insert-mid-function.md"]
tags: ["edit_code", "ast", "python", "silent-corruption", "insert-after", "parent-clamp"]
kind: bug
---

# BUG: `edit_code insert after` splices new sibling mid-body when inserting after the LAST child of a Python (dedent-delimited) class

## Summary
`edit_code(action="insert", position="after")` targeting the **last** method of a
Python class inserted the new sibling **before** that method's trailing statement,
orphaning it. The call returned `status: "ok"` — silent file corruption, invisible to
the error-rate metric. Observed in MRV-poc (`tests/lane_aware/test_contracts.py`,
`test_anchor_chunk_citation`); reproduced deterministically in codescout.

## Symptom (Effect)
`edit_code insert-after` returned success but produced:

```python
    def test_anchor_chunk_citation(self) -> None:
        draft = make_draft( ... )

    def test_newly_inserted(self) -> None:          # ← new method spliced in here
        assert True

        assert CID_ANCHOR in draft.used_chunks       # ← orphaned; belongs to the method above
```

The trailing `assert CID_ANCHOR in draft.used_chunks` (the last statement of
`test_anchor_chunk_citation`) ended up *after* the newly inserted method, at the wrong
nesting. No error; `"status": "ok", "inserted_at_line": N`.

## Reproduction
Commit `c99d4228e42f98d30a60ab8a55b1c431f4547ffb` (branch `experiments`).

Minimal fixture (`mod.py`) — the target method is the **last** child of its class and
ends with a trailing statement:

```python
class C:
    def m(self):
        x = compute(
            a=1,
        )

        assert x
```

```
edit_code(path="mod.py", symbol="C/m", action="insert", position="after",
          body="\n    def added(self):\n        assert True\n")
```

Result: the new `def added` lands **before** `assert x`, orphaning it. The tool reports
`inserted_at_line` one less than the correct position.

Deterministic — no LSP staleness or timing race required. Fires only for **insert-after
the last child** of a class in a **dedent-delimited language** (Python, …). Non-last
children and brace languages (Rust/Go/Java/Kotlin/TS) are unaffected.

## Environment
- Date: 2026-06-05
- Tool: `mcp__codescout__edit_code` (`action=insert`, `position=after`)
- Project observed: MRV-poc (Python); root-caused + fixed in codescout
- Branch: `experiments`

## Root cause
`src/tools/symbol/edit_code.rs::do_insert` computes the insertion index as
`insert_at0 = editing_end_line_strict(&sym) + 1` (the strict-AST end of the target
child + 1 — **correct**, proven robust), then clamps it to the parent body:

```rust
let parent_body_end_exclusive = parent.end_line as usize;   // <-- off by one
insert_at0.max(parent_body_start).min(parent_body_end_exclusive)
```

Tree-sitter's `end_line` is the last line a node **spans (inclusive)**. For brace
languages that is the closer `}` line — *not* a child line — so an exclusive bound of
`parent.end_line` is correct, and a child's strict-AST end is always strictly below it
(no clamping occurs). For **dedent-delimited languages there is no closer line**: the
class node ends on the *last child's last body line*, so `parent.end_line ==
last_child.end_line`. For the last child, `insert_at0 = last_child.end_line + 1 >
parent.end_line`, and `.min(parent.end_line)` pulls the insert index back by one — into
the child body, immediately before its final statement.

The corruption geometry (new sibling lands *above* the strict-AST-correct point) is a
fingerprint: `.min(parent_body_end_exclusive)` is the **only** index-reducing operation
in `do_insert`. `validate_symbol_position` validates only the target symbol, never its
parent, so the under-extended parent bound is never caught upstream.

This is a sibling of the archived `2026-05-02-edit-code-insert-mid-function.md` family
(insert-after under-extension on a parented symbol), but a **distinct mechanism**: that
bug was the child's end resolving to a short LSP value with AST failing (→ refusal). Here
the child end is correct and the *parent* bound is the off-by-one. Every fixture in the
archived bug was Rust, so the dedent-language case was never exercised.

## Evidence

### 1. AST extractor is correct (ruled out as cause)
A probe on `extract_python_symbols` (`src/ast/parser.rs`) showed the method's `end_line`
correctly includes a trailing statement after a blank line, even with syntax errors in a
*following* method:

```
[A_clean] m.end_line=8  syntax_err=false
[B_open_paren] m.end_line=8  syntax_err=true
[C_empty_body] m.end_line=8  syntax_err=false
[D_dangling] m.end_line=8  syntax_err=true
```

And class vs last-method end coincide (the structural key):

```
class C end_line=9 ; last method end_line=9
```

### 2. End-to-end repro against the shipped binary
`edit_code insert-after` on `C.test_anchor_chunk_citation` (last method) returned
`inserted_at_line: 15` (= index 14, the `assert` line) instead of 16, splicing the new
method before the trailing `assert` — see Symptom.

### 3. Regression test fails without the fix
`insert_code_after_last_python_method_keeps_trailing_stmt`: with `parent.end_line`
(no `+1`) the panic dumps `def added` before `assert x`; with the fix it passes.

## Hypotheses tried
1. **Blank line before the trailing statement confuses extraction.** Test: probe clean
   Python with trailing assert after a blank line. Verdict: **rejected** — `end_line`
   correct (Evidence 1).
2. **f-string `[{expr}]` / list literal confuses tree-sitter-python.** Test: probe exact
   MRV-poc body. Verdict: **rejected** — `end_line` correct.
3. **Error-recovery truncates the method node when a following method is broken.** Test:
   probe with unclosed paren / empty body / dangling token in a following method.
   Verdict: **rejected** — `end_line` stable at the correct value (Evidence 1).
4. **LSP staleness from the prior consecutive insert (9s earlier) gives a short
   `sym.start_line` / wrong AST match.** Test: traced `fetch_validated_symbol` +
   `validate_symbol_position`. Verdict: **rejected** — that path refuses (`None`), it
   does not silently under-extend; and the bug reproduces with a clean single insert.
5. **Parent clamp uses an off-by-one exclusive bound for dedent languages.** Test: traced
   every index-reducing op in `do_insert`; only `.min(parent.end_line)` qualifies;
   reproduced end-to-end (Evidence 2) and with a unit test (Evidence 3). Verdict:
   **confirmed**.

## Fix
The same off-by-one lived at **three** call sites in `src/tools/symbol/edit_code.rs`, all
converting `parent.end_line` into an *exclusive* clamp bound. Fix = use
`parent.end_line + 1` (the first line not in the parent body, correct for inclusive
tree-sitter node ends):

- `do_insert` (~line 769) — inline clamp. Fixed.
- `do_remove` (~line 454) — `clamp_range_to_parent(...)` call. Fixed.
- `do_replace` (~line 515) — `clamp_range_to_parent(...)` call. Fixed.

`do_remove`/`do_replace` were found by `references(clamp_range_to_parent)` during a
spot-check (session-log W-9, `docs/trackers/bug-fix-session-log.md`) and reproduced live
before fixing: replacing the last method left its trailing statement orphaned after the
new body; removing it left the statement behind. Brace languages are unaffected at all
three sites because a child's end is always strictly below the parent closer, so the
clamp never binds. The shared helper `clamp_range_to_parent` (`src/symbol/edit.rs:204`)
is unchanged — it is a pure clamp utility; the `+1` conversion lives at each call site.

Uncommitted on `experiments` as of writing (working-tree changes in `edit_code.rs` +
`tests/symbol_lsp.rs`). **Cite the master-side SHA here after cherry-pick** (per CLAUDE.md
§ "After cherry-pick: cite the master SHA").

## Tests added
All in `tests/symbol_lsp.rs`, after `insert_code_after_caps_overextended_lsp_end`. Each
uses `MockLspClient` with a Python class whose method shares `end_line` with the class (no
closer line). All verified **fails-without-fix / passes-with-fix** (toggle confirmed each
fails when the relevant `+1` is reverted):

- `insert_code_after_last_python_method_keeps_trailing_stmt`
- `replace_last_python_method_replaces_trailing_stmt`
- `remove_last_python_method_removes_trailing_stmt`

All 54 `symbol_lsp` integration tests pass, including every `bug034_guard_*` (parent-clamp
over-extension protection across Rust/Python/Java/Kotlin/TS) — the `+1` does not regress
the down-clamp guards.

## Workarounds
While unfixed: insert after a **non-last** method of the class, or use
`edit_code(action="replace")` on the whole class, or `edit_file` with explicit context.
The corruption only hits insert-after the *last* child of a dedent-delimited class.

## Resume
N/A — fixed across all three actions (insert/replace/remove). The spot-check that the
original Resume flagged ("`clamp_range_to_parent` may carry the same off-by-one for
dedent-language `replace`") was performed and **confirmed** — both `do_remove` and
`do_replace` shared the bug and are now fixed + regression-tested (W-9). One residual lead
if anything resurfaces: `position="before"` inserts use `editing_start_line` (a different
path) and were not exercised here — scout that path if a before-insert corruption is ever
reported. The shared `clamp_range_to_parent` helper itself is correct (pure clamp); the
fix is at the three call sites.

## References
- `src/tools/symbol/edit_code.rs::do_insert` — fix site.
- `src/symbol/edit.rs::clamp_range_to_parent` — shared replace-path clamp (potential same off-by-one, untested).
- `docs/issues/archive/2026-05-02-edit-code-insert-mid-function.md` — sibling bug, distinct mechanism (Rust, AST-fail → refusal).
- MRV-poc usage analysis that surfaced this: `docs/usage-reports/2026-06-05-usage-analysis-mrv-poc.md`.

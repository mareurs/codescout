# Diff-aware structural guard for `edit_file`

**Date:** 2026-06-16
**Status:** approved (design) — awaiting implementation plan
**Component:** `src/tools/edit_file/mod.rs` (`guard_structural_rewrite`)
**Related designs:**
- `2026-03-15-edit-file-hard-block-design.md` — the original structural hard-block
- `2026-06-04-edit-file-whitespace-normalized-fallback-design.md` — most recent `edit_file` change
- `2026-05-05-lsp-tool-enforcement-design.md` — the broader "route structural edits to `edit_code`" policy

## Problem

`edit_file`'s structural guard rejects an edit when `old_string` **or** `new_string`
merely *contains* a definition keyword (`fn`, `fun`, `class`, `struct`, `impl`,
`trait`, `enum`, `def`, `interface`, …) on a non-comment line — even when that
keyword line is **unchanged context** used only as an edit anchor, and the actual
change is pure formatting.

**Concrete repro (backend-kotlin, 2026-06-16).** Adding a single blank line before a
local `fun coveredFlat` to satisfy a ktlint `Expected a blank line for this declaration`
violation took **3 round-trips**:

1. `old_string` = `…val flatByTeacher…()\n            fun coveredFlat(…): Set<Int> =` → rejected (`edit contains a symbol definition ("fun ")`).
2. `old_string` = `…val flatByTeacher…()\n            fun coveredFlat` → rejected (still contains `fun `).
3. `old_string` = `…val flatByTeacher…()\n` (anchor on the `val` line only, `fun` never mentioned) → **accepted**.

The `fun coveredFlat` line was byte-identical in old and new on attempts 1–2 — it was
context, not the thing being changed. The guard could not tell "unchanged keyword line
used as anchor" from "structural rewrite of a keyword line".

## Root cause

`guard_structural_rewrite` (`src/tools/edit_file/mod.rs:201`) blocks when:

1. `old_string` **or** `new_string` is **multi-line** (`contains('\n')`), **and**
2. one of its **non-comment lines** contains a def keyword from
   `def_keywords_for_lang` (Kotlin: `fun `, `class `, `interface `, `enum `).

It inspects each string **in isolation for keyword presence** — it never compares
`old_string` against `new_string`, so it cannot distinguish an unchanged anchor line
from a line the edit actually rewrites.

Two non-obvious facts about the current guard (confirmed by reading the code
2026-06-16):

- **It only fires on multi-line strings.** A single-line `old_string`/`new_string`
  short-circuits to `None` before the keyword check. (This is why repro attempt 3 —
  whose `old_string` had only a trailing `\n` and no keyword — passed.)
- **Consequence:** single-line *structural* edits already bypass the guard entirely
  (`old="fun foo()"` → `new="fun bar()"` is allowed today). See **Out of scope**.

## Goal / success criteria

- A def keyword on a line that is **identical in `old_string` and `new_string`** (pure
  context/anchor) does **not** trip the guard.
- Genuine structural rewrites stay blocked:
  - **rename** — the def line differs between old and new, so the keyword is on a
    changed line;
  - **new-symbol introduction (BUG-050)** — a new def line in `new_string` is, by
    definition, an *added* line absent from `old_string`.
- **Relaxation-only.** The change can only *allow* edits the current guard blocks; it
  never blocks an edit the current guard allows.

## Design

Make the guard **diff-aware**: a def keyword trips it only when the keyword sits on a
line the edit *introduces or removes*, not on a line present (byte-identical) in both
strings.

Add a small helper that returns the lines of `from` that do not appear in `to`:

```rust
/// Lines present in `from` but not in `to` (exact, whitespace-sensitive match).
fn lines_only_in<'a>(from: &'a str, to: &str) -> impl Iterator<Item = &'a str> {
    let to_lines: std::collections::HashSet<&str> = to.lines().collect();
    from.lines().filter(move |l| !to_lines.contains(l))
}
```

Refactor `find_def_keyword` to scan an arbitrary line iterator (it already iterates
lines and skips comment lines internally — extract that predicate so it can run over
the changed-lines iterator). Then `guard_structural_rewrite` becomes:

```rust
let old_kw = old_string
    .contains('\n')
    .then(|| find_def_keyword_in_lines(lines_only_in(old_string, new_string), lang))
    .flatten();
let new_kw = new_string
    .contains('\n')
    .then(|| find_def_keyword_in_lines(lines_only_in(new_string, old_string), lang))
    .flatten();

let Some(keyword) = old_kw.or(new_kw) else { return Ok(()) };
// …unchanged Err(RecoverableError::with_hint(…)) below
```

Everything else in the function is unchanged: the `is_source_path` / `detect_lsp_language`
early returns, the multi-line gate, the error message, and the hint.

## Invariants preserved

- **Multi-line gate** stays (`contains('\n')`), so single-line behavior is byte-for-byte
  identical to today.
- **Comment skipping** stays (the extracted predicate carries it).
- **`def_keywords_for_lang`** is untouched.
- **BUG-050** protection holds: a new symbol in `new_string` is an added line → still blocked.
- **Rename detection** holds: a changed def line appears in both `lines_only_in(old,new)`
  and `lines_only_in(new,old)` → still blocked.
- **`is_structural_edit` (line 239)** calls `guard_structural_rewrite`, so the
  `debug_enforce_symbol_tools` routing inherits the fix automatically — no separate change.

## Decisions / edge cases

- **Exact line equality (whitespace-sensitive), by design.** Inserting a blank line
  leaves the keyword line's bytes unchanged → allowed (the repro case). Re-*indenting* a
  signature line changes its bytes → still blocked → falls back to `edit_code`. That is an
  acceptable conservative edge; covering it would require trim-based equality, which is a
  possible future relaxation, **not** part of this change.
- **Set-difference, not positional diff.** A `HashSet` line-membership check is sufficient
  for the guard's purpose; line multiplicity and position do not matter here. Simpler and
  allocation-light.

## Out of scope (documented follow-up, not fixed here)

Single-line structural edits bypass the guard entirely today because of the multi-line
gate (`old="fun foo()"` → `new="fun bar()"`). That is a separate *under-blocking* gap.
Fixing it would **tighten** behavior (start blocking edits allowed today) and risks
surprising existing callers — out of scope for this relaxation-only change. Track
separately if it becomes a real friction.

## Tests (in the `tests` module, `src/tools/edit_file/mod.rs:779`)

1. **Regression (the reported friction).** Kotlin file; `old` = `…val …()\n            fun coveredFlat`,
   `new` = `…val …()\n\n            fun coveredFlat`. Assert `guard_structural_rewrite(...).is_ok()`.
2. **Rename still blocked.** `old` = `fun foo() {\n  body\n}`, `new` = `fun bar() {\n  body\n}`
   (multi-line). Assert `Err` and that the reported keyword is `fun `.
3. **BUG-050 still blocked.** `old` = `anchorA\nanchorB`, `new` = `anchorA\nfun newFn() {}\nanchorB`
   (new symbol introduced in `new_string`). Assert `Err`.
4. **Comment-before-fun now allowed.** `old` = `val x\n    fun foo`, `new` = `val x\n    // helper\n    fun foo`.
   Assert `Ok` (the only added line is the comment; the `fun` line is unchanged).

**Risk flagged for implementation:** an existing test in the `tests` module may assert that
a *context-only-keyword* multi-line edit is **blocked** — i.e. it encodes the current bug as
expected behavior. The implementer must read the `tests` module first; if such a test exists,
update it to the new contract and call it out explicitly in the commit (do not silently flip an
assertion). Per CLAUDE.md's edit_file testing note, assert on a path-specific marker (the exact
keyword in the error, or `Ok`/`Err`) so a mis-routed test fails loudly.

## Prompt surface impact

- `src/prompts/guides/iron-laws-detail.md` (~L52) describes when `edit_file` is blocked.
  Add a one-line note: keyword-bearing **context** lines (identical in old and new) do not
  trip the guard. This is a `get_guide` topic — loaded fresh per session — so **no
  `ONBOARDING_VERSION` bump** (per the surface table in CLAUDE.md).
- No tool rename or parameter change → `prompt_surfaces_reference_only_real_tools` unaffected.

## Implementation notes (as shipped 2026-06-16)

- **Simpler than the Design sketch above.** `find_def_keyword` was *not* refactored to take an
  iterator; it is unchanged. The guard feeds it the joined changed lines instead:
  `find_def_keyword(&lines_only_in(old, new).join("\n"), lang)` (and the symmetric new→old),
  reusing its existing comment-skip + multi-line scan. The only new symbol is
  `lines_only_in(from, to) -> Vec<&str>`. This avoided touching the existing
  `find_def_keyword_ignores_class_in_comment` test and any dead-code (clippy `-D warnings`).
- **Decision (user, 2026-06-16): embrace body-internal edits.** A multi-line `edit_file` whose
  definition line is byte-identical in old/new (a function-body edit) is now *allowed*, not routed
  to `edit_code`. This reverses the prior policy. Four existing integration tests
  (`edit_file_blocks_def_keyword_on_lsp_language`, `edit_file_warns_multiline_python`,
  `batch_edit_blocks_structural_rewrite`, `edit_file_batch_mixed_structural_lists_safe_indices_in_hint`)
  were repurposed to use **rename** fixtures so they still exercise structural *blocking* under the
  refined definition; a new `edit_file_allows_body_edit_on_lsp_language` covers the allow path.
- The single-line structural-edit gap remains out of scope (unchanged).

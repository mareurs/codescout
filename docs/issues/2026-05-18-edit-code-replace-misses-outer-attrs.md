---
status: wontfix
opened: 2026-05-18
closed: 2026-05-18
severity: low
owner: marius
related: [src/tools/edit_code/]
tags: [edit_code, range, outer-attributes]
kind: bug
---

# BUG: `edit_code(action="replace")` does not include outer attributes in the replace range

## Summary


When replacing a symbol via `edit_code(action="replace")`, outer attributes
(`#[allow(...)]`, `#[derive(...)]`, etc.) that sit directly above the
symbol — with NO doc comment above the attribute block — are
intentionally PRESERVED, not replaced. The `new_body` only overrides the
declaration body; existing attributes stay in the file. Users wanting to
drop a stranded attribute must use `edit_file` separately. This is a
documentation gap, not a behavior bug.
## Symptom (Effect)

Observed during Task 4 of the jsonpath-negative-slice plan execution
(session 2026-05-17, before the current 2026-05-18 session).
The implementer agent reported it had to use:

```
python3 -c "
import re
s = open('src/tools/file_summary/file_summary.rs').read()
open('src/tools/file_summary/file_summary.rs','w').write(
    re.sub(r'#\\[allow\\(dead_code\\)\\]\\n', '', s, count=1)
)
"
```

…because `edit_code(action="replace", symbol="extract_json_path_v2")`
replaced the function body but left the `#[allow(dead_code)]` attribute
line above it stranded — pointing at a now-unrelated function.

## Reproduction

1. Create a source file with an outer attribute above a symbol:

   ```rust
   // src/lib.rs
   #[allow(dead_code)]
   fn old_name() -> u32 { 0 }
   ```

2. Call:

   ```
   edit_code(action="replace",
             path="src/lib.rs",
             symbol="old_name",
             body="fn old_name() -> u32 { 42 }")
   ```

3. Observe file content:

   ```rust
   #[allow(dead_code)]              ← stranded
   fn old_name() -> u32 { 42 }
   ```

   Expected: either include the attribute in the replace range so the
   caller can drop it, or document that attributes must be re-stated in
   the new body.

## Environment

- codescout v0.12.1 release build.
- Linux 7.0.0-15-generic.
- LSP backend rust-analyzer (range comes from LSP `documentSymbol` or
  similar — outer attributes are typically reported as separate
  trivia, not part of the symbol's `range`).

## Root cause


**Not a bug — intentional behavior.** Confirmed via reconnaissance against
`src/symbol/edit.rs:43-104` (`editing_start_line`) and the regression test
`editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block`
at `src/tools/symbol/tests.rs:2802-2827`.

History:

- **BUG-031:** initially extended the replace range backward to include
  `///` doc comments above a function, so `new_body` containing doc
  comments could fully replace them without duplication.
- **BUG-037:** added a guard preventing the walk-back when only `#[...]`
  attributes (no doc comments) sit above. Reason: an LLM's `new_body`
  typically starts at `impl`/`fn` (matching what `symbols(name=...)`
  reports), so walking back would silently DELETE the attributes that
  the LLM didn't include. The current code at
  `src/tools/symbol/edit_code.rs:466-543` further narrows the start
  forward if the LLM's `new_body` doesn't lead with a decorator —
  belt-and-braces protection.

Result: stranded attributes are a feature, not a bug. They protect
against the more dangerous failure mode (silent attribute drop).
## Evidence

- Subagent task log in the prior session (compaction summary cited the
  `python3 -c re.sub` fallback).
- Conventionally, every LSP backend reports `documentSymbol.range`
  starting at the declaration keyword, not the attribute line.

## Hypotheses tried

1. **Hypothesis:** `edit_code(action="remove")` followed by `insert`
   might be a workable workaround pair.
   **Test:** Not yet exercised. Would require knowing the attribute
   range separately.
   **Verdict:** deferred — needs experimentation.

## Fix


**Wontfix — current behavior is correct.** The Task 4 implementer's
pain was real but the fix is documentation, not code:

1. Update the `edit_code` tool description (in `src/tools/symbol/edit_code.rs::EditCode/description`)
   to call out: "Outer attributes immediately above the symbol (with no
   doc comment between them and the attribute block) are preserved by
   `replace`. To drop or replace them, use `edit_file` with the
   attribute text as `old_string` after the `replace` lands."

2. Optional: surface this in the tool's success response when the LLM
   asks for `replace` on a symbol that has stranded attributes. E.g.
   `replaced_lines: "N-M", preserved_attributes: ["#[allow(dead_code)]"]`.
   Lets the LLM know it might want a follow-up `edit_file` call.

Future opt-in (separate ticket if needed): an explicit param like
`include_outer_attributes: true` on `edit_code(replace)` that disables
the BUG-037 guard for this call. Only justify if multiple sessions hit
this. Currently 1 datapoint (Task 4 of jsonpath fix).
## Tests added

`N/A — bug only filed.` Future fix should add:

- `replace_includes_outer_allow_attribute_above_fn`
- `replace_includes_outer_derive_above_struct`
- `replace_includes_outer_doc_comment_above_fn`
- `replace_includes_multiple_attributes_above_symbol`
- `replace_does_not_extend_past_blank_line_above_attributes`
- `remove_drops_outer_attribute_too`

## Workarounds


After `edit_code(action="replace")` leaves a stranded attribute, use
`edit_file` to drop the attribute line:

```
edit_file(path="...", old_string="#[allow(dead_code)]\n", new_string="")
```

No need for `python3 re.sub` — `edit_file` is the supported workaround.
The Task 4 implementer reached for Python because the discoverability
of this pattern is poor (the tool description doesn't surface it).
That's the actual fix surface: prompt/docs, not code.
## Resume

Locate the symbol-range resolver in `src/tools/edit_code/` (suspected
file: `mod.rs` or a `range.rs` helper). Read the symbol-resolution
path and find where the LSP/tree-sitter range is returned. Wrap that
range with an `extend_backward_to_include_attributes` helper following
the algorithm in the Fix section. TDD with the 6 test cases above.
Update the tool description in `src/tools/edit_code/mod.rs` and the
schema in the registration site (`src/server.rs::CodeScoutServer::
from_parts`) if user-visible behavior changes warrant it.

## References

- Discovered 2026-05-17 during jsonpath fix (session compacted before
  this file was written).
- Workaround pattern documented in the jsonpath bug's Resume section,
  closed as `4011ed2a`-adjacent work.

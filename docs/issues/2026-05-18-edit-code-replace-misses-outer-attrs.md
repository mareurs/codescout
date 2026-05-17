---
status: open
opened: 2026-05-18
closed:
severity: medium
owner: marius
related: [src/tools/edit_code/]
tags: [edit_code, range, outer-attributes]
---

# BUG: `edit_code(action="replace")` does not include outer attributes in the replace range

## Summary

When replacing a symbol via `edit_code(action="replace")`, the range used
is the symbol's declaration body — but outer attributes (`#[allow(...)]`,
`#[derive(...)]`, `#[cfg(...)]`, doc comments, etc.) that sit on the lines
ABOVE the symbol are NOT included in the replace span. The result: if the
new body needs to drop or change an outer attribute, the old attribute
lines remain in the file alongside the new declaration. Users have to
fall back to `edit_file` or shell tools to strip the leftover attributes.

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

`Unknown — best lead:` the symbol-range resolution in `edit_code` uses
the LSP-reported `Range` which conventionally starts AT the declaration
keyword (`fn`/`struct`/`impl`/etc.), not at the first attribute or doc
comment. Tree-sitter-based fallback paths may have the same convention.

To include outer attributes, the replace-range must be extended backward
from the symbol's `start_line` to include any contiguous lines that
start with `#[...]` or `#![...]` or `///` or `//!` (and preserve
trailing blank lines if any between the attribute cluster and a leading
doc/comment cluster).

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

In the symbol-range resolver inside `src/tools/edit_code/`:

1. Read the file lines.
2. Walk backward from `start_line - 1` while the line (after trimming)
   starts with `#[`, `#![`, `///`, `//!`, or is a contiguous-trivia
   blank line BETWEEN such markers.
3. The earliest such line becomes the new `start_line` for `replace`.
4. Document the behavior in the tool description: "replace includes
   outer attributes and doc comments above the symbol".

Edge cases: do NOT include preceding `//` line comments that aren't
doc comments — those are unrelated. Do NOT cross a blank line that
breaks the attribute cluster from arbitrary code above. Specifically:
the extension stops at the first line that's neither an attribute
nor a doc comment nor a blank-line-within-trivia.

For `action="remove"` and `action="rename"`: the same logic should
apply — removing a symbol should remove its attributes too (orphan
attributes after a remove is at least as bad as after a replace).

## Tests added

`N/A — bug only filed.` Future fix should add:

- `replace_includes_outer_allow_attribute_above_fn`
- `replace_includes_outer_derive_above_struct`
- `replace_includes_outer_doc_comment_above_fn`
- `replace_includes_multiple_attributes_above_symbol`
- `replace_does_not_extend_past_blank_line_above_attributes`
- `remove_drops_outer_attribute_too`

## Workarounds

After `edit_code(action="replace")`, follow up with a targeted
`edit_file` to drop the stranded attribute line(s):

```
edit_file(path="...", old_string="#[allow(dead_code)]\n", new_string="")
```

Or use shell — but `edit_file` is preferred (no pipe-IL3 risk).

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

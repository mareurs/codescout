---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: medium
owner: marius
related: []
tags: [read_file, format_compact, line-numbers, regression]
kind: bug
---

# BUG: read_file renders slice-relative (wrong) line numbers on ranged reads, and they pollute output

## Summary
A `read_file(path, start_line=N, end_line=M)` whose slice is small enough to
return inline renders each content line prefixed with a line number that starts
at **1**, not at **N**. So `read_file(start_line=4817, end_line=4965)` shows the
content as `1| … 149|` instead of `4817| … 4965|`. The numbers are both **wrong**
(misleading) and **noise** — the caller already passed the range, so it knows the
bounds.

## Symptom (Effect)
`read_file("src/tools/symbol/tests.rs", start_line=4817, end_line=4965, force=true)`
renders:
```
149 lines

  1| #[test]
  2| fn find_references_basic() {
  ...
149| }
```
The `1|…149|` prefixes correspond to file lines `4817…4965`. A reader citing
`file:line` from this output would cite the wrong line.

## Reproduction
1. `read_file(path, start_line=N, end_line=M)` where the slice is under the inline
   byte budget (so it returns `{content}` inline, not a buffered `@file_*` ref).
2. Observe the rendered text: line numbers run `1..(M-N+1)`, not `N..M`.

Large ranged reads are NOT affected: they buffer and return `shown_lines`, and the
renderer's chunked branch numbers correctly with `start + i`.

## Environment
codescout v0.13.0 (post-`40c4b828`), MCP, project code-explorer, branch experiments.

## Root cause
`format_read_file` (`src/tools/read_file.rs`, content-mode branch ~line 740) numbers
lines with `let lineno = i + 1;` — slice-relative — because the small-range result
shape is just `{ "content": ... }` with **no start offset** (no `shown_lines`). The
same branch serves two cases that need different numbering:
- **full small file** (no range) → `1..N` is correct.
- **small explicit range** → should be `start..end`, but the branch has no `start`.

The chunked/buffered branch carries `shown_lines` and numbers correctly
(`start + i`); only the inline content-mode branch lacks the offset.

**Regression exposure:** before `40c4b828` (the OutputForm::Text compaction flip),
small `read_file` results were emitted as raw JSON `{content}` with no rendered line
numbers, so the wrong numbers were never shown. The flip routed the small path
through `format_compact`/`format_read_file`, surfacing the latent slice-relative
numbering.

## Evidence
- Renderer: `src/tools/read_file.rs` `format_read_file` content-mode loop uses
  `i + 1`; chunked branch (shown_lines present) uses `start + i`.
- Result shape: read_file's line-range branch returns `json!({ "content": content })`
  for sub-budget slices (no `shown_lines`/start).
- Commit `40c4b828` added `OutputForm::Text` to ReadFile, routing the small path
  through the renderer.

## Hypotheses tried
N/A — root cause clear from the renderer + result-shape code.

## Fix

**Direction chosen (reporter): drop line-number prefixes entirely.** `format_read_file`
(`src/tools/read_file.rs`) no longer prefixes content lines with `N| `. Both the
chunked branch (was `start + i`) and the inline content-mode branch (was the buggy
`i + 1`) now emit the content verbatim after the `N lines` header. Buffer/Next/overflow
footers are unchanged. Scope: `read_file` only — `read_markdown` (already prefix-free)
and `grep` (line numbers are its purpose) are untouched.
## Tests added

Updated existing `format_read_file` tests in `src/tools/edit_file/tests.rs` to the
no-prefix contract: `read_file_content_mode_basic`, `read_file_content_mode_single_line`,
`format_read_file_auto_chunked`, `format_read_file_auto_chunked_mid_file`, and
`read_file_lineno_alignment` (repurposed to assert prefixes are dropped). Also updated
`read_file_call_content_returns_line_numbered_text_not_json` in `src/tools/read_file.rs`
(now asserts raw text + no `N| ` prefix). Full lib suite green (2427 pass; the only
failures are the 5 pre-existing unrelated ones: artifact-not-registered ×4, probe ×1).
## Workarounds
Ignore the numeric prefixes on ranged reads; the real file lines are
`start_line + (shown - 1)`. Or read without a range for small files.

## Resume
Pick fix direction (1 vs 2). For (1): in `src/tools/read_file.rs` line-range branch,
include `shown_lines: [start, end]` on the inline `{content}` result so
`format_read_file` takes its chunked numbering path. For (2): in `format_read_file`
content-mode branch, drop the `{lineno}| ` prefix (or gate it on a full-file read).
Update/extend the `format_read_file_*` tests in `src/tools/edit_file/tests.rs`.

## References
- `src/tools/read_file.rs` — `format_read_file`, line-range branch in `call`.
- `docs/trackers/output-form-text-compaction.md` — the OutputForm::Text work that
  exposed this (`40c4b828`).

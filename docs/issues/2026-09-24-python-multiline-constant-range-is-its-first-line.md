---
status: open
opened: 2026-09-24
closed:
severity: medium
owner: marius
related: []
tags: [cluster/capped-result-presented-as-complete]
kind: bug
---

# BUG: a multi-line Python constant's range is its first line, so `symbols` shows a fragment and `edit_code` cannot edit it

## Summary
pyright reports a module-level assignment's `DocumentSymbol.range` as the name alone.
codescout takes `end_line` from that range verbatim, so every consumer of the range sees
a one-line symbol: `symbols(include_body=true)` returns `NAME = (` as the whole body, and
`edit_code(replace)` rewrites only that line. Rust `const` bodies are unaffected.

## Symptom (Effect)
```
symbols(name="LIBRARIAN_TOOLS", path="scripts/probe_librarian_scope.py", include_body=true)
→ Constant  80  LIBRARIAN_TOOLS
      LIBRARIAN_TOOLS = (
```
Control, same call on a Rust constant — full body:
```
symbols(name="SYSTEM_PATHS", path="src/embed/preflight.rs", include_body=true)
→ Constant  54-56  SYSTEM_PATHS   (all three lines)
```
Sibling, on a 4-line tuple `MARKERS = (\n "alpha",\n "beta",\n)`:
```
edit_code(action="replace", symbol="MARKERS", body='MARKERS = ("gamma",)')
→ edit_code('MARKERS') left the file syntactically invalid — ... the edit most likely
  overshot into adjacent code and took a delimiter with it. File restored.
```
The diagnosis is backwards: the edit **undershot** — it replaced line 1 and orphaned the
tuple's contents. The syntax guard is the only reason this did not write a broken file.

## Reproduction
Live binary at `506924f2`. Any `.py` file with a multi-line tuple/list/dict/call assigned at
module level; run the two calls above.

## Environment
Linux, pyright-langserver, stdio MCP, branch `experiments`.

## Root cause
`convert_document_symbols` (`src/lsp/client.rs:255`) sets `end_line: ds.range.end.line`;
`workspace_symbols` (`src/lsp/client.rs:1024`) likewise uses `location.range.end.line`.
pyright's range for a variable covers the name node only. `symbol_to_json`
(`src/symbol/query.rs:103`) slices the body with that `end_line` on purpose — its comment says
it must "match what replace_symbol would replace" — so the body and the edit are wrong together.
The tree-sitter Python extractor (`src/ast/parser.rs:477`) emits no assignments, so nothing
corrects it.
measured 2026-09-24: the two `symbols` calls above, and the `edit_code` call on a scratch file.

## Fix

Extend the range where Python symbols enter codescout, so `symbols` bodies and `edit_code`
ranges are both corrected:

- `extend_python_assignment_ranges` (`src/ast/python_ranges.rs`) — pure tree-sitter pass. Maps
  each assignment target identifier's `(row, col)` to its statement's last row, and widens every
  `Constant`/`Variable` symbol starting there. Never shrinks; recurses into children; skips source
  that does not parse cleanly.
- Called from `with_python_ranges` in both branches of `LspClient::document_symbols`, and from
  `with_python_ranges_by_file` in `LspClient::workspace_symbols` (in place, order preserved).

Fix SHA / patch-id: _recorded at commit time_.
## Tests added

- 11 unit tests, `src/ast/python_ranges.rs` `tests` — every load-bearing site was mutated and
  each mutation was killed (7/7). One fixture needed a probe first: the `has_error` guard survived
  two broken-source fixtures that recover into no `assignment` node at all.
- `with_python_ranges_widens_a_multi_line_constant_for_python_only`,
  `with_python_ranges_by_file_scopes_each_parse_to_its_own_file_and_keeps_order` —
  `src/lsp/client.rs` tests; run in the gate.
- `python_multi_line_constant_body_and_replace_cover_the_whole_statement` —
  `tests/bug_regression.rs`, `#[ignore]` (needs pyright), so **the gate does not run it**. Run by
  hand 2026-09-24: green; RED with the language gate in `with_python_ranges` disabled
  ("body must run to the closing paren"), which is what shows it exercises pyright rather than
  skipping.
## Workarounds
`read_file(path, start_line, end_line, force=true)` with a guessed range; `edit_file` with
unique anchors instead of `edit_code`.

## Resume

N/A once committed. Tag this file through the catalog after merge (a frontmatter edit does not
reach it — BL-48 — and the catalog cannot see worktree files).
## References
- Source report: `codescout-lessons.md` § 6.4 (repo root, untracked).

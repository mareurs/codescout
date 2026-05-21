---
kind: tracker
status: active
title: OutputForm::Text compaction sweep
owners: []
tags:
  - output
  - token-efficiency
  - format_compact
---

# OutputForm::Text compaction sweep

Sweep of tools that own a `format_compact` renderer but never declared
`OutputForm::Text`, so the renderer only fires on the buffered (large-byte)
axis — small outputs fall through to pretty-JSON. Started after a user hit raw
JSON from `read_markdown` on a 160-line ToC.

## Root cause

`Tool::call_content` (`src/tools/core/types.rs`) picks output form on two axes:

- **size** — output > inline budget → buffer + compact summary (uses `format_compact`)
- **form** — small output: `OutputForm::Text` → `format_compact`; `OutputForm::Json` (default) → pretty JSON

A tool with a `format_compact` but the default `output_form()` renders JSON on
every sub-threshold call. Fix is a one-line override **only when the renderer is
lossless** (reformat, not summary). Lossy summary renderers must stay JSON on the
small path or they hide data.

## Classification rule

- **Lossless reformat** (text strictly tighter, same data) → add `OutputForm::Text` + regression test.
- **Lossy summary** (counts, truncated previews) → leave JSON on small path; flipping hides info.

## Candidates

| Tool | Renderer | Class | Verdict | Status |
|---|---|---|---|---|
| `read_markdown` | MAP/CONTENT/ERROR, full | lossless | flip | **done** (this session) |
| `read_file` | `format_read_file` — full content + `N\|` line nums | lossless | flip | **done** (this session) |
| `symbol_at` | def + hover, full text | lossless | flip | **done** (this session) |
| `ast` (list_functions/list_docs) | name+location lists | lossless (locator) | flip after verify | open — next concrete |
| `memory` (list/read) | topic list / content | lossless if `format_read_memory` shows full body | verify then flip | open |
| `library` (list) | lib name+language rows | lossless (locator) | flip after verify | open |
| `semantic_search` | `format_semantic_search` — **50-char snippet truncation** | lossy | decision needed (snippet len vs locator) | open |
| `config`/workspace status | `format_project_status` | maybe drops index-staleness fields | verify field-by-field | open |
| `index` / `run_command` / `usage` / `onboarding` | count/status summaries; JSON already compact or pre-buffered | low value | skip | open |

Already correct (Text declared): `grep`, `tree`, `symbols`, `references`, `call_graph`.

## Proposed guard

A test that enumerates tools with `format_compact` but no `output_form` override,
forcing each new tool to make an explicit lossless/lossy decision rather than
silently defaulting to JSON. Not a blanket flip — class (b) renderers are
intentionally lossy.

## Done log

- `read_markdown` — `OutputForm::Text` override + `read_markdown_call_content_returns_text_map_not_json` regression test. markdown suite 138/138, clippy clean. On `experiments`, not yet committed.
- `read_file` — `OutputForm::Text` override + `read_file_call_content_returns_line_numbered_text_not_json` regression test. Verified `format_read_file` lossless across all branches (content/chunked + source/markdown/json/toml/yaml/config/generic summaries). read_file suite 67/67, clippy clean. On `experiments`, not yet committed.
- `symbol_at` — `OutputForm::Text` override + `symbol_at_declares_output_form_text` + `symbol_at_format_compact_preserves_def_and_hover` tests. `format_goto_definition`/`format_hover` lossless (full def context + hover content; only strips ``` fences). symbol_at suite 9/9, clippy clean. On `experiments`, not yet committed.

## Resume

Next: `ast` (list_functions/list_docs) — verify `format_list_functions` /
`format_list_docs` are pure name+location locators (no dropped fields), then flip.
Then `library`/`memory` list views after the same verify. Hold `semantic_search`
pending a snippet-length decision (50-char truncation is the only
meaningful-info-loss risk in the set). Consider the guard test once a 4th concrete
lands.

Note: `librarian`/`artifact` tool registration — check if the reconnected/rebuilt
MCP now registers it (the 4 `server::guide_hint_tests` failures were
`tool 'artifact' not registered`). If registered, reindex so `kind: tracker`
queries surface this file.

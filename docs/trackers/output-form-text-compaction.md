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
- **Lossy summary** (counts, truncated previews, capped lists) → leave JSON on small path; flipping hides info.

The verify step is load-bearing: `ast` was guessed "lossless locator" but turned
out lossy (see below). Read the renderer body before flipping — never trust the
table's prior guess.

## Candidates

| Tool | Renderer | Class | Verdict | Status |
|---|---|---|---|---|
| `read_markdown` | MAP/CONTENT/ERROR, full | lossless | flip | **done** |
| `read_file` | `format_read_file` — full content + `N\|` line nums | lossless | flip | **done** |
| `symbol_at` | def + hover, full text | lossless | flip | **done** |
| `library` | `format_list_libraries` — all libs + indexed/stale flags, no cap | lossless | flip | **done** |
| `memory` | `format_list_memories` (all names) / `format_read_memory` (full content) | lossless | flip | **done** |
| `ast` (list_functions/list_docs) | `format_list_functions` **drops start/end_line + caps at 8**; `format_list_docs` **caps at 3 + truncates to 72 chars** | **lossy** | **do NOT flip** (or redesign renderer to be lossless first) | closed — won't flip |
| `semantic_search` | `format_semantic_search` — **50-char snippet truncation** | lossy | decision needed (snippet len vs locator) | open |
| `config`/workspace status | `format_project_status` | maybe drops index-staleness fields | verify field-by-field | open |
| `index` / `run_command` / `usage` / `onboarding` | count/status summaries; JSON already compact or pre-buffered | low value | skip | open |

Already correct (Text declared): `grep`, `tree`, `symbols`, `references`, `call_graph`.

## Proposed guard

5 concretes have now landed (read_markdown, read_file, symbol_at, library,
memory) and 2 lossy renderers correctly stayed JSON (ast, semantic_search) — the
pattern is mechanical enough to warrant a guard. Add a test that enumerates tools
with `format_compact` but no `output_form` override, forcing each new tool to make
an explicit lossless/lossy decision (allowlist the intentional lossy ones) rather
than silently defaulting to JSON. Not a blanket flip — lossy renderers are
intentional.

## Done log

- `read_markdown` — `OutputForm::Text` + `read_markdown_call_content_returns_text_map_not_json`. Shipped in `experiments:40c4b828`.
- `read_file` — `OutputForm::Text` + `read_file_call_content_returns_line_numbered_text_not_json`. `format_read_file` lossless across all branches. Shipped in `40c4b828`.
- `symbol_at` — `OutputForm::Text` + `symbol_at_declares_output_form_text` + `symbol_at_format_compact_preserves_def_and_hover`. Shipped in `40c4b828`.
- `library` — `OutputForm::Text` on the unified `Library` tool + `library_declares_output_form_text`. `format_list_libraries` lossless (all libs, no cap). library suite 12/12, clippy clean. On `experiments`, **not yet committed**.
- `memory` — `OutputForm::Text` on the unified `Memory` tool + `memory_declares_output_form_text`. `format_list_memories`/`format_read_memory` lossless. memory suite 52/52, clippy clean. On `experiments`, **not yet committed**.
- `ast` — **investigated, will NOT flip.** Both renderers are lossy (drop line numbers, cap lists, truncate docs); the JSON is the better small-path output. Verdict recorded; no code change.

## Resume

Two paths remaining:
1. **The guard test** (recommended next) — pin the lossless/lossy decision so new
   tools can't silently default to JSON. Allowlist `ast`, `semantic_search`, and
   the skip-tier tools as intentional JSON.
2. `config`/workspace status — verify whether `format_project_status` drops any
   index-staleness sub-fields before deciding; flip only if lossless.

`semantic_search` stays JSON pending a deliberate snippet-length decision (50-char
truncation is real info loss for inline snippet reading).

Live verification: `~/.cargo/bin/codescout` is now a symlink → `target/release/codescout`,
so `cargo build --release` + `/mcp` reconnect picks up changes (no `cargo install`).
read_markdown/read_file/symbol_at verified live this session.

Note: `librarian`/`artifact` tool is NOT registered in the current build, so this
tracker can't be reindexed via MCP and `kind: tracker` queries won't surface it.
Tracked as task #6 (verify reindex once the tool is registered; same root cause as
the 4 `server::guide_hint_tests` failures, `tool 'artifact' not registered`).

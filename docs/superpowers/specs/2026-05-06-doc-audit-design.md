# Doc Audit Design — 2026-05-06

Comprehensive audit and update of all internal and external codescout documentation.
Scope: README.md, `docs/manual/src/` (external/GitHub-visible), CLAUDE.md, MEMORY.md (internal).
Prompt surfaces (`server_instructions.md`, `onboarding_prompt.md`) are excluded.

## Approach

Thematic pass — four themes executed in order, each touching all relevant surfaces atomically.

---

## Theme 1 — Deletions

Remove stale or superseded files entirely.

### 1a. Delete `docs/ARCHITECTURE.md`

File is internal-only detail that either duplicates manual content or belongs nowhere.
No replacement; no links to update (manual links go to manual pages, not this file).

**Actions:**
- `git rm docs/ARCHITECTURE.md`

### 1b. Remove superseded librarian consolidation page

`docs/manual/src/experimental/librarian-tool-consolidation.md` describes the 22→16 tool
consolidation. This is superseded by `librarian-tools-collapse.md` (16→5 consolidation).
Keeping both causes confusion about the current tool surface.

**Actions:**
- `git rm docs/manual/src/experimental/librarian-tool-consolidation.md`
- Remove its entry from `docs/manual/src/experimental/index.md`

---

## Theme 2 — Correctness Fixes

Fix factually wrong information in existing docs.

### 2a. README: tool count

`## Tools (22)` is wrong. Current registered tools in `src/server.rs` (as of 2026-05-06):

| Category | Tools |
|---|---|
| File operations | `read_file`, `tree`, `grep`, `create_file`, `edit_file`, `edit_markdown`, `read_markdown` |
| Shell | `run_command` |
| Workflow | `onboarding`, `approve_write`, `workspace` |
| Symbol navigation | `symbols`, `references`, `symbol_at`, `call_graph`, `edit_code` |
| Memory | `memory` |
| Semantic search | `semantic_search`, `index` |
| Library | `library` |

Total: **20 tools**.

**Actions:**
- Update heading: `## Tools (20)`
- Update category breakdown line to match the table above

### 2b. README: MCP config path

Prose says to add MCP config to `~/.claude.json` — this file does not exist in Claude Code.
Correct path is `~/.claude/settings.json`.

**Actions:**
- Fix the prose around the JSON snippet: `~/.claude.json` → `~/.claude/settings.json`

### 2c. MEMORY.md: stale tool count and names

The "Key Architecture Facts" section of the auto-memory file
(`~/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory/MEMORY.md`)
says "25 tools registered (as of 2026-03-21)" and lists
old pre-rename tool names (`list_dir`, `search_pattern`, `find_symbol`, `find_references`,
`list_symbols`, `replace_symbol`, `insert_code`, `rename_symbol`, `remove_symbol`,
`goto_definition`, `hover`, `index_project`, `index_status`, `activate_project`,
`project_status`, `list_libraries`, `register_library`).

Current count is 20; current names differ (see 2a table above).

**Actions:**
- Update the count to 20
- Replace the old tool name list with the current names from the table in 2a
- Update the "(as of 2026-03-21)" date to 2026-05-06

### 2d. MEMORY.md: remove stale Tool API Refactor section

In the same auto-memory file, the "Tool API Refactor (2026-02-27)" section documents 9 old→new name mappings from a
refactor over a year ago. The old names no longer exist anywhere in the codebase.
This section adds noise and could mislead.

**Actions:**
- Remove the "Tool API Refactor (2026-02-27)" section from MEMORY.md entirely

---

## Theme 3 — Graduation Audit

Move experimental features that have fully shipped on `master` into the stable manual.

Per the graduation lifecycle: `git mv` page to target chapter, remove `> ⚠ Experimental`
callout, add to `SUMMARY.md`, remove from `experimental/index.md`.

### 3a. `call_graph`

Registered in `src/server.rs` as a stable tool. Source: `src/tools/call_graph/`.

- **Source:** `docs/manual/src/experimental/call-graph.md`
- **Target:** `docs/manual/src/tools/call-graph.md`
- **SUMMARY.md placement:** under Tool Reference → Symbol Navigation (after `symbol_at`)

### 3b. Auto-Reindex on Edit

Active in `src/tools/semantic/semantic_search.rs` — fires before every semantic search.

- **Source:** `docs/manual/src/experimental/auto-reindex-on-edit.md`
- **Target:** `docs/manual/src/concepts/auto-reindex-on-edit.md`
- **SUMMARY.md placement:** under Semantic Search (after the main semantic-search entry)

### 3c. Hybrid BM25 + Vector Retrieval

Fully implemented in `src/embed/bm25.rs` + `src/embed/fusion.rs`; fused via RRF in
`semantic_search.rs`. Tantivy index built alongside the vector index.

- **Source:** `docs/manual/src/experimental/hybrid-bm25-vector.md`
- **Target:** `docs/manual/src/concepts/hybrid-bm25-vector.md`
- **SUMMARY.md placement:** under Semantic Search (after auto-reindex entry)

---

## Theme 4 — New Content

Write documentation for shipped features that have no manual page.

### 4a. `edit_code` tool

The consolidated symbol-editing tool; replaced `replace_symbol`, `insert_code`,
`rename_symbol`, `remove_symbol`.

- **New page:** `docs/manual/src/tools/edit-code.md`
- **SUMMARY.md placement:** under Tool Reference → Editing
- **Content to cover:**
  - Four actions: `replace` (overwrite symbol body), `insert` (inject adjacent to symbol,
    `position: before|after`), `rename` (LSP-wide rename across codebase), `remove` (delete symbol)
  - When to use each action vs `edit_file`
  - Caller-hint on rename/replace success (hints to verify callers)
  - The `new_name` param (rename only)

### 4b. `approve_write` tool

Grants session-scoped write access to directories outside the project root.
Security gate: write tools reject out-of-root paths until approved.

- **New section in:** `docs/manual/src/tools/workflow-and-config.md` (not a new page —
  approve_write is a helper for other write tools, not a primary workflow)
- **Content to cover:**
  - Why it exists: write tools block paths outside project root by default
  - Usage: call once per directory per session; approval is cleared on re-activation
  - Protected paths (e.g. `~/.ssh`) cannot be approved
  - Session scope: approval lasts until project is re-activated

### 4c. `read_file` source-range gate

When a requested line range overlaps a named symbol body in a source file, `read_file`
blocks and names the overlapping symbol. `force: true` bypasses the gate.

- **New section in:** `docs/manual/src/tools/file-operations.md`
- **Content to cover:**
  - Behaviour: blocked ranges return an error naming the overlapping symbol
  - Why: steers the agent toward `symbols(include_body=true)` for symbol reads
  - Bypass: `force: true` returns raw content without the gate
  - Scope: only source files (`.rs`, `.ts`, `.py`, etc.); config/markdown files are never gated

### 4d. JVM LSP pre-warming

`prewarm_lsp_background()` fires on server start and on every `activate_project` call
when the project declares `java` or `kotlin` in its language list. Eliminates the
8–15s cold-start penalty on the first LSP query.

- **New section in:** `docs/manual/src/concepts/kotlin-lsp-multiplexer.md`
  (part of the existing JVM cold-start story)
- **Content to cover:**
  - When it fires: server start + activate_project
  - What it does: spawns a background `get_or_start`; safe under concurrent activation
    (LspManager serialises parallel starters via watch channel)
  - Effect: LSP is warm before the first symbol query arrives

---

## Commit Strategy

Each theme as one commit:

```
docs: remove ARCHITECTURE.md and superseded librarian consolidation page
docs: fix tool count, MCP config path, and stale MEMORY.md entries
docs: graduate call_graph, auto-reindex, and hybrid-search from experimental
docs: add edit_code, approve_write, read_file gate, and JVM pre-warm docs
```

All commits on `experiments` branch; no cherry-pick to `master` until manually verified.

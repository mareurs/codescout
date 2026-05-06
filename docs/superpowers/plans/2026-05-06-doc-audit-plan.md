# Doc Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring all internal and external codescout documentation up to date — remove stale files, fix factual errors, graduate experimental features that have shipped, and write docs for four undocumented stable features.

**Architecture:** Four thematic commits in order; each commit is self-contained and leaves docs in a consistent state. No code changes — documentation only. MEMORY.md lives outside the project root and requires `approve_write` before editing.

**Tech Stack:** mdBook (docs/manual/src/), Markdown, git mv for graduations.

---

## Task 1: Theme 1 — Deletions

**Files:**
- Delete: `docs/ARCHITECTURE.md`
- Delete: `docs/manual/src/experimental/librarian-tool-consolidation.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Delete ARCHITECTURE.md**

```bash
git rm docs/ARCHITECTURE.md
```

- [ ] **Step 2: Delete the superseded librarian consolidation page**

The 22→16 consolidation is superseded by the 16→5 consolidation (`librarian-tools-collapse.md`).

```bash
git rm docs/manual/src/experimental/librarian-tool-consolidation.md
```

- [ ] **Step 3: Remove the superseded entry from experimental/index.md**

In `docs/manual/src/experimental/index.md`, remove this line:

```
- [Librarian tool consolidation](./librarian-tool-consolidation.md) — 22 → 16 tools: six single-purpose tools absorbed into parent tools (`artifact_find`, `artifact_create`, `artifact_augment`, `artifact_get`, `artifact_update`).
```

- [ ] **Step 4: Verify**

Check that both deleted files no longer exist:

```bash
ls docs/ARCHITECTURE.md docs/manual/src/experimental/librarian-tool-consolidation.md 2>&1
```

Expected: `No such file or directory` for both.

Check that experimental/index.md no longer contains the removed entry:

```bash
grep "librarian-tool-consolidation" docs/manual/src/experimental/index.md
```

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add docs/manual/src/experimental/index.md
git commit -m "docs: remove ARCHITECTURE.md and superseded librarian consolidation page"
```

---

## Task 2: Theme 2 — Correctness Fixes

**Files:**
- Modify: `README.md`
- Modify: `~/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory/MEMORY.md`

### 2a — README: tool count

- [ ] **Step 1: Fix the tool count heading and category breakdown**

In `README.md`, replace:

```
## Tools (22)

`Symbol navigation (9)` · `File operations (6)` · `Semantic search (3)` · `Memory (1)` · `Library navigation (1)` · `Workflow & Config (2)`
```

With:

```
## Tools (20)

`Symbol navigation (5)` · `File operations (7)` · `Shell (1)` · `Semantic search (2)` · `Memory (1)` · `Library navigation (1)` · `Workflow & Config (3)`
```

Current tool inventory (as of 2026-05-06, from `src/server.rs`):

| Category | Tools |
|---|---|
| Symbol navigation (5) | `symbols`, `references`, `symbol_at`, `call_graph`, `edit_code` |
| File operations (7) | `read_file`, `tree`, `grep`, `create_file`, `edit_file`, `edit_markdown`, `read_markdown` |
| Shell (1) | `run_command` |
| Semantic search (2) | `semantic_search`, `index` |
| Memory (1) | `memory` |
| Library navigation (1) | `library` |
| Workflow & Config (3) | `onboarding`, `approve_write`, `workspace` |

### 2b — README: MCP config path

- [ ] **Step 2: Fix the MCP config path in README.md**

Find:

```
Add codescout as an MCP server in `~/.claude.json`:
```

Replace with:

```
Add codescout as an MCP server in `~/.claude/settings.json`:
```

### 2c — README: stale `replace_symbol` tip in file-operations.md

While checking file-operations.md during the audit, a stale tip was found. In
`docs/manual/src/tools/file-operations.md`, under `## edit_file` Tips, find:

```
- For changes to a function or struct body, prefer `replace_symbol` — it's robust to line number shifts.
```

Replace with:

```
- For changes to a function or struct body, prefer `edit_code(action="replace")` — it's robust to line number shifts.
```

### 2d — MEMORY.md: stale tool count and names

- [ ] **Step 3: Approve write access to the memory directory**

```json
{ "tool": "approve_write", "arguments": { "path": "/home/marius/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory" } }
```

- [ ] **Step 4: Update the tool count and names**

In `~/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory/MEMORY.md`, find the line:

```
- 25 tools registered (as of 2026-03-21): read_file, list_dir, search_pattern, create_file, find_file, edit_file, run_command, onboarding, find_symbol, find_references, list_symbols, replace_symbol, insert_code, rename_symbol, remove_symbol, goto_definition, hover, memory, semantic_search, index_project, index_status, activate_project, project_status, list_libraries, register_library
```

Replace with:

```
- 20 tools registered (as of 2026-05-06): read_file, tree, grep, create_file, edit_file, edit_markdown, read_markdown, run_command, onboarding, approve_write, symbols, references, symbol_at, call_graph, edit_code, memory, semantic_search, index, workspace, library
```

### 2e — MEMORY.md: remove stale Tool API Refactor section

- [ ] **Step 5: Remove the stale rename section**

In `~/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory/MEMORY.md`, remove the entire section that begins:

```
## Tool API Refactor (2026-02-27)
```

...through the end of that section (ends before the next `##` heading). The old→new name mappings are ancient history and the old names no longer exist in the codebase.

- [ ] **Step 6: Verify README changes**

```bash
grep "Tools (2" README.md
grep "claude.json\|settings.json" README.md
```

Expected:
- `## Tools (20)`
- Only `~/.claude/settings.json`, no `~/.claude.json`

- [ ] **Step 7: Verify file-operations.md change**

```bash
grep "replace_symbol\|edit_code" docs/manual/src/tools/file-operations.md
```

Expected: `edit_code(action="replace")` present; `replace_symbol` absent.

- [ ] **Step 8: Commit**

```bash
git add README.md docs/manual/src/tools/file-operations.md
git commit -m "docs: fix tool count, MCP config path, and stale replace_symbol tip"
```

Note: MEMORY.md is outside the git repo — no staging needed.

---

## Task 3: Theme 3 — Graduation Audit

Graduate three experimental features that have fully shipped in the codebase.

**Files:**
- Move: `docs/manual/src/experimental/call-graph.md` → `docs/manual/src/tools/call-graph.md`
- Move: `docs/manual/src/experimental/auto-reindex-on-edit.md` → `docs/manual/src/concepts/auto-reindex-on-edit.md`
- Move: `docs/manual/src/experimental/hybrid-bm25-vector.md` → `docs/manual/src/concepts/hybrid-bm25-vector.md`
- Modify: `docs/manual/src/experimental/index.md` (remove 3 entries)
- Modify: `docs/manual/src/SUMMARY.md` (add 3 entries)

For each: move the file, remove the `> ⚠ Experimental — may change without notice.` callout at the top, add to SUMMARY.md, remove from experimental/index.md.

### 3a — Graduate `call_graph`

- [ ] **Step 1: Move the file**

```bash
git mv docs/manual/src/experimental/call-graph.md docs/manual/src/tools/call-graph.md
```

- [ ] **Step 2: Remove the ⚠ callout**

In `docs/manual/src/tools/call-graph.md`, remove the first 3 lines:

```
> ⚠ Experimental — may change without notice.

```

(The callout line plus the blank line after it.)

- [ ] **Step 3: Add to SUMMARY.md**

In `docs/manual/src/SUMMARY.md`, find:

```
  - [Symbol Navigation](tools/symbol-navigation.md)
    - [Progressive Directory Overview](tools/list-symbols-progressive.md)
```

Replace with:

```
  - [Symbol Navigation](tools/symbol-navigation.md)
    - [Progressive Directory Overview](tools/list-symbols-progressive.md)
    - [Call Graph](tools/call-graph.md)
```

- [ ] **Step 4: Remove from experimental/index.md**

In `docs/manual/src/experimental/index.md`, remove the line:

```
- [`call_graph`](./call-graph.md) — transitive call graph for any symbol; supports `callers`, `callees`, and `both` directions with LSP + tree-sitter fallback, sqlite edge caching, and per-file cache invalidation.
```

### 3b — Graduate Auto-Reindex on Edit

- [ ] **Step 5: Move the file**

```bash
git mv docs/manual/src/experimental/auto-reindex-on-edit.md docs/manual/src/concepts/auto-reindex-on-edit.md
```

- [ ] **Step 6: Remove the ⚠ callout**

In `docs/manual/src/concepts/auto-reindex-on-edit.md`, the file starts with the callout before the `#` heading:

```
> ⚠ Experimental — may change without notice.

# Auto-Reindex on Edit
```

Remove the first two lines (callout + blank line), leaving `# Auto-Reindex on Edit` as the first line.

- [ ] **Step 7: Add to SUMMARY.md**

In `docs/manual/src/SUMMARY.md`, find:

```
- [Semantic Search](concepts/semantic-search.md)
  - [Setup Guide](semantic-search-guide.md)
  - [Asymmetric Query Prefix](concepts/asymmetric-query-prefix.md)
  - [Metadata-Enriched Chunks](concepts/metadata-enriched-chunks.md)
  - [Index Scope Guard](concepts/index-scope-guard.md)
```

Replace with:

```
- [Semantic Search](concepts/semantic-search.md)
  - [Setup Guide](semantic-search-guide.md)
  - [Asymmetric Query Prefix](concepts/asymmetric-query-prefix.md)
  - [Metadata-Enriched Chunks](concepts/metadata-enriched-chunks.md)
  - [Index Scope Guard](concepts/index-scope-guard.md)
  - [Auto-Reindex on Edit](concepts/auto-reindex-on-edit.md)
  - [Hybrid BM25 + Vector Retrieval](concepts/hybrid-bm25-vector.md)
```

(Both auto-reindex and hybrid-bm25 SUMMARY.md entries are added here in one edit — no separate SUMMARY.md step in 3c.)

- [ ] **Step 8: Remove from experimental/index.md**

In `docs/manual/src/experimental/index.md`, remove the line:

```
- [Auto-Reindex on Edit](./auto-reindex-on-edit.md)
```

### 3c — Graduate Hybrid BM25 + Vector Retrieval

- [ ] **Step 9: Move the file**

```bash
git mv docs/manual/src/experimental/hybrid-bm25-vector.md docs/manual/src/concepts/hybrid-bm25-vector.md
```

- [ ] **Step 10: Remove the ⚠ callout**

In `docs/manual/src/concepts/hybrid-bm25-vector.md`, the file starts with:

```
> ⚠ Experimental — may change without notice.

# Hybrid BM25 + Vector Retrieval
```

Remove the first two lines (callout + blank line).

- [ ] **Step 11: Remove from experimental/index.md**

In `docs/manual/src/experimental/index.md`, remove the line:

```
- [Hybrid BM25 + Vector Retrieval](./hybrid-bm25-vector.md) — `semantic_search` now fuses dense vector search with sparse BM25 keyword search via Reciprocal Rank Fusion (RRF), using a code-aware tokenizer that handles camelCase and snake_case.
```

- [ ] **Step 12: Verify graduations**

```bash
# No ⚠ callouts remain in graduated files
grep "Experimental" docs/manual/src/tools/call-graph.md \
  docs/manual/src/concepts/auto-reindex-on-edit.md \
  docs/manual/src/concepts/hybrid-bm25-vector.md
```

Expected: no output.

```bash
# All three removed from experimental index
grep "call-graph\|auto-reindex\|hybrid-bm25" docs/manual/src/experimental/index.md
```

Expected: no output.

```bash
# All three appear in SUMMARY.md
grep "call-graph\|auto-reindex\|hybrid-bm25" docs/manual/src/SUMMARY.md
```

Expected: three matching lines.

- [ ] **Step 13: Commit**

```bash
git add docs/manual/src/
git commit -m "docs: graduate call_graph, auto-reindex, and hybrid-search from experimental"
```

---

## Task 4: Theme 4 — New Content

Write documentation for four stable features with no existing manual page.

**Files:**
- Create: `docs/manual/src/tools/edit-code.md`
- Modify: `docs/manual/src/tools/workflow-and-config.md` (add `approve_write` section)
- Modify: `docs/manual/src/tools/file-operations.md` (add source-range gate section)
- Modify: `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (add JVM pre-warming section)
- Modify: `docs/manual/src/SUMMARY.md` (add `edit_code` page link)

### 4a — `edit_code` tool page

- [ ] **Step 1: Create the page**

Create `docs/manual/src/tools/edit-code.md` with this content:

````markdown
# `edit_code`

Mutate a symbol in the codebase. Four actions: `replace`, `insert`, `remove`, `rename`.

Consolidates the older individual tools (`replace_symbol`, `insert_code`, `rename_symbol`, `remove_symbol`) into a single action-dispatched tool.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `symbol` | string | yes | Symbol identifier — plain name (`my_fn`) or hierarchical (`MyStruct/my_method`) |
| `path` | string | yes | File containing the symbol |
| `action` | string | yes | One of `replace`, `insert`, `remove`, `rename` |
| `body` | string | action-dependent | `replace`: new full body; `insert`: code to inject |
| `position` | string | no | `insert` only — `"before"` or `"after"` the symbol (default `"after"`) |
| `new_name` | string | rename only | New identifier for the symbol |

## Actions

### `replace`

Overwrites the symbol's body with new content. The declaration line is preserved — only the body between the braces changes.

```json
{
  "symbol": "MyStruct/validate",
  "path": "src/model.rs",
  "action": "replace",
  "body": "    fn validate(&self) -> bool {\n        !self.name.is_empty()\n    }"
}
```

On success, returns a hint listing callers if any were found — use it to verify that the new body is compatible.

### `insert`

Injects code adjacent to a symbol. `"after"` (default) places the new code immediately below the symbol; `"before"` places it above. Use this to add a sibling method or a helper next to an existing definition.

```json
{
  "symbol": "MyStruct/validate",
  "path": "src/model.rs",
  "action": "insert",
  "body": "    fn is_empty(&self) -> bool {\n        self.name.is_empty()\n    }",
  "position": "after"
}
```

### `remove`

Deletes the symbol and its full body from the file.

```json
{
  "symbol": "MyStruct/deprecated_helper",
  "path": "src/model.rs",
  "action": "remove"
}
```

### `rename`

Renames the symbol across the entire codebase via LSP `workspace/rename`. Follows references through type aliases, trait implementations, and macro invocations. Also sweeps textual occurrences in comments and string literals.

```json
{
  "symbol": "process_payload",
  "path": "src/handler.rs",
  "action": "rename",
  "new_name": "handle_payload"
}
```

On success, reports how many files were changed and hints at verifying call sites.

## When to use `edit_code` vs `edit_file`

| Scenario | Tool |
|----------|------|
| Change a function or method body | `edit_code(action="replace")` |
| Add a sibling method or definition | `edit_code(action="insert")` |
| Delete a function, struct, or method | `edit_code(action="remove")` |
| Rename a symbol project-wide | `edit_code(action="rename")` |
| Change an import or `use` line | `edit_file` |
| Change a constant value | `edit_file` |
| Edit a config or data file | `edit_file` |

`edit_code` uses LSP for symbol resolution and is robust to line number shifts. `edit_file` is a plain text find-and-replace — use it for lines that are not part of a symbol body.
````

- [ ] **Step 2: Add to SUMMARY.md**

In `docs/manual/src/SUMMARY.md`, find:

```
  - [Editing](tools/editing.md)
    - [Structural Edit Gate](tools/edit-file-structural-gate.md)
    - [Document Section Editing](tools/document-section-editing.md)
    - [Markdown Tools](tools/markdown-tools.md)
    - [read_markdown](tools/read-markdown.md)
```

Replace with:

```
  - [Editing](tools/editing.md)
    - [edit_code](tools/edit-code.md)
    - [Structural Edit Gate](tools/edit-file-structural-gate.md)
    - [Document Section Editing](tools/document-section-editing.md)
    - [Markdown Tools](tools/markdown-tools.md)
    - [read_markdown](tools/read-markdown.md)
```

### 4b — `approve_write` section in workflow-and-config.md

- [ ] **Step 3: Add section at the end of workflow-and-config.md**

Append this section to `docs/manual/src/tools/workflow-and-config.md`:

````markdown

---

## `approve_write`

Grant write access to a directory **outside the project root** for this session.

Write tools (`edit_file`, `edit_code`, `create_file`) reject paths outside the active project root by default. `approve_write` lifts that restriction for a specific directory.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Directory to approve (absolute or project-relative) |

**Example — allow writes to a sibling plugin directory:**

```json
{ "path": "/home/user/plugins/my-plugin" }
```

**Output:** `"ok"` on success; an error if the path is protected or too broad.

**Session scope.** Approval lasts until the project is re-activated via `workspace(action="activate", ...)`. Re-activating clears all approvals.

**Protected paths.** Sensitive locations such as `~/.ssh` and `~/.gnupg` are permanently blocked and cannot be approved regardless of the argument.

**Overly broad paths are rejected.** Approving a root like `/home/user` or `/` is not allowed — the path must point to a specific subdirectory.
````

### 4c — `read_file` source-range gate section in file-operations.md

- [ ] **Step 4: Add gate section under the `read_file` entry**

In `docs/manual/src/tools/file-operations.md`, use `edit_file`. Find the exact last tip line under `read_file` (old_string) and replace it with the same line followed by the new section (new_string):

old_string:
```
- If you want to search for a pattern rather than read, use `grep` instead.
```

new_string:
```
- If you want to search for a pattern rather than read, use `grep` instead.

### Source-range gate

When `start_line` / `end_line` is supplied on a **source file** (`.rs`, `.ts`, `.py`, and other languages codescout recognises), the tool checks whether the requested range overlaps a named symbol body. If it does, the request is blocked and the error names the overlapping symbol:

\`\`\`
Error: range 45–72 overlaps symbol `MyStruct/validate` — use symbols(name='validate', include_body=true) instead
\`\`\`

This steers you toward `symbols(include_body=true)`, which is robust to line-number shifts caused by later edits.

**Bypass:** Add `"force": true` to skip the gate and return the raw content regardless:

\`\`\`json
{ "path": "src/model.rs", "start_line": 45, "end_line": 72, "force": true }
\`\`\`

**Scope:** Only recognised source files are gated. Config files, Markdown, TOML, JSON, and other non-code formats are never affected.
```

The exact insertion point is after the **Tips** bullet list for `read_file` and before the `---` separator.

````markdown

### Source-range gate

When `start_line` / `end_line` is supplied on a **source file** (`.rs`, `.ts`, `.py`, and other languages codescout recognises), the tool checks whether the requested range overlaps a named symbol body. If it does, the request is blocked and the error names the overlapping symbol:

```
Error: range 45–72 overlaps symbol `MyStruct/validate` — use symbols(name='validate', include_body=true) instead
```

This steers you toward `symbols(include_body=true)`, which is robust to line-number shifts caused by later edits.

**Bypass:** Add `"force": true` to skip the gate and return the raw content regardless:

```json
{ "path": "src/model.rs", "start_line": 45, "end_line": 72, "force": true }
```

**Scope:** Only recognised source files are gated. Config files, Markdown, TOML, JSON, and other non-code formats are never affected.
````

The exact insertion point in `docs/manual/src/tools/file-operations.md` is after the `**Tips:**` bullet list for `read_file` and before the `---` separator (around line 55 in the current file).

### 4d — JVM pre-warming section in kotlin-lsp-multiplexer.md

- [ ] **Step 5: Add pre-warming section**

In `docs/manual/src/concepts/kotlin-lsp-multiplexer.md`, find the `## Activation` heading and insert a new section **after** the `## Activation` section (before `## How It Works`):

````markdown

## JVM Pre-warming

When a project declares `java` or `kotlin` in its language list, codescout spawns background LSP `get_or_start` tasks immediately on **server startup** and on every **`activate_project`** call.

```toml
# .codescout/project.toml
[project]
languages = ["kotlin"]
```

Pre-warming eliminates the 8–15 s cold-start penalty that would otherwise occur on the first symbol query after startup. The warm-up runs in the background — server startup and `activate_project` return immediately without waiting for the LSP to be ready.

**Concurrency safety:** `LspManager`'s watch-channel serialises parallel starters. Calling `activate_project` from concurrent sessions cannot trigger duplicate LSP processes.

The multiplexer handles the rest of the connection lifecycle — see [How It Works](#how-it-works) below.
````

- [ ] **Step 6: Verify new content**

```bash
# edit_code page exists and has all four action headings
grep "### \`replace\`\|### \`insert\`\|### \`remove\`\|### \`rename\`" \
  docs/manual/src/tools/edit-code.md
```

Expected: four matching lines.

```bash
# edit_code appears in SUMMARY.md
grep "edit-code" docs/manual/src/SUMMARY.md
```

Expected: one match.

```bash
# approve_write appears in workflow-and-config.md
grep "approve_write" docs/manual/src/tools/workflow-and-config.md
```

Expected: matches in the new section.

```bash
# source-range gate section exists in file-operations.md
grep "Source-range gate" docs/manual/src/tools/file-operations.md
```

Expected: one match.

```bash
# pre-warming section exists in kotlin-lsp-multiplexer.md
grep "Pre-warming" docs/manual/src/concepts/kotlin-lsp-multiplexer.md
```

Expected: one match.

- [ ] **Step 7: Commit**

```bash
git add docs/manual/src/
git commit -m "docs: add edit_code, approve_write, read_file gate, and JVM pre-warm docs"
```

---

## Summary

Four commits, all on `experiments` branch:

```
docs: remove ARCHITECTURE.md and superseded librarian consolidation page
docs: fix tool count, MCP config path, and stale replace_symbol tip
docs: graduate call_graph, auto-reindex, and hybrid-search from experimental
docs: add edit_code, approve_write, read_file gate, and JVM pre-warm docs
```

No cherry-picks to `master` until manually verified via the live docs build.

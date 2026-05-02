# Grep Rules & Search Guidance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add Iron Law #7 (grep rule), an Anti-Patterns row (edit_code schema loading), and improved Search Routing guidance to `server_instructions.md`, then bump `ONBOARDING_VERSION`.

**Architecture:** Three `edit_markdown` calls on `src/prompts/server_instructions.md` (one per section), one `edit_code` call to bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs`. No new files, no new tests — the existing `prompt_surfaces_reference_only_real_tools` test catches stale tool refs automatically.

**Tech Stack:** Rust, codescout MCP tools (`edit_markdown`, `edit_code`, `run_command`)

---

### Task 1: Add Iron Law #7 — grep is for data files only

**Files:**
- Modify: `src/prompts/server_instructions.md` (Iron Laws section)

- [x] **Step 1: Add Iron Law #7 after Iron Law #6**

Call `edit_markdown` with:
```json
{
  "path": "src/prompts/server_instructions.md",
  "heading": "## Iron Laws",
  "action": "edit",
  "old_string": "6. **REUSE `@file_*` BUFFER REFS.** After a tool emits `file_id: \"@file_*\"`, subsequent\n   reads of that content MUST use the buffer ref, not the original path.\n   Re-reading the original path duplicates disk work and destroys the\n   progressive-disclosure contract. Applies to `read_file`, `read_markdown`,\n   and any tool that consumes `@file_*`.",
  "new_string": "6. **REUSE `@file_*` BUFFER REFS.** After a tool emits `file_id: \"@file_*\"`, subsequent\n   reads of that content MUST use the buffer ref, not the original path.\n   Re-reading the original path duplicates disk work and destroys the\n   progressive-disclosure contract. Applies to `read_file`, `read_markdown`,\n   and any tool that consumes `@file_*`.\n\n7. **`grep` IS FOR DATA FILES AND STRING LITERALS, NOT CODE STRUCTURE.**\n   Use `symbols`, `references`, or `semantic_search` for code.\n   Decision tree:\n   - \"What does symbol X look like?\" → `symbols(name=X, include_body=true)`\n   - \"What's in this file/dir?\" → `symbols(path=...)`\n   - \"How does X work / what calls Y?\" → `semantic_search` or `references(symbol, path)`\n   - \"Find a string literal in JSONL/YAML/config\" → `grep` ✓\n\n   `grep` on code gives raw text you must interpret; `symbols` gives structured\n   output (signature, body, line range) in fewer tokens with zero ambiguity."
}
```

- [x] **Step 2: Verify the edit**

Call `read_markdown` on `src/prompts/server_instructions.md` with `heading="## Iron Laws"` and confirm law #7 appears after law #6 with correct formatting.

---

### Task 2: Add Anti-Patterns row — edit_code schema loading

**Files:**
- Modify: `src/prompts/server_instructions.md` (Anti-Patterns section)

- [x] **Step 1: Append new row to Anti-Patterns table**

Call `edit_markdown` with:
```json
{
  "path": "src/prompts/server_instructions.md",
  "heading": "## Anti-Patterns — STOP if you catch yourself doing these",
  "action": "edit",
  "old_string": "| `symbols(query=\"foo\\|bar\")` | `grep(pattern=\"foo\\|bar\")` or separate `symbols` calls | `symbols` rejects regex-like patterns |",
  "new_string": "| `symbols(query=\"foo\\|bar\")` | `grep(pattern=\"foo\\|bar\")` or separate `symbols` calls | `symbols` rejects regex-like patterns |\n| Call `edit_code(...)` without loading schema | `ToolSearch(\"select:mcp__codescout__edit_code\")` before first call each session | Schema is deferred — fails with \"missing 'action' parameter\" until loaded |"
}
```

- [x] **Step 2: Verify the edit**

Call `read_markdown` on `src/prompts/server_instructions.md` with `heading="## Anti-Patterns — STOP if you catch yourself doing these"` and confirm the new row is present at the bottom of the table.

---

### Task 3: Update Search Routing — qualifier + positive semantic_search bullet

**Files:**
- Modify: `src/prompts/server_instructions.md` (Search Routing section)

- [x] **Step 1: Update grep bullet and enhance semantic_search bullet**

Call `edit_markdown` with:
```json
{
  "path": "src/prompts/server_instructions.md",
  "heading": "### Search Routing",
  "action": "edit",
  "old_string": "- **Know the concept** → `semantic_search(query)` then drill with symbol tools\n- **Know a text pattern** → `grep(pattern)`",
  "new_string": "- **Know the concept / \"How does X work?\"** → `semantic_search(query)` — faster and more relevant than grep for conceptual questions; drill with symbol tools after\n- **Know a text pattern in data/config files** → `grep(pattern)` (not for code structure — see Iron Law #7)"
}
```

- [x] **Step 2: Verify the edit**

Call `read_markdown` on `src/prompts/server_instructions.md` with `heading="### Search Routing"` and confirm both bullets updated correctly.

---

### Task 4: Bump ONBOARDING_VERSION

**Files:**
- Modify: `src/tools/onboarding.rs` (line 19, constant `ONBOARDING_VERSION`)

- [x] **Step 1: Bump ONBOARDING_VERSION from 18 to 19**

Call `edit_file` with:
```json
{
  "path": "src/tools/onboarding.rs",
  "old_string": "pub(crate) const ONBOARDING_VERSION: u32 = 18;",
  "new_string": "pub(crate) const ONBOARDING_VERSION: u32 = 19;"
}
```

> Note: if `edit_file` rejects with "use `edit_code`", load `edit_code` schema via `ToolSearch("select:mcp__codescout__edit_code")` and use `edit_code` instead.

- [x] **Step 2: Verify the bump**

Call `symbols(name="ONBOARDING_VERSION", include_body=true)` and confirm value is `19`.
### Task 5: Verify and commit

**Files:** None new

- [x] **Step 1: Run cargo fmt**

```
run_command("cargo fmt")
```

Expected: exits 0, no output.

- [x] **Step 2: Run clippy**

```
run_command("cargo clippy -- -D warnings")
```

Expected: exits 0. If warnings appear, fix before proceeding.

- [x] **Step 3: Run tests**

```
run_command("cargo test")
```

Expected: all tests pass. In particular `prompt_surfaces_reference_only_real_tools` must pass — it catches stale tool name references in all three prompt surfaces.

- [x] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md src/tools/onboarding.rs
git commit -m "docs(prompts): add Iron Law #7 (grep rule), edit_code schema reminder, semantic_search positive guidance

- Iron Law #7: grep is for data files and string literals only; use
  symbols/references/semantic_search for code structure
- Anti-Patterns: call edit_code without ToolSearch → schema deferred error
- Search Routing: qualifier on grep bullet, positive semantic_search example
- Bump ONBOARDING_VERSION 18 → 19 to trigger system prompt refresh"
```

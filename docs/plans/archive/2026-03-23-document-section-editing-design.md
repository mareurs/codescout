# Document Section Editing — Design Spec

**Date:** 2026-03-23
**Status:** Draft
**Branch:** experiments

## Problem

LLMs editing structured text files (markdown, eventually TOML/YAML) face two problems:

1. **No surgical section editing.** To update a section in a markdown file, the LLM must either rewrite the entire file (`create_file`) or do fragile exact string matching (`edit_file`). There is no heading-addressed editing equivalent to the symbol tools (`replace_symbol`, `insert_code`, `remove_symbol`) that code files enjoy.

2. **No read-coverage verification.** When an LLM reads a large document (plan, design spec, architecture doc), it may skim or miss sections — especially middle sections ("lost in the middle"). There is no mechanism to track whether all sections were actually consumed before the LLM acts on the file.

## Solution Overview

Six features that share the same heading-parsing infrastructure:

1. **`edit_section` tool** — a single action-dispatched tool for heading-addressed markdown editing (replace, insert_before, insert_after, remove).
2. **`SectionCoverage` tracker** — session-level state that records which headings have been read, emitting hints on reads and writes when sections remain unread.
3. **Multi-heading read** — `read_file` enhancement accepting a list of headings to return multiple sections in one call.
4. **Section-scoped `edit_file`** — optional `heading` param on `edit_file` to restrict string matching to a section's line range.
5. **Batch `edit_file`** — `edits` array param on `edit_file` for applying multiple edits atomically in one call.
6. **Inline-format-aware heading matching** — strip backticks, bold, italic from headings before matching, so LLMs don't need to reproduce exact inline formatting.
7. **Complete read mode for plan files** — `read_file(mode="complete")` bypasses buffer/pagination, delivers entire plan inline with a delivery receipt (section list, checkbox progress). Scoped to `plans/` directories only.

All markdown-only for now. The design includes extension seams for TOML/YAML/JSON later.

## Feature 1: `edit_section` Tool

### Tool Interface

**Name:** `edit_section`

**Description:** *"Edit a document section by heading. Actions: replace, insert_before, insert_after, remove. Supports Markdown; extensible to TOML/YAML."*

**Schema:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | yes | File path relative to project root |
| `heading` | string | yes | Section heading to target, e.g. `## Auth` |
| `action` | enum | yes | `replace` \| `insert_before` \| `insert_after` \| `remove` |
| `content` | string | for replace/insert_before/insert_after | New content. Required for `replace`, `insert_before`, and `insert_after`. Ignored for `remove`. |

**Return value:** `json!("ok")` — consistent with all write tools.

### Action Semantics

| Action | Behavior |
|--------|----------|
| `replace` | **Smart replace:** if `content` starts with `^#{1,6}\s`, replaces heading + body. Otherwise replaces body only, preserving the heading line. |
| `insert_before` | Inserts `content` on the line immediately before the matched heading. |
| `insert_after` | Inserts `content` on the line immediately after the section's last line. |
| `remove` | Deletes the entire section (heading + body). Consumes one trailing blank line if present to avoid double-blank gaps. |

### Heading Resolution

Reuses the existing exact → prefix → substring cascade from `extract_markdown_section`:

1. Exact match: `heading_query == heading_text`
2. Case-insensitive prefix: `heading_text.lower().starts_with(query.lower())`
3. Case-insensitive substring: `heading_text.lower().contains(query.lower())`

On no match: `RecoverableError` listing available headings (up to 15).

**Duplicate headings:** When multiple headings have identical text (e.g., repeated `### Parameters` under different parents), the exact-match step may find multiple candidates. In this case, return a `RecoverableError` listing all matches with their line numbers (e.g., `"## Example at lines 12, 45, 89"`), so the caller can disambiguate by providing a more specific query or using `start_line`-based targeting via `edit_file` as a fallback. This mirrors how `edit_file` handles multiple string matches — it errors with line numbers rather than silently picking the first.

### `resolve_section_range` — Core Resolution Layer

**Location:** `src/tools/file_summary.rs`

```rust
pub struct SectionRange {
    pub heading_line: usize,    // 1-indexed, the ## Foo line
    pub body_start_line: usize, // heading_line + 1
    pub end_line: usize,        // last line of section (inclusive), before next sibling/parent or EOF
    pub heading_text: String,   // matched heading, e.g. "## Auth"
    pub level: usize,           // heading level (1-6)
}

pub fn resolve_section_range(
    content: &str,
    heading_query: &str,
) -> Result<SectionRange, RecoverableError>
```

Key behaviors:
- Does **not** reuse `summarize_markdown` directly — that function truncates to 30 headings (a display optimization for `read_file` summaries). Instead, `resolve_section_range` calls a new internal `parse_all_headings(content) -> Vec<HeadingInfo>` helper that parses without truncation. `summarize_markdown` should be refactored to call `parse_all_headings` internally and truncate only at the display layer. This avoids two parallel heading-parsing code paths that could drift.
- `body_start_line` is always `heading_line + 1`, even if blank (blank line is part of section body)
- `end_line` is inclusive — last line belonging to this section
- Empty sections (heading with no body, `body_start_line > end_line`) are valid: `replace` inserts after heading, `remove` still removes the heading line

**Extension seam:** Later, `resolve_toml_table_range`, `resolve_yaml_key_range` etc. can be added. `edit_section` detects format from file extension and calls the right resolver.

### Section Edit Operations

**Location:** New file `src/tools/section_edit.rs`

All four actions are line-range manipulations:

```
replace(range, content):
  if content starts with ^#{1,6}\s:
    replace lines [heading_line..=end_line] with content
  else:
    replace lines [body_start_line..=end_line] with content (preserve heading)
  edge case: empty section → insert content after heading_line

insert_before(range, content):
  insert content at heading_line (pushes section down)

insert_after(range, content):
  insert content at end_line + 1

remove(range):
  delete lines [heading_line..=end_line]
  consume one trailing blank line if present
```

After any operation:
- Ensure file ends with exactly one newline
- Call `ctx.lsp.notify_file_changed()` and `ctx.agent.mark_file_dirty()`
- Same `validate_write_path` + security checks as all write tools

## Feature 2: Read-Coverage Tracking

### State

```rust
pub struct SectionCoverage {
    /// canonical_path → set of heading texts seen
    seen: HashMap<PathBuf, HashSet<String>>,
    /// mtime at time of recording — invalidate if file changed on disk
    mtimes: HashMap<PathBuf, SystemTime>,
}
```

Held as a new field on `ToolContext` (mirroring `output_buffer`): `pub section_coverage: Arc<Mutex<SectionCoverage>>`. This requires adding the field to the `ToolContext` struct in `src/tools/mod.rs` and initializing it in `src/server.rs` alongside the other session-scoped state.

### When Sections Are Recorded as "Seen"

| `read_file` call | Sections marked seen |
|---|---|
| Full file read (no `heading=`) | All headings in the file |
| `heading="## Auth"` | Just that one heading |
| `start_line`/`end_line` range | Any headings whose `heading_line` falls within the range |

### When Hints Are Emitted

**On `read_file` for markdown files:** If the file has sections and not all are seen, append a `coverage` field to the response:
```json
{ "coverage": { "read": 3, "total": 7, "unread": ["## Testing", "## Deployment", "## FAQ"] } }
```

**On write tools targeting a markdown file with unread sections:** Include a lighter hint:
```json
{ "hint": "4 unread sections in this file: ## Testing, ## Deployment, ..." }
```

This is informational — writes are not blocked by unread sections. The primary value is on the read side (did you actually consume the full document?).

### Invalidation

When a file's mtime changes from an **external** source (detected on next access), clear its coverage entry — sections may have changed.

**In-session write handling:** When `edit_section` or `edit_file` modifies a markdown file, it updates `SectionCoverage.mtimes` to the post-write mtime immediately after the write. This prevents the next `read_file` from seeing a stale mtime delta and spuriously clearing all coverage. The sequence `read_file (marks 7/7 seen) → edit_section → read_file` should retain coverage for unmodified sections, not reset to 0/7.

### Not Tracked

- Files with no headings
- Non-markdown files
- Very small files (optional threshold, TBD during implementation)

## Feature 3: Multi-Heading Read (`read_file` enhancement)

### Motivation

Reading multiple specific sections currently requires either (a) reading the entire file through a buffer or (b) multiple `read_file` calls with individual `heading=` params. Both waste tokens or round trips.

### Interface Change

Add a `headings` param to `read_file` (list of strings), analogous to the `sections` param on `memory(action="read")`:

```json
{ "path": "docs/plan.md", "headings": ["## Architecture", "## Testing", "## Deployment"] }
```

**Behavior:**
- Returns the content of all matched sections concatenated, separated by a blank line
- Each section includes its heading line and breadcrumb
- Heading resolution uses the same exact → prefix → substring cascade per heading
- If any heading is not found, returns a `RecoverableError` listing the failed heading and available headings
- The existing single `heading` param continues to work — `headings` is the list form

**Coverage integration:** All matched headings are marked as "seen" in `SectionCoverage`.

**Mutual exclusivity:** `heading` (singular) and `headings` (list) are mutually exclusive. Providing both is a `RecoverableError`.

## Feature 4: Section-Scoped `edit_file`

### Motivation

When using `edit_file` on markdown, `old_string` must be unique across the entire file. But the LLM often knows which section it's targeting. Scoping the match to a section makes shorter, less fragile `old_string` values sufficient.

### Interface Change

Add an optional `heading` param to `edit_file`:

```json
{ "path": "README.md", "heading": "## API Reference", "old_string": "Returns a list", "new_string": "Returns a paginated list" }
```

**Behavior:**
- When `heading` is provided, resolve the section range first (via `resolve_section_range`)
- `old_string` matching is restricted to lines within `[heading_line..=end_line]`
- If `old_string` is not found within the section, the error hint mentions the section scope: `"old_string not found in section '## API Reference' (lines 45-89)"`
- If `old_string` matches multiple times within the section and `replace_all` is not set, same error behavior as today but scoped to the section
- The `heading` param is only valid for markdown files. On non-markdown files, it returns a `RecoverableError` suggesting `toml_key` or `json_path` for future format support.

**No behavior change without `heading`:** Existing `edit_file` calls are unaffected.

## Feature 5: Batch `edit_file`

### Motivation

Multi-edit sessions (e.g., applying review feedback) require sequential tool calls, each re-reading and re-writing the file. A batch mode applies multiple edits atomically in one call.

### Interface Change

Add an optional `edits` param to `edit_file` — an array of edit operations:

```json
{
  "path": "README.md",
  "edits": [
    { "old_string": "foo", "new_string": "bar" },
    { "old_string": "baz", "new_string": "qux", "heading": "## Setup" },
    { "old_string": "v1.0", "new_string": "v2.0", "replace_all": true }
  ]
}
```

**Behavior:**
- Edits are applied sequentially on the in-memory content (not written between each edit)
- Each edit can optionally include `heading` for section scoping and `replace_all`
- If any edit fails (no match, ambiguous match), the entire batch is rejected — no partial writes. The error identifies which edit (by index) failed and why.
- The file is written once after all edits succeed
- Single-edit mode (existing `old_string`/`new_string` top-level params) continues to work — `edits` array is the batch form
- `edits` and top-level `old_string` are mutually exclusive

**Line-shift handling:** Because edits are applied sequentially on the in-memory string, earlier edits may shift content for later ones. Each edit re-resolves its `heading` scope (if any) on the current state of the content. This is correct because headings are stable anchors — an edit to `## Auth` body doesn't move `## Testing`'s heading line in the string.

## Feature 6: Inline-Format-Aware Heading Matching

### Motivation

Markdown headings often contain inline formatting: `` ## The `auth` Module ``, `## **Important** Notes`, `## Setup & Config`. The current matching compares raw text, which fails when the LLM provides the heading without formatting (e.g., `heading="## The auth Module"`).

### Behavior

Before matching, strip inline markdown formatting from both the query and the candidate heading text:

1. Remove backtick spans: `` `foo` `` → `foo`
2. Remove bold/italic markers: `**foo**` → `foo`, `*foo*` → `foo`, `__foo__` → `foo`, `_foo_` → `foo`
3. Collapse multiple spaces to single space
4. Trim

This stripping is applied in `resolve_section_range` (and by extension `extract_markdown_section`) as a normalization step before the exact → prefix → substring cascade. The raw heading text is preserved in `SectionRange.heading_text` for display.

**Backward compatible:** Exact matches still work first (raw text). Stripped matching is a fallback tier inserted between exact and prefix matching:

1. Exact match (raw)
2. Exact match (stripped)
3. Prefix match (stripped, case-insensitive)
4. Substring match (stripped, case-insensitive)

## Integration Points

### New Files

| File | Content |
|------|---------|
| `src/tools/section_edit.rs` | `EditSection` tool struct, `resolve_section_range()`, `perform_section_edit()` action dispatcher |

### Modified Files

| File | Change |
|------|--------|
| `src/tools/mod.rs` | Add `pub mod section_edit;` |
| `src/tools/file_summary.rs` | Add `resolve_section_range()`, `SectionRange` struct, `parse_all_headings()` helper, `strip_inline_formatting()` helper. Refactor `summarize_markdown` to use `parse_all_headings` internally. Update heading matching cascade to include stripped-match tier (Feature 6). |
| `src/server.rs` (`from_parts`) | Register `EditSection` (tool #30) |
| `src/util/path_security.rs` (`check_tool_access`) | **CRITICAL:** Add `"edit_section"` to the write-tool match arm (`"create_file" | "edit_file" | ... | "edit_section"`). The fallthrough is `_ => {}` (always allowed), so a missing entry silently bypasses write-access gates on read-only projects. |
| `src/tools/file.rs` (`ReadFile::call`) | After markdown read, record seen sections in `SectionCoverage`; include `coverage` field. Add `headings` list param support (Feature 3). |
| `src/tools/file.rs` (`ReadFile::input_schema`) | Add `headings` param (array of strings), `mode` param (string, enum: "complete"). Mutual exclusivity with `heading`. |
| `src/tools/file.rs` (`EditFile::call`) | Add optional `heading` param for section-scoped matching (Feature 4). Add `edits` array param for batch mode (Feature 5). On markdown write, check `SectionCoverage` (only if coverage data exists for the file), include unread hint. Update mtime in coverage after write. |
| `src/tools/file.rs` (`EditFile::input_schema`) | Add `heading` (string, optional) and `edits` (array, optional) params to schema |
| `src/tools/mod.rs` (`ToolContext`) | Add `section_coverage: Arc<Mutex<SectionCoverage>>` field |
| `src/server.rs` | Initialize `SectionCoverage` in session setup, pass to `ToolContext` |
| `src/prompts/server_instructions.md` | Add `edit_section` to tool reference table, anti-pattern: "don't rewrite entire markdown, use edit_section" |
| `src/prompts/onboarding_prompt.md` | Mention section editing in file editing guidance |
| `src/tools/workflow.rs` (`build_system_prompt_draft`) | Only if it lists tools explicitly |

### Prompt Surface Updates

All 3 prompt surfaces need coordinated updates:
- `server_instructions.md` — tool reference + anti-pattern entry
- `onboarding_prompt.md` — mention in file editing section
- `build_system_prompt_draft()` — if it enumerates tools

## Test Plan

### `section_edit.rs` unit tests

| Test | Validates |
|------|-----------|
| `replace_body_only` | Content without heading preserves heading line |
| `replace_with_heading` | Content starting with `#` replaces heading + body |
| `replace_empty_section` | Heading with no body — inserts content after heading |
| `insert_before` | Content placed immediately before heading line |
| `insert_after` | Content placed immediately after section's last line |
| `remove_section` | Deletes heading + body, consumes trailing blank |
| `remove_last_section` | Deletes section at EOF without trailing blank issues |
| `ambiguous_heading_error` | RecoverableError with candidate list |
| `heading_not_found_error` | RecoverableError listing available headings |
| `smart_replace_detection` | Regex correctly distinguishes heading vs body content |
| `trailing_newline_normalization` | File ends with exactly one newline after any op |
| `nested_section_replace` | Replacing `## Parent` that contains `### Child` subsections replaces the entire subtree |
| `consecutive_edits` | Two `edit_section` calls on same file in sequence — line numbers recomputed correctly |
| `heading_inside_code_block` | `## Foo` inside a fenced code block is not matched as a heading |
| `duplicate_heading_error` | Multiple identical headings produce RecoverableError with line numbers |
| `remove_only_section` | Removing the only heading in a file produces a valid (possibly empty) file |

### Coverage tracking tests

| Test | Validates |
|------|-----------|
| `read_full_marks_all` | Full file read marks all headings seen |
| `read_heading_marks_one` | `heading=` param marks only that heading |
| `read_range_marks_overlapping` | Line range marks headings within range |
| `coverage_hint_on_partial_read` | Response includes unread sections list |
| `mtime_invalidation` | Externally changed file clears coverage state |
| `in_session_write_preserves_coverage` | `edit_section` updates mtime, coverage not spuriously cleared |
| `write_hint_on_unread` | Write tools include unread hint |
| `write_hint_skipped_when_no_coverage` | No hint if file was never read in this session |

### Complete read mode tests

| Test | Validates |
|------|-----------|
| `complete_mode_returns_full_content` | Entire file returned inline, no buffer ref |
| `complete_mode_delivery_receipt` | Receipt appended with section list, line count, checkbox counts |
| `complete_mode_marks_all_seen` | All headings recorded in SectionCoverage |
| `complete_mode_rejects_non_plan_path` | RecoverableError for files outside `plans/` directory |
| `complete_mode_mutual_exclusivity` | Error when combined with heading/start_line/json_path |
| `complete_mode_nested_plans_dir` | Works for `docs/superpowers/plans/`, not just `plans/` |

### Multi-heading read tests

| Test | Validates |
|------|-----------|
| `read_multiple_headings` | Returns concatenated content of requested sections |
| `read_headings_partial_miss` | RecoverableError when one heading not found, lists which one |
| `read_headings_and_heading_mutual_exclusion` | Error when both `heading` and `headings` provided |
| `read_headings_marks_all_seen` | All requested headings recorded in coverage |

### Section-scoped `edit_file` tests

| Test | Validates |
|------|-----------|
| `edit_file_heading_scoped_match` | `old_string` matched only within section |
| `edit_file_heading_scoped_not_found` | Error message includes section name and line range |
| `edit_file_heading_on_non_markdown` | RecoverableError suggesting format-appropriate param |
| `edit_file_heading_with_replace_all` | `replace_all` scoped to section, not whole file |

### Batch `edit_file` tests

| Test | Validates |
|------|-----------|
| `batch_edit_applies_all` | Multiple edits applied in one write |
| `batch_edit_atomic_rollback` | If second edit fails, no edits are written |
| `batch_edit_with_heading_scope` | Individual edits can have `heading` scope |
| `batch_edit_and_old_string_mutual_exclusion` | Error when both `edits` and top-level `old_string` provided |
| `batch_edit_line_shift` | Earlier edit changing content length doesn't break later edit |

### Heading matching tests

| Test | Validates |
|------|-----------|
| `match_heading_with_backticks` | `` ## The `auth` Module `` matched by `## The auth Module` |
| `match_heading_with_bold` | `## **Important** Notes` matched by `## Important Notes` |
| `match_raw_exact_takes_priority` | Raw exact match wins over stripped match |
| `stripped_match_before_prefix` | Stripped exact match tried before prefix match |

### Integration tests

| Test | Validates |
|------|-----------|
| `edit_section_end_to_end` | Full flow: read → edit_section → read back |
| `security_rejects_outside_project` | Write path validation enforced |

## Future Extensions

- **TOML table editing:** `resolve_toml_table_range` + `edit_section` auto-detects `.toml`
- **YAML block editing:** `resolve_yaml_key_range` + same dispatcher
- **JSON key editing:** `resolve_json_key_range` (trickier — JSON has no line-oriented structure)
- **Finer granularity:** Paragraph-level or list-item-level operations within sections
- **Section rename:** Dedicated action or handled via `replace` with heading in content (already works)
- **`occurrence: N` param for `edit_file`:** Replace the Nth match instead of requiring unique context. Avoids the LLM needing to expand `old_string` for disambiguation in repetitive files.
- **`heading_path` nested resolution:** Support `"## Parent / ### Child"` path syntax in `resolve_section_range`, analogous to `name_path` for symbols. Solves ambiguity when repeated subsection names (e.g., `### Motivation`, `### Parameters`) appear under different parent sections.
- **Batch edit guidance:** Server instructions should recommend heading-scoped edits within batches — headings are stable anchors that survive earlier edits, while unscoped edits are order-sensitive.
- **Plan file reading verification:** Structured plan files (with checkbox tasks, numbered steps) need stronger coverage guarantees than generic markdown. See dedicated section below.

## Feature 7: Complete Read Mode for Plan Files

Plan documents are a critical use case for section coverage tracking. They have unique properties:

1. **They are consumed sequentially** — an agent must read tasks in order because later tasks depend on earlier ones.
2. **They are large** — implementation plans routinely exceed 1000 lines (the plan for this very feature is 2185 lines).
3. **"Seen" ≠ "understood"** — the LLM may receive content in a buffer but skim middle sections (lost-in-the-middle effect).
4. **Partial reading causes cascading errors** — skipping Task 3 means Task 5 (which depends on it) will be implemented wrong.

### Motivation

In subagent-driven development (SDD), the controller agent reads the plan once, extracts all tasks with full text, and dispatches subagents with curated context. The subagents never read the plan file — the controller provides exactly what they need.

The problem: `read_file` on large plan files (1000-2000+ lines) routes through the buffer system, requiring pagination. The LLM may skim middle sections. Coverage tracking marks everything "seen" on a full read even if content was only partially processed.

Plan files need a "just give me everything" mode — bypass buffer/pagination and deliver the entire file inline with a delivery receipt.

### Interface Change

Add `mode="complete"` to `read_file`:

```json
{ "path": "docs/plans/my-feature-plan.md", "mode": "complete" }
```

**Behavior:**
- Returns the entire file content inline, bypassing the buffer system and `MAX_INLINE_TOKENS` cap
- Appends a delivery summary at the end of the content:
  ```
  --- delivery receipt ---
  File: docs/plans/my-feature-plan.md
  Lines: 2185 | Sections: 24 | Checkboxes: 48 (12 done, 36 pending)
  Sections delivered: [## Chunk 1: Foundation, ### Task 1: ..., ### Task 2: ..., ...]
  ```
- All sections marked as "seen" in `SectionCoverage`
- Mutually exclusive with `heading`, `headings`, `start_line`/`end_line`, `json_path`, `toml_key`

**Scope restriction:** `mode="complete"` is only allowed for files under a `plans/` directory (any depth — `docs/plans/`, `docs/superpowers/plans/`, etc.). On files outside `plans/`, returns a `RecoverableError`:
```json
{ "error": "mode=complete is restricted to plan files (paths containing /plans/)", "hint": "Use heading= or headings= to read specific sections of non-plan files." }
```

This prevents LLMs from using complete mode as a shortcut to dump arbitrary large files into context.

**No behavior change without `mode`:** Existing `read_file` calls are unaffected. The default buffer/pagination behavior remains for all other files.

### Delivery Receipt

The receipt serves two purposes:
1. **Verification** — the LLM can confirm it received all sections by checking the list
2. **Quick reference** — checkbox counts give the controller a progress snapshot without parsing

The receipt is appended as a markdown block at the end of the content (after a `---` separator), not as a separate JSON field. This keeps it in the LLM's text stream where it's most visible.

### Coverage Integration

All sections are marked "seen" on a complete read. The delivery receipt acts as an additional verification layer — even if coverage says "24/24 seen", the receipt lists them explicitly so the LLM can cross-reference.

### Future Relaxation

The `plans/` restriction can be relaxed later if needed — e.g., allowing `mode="complete"` for any file under a configurable size threshold, or for files matching specific patterns. For now, the tight scope prevents misuse.

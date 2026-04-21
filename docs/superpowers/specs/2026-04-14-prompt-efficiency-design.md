# Prompt Efficiency Overhaul

**Date:** 2026-04-14
**Branch:** experiments
**Status:** Design approved, pending implementation

## Motivation

codescout's two prompt surfaces — `server_instructions.md` (injected every MCP session,
~2765 words) and `onboarding_prompt.md` (delivered once during onboarding, ~4329 words)
— contain significant redundancy and content the model can already discover from tool
schemas or fetch on demand from MCP resources.

Inspired by the caveman plugin's approach (filter content to only what's active, emit
full rules rather than pointers, trust the model to discover what it can), this overhaul
reduces token cost while preserving behavioral compliance.

**Target:** ~38% reduction in server_instructions.md, ~21% in onboarding_prompt.md.

## Design Decisions

Each decision was explored as an explicit trade-off during brainstorming.

### D1: Tool Reference — strip to non-obvious behaviors only

**Current:** 112 lines describing all 25+ tools with parameters and usage.
**Problem:** MCP already sends each tool's `description` + `input_schema` to the model.
The prompt duplicates this.

**Change:** Remove parameter lists and basic descriptions. Keep only:
- Cross-tool routing rules ("prefer X over Y for Z")
- Gotchas not expressible in schemas (rename_symbol string literal corruption,
  edit_file gating .md to edit_markdown, read_file blocking source code)
- Buffer ref type table (@cmd/@file/@tool/@bg distinction) — unique info not in any schema

**Target:** ~112 → ~35 lines.

### D2: Anti-Patterns — deduplicate against Iron Laws

**Current:** 15-row table (22 lines) with significant overlap with the 5 Iron Laws (27 lines).
Combined: 49 lines.

**Change:** Remove rows that restate Iron Laws:
- Kill: `edit_file` multi-line → `replace_symbol` (restates Iron Law #2)
- Kill: `edit_file` to delete → `remove_symbol` (restates Iron Law #2)
- Kill: `edit_file` to add after → `insert_code` (restates Iron Law #2)
- Kill: native Edit/Write → codescout tools (restates Iron Law #2 + companion hook)
- Kill: `run_command` with `cd` prefix (restates Iron Law #3)

Keep rows that teach something genuinely new:
- `read_file(path, json_path=)` over `run_command("jq ... @file_ref")`
- `edit_markdown` over `edit_file` for .md files
- `read_markdown` over `read_file` for .md files
- `find_references` over `grep` for finding callers
- Separate `find_symbol` calls over regex-like patterns
- Follow overflow `by_file` hints

**Target:** ~22 → ~12 lines. Combined with Iron Laws: ~39 lines (was 49).

### D3: Workflows — keep 2 of 4, move rest to MCP resource

**Current:** 4 multi-step workflow tables (42 lines): Markdown Editing, Impact Analysis,
Dependency Tracing, Safe Rename.

**Change:**
- Keep: **Impact Analysis** and **Safe Rename** (most commonly needed)
- Move to `doc://codescout-tool-guide` resource: Markdown Editing, Dependency Tracing
- Add one-liner in prompt: "More workflows available via `resources/read doc://codescout-tool-guide`"

**Rationale:** Markdown Editing is niche (only when editing docs). Dependency Tracing
overlaps significantly with Impact Analysis. The two retained chains cover the highest-frequency
use cases.

**Target:** ~42 → ~22 lines.

### D4: Language Support — dynamic filtering by project languages

**Current:** Kotlin known issues (20 lines) emitted for every project, including Rust-only.

**Change:** `build_server_instructions()` in `src/prompts/mod.rs` checks
`ProjectStatus.languages`. Only emits the Kotlin section if the project includes Kotlin.
Pattern is extensible for future language-specific warnings.

**Implementation:** The function already receives `Option<&ProjectStatus>`. Add a
conditional check on `status.languages` before appending the Kotlin section. Extract the Kotlin section into a separate constant in `src/prompts/mod.rs`
(e.g., `KOTLIN_KNOWN_ISSUES`) that `build_server_instructions()` conditionally appends.
This avoids polluting the markdown with non-standard marker syntax.

**Target:** 20 lines saved for non-Kotlin projects (majority of users).

### D5: Onboarding Phase 2 — goals + gate checklist

**Current:** Phase 2 "Explore the Code" is 135 lines of 7 prescriptive steps telling
the model exactly which tools to call in which order.

**Problem:** The onboarding subagent already has all codescout tools and knows how to
use them (from server_instructions.md). Prescribing "call `list_dir` then `list_symbols`
then `find_symbol`" is redundant — the model can determine exploration strategy from the
tool descriptions.

**Change:** Replace 7 prescriptive steps with:
1. Brief orientation paragraph (what Phase 2 achieves, ~3 lines)
2. Goal statements (~10 lines):
   - Map codebase structure (directories, modules, entry points)
   - Understand core abstractions (key types/traits that form the skeleton)
   - Trace at least 2 complete data flows end-to-end
   - Read all architecture documentation
   - Run semantic search for at least 5 conceptual queries
   - Verify build and tests pass
3. Gate checklist (already exists, 16 lines) — hard constraint on Phase 2 completion
4. Exploration summary template (already exists, keep as-is)

Phase 3 memory templates remain intact — they are the actual deliverable specification
and cannot be compressed without losing output quality.

**Target:** ~135 → ~40 lines.

## Files to Change

| File | Change | Risk |
|---|---|---|
| `src/prompts/server_instructions.md` | Rewrite Tool Reference (D1), deduplicate Anti-Patterns (D2), remove 2 workflows (D3) | Medium — behavioral regression if critical guidance removed |
| `src/prompts/onboarding_prompt.md` | Compress Phase 2 (D5) | Low — gate checklist is the real constraint |
| `src/prompts/mod.rs` | Add language filtering in `build_server_instructions()` (D4) | Low — additive conditional |
| MCP resource content | Add moved workflows + moved tool details to tool-guide resource | Low — new content |
| Tests | Update `system_prompt_draft_is_concise`, instruction-building tests | Low |

## What Stays Unchanged

- **Iron Laws** (5 rules, 27 lines) — core behavioral constraints, already compact
- **How to Choose the Right Tool** table — high-value decision matrix
- **Output System** — buffer ref table is unique info not in schemas
- **Project Management** — already compact (18 lines)
- **MCP Resources** table — already compact (11 lines)
- **Rules** closing section — end-of-prompt placement per CLAUDE.md guidance
- **Phase 0/1** of onboarding — embedding selection + index check
- **Phase 3** memory templates — the actual deliverable spec, quality-critical
- **Red Flags / Common Rationalizations** in onboarding — behavioral guardrails

## Verification

After implementation:
1. Word count check: `server_instructions.md` < 1800 words, `onboarding_prompt.md` < 3500 words
2. `cargo test` — all prompt-related tests pass
3. Manual verification: run onboarding on a test project, confirm memories are equivalent quality
4. Run a coding task session, confirm Iron Laws are still followed (no read_file on source, no edit_file for structural changes)
5. Verify Kotlin section appears for Kotlin projects, absent for Rust-only

## Open Questions

None — all decisions resolved during brainstorming.

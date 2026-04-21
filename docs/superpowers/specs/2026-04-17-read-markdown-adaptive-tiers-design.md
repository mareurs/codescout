# Design: Adaptive `read_markdown` Tiers

**Date:** 2026-04-17
**Status:** Draft — awaiting approval
**Scope:** codescout (`src/tools/markdown.rs`, `src/tools/file.rs`) + `codescout-companion` plugin (`hooks/pre-tool-guard.sh`)

## Problem

Agents executing markdown-heavy workflows (e.g., `superpowers:executing-plans`) stumble through a chain of wrong tool calls when reading plan files:

1. Agent instinct is `read_markdown` — right tool for `.md`.
2. Native `Read` on `.md` is blocked by the companion's `PreToolUse` hook. Hook's guidance message points to `read_file(path, heading=...)` — but `read_file` rejects `.md` files and redirects to `read_markdown`. Stale hook message.
3. For larger plans (>200 lines), `read_markdown`'s default path truncates via exploring-mode line cap. Agent falls back to `read_file(mode="complete")` — a plan-path-restricted escape hatch.
4. Agent sometimes mis-applies `@tool_*` buffer refs to `read_file`, assuming any tool output is reusable.

Net effect: 5–6 wasted calls, token burn, confused error chain for what should be a single `read_markdown(path)`.

## Goal

One mental model: `read_markdown` is the sole entry for `.md`. Its output adapts to file size so the agent's natural instinct produces the right amount of context on the first call.

## Non-goals

- No changes to `edit_markdown` semantics.
- No changes to `read_file` beyond stripping markdown-specific escape hatches.
- No plan-specific heuristics (task-count detection) in this iteration — tier logic is flavor-agnostic.
- No redesign of `@tool_*` buffer refs; agent misuse is a training issue, not a tool bug.

## Design

### Three-tier response from `read_markdown`

Tier is chosen by content size relative to `INLINE_BYTE_BUDGET` (existing constant) and a `LINE_SOFT_CAP` (new, proposed: 150 lines).

| Tier       | Condition                                                             | Response shape                                                                              |
| ---------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| **Small**  | `bytes ≤ INLINE_BYTE_BUDGET` AND `lines ≤ LINE_SOFT_CAP`              | `{ content, total_lines, format }`. No hint, no buffer.                                     |
| **Medium** | `bytes ≤ INLINE_BYTE_BUDGET` AND `lines > LINE_SOFT_CAP`              | `{ content, total_lines, heading_count, format, hint }`. Full content + focused-read nudge. |
| **Large**  | `bytes > INLINE_BYTE_BUDGET`                                          | `{ heading_map, total_lines, total_bytes, heading_count, file_id, format, recipe }`. Heading tree + stats + recipe, no body. |

#### Tier 1 (Small) example

```json
{
  "format": "markdown",
  "content": "…full text…",
  "total_lines": 42
}
```

#### Tier 2 (Medium) example

```json
{
  "format": "markdown",
  "content": "…full text…",
  "total_lines": 380,
  "heading_count": 12,
  "hint": "380 lines, 12 sections. For focused reads: read_markdown(path, heading=\"## Section\")."
}
```

#### Tier 3 (Large) example

```json
{
  "format": "markdown",
  "total_lines": 2134,
  "total_bytes": 87432,
  "heading_count": 18,
  "heading_map": [
    { "level": 1, "text": "Plan Title", "line": 1 },
    { "level": 2, "text": "Task 1: Extract helper", "line": 34 }
  ],
  "file_id": "@file_9868…",
  "recipe": "2134-line markdown. Use read_markdown(path, heading=\"## Task 1: Extract helper\") for one section, or read_markdown(\"@file_9868…\", start_line=N, end_line=M) for line slices."
}
```

### Behavioral changes (current → new)

1. **Remove exploring-mode line cap in default branch.** Today `read_markdown` default truncates any file whose line count exceeds `max_lines` (default 200) in exploring mode, even if bytes fit the inline budget (`src/tools/markdown.rs`, end of `call`). Byte budget becomes the sole hard ceiling; line count becomes a soft nudge.
2. **Unify "large file" path.** Current default already buffers oversized files with `file_id` + `summarize_markdown`. Tier 3 formalizes the shape: stats (`total_lines`, `total_bytes`, `heading_count`), human-readable `recipe`, and structured `heading_map`.
3. **Medium tier is new.** Today there's a hard fork: tiny file → content; large file → summary. Medium fills the gap — agent sees full content plus a nudge to narrow on follow-ups.
4. **Buffer-ref symmetry.** `read_markdown` accepts `@file_*` paths for line-range slicing over previously-buffered content (currently unsupported; agents are forced to `read_file` for this). No heading navigation over buffer refs — buffers are plain text.

### Deletions in `read_file`

- Remove `mode` parameter + `"complete"` enum value (`src/tools/file.rs:37-40`).
- Remove `is_complete_mode` branch and all `/plans/` path validation (`file.rs:71, 90-115`).
- Remove `read_complete_mode` helper (`file.rs:~350`).
- Keep the `.md` rejection gate (`file.rs:69-75`); simplify its hint (no longer mentions `mode=complete`).
- Remove all `ReadFile — mode=complete` tests (`file.rs:~4238-4430`).

### Hook fix

File: `claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`, lines 184-196.

Replace the `read_file(path, heading=...)` guidance with `read_markdown` equivalents:

```
read_markdown(path)                         — heading tree + stats (start here)
read_markdown(path, heading="## Section")   — single section
read_markdown(path, headings=[...])         — multiple sections
search_pattern("pattern", path=...)         — content search
```

Workflow line: `read_markdown first → heading=/headings= for targets → line ranges only if no headings`.

This lives in a separate repo and ships as a separate commit.

### Error handling & edge cases

- **`read_markdown` on non-`.md`**: existing `.md` / `.markdown` gate stays. Error redirects to `read_file`.
- **`heading` + `start_line`/`end_line` together**: existing mutual-exclusivity check stays.
- **Empty `.md`** (0 lines): tier 1 shape with `content: ""`, `total_lines: 0`.
- **`.md` with no headings**: tier 3 returns empty `heading_map: []`; recipe pivots to line-range-only: `"No headings detected. Use read_markdown(path, start_line=N, end_line=M) for slices."`
- **Buffer refs (`@file_*`)**: accept as `path`. Only line-range nav is meaningful (buffers have no heading structure). Return `{ content, total_lines }` or paginated buffer view. Reject `heading`/`headings` params on buffer refs with a clear error.
- **Existing coverage tracking (`section_coverage`)**: fires in all tiers unchanged.

## Thresholds

- `INLINE_BYTE_BUDGET` — reuse existing project constant.
- `LINE_SOFT_CAP` — new, proposed: **150 lines**. Above this, nudge toward heading nav even if content fits.

## Testing

Add:

1. `read_markdown_small_returns_full_content` — <50 lines. Asserts `content`, `total_lines`. No `hint`, no `file_id`, no truncation.
2. `read_markdown_medium_returns_content_with_hint` — 300-line file. Asserts `content` + `hint` + `heading_count`. No `file_id`.
3. `read_markdown_large_returns_summary_no_content` — oversized. Asserts `heading_map`, `file_id`, `recipe`, `total_lines`, `total_bytes`, `heading_count`. `content` absent.
4. `read_markdown_large_empty_headings_pivots_recipe` — oversized file with no headings. Recipe string mentions line ranges, not `heading=`.
5. `read_markdown_accepts_file_id_buffer_ref` — symmetry test. Line-range slice over `@file_*`.
6. `read_markdown_rejects_heading_nav_on_buffer_ref` — clear error when `heading=` passed with `@file_*`.
7. `read_file_rejects_md_with_simplified_hint` — gate still works; error hint no longer mentions `mode=complete`.

Delete: all `ReadFile` `mode=complete` tests (~6 tests per grep).

Integration: plan-execution scenario. Agent reads a 400-line plan; single `read_markdown` returns medium tier with full content + hint. No `read_file` fallback needed.

## Prompt surface review

Per `CLAUDE.md § Prompt Surface Consistency`:

- `src/prompts/server_instructions.md`
- `src/prompts/onboarding_prompt.md`
- `build_system_prompt_draft()` in `src/tools/workflow.rs`

Grep all three for `mode=complete`, `read_file` + markdown hints, `read_markdown` usage patterns. Update any references to removed features.

Bump `ONBOARDING_VERSION` in `src/tools/workflow.rs` — tool parameter semantics change.

## Touchpoints summary

| File                                                        | Change                                                                |
| ----------------------------------------------------------- | --------------------------------------------------------------------- |
| `src/tools/markdown.rs`                                     | Three-tier logic in default (no-heading, no-range) branch; `LINE_SOFT_CAP` constant; buffer-ref support. |
| `src/tools/file.rs`                                         | Strip `mode=complete` + helper + tests. Simplify `.md` gate hint.     |
| `src/prompts/server_instructions.md`                        | Update markdown-reading guidance; remove `mode=complete` refs.        |
| `src/prompts/onboarding_prompt.md`                          | Same.                                                                 |
| `src/tools/workflow.rs`                                     | Update draft-prompt template; bump `ONBOARDING_VERSION`.              |
| `claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` | Rewrite markdown branch of Read guard message.                        |

## Out of scope (noted, deferred)

- `@tool_*` buffer ref misuse: tool behavior is correct; agent mistraining is orthogonal.
- Flavor-aware summaries (plan vs ADR vs notes): not justified at current complexity; generic heading map + recipe covers the need.

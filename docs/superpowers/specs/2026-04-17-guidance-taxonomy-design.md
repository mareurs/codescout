# Three-Level Guidance Taxonomy + `read_markdown` Overflow

**Date:** 2026-04-17
**Branch:** experiments
**Status:** Design approved, pending implementation

## Motivation

Agents routinely ignore `hint` fields in tool responses. Two observed failure modes:

1. **Agent never reuses `@file_*` buffer refs.** Large markdown file returns heading
   map + `@file_*`; agent then paginates by re-reading the original path 5+ times.
   Observed in a `ktor-server` session; reproduced in-session during this design
   discussion.
2. **Agent picks root-H1 heading on large plans.** The matched section spans the
   entire document, overflows, agent falls back to blind `start_line/end_line`
   pagination.

Root cause: `hint` is a weak register. Agents skim past it, especially after
context compression. The severity of "use `@file_*` or you duplicate disk reads
and waste context" is not communicated by a field called `hint`.

Recipes and hints alone cannot carry iron-law-grade constraints. Even fresh in
this session — moments after the design discussion verified the
`heading nav on @file_*` error — the assistant made the same mistake, firing
three parallel heading-nav calls on a buffer ref. Error messages are
post-violation. The rule must be primed *before* the call.

## Design Decisions

### D1: Three distinct fields, not one tagged field

Tool responses carry guidance as one of:

| Field | Severity | Agent reading the JSON sees |
|---|---|---|
| `hint` | take-it-or-leave-it | "you could try …" |
| `warning` | off-golden-path | "reconsider before paginating" |
| `must_follow` | binding / iron-law | "IRON LAW #6: use `@file_abc` …" |

Separate field names carry the register. A tagged `{guidance: {level: "must_follow", text: ...}}`
reads as metadata and is easier to skim past. The prominent field name *is* the prompt.

Fields are mutually exclusive — at most one appears on a given response.

### D2: New Iron Law #6 — reuse `@file_*` buffer refs

Added to `src/prompts/server_instructions.md` Iron Laws section (currently 5 laws,
below the 5–8 cap from `CLAUDE.md § Prompt Surface Consistency`):

> **6. REUSE `@file_*` BUFFER REFS.** After a tool emits `@file_*`, subsequent
> reads of that content MUST use the buffer ref, not the original path. Re-reading
> the original path duplicates disk work and destroys the progressive-disclosure
> contract. Applies to `read_file`, `read_markdown`, and any tool that consumes
> `@file_*`.

### D3: `read_markdown` overflow response becomes `ok: false` with `must_follow`

Current behavior (line 218-235 of `src/tools/markdown.rs`): heading-nav against a
section whose content exceeds the inline limit buffers the content, returns
`ok: true` with `file_id` + `breadcrumb` + `siblings`. No `must_follow`, no
nested heading map, no `next_actions`.

New behavior when `heading=X` match OR bare call on large file OR `headings=[...]`
join exceeds the large-tier threshold (`crate::tools::exceeds_inline_limit`):

```json
{
  "ok": false,
  "error": "heading '# Title' spans 1199 lines — exceeds inline threshold",
  "must_follow": "IRON LAW #6: Use @file_abc for subsequent reads — NOT the original path. Pick a sub-heading from section_map OR use start_line/end_line on the buffer ref.",
  "file_id": "@file_abc",
  "section_map": [
    {"level": 2, "text": "## Context", "line": 8},
    {"level": 2, "text": "## Phase 1", "line": 45},
    {"level": 3, "text": "### Phase 1a", "line": 52}
  ],
  "next_actions": [
    "read_markdown('path.md', heading='## Context')",
    "read_markdown('@file_abc', start_line=100, end_line=250)"
  ]
}
```

`section_map` contains only headings nested *under* the matched heading (not the
full file map, to avoid repeating what the bare call already returned).

Bare-call (no heading, no range) on a large file keeps the existing Tier-3 shape
but upgrades its `recipe` string to a `must_follow`:

```json
{
  "ok": true,
  "format": "markdown",
  "total_lines": 1199,
  "heading_count": 23,
  "heading_map": [ ... ],
  "file_id": "@file_abc",
  "must_follow": "IRON LAW #6: For subsequent reads, use @file_abc (NOT the original path). Pick a heading: read_markdown('@file_abc', heading='## Section'). Or slice: read_markdown('@file_abc', start_line=N, end_line=M)."
}
```

Note: commit `a73b6e7` added **line-range** support on `@file_*` buffer refs, but
heading nav is still rejected (`src/tools/markdown.rs:66-71`). The `must_follow`
text and `next_actions` above assume heading nav works on buffer refs. This spec
therefore extends the buffer-ref branch in `read_markdown` to also accept
`heading=` and `headings=`. Without that extension, the guidance would mislead
the agent into a second recoverable error.

### D4: Rust type for guidance

Extend `RecoverableError` (currently: `message` + `hint: Option<String>`) with
severity:

```rust
pub enum Guidance {
    Hint(String),
    Warning(String),
    MustFollow(String),
}

pub struct RecoverableError {
    pub message: String,
    pub guidance: Option<Guidance>,
    pub extra: serde_json::Map<String, Value>, // section_map, file_id, next_actions
}
```

Serialization emits the appropriate field name (`hint` / `warning` / `must_follow`)
based on variant. `extra` fields are spliced into the top-level response object.

Existing `RecoverableError::with_hint(...)` continues to work (sets `Hint` variant).
Add `::with_warning(...)` and `::with_must_follow(...)` builders. Add `::with_extra(...)`
for structured payload.

Non-error responses (success with guidance) use free-form `json!(...)` with the
appropriate field name, same as today. No type-safety wrapper required for the
success path — it's boilerplate avoidance, not correctness.

### D5: Priming via generated system prompt

Update `build_system_prompt_draft()` in `src/tools/workflow.rs`. Extend the
`read_markdown` line (appears in both single-project and multi-project branches):

```
read_markdown("path.md") — returns heading map + @file_ref for large files.
Subsequent reads MUST use @file_ref (IRON LAW #6):
  read_markdown("@file_ref", heading="## Section")
  read_markdown("@file_ref", start_line=N, end_line=M)
```

Bump `ONBOARDING_VERSION` from 6 → 7 to force regeneration of stale system
prompts on next onboarding.

### D6: Reclassify 2–3 existing high-severity `hint`s to `must_follow`

Three candidates to confirm with `grep` during implementation:

1. **"Don't grep `@tool_*` for code"** (currently in Output Buffers section of
   `server_instructions.md`). Violation produces JSON-escaped code, not raw text
   — wrong results, not just suboptimal.
2. **"Write tools return `json!(\"ok\")` only"** — project-wide invariant; if a
   future tool echoes content back, reclassify the response validation hint.
3. **"`rename_symbol` may corrupt string literals"** — already in Gotchas;
   currently soft, but is a correctness hazard if the user skips verification.

Final pick (2 or 3) determined during implementation by audit. Criterion:
severity is "violating produces wrong results or wastes significant context",
not just "author prefers X".

### D7: What stays `hint`

Guidance that is strictly optional narrowing — e.g. `json_path` suggestions on
`@tool_*` responses, overflow `by_file` narrowing hints in symbol search, next
pagination cursor — remains `hint`. The taxonomy is only meaningful if most
existing hints stay hints.

## Files to Change

| File | Change | Risk |
|---|---|---|
| `src/tools/mod.rs` | Add `Guidance` enum + extend `RecoverableError` with `guidance` + `extra` fields; update serialization | Medium — touches error routing |
| `src/tools/markdown.rs` | Replace oversized-heading branch with `ok: false` + `must_follow` + `section_map` + `next_actions`; extend buffer-ref branch to accept `heading`/`headings`; upgrade Tier-3 bare-call `recipe` to `must_follow` | Medium |
| `src/prompts/server_instructions.md` | Add Iron Law #6; update Output Buffers table (add `read_markdown` as `@file_*` producer); reclassify the 2–3 hints from D6 | Low — additive + relabeling |
| `src/tools/workflow.rs` | Extend `read_markdown` navigation hint in `build_system_prompt_draft`; bump `ONBOARDING_VERSION` 6 → 7 | Low |
| `docs/PROGRESSIVE_DISCOVERABILITY.md` | Document the taxonomy; add severity-choice guidance for new tools | Low — docs only |
| Tests in `src/tools/markdown.rs` | New: heading-on-large-section returns `ok: false` with `must_follow` + `section_map`; heading-nav on `@file_*` works; existing small/medium tier tests unchanged | Low |

## What Stays Unchanged

- MCP wire shape: `ok: false` responses already serialize as `isError: false`
  with JSON body (via `RecoverableError`). No MCP-level change.
- Small-tier and medium-tier `read_markdown` responses: no structural change.
- Heading-nav on small matched sections: unchanged (returns content inline).
- All other tools' response shapes: unchanged (the vast majority of existing
  `hint`s stay as-is per D7).
- Companion plugin (`codescout-companion`) hooks: unchanged. Iron Law #6 is
  prompt-level, not hook-enforced.
- Error handling for unrecoverable failures (`anyhow::bail!`): unchanged.

## Verification

1. **Tool unit tests** (in `src/tools/markdown.rs`):
   - `heading_on_large_section_returns_must_follow_with_section_map`
   - `buffer_ref_accepts_heading_nav`
   - `bare_read_on_large_file_emits_must_follow` (replaces existing recipe assertion)
   - Existing small/medium tier tests confirm unchanged shape.

2. **Taxonomy serialization tests** (in `src/tools/mod.rs` or new module):
   - `RecoverableError::with_must_follow` serializes with `must_follow` field, not `hint`.
   - `with_warning` serializes with `warning`.
   - `extra` fields spliced into top-level object.

3. **Manual MCP verification** (per `CLAUDE.md`: `cargo build --release` + `/mcp` restart):
   - Call `read_markdown` on a known large plan with `heading="# Title"`. Verify
     response has `ok: false`, `must_follow` citing IRON LAW #6, `section_map`
     with nested headings, `next_actions`.
   - Call `read_markdown("@file_xxx", heading="## Section")` — verify content
     returned.
   - Run onboarding on a test project; verify generated system prompt mentions
     `@file_ref` MUST-reuse.

4. **Prompt surface audit** (grep):
   - `grep -r "must_follow" src/` — confirm new field appears in at least 2
     places (read_markdown overflow + 1–2 reclassified hints).
   - `grep "IRON LAW #6" src/prompts/` — confirm rule present in
     `server_instructions.md`.

5. **Cargo checks** (per `CLAUDE.md`):
   - `cargo fmt`
   - `cargo clippy -- -D warnings`
   - `cargo test`

## Open Questions

None. Threshold for "large" reuses `crate::tools::exceeds_inline_limit`; no new
tuning knob introduced.

## Out of Scope

- Full audit + reclassification of every existing `hint` field. Per D6, only 2–3
  obvious cases land in this PR. Further reclassifications happen incrementally
  as files are touched.
- Adding `warning` as a new severity level in existing responses. The taxonomy
  supports it, but no existing response is reclassified to `warning` in this PR.
  `warning` exists for future use.
- Experimental doc page (`docs/manual/src/experimental/…`). Per `CLAUDE.md`,
  experimental pages are for user-facing features. This is an internal
  error/response shape change — no user-visible surface change beyond what
  LLM agents see.

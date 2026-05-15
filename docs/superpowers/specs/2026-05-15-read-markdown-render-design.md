# read_markdown render redesign

**Status:** spec ready for review
**Author:** Marius + Claude (Hamsa-audited)
**Date:** 2026-05-15
**Tracks:** session work after grouped-output refactor (`file_groups[]`)

## Goal

Cut Tier 3 token cost ~40% and close four behavioral defects in `read_markdown`. Mirror the recently-shipped grouped-output pattern: a slim, machine-consumable JSON shape paired with an LLM-facing `format_compact` text view.

## Why now

The grouped-output refactor (commits `f3b465c2` … `38559135`) settled the project's conventions for output sizing, `format_compact`, and contract consistency. `read_markdown` predates those conventions. A 15-case empirical eval (5 baseline + 10 trip) found:

- Tier 3 median signal density **0.40** (target ≥ 0.65)
- ~30 bytes of redundant JSON keys per heading entry × 56 entries on a single 3008-line plan = 1.7 KB pure overhead
- 4 ship-blocking behavioral defects untouched by any shape trim
- 1 error-path contract inconsistency (heading-not-found uses a different shape than success)

The eval results are recorded at `docs/superpowers/specs/2026-05-15-read-markdown-eval.md` (companion artifact, to be created at implementation time).

## Non-goals

- Changing `edit_markdown`'s response shape (out of scope; same caller pattern, different problem)
- Changing the `read_file` markdown path (the tool dispatches to `read_markdown` for `.md` paths; downstream of this redesign)
- Adding new query modes (e.g. heading-by-regex, by-level filter) — separate spec if desired

## Architecture overview

Five emission sites today, three target shapes after redesign:

| Shape | Used by |
|---|---|
| **CONTENT** | Tier 1 small (full file fits inline); Tier 2 medium (full content + nav hint); single-heading reads under inline limit |
| **MAP** | Tier 3 oversized (no content, headings index + file_id); heading-targeted oversized (no content, section_map + file_id) |
| **ERROR** | Heading not found; line range out of file; file empty (when caller asked for content) |

Each shape has one well-defined JSON skeleton and one `format_compact` rendering. No tier hides fields the next tier emits.

## JSON shapes

### CONTENT shape

```json
{
  "content": "<raw markdown text>",
  "lines": 247
}
```

Optional fields, added only when load-bearing:
- `coverage: { read: N, total: M, unread: [...] }` — only when the coverage tracker has data for this path
- `hint: "247 lines, 12 sections — read_markdown(path, heading=\"## Section\") to focus"` — added only when content was returned in full AND heading count ≥ 2 (so caller knows pivot exists)
- `line_range: [start, end]` — when caller specified `start_line`/`end_line`
- `breadcrumb: ["# Top", "## Mid"]` — when caller specified `heading`
- `siblings: ["## Other", "## More"]` — when caller specified `heading`

**Fields dropped from current shape:**
- `format: "markdown"` — derivable from tool name
- `heading_count` — when not returning a map, embed in hint text only
- `total_lines` → renamed `lines` (drop `total_` prefix; there's only one count)
- `total_bytes` — never used by callers we audited; dropped
- `sections_returned` (multi-heading branch) — derivable from heading lines preserved in concatenated content; dropped

### MAP shape

```json
{
  "lines": 329,
  "headings": [
    {"h": "# codescout", "l": 1},
    {"h": "## Development Commands", "l": 7},
    {"h": "## Tool Misbehavior Log — MANDATORY", "l": 15},
    {"h": "### Skill Frictions", "l": 32}
  ],
  "file_id": "@file_xxx",
  "hint": "use \"@file_xxx\" — heading=\"## Section\" or start_line/end_line"
}
```

Each heading entry: `{h, l}` (short keys). Level is encoded by the `#` prefix in `h` — no separate `level` field. Caller can pass `h` verbatim as the `heading=` argument.

Optional fields:
- `coverage` — same shape as CONTENT
- `section_map` (instead of `headings`) — for heading-targeted oversized: the nested sub-headings under the requested section, plus `breadcrumb` and `line_range`

**Drop:** `format`, `heading_count`, `total_bytes`, `must_follow` prose (replaced by terse `hint`).

### ERROR shape

```json
{
  "ok": false,
  "error": "heading '## Foo' not found",
  "lines": 329,
  "headings": [
    {"h": "# codescout", "l": 1},
    ...
  ],
  "file_id": "@file_xxx",
  "hint": "pick a heading from headings[] or read_markdown(\"@file_xxx\", start_line=N, end_line=M)"
}
```

Uses the **same `headings` array** as MAP — no truncated comma-joined English list. Caller can recover by picking any entry.

Three error triggers:
- Heading not found → headings[] is the full map (or the nested section_map if caller specified an oversized heading-targeted query)
- Line range out of file → no headings[] needed; `lines` field tells the caller the actual file length
- Empty file when caller asked for `heading` or specific range → returns this shape with `lines: 0`

Errors use `RecoverableError`. `isError: false` on the MCP envelope so sibling calls survive.

## Tier boundary fix (B1)

Current logic:
- **Tier 3 oversized** if `exceeds_inline_limit(text)` (byte-based)
- **Tier 2 medium** if `total_lines > LINE_SOFT_CAP` (line-based)
- **Tier 1 small** otherwise

Hole: many short repetitive lines pass the byte gate but trip the line gate, landing in Tier 2 — which still returns full content. Eval case `many-headings.md` (1002 lines, 251 sections, ~30 KB) hit this exact path.

**Fix:** add a third gate — if `headings.len() > HEADINGS_HARD_CAP` (proposed: 40), escalate to Tier 3 MAP shape regardless of byte/line counts. The reasoning: a file with that many sections is structurally a directory; content is skim-only and the caller wants to pivot.

Constant lives in `src/tools/mod.rs` alongside `LINE_SOFT_CAP` and `INLINE_LIMIT`. Document with a one-line `//` comment naming the bug it closes (this is one of the few cases where a "why" comment earns its place — eval round 2 will regress if anyone removes it without thinking).

## Behavioral fixes

### B2 — Line range out of file

Today: `read_markdown(path, start_line=9000, end_line=9999)` on a 94-line file returns `{content: ""}` silently.

Fix: detect `start_line > total_lines` before slicing. Return ERROR shape with `lines: <actual>` and hint suggesting the valid range. `end_line > total_lines` clamps to file end (current behavior, retained).

### B4 — Empty file

Today: empty file returns `{content: "", total_lines: 0, heading_count: 0, format: "markdown"}`.

Fix when caller asked for no specific selector (default branch): return `{content: "", lines: 0}` — the slim CONTENT shape. Acceptable; no decoration. When caller asked for `heading` or a non-trivial line range on an empty file: ERROR shape.

### F1 — Error contract consistency

Today: heading-not-found returns `{ok:false, error, hint: "Available headings: A, B, C, ..."}` where the hint is a truncated comma-joined English string capped at ~15 entries.

Fix: emit the same `headings[]` array the MAP shape uses. Caller decodes once, reuses navigation logic.

### F3 — Tier 1 medium gets a nav cue

Today: `observations.md` (94 lines, 4 sections) returns content with no hint at all. Caller has the body but no signal that headings exist for follow-up navigation.

Fix: when content is returned in full AND `headings.len() ≥ 2`, append `hint: "<N> lines, <S> sections — read_markdown(path, heading=\"## Section\") to focus"`.

## format_compact

The headline LLM-facing rendering. One implementation, branches on shape.

### CONTENT shape rendering

Pass through `content` verbatim (it is already markdown text). If `hint` is present, append a blank line and the hint. No header, no decoration.

For heading-targeted reads where `breadcrumb` is present, prepend a one-line header:

```
<path> § <last-breadcrumb>  L<start>-L<end>

<content>
```

### MAP shape rendering

```
CLAUDE.md  329 lines  @file_2b366ead

# codescout  L1
## Development Commands  L7
## Tool Misbehavior Log — MANDATORY  L15
## Session Intelligence Trackers  L27
  ### Skill Frictions  L32
  ### Tool Usage Patterns  L52
## Git Workflow  L81
  ### Branch Strategy  L85
...

next: @file_2b366ead heading="## …" or start_line/end_line
```

Rules:
- Header line: `<path>  <lines> lines  <file_id>` — no section count (the list below carries it)
- Each entry: `<indent><h>  L<l>` where `indent = (level - 1) * 2 spaces`. Level derived by counting `#` prefix in `h`.
- Single space between `<h>` and `L<l>`. **No column padding.** Hamsa cut: padding is decoration with no CC-side behavioral consequence.
- Trailing blank line, then `next: ...` cue. file_id repeated on the cue line (placement heuristic: surface the action token near the decision point).

For heading-targeted oversized responses, the same rendering applies with `section_map` substituted for `headings` and the header line carrying the section breadcrumb: `<path> § <last-breadcrumb>  <span> lines  <file_id>`.
### ERROR shape rendering

```
read_markdown: heading '## Foo' not found in CLAUDE.md (329 lines)

available headings:
# codescout  L1
## Development Commands  L7
...

next: pick from above or read_markdown("@file_xxx", start_line=N, end_line=M)
```

Same heading rendering as MAP, prefaced by the error sentence.

## Eval acceptance gate

Implementation is not complete until the round-2 eval passes the following:

**Hard gates (ship-blockers):**
- T1 empty file: returns ≤ 30 bytes JSON, no `format` field
- T6 bogus heading: response shape is the new ERROR shape with `headings[]` array, not a comma-joined string
- T8 line range past EOF: returns ERROR shape with `lines` field set to actual file length
- **B1 many-headings tier escalation:** `many-headings.md` (1002 lines, 251 sections) returns MAP shape (no `content` field), not full content. Byte count is bounded by heading-text floor, not by a fixed cap — the invariant is "no body when sections exceed HEADINGS_HARD_CAP", not a byte target.

**Signal-density gates:**
- E3 (skill-frictions.md): R1 ≥ 0.65 (was 0.45)
- E4 (CLAUDE.md): R1 ≥ 0.65 (was 0.40)
- E5 (librarian-mcp.md): R1 ≥ 0.60 (was 0.35)

**Regression gates:**
- T4 code-fence-traps still correctly identifies 2 (not 4) headings
- T5 weird-chars still preserves heading text exactly (backticks, em-dash)
- T10 file_id reuse still works after a Tier 3 response

If any hard gate fails, the implementation does not ship. Signal-density misses by < 5% may ship with a follow-up tracker entry.

## Implementation surface

Files to touch:
- `src/tools/markdown/read_markdown.rs` — all 5 emission sites; introduce one private helper per shape (`build_content_response`, `build_map_response`, `build_error_response`); `format_compact` impl
- `src/tools/mod.rs` — add `HEADINGS_HARD_CAP` constant
- `src/tools/markdown/tests.rs` — update existing tests to new shapes; add B1/B2/B4/F1/F3 regression tests; add `format_compact` snapshot tests
- `src/prompts/server_instructions.md` — update tool description if the param surface changes (it does not, so likely no edit)
- `src/tools/onboarding.rs` — bump `ONBOARDING_VERSION` is **not** required (no system-prompt-affecting change)

The introduction of `format_compact` for read_markdown follows the convention established by the grouped-output refactor — same call-site pattern (`call()` returns JSON; `format_compact` renders for LLM). No new shared module is needed yet; if `edit_markdown` follows a similar redesign later, extract then.

## Risks and rollback

- **Risk:** existing callers (tests, agents) depend on the old field names (`total_lines`, `heading_count`, `format`). Mitigation: tests updated in the same commit; agents read `format_compact` text not raw JSON in practice.
- **Risk:** the `HEADINGS_HARD_CAP=40` value is a guess. Mitigation: log a one-line note in the test rationale; tune in round 2 eval if a real file lands at the boundary.
- **Risk:** dropping `total_bytes` from JSON. Mitigation: audit consumers (grep `total_bytes` in repo); none found in this audit.
- **Rollback:** changes are localized to one tool. Single revert commit restores prior shapes.

## Decisions settled

| Question | Decision | Rationale |
|---|---|---|
| heading entries: strings or objects? | objects `{h, l}` | Strings force every JSON consumer to regex-split |
| Keep `level` field? | No | Encoded by `#` count in `h` — one source of truth |
| Header column padding in format_compact? | No | Pure decoration; CC does not scan visually |
| Keep `must_follow` prose? | No | Replaced with terse `hint`; iron-law is enforced by buffer system |
| Keep `siblings` and `breadcrumb` on heading-targeted reads? | Yes | Eval found them load-bearing for navigation |
| Multi-heading: concat or array of sections? | Concat | Heading lines survive in body — caller can slice; lower blast radius |
| New `HEADINGS_HARD_CAP` gate value? | 40 | Eval showed 1002-line/251-section file landing in Tier 2; 40 is well below 251, well above any realistic small file's section count |
| Error shape parity with MAP? | Yes | Single contract for "navigate this file" across success and error |

## Open questions

None blocking. One follow-up worth noting:
- The `coverage` field was not exercised by round 1 eval (fresh paths only). A round-3 eval pass with repeated reads against the same file would score coverage behavior, but is not a ship gate for this redesign.

## Eval inputs (verbatim, for round-2 reproduction)

```bash
mkdir -p /tmp/md-eval
: > /tmp/md-eval/empty.md
yes "plain text line no markers here" | head -50 > /tmp/md-eval/no-headings.md
printf '# Solo Top\n\nbody here\n' > /tmp/md-eval/single-h1.md
printf '# Real\n\nbody\n\n```\n# fake heading inside fence\n## another fake\n```\n\n## Real Two\n\nmore\n' > /tmp/md-eval/code-fence-traps.md
printf '%s\n' '# Top' '## Has `backtick code` — and em-dash' '## Plain' '### Deep — `link.md`' 'body' > /tmp/md-eval/weird-chars.md
{ printf '# Big\n\n'; for i in $(seq 1 250); do printf '## Section %d\n\nbody line\n\n' "$i"; done } > /tmp/md-eval/many-headings.md
```

Baseline targets: `docs/TODO-lsp-cancelled-kotlin.md`, `docs/observations.md`, `docs/trackers/skill-frictions.md`, `CLAUDE.md`, `docs/superpowers/plans/2026-04-19-librarian-mcp.md`.

Trip operations: bogus heading on CLAUDE.md, line range 9000-9999 on observations.md, multi-heading `["## Design Principles", "## Key Patterns"]` on CLAUDE.md, heading-targeted via file_id after Tier 3 read.

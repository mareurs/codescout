---
kind: spec
status: draft
title: artifact_get — Kind-Aware Previews + Progressive Body Access
topic: librarian-mcp
time_scope: 2026-04-20
owners: [marius]
tags: [librarian-mcp, progressive-discoverability, artifact-get]
---

# artifact_get — Kind-Aware Previews + Progressive Body Access

## Problem

`artifact_get` returns only catalog metadata — no body, no preview. Callers
resort to reading the underlying markdown file directly, defeating the point
of the catalog as an access layer.

A naive fix ("add `include_body: true`") dumps the entire file when set. Plans
and specs can be 500+ lines, so this wastes context on the common case where
the caller only wants enough signal to decide whether an artifact is relevant.

## Goals

1. Every `artifact_get` response carries a **cheap, kind-aware preview** that
   gives the caller enough information to decide whether to drill in.
2. Full body is accessible, but bounded by a soft cap with actionable
   overflow guidance. No silent truncation.
3. Targeted section reads (by heading or line range) let callers grab just
   the part they want without loading the whole file.
4. Zero schema migration. Previews are computed on the fly at call time.

## Non-Goals

- Bulk previews across many artifacts (that would live on `artifact_find` /
  `artifact_list_by_kind` — out of scope for v1).
- Persisting previews in the catalog DB (explicitly rejected; see
  Decision: On-The-Fly Computation).
- Support for non-markdown artifact files. Indexer currently admits only
  `.md`; if that ever changes, each new extension needs its own extractor.

## Tool Contract

### Input parameters

| Field                  | Type       | Default | Notes                                         |
|------------------------|------------|---------|-----------------------------------------------|
| `id`                   | string     | —       | Required.                                     |
| `include_observations` | bool       | false   | Existing — unchanged.                         |
| `include_links`        | bool       | false   | Existing — unchanged.                         |
| `full`                 | bool       | false   | Include full body (subject to soft cap).      |
| `heading`              | string     | —       | Fetch one section by heading match.           |
| `headings`             | string[]   | —       | Fetch multiple sections.                      |
| `start_line`           | int        | —       | 1-indexed start of line slice.                |
| `end_line`             | int        | —       | 1-indexed inclusive end of line slice.        |

Body selectors (`full`, `heading`, `headings`, `start_line` / `end_line`) are
mutually exclusive. Passing more than one returns a `RecoverableError` naming
the conflict.

The existing `include_body` parameter added in the interim fix is removed in
favor of `full`. Callers that set `include_body` get a `RecoverableError`
explaining the rename.

### Response shape

```jsonc
{
  // Existing metadata (unchanged):
  "id": "...", "repo": "...", "rel_path": "...",
  "kind": "plan", "status": "draft", "title": "...",
  "owners": [], "tags": [], "topic": null, "time_scope": null,
  "created_at": 0, "updated_at": 0,

  // Always present when the artifact file is readable:
  "preview": { "shape": "plan", /* kind-specific fields */ },

  // Present only when a body selector is set:
  "body": "string",
  "body_meta": { "line_count": 243, "bytes": 9821 },

  // Present only when `full=true` on a doc over the soft cap:
  "overflow": {
    "shown_lines": 500,
    "total_lines": 2450,
    "hint": "Body exceeds soft cap (500 lines). Narrow with heading=\"<one of the headings>\" or start_line=N, end_line=M. Top-level headings: [\"Phase 1\", \"Phase 2\", \"Phase 3\"]"
  },

  // Preview/body unavailable (file missing, repo not in workspace.roots):
  "preview": null,
  "body_error": "No such file or directory (os error 2)",

  // Optional, driven by existing flags:
  "observations": [ ... ],
  "links": { "outgoing": [ ... ], "incoming": [ ... ] }
}
```

### Preview shapes

Tagged by `shape` field. Consumers switch on `preview.shape` to know which
other fields to expect.

**`plan`**
```jsonc
{
  "shape": "plan",
  "headings": [{"level": 2, "text": "Phase 1", "line": 10}, ...],
  "tasks": {
    "total": 12,
    "done": 5,
    "open_next": ["Wire glossary into prompt", "Add metadata filter", "..."]
  }
}
```
- `tasks.total` / `tasks.done` counted from markdown task list lines
  (`- [ ]` / `- [x]`, any indent).
- `tasks.open_next` = first 3 unchecked task texts in document order,
  trimmed to 100 chars each.

**`spec`**
```jsonc
{
  "shape": "spec",
  "headings": [{"level": 2, "text": "Architecture", "line": 15}, ...],
  "summary": "First non-empty paragraph after the H1 title, trimmed to 200 chars."
}
```

**`memory`**
```jsonc
{
  "shape": "memory",
  "observation_count": 7,
  "latest_observations": [
    {"text": "trimmed to 200 chars", "created_at": 1776691248753}, ...
  ],
  "summary": "First paragraph of body, trimmed to 200 chars."
}
```
- Observations pulled from the `observations` catalog table, ordered by
  `created_at DESC`, top 3. Body parsing is not used for observations —
  the catalog is the source of truth.

**`default` (fallback for unknown kinds)**
```jsonc
{
  "shape": "default",
  "headings": [...],
  "summary": "First paragraph trimmed to 200 chars.",
  "line_count": 243
}
```

### Selector priority (defensive — enforced as mutual exclusion)

If a caller ignores mutual exclusion and the server has to pick: `headings` >
`heading` > `start_line` / `end_line` > `full`. But this path is only hit if
validation is bypassed; normal callers get a `RecoverableError` first.

### Soft cap

Default: **500 lines**. Applies only to `full=true`. Cap is not imposed on
`heading(s)` or `start_line/end_line` reads — the caller already narrowed.

When truncated, `overflow.hint` lists up to 10 headings of level 1 or 2 (by
text, in document order) so the caller can re-request with `heading=...`.
Headings deeper than H2 are elided to keep the hint compact; callers who
want a full heading map can re-call with no body selector and read
`preview.headings`.
## Module Layout

```
crates/librarian-mcp/src/preview/
├── mod.rs        // pub fn extract(kind, row, body, ctx) -> Value — dispatch
├── plan.rs       // plan extractor + tests
├── spec.rs       // spec extractor + tests
├── memory.rs     // memory extractor + tests (reads observations via ctx.catalog)
├── default.rs    // fallback extractor + tests
├── headings.rs   // shared heading parser (#, ##, ###; respects fenced code)
└── summary.rs    // shared first-paragraph extractor
```

**Dispatch signature:**
```rust
pub fn extract(
    kind: &str,
    row: &ArtifactRow,
    body: &str,
    ctx: &ToolContext,
) -> serde_json::Value
```

- `kind` is matched against `"plan" | "spec" | "memory"`; anything else →
  `default::extract`.
- `ctx` passed through so `memory::extract` can query the `observations`
  table without additional plumbing.
- Returned `Value` is flat-tagged (`"shape": "plan", ...fields`) so the
  shape-switch works at the JSON layer.

## Heading Parser Rules (shared)

The shared heading parser (`preview/headings.rs`) must:

1. Ignore `#` characters inside fenced code blocks (```...```).
2. Recognize ATX headings (`# `, `## `, ...) only. Setext headings
   (`===` / `---` underlines) are ignored in v1 — negligible in the corpus.
3. Emit `{ level: u8, text: String, line: usize }` records. `line` is
   1-indexed.
4. Return ALL headings regardless of level. Callers cap if needed.

Preview shapes cap displayed headings at 20 (keeps response compact on deep
docs).

## Summary Extractor Rules (shared)

The shared summary extractor (`preview/summary.rs`) returns the document's
first prose paragraph, trimmed to 200 chars. Algorithm:

1. Skip frontmatter (already stripped by caller).
2. Skip any leading H1 heading line.
3. Skip blank lines.
4. Collect contiguous non-heading non-blank lines into the paragraph buffer
   until a blank line or heading is encountered.
5. Collapse internal whitespace to single spaces; trim.
6. Truncate to 200 chars on a word boundary where possible; append `…` if
   truncated.
7. Return empty string if no prose paragraph exists (e.g. pure checklist).

Lines inside fenced code blocks are not treated as paragraph content.

## Heading Match Semantics (`heading` / `headings`)

Match rules for `heading="..."` and entries in `headings=[...]`:

1. **Normalize** the input: trim whitespace, strip leading `#` characters
   and following space, lowercase.
2. **Normalize** each candidate heading from the parsed heading map in the
   same way.
3. **Match** on exact equality of normalized forms. No substring / prefix /
   fuzzy match in v1.
4. **Tie-break** on duplicate headings: earliest line number wins.

Extracted section = the heading line itself plus all following lines up to
(but not including) the next heading of equal or lower level, or EOF.
Nested subsections (higher level numbers) are included.

Misses are not errors. For single `heading`: `body: ""` and
`body_meta.heading_missing: true`. For `headings[]`: returned body contains
all matched sections concatenated with `\n\n`, and `body_meta.headings_missing`
lists the unmatched query strings.
## Decision: On-The-Fly Computation

Preview, body, heading-slice, and line-slice are all computed fresh on every
call. No caching, no schema changes, no indexer changes.

**Rationale:**
- File I/O is cheap; artifacts are KB-scale.
- No staleness risk. Catalog already owns metadata freshness via `file_mtime`
  / `file_sha256`; adding a second cache layer doubles invalidation surface.
- `artifact_get` is a point lookup — this is not a bulk hot path.

**If performance later matters:** measure first. The likely win would be
caching parsed heading maps keyed by `(id, file_sha256)` in-process, not
persisted. Don't build it preemptively.

## Error Handling

| Condition                                   | Response                                           |
|---------------------------------------------|----------------------------------------------------|
| Artifact id not found                       | `null` (existing behavior, unchanged)              |
| Repo not in `workspace.roots`               | Metadata returned; `preview: null`, `body_error`   |
| File missing on disk                        | Metadata returned; `preview: null`, `body_error`   |
| Conflicting body selectors                  | `RecoverableError` naming the conflict             |
| `start_line > end_line`                     | `RecoverableError`                                 |
| `heading` not found                         | `body: ""`, `body_meta.heading_missing: true`      |
| `headings` partially missing                | Return what matched; list misses in `body_meta`    |
| Malformed frontmatter                       | Use raw file as body; preview extractor runs anyway|
| Obsolete `include_body` passed              | `RecoverableError`: "use `full` instead"           |

## Testing

### Unit tests (colocated with extractors)

- `preview/plan.rs`
  - `counts_tasks_total_and_done`
  - `open_next_returns_first_three_unchecked`
  - `open_next_empty_when_no_tasks`
  - `ignores_task_syntax_inside_fenced_code`
- `preview/spec.rs`
  - `extracts_headings_with_levels_and_lines`
  - `summary_trims_to_200_chars`
  - `summary_skips_empty_lines`
- `preview/memory.rs`
  - `latest_observations_ordered_desc_by_created_at`
  - `observation_count_matches_catalog`
  - `summary_falls_back_when_body_empty`
- `preview/default.rs`
  - `line_count_matches_body`
  - `headings_empty_when_no_headings_in_body`
- `preview/headings.rs`
  - `ignores_hash_inside_fenced_code`
  - `captures_level_and_line_number`

### Integration tests on `tools/get.rs`

- `get_includes_preview_by_default` — preview present, no body
- `get_full_true_returns_body_within_cap` — full body returned, no overflow
- `get_full_true_triggers_overflow_over_cap` — truncated body + overflow hint
  lists top-level headings
- `get_heading_targeted_read_returns_single_section`
- `get_heading_missing_returns_body_meta_flag`
- `get_start_end_line_slice`
- `get_conflicting_body_selectors_errors` — `full=true` + `heading=...`
- `get_preview_null_when_file_missing`
- `get_include_body_param_returns_error` — backward-incompat migration signal

### End-to-end

One test driving all three modes (`preview`, `full=true`, `heading=...`) on a
real plan file in a tempdir, verifying complete response shapes.

## Backward Incompatibility

`include_body` was introduced in the interim fix (earlier this session) and
ships in an unreleased build. It is removed before release in favor of
`full`. Removal is enforced by returning a `RecoverableError` with a
migration hint rather than silently ignoring the field.

Any downstream code already calling the interim tool with `include_body:
true` must switch to `full: true`. Since no release has shipped, blast
radius is zero in-tree.

## Out of Scope (Explicitly Deferred)

- Caching preview data in the catalog or in memory.
- `include_preview` on `artifact_find` and `artifact_list_by_kind`. Likely
  desirable; needs its own scoping round.
- Kind extractors beyond `plan`, `spec`, `memory`. `adr`, `runbook`,
  `handoff`, `audit`, `roadmap` fall back to `default` in v1.
- Setext headings (`===` / `---`), nested task parsing, per-section
  task counts.
- Configurable soft cap via `project.toml`.

## Open Questions

None at spec time. All decisions locked during brainstorming:
- Preview format: structured kind-specific fields (Question 4 → a)
- MVP kinds: plan, spec, memory + default fallback (Question 5 → b)
- Overflow: soft cap + actionable hint (Question 3 → b)
- Body access: preview default, `full`, `heading(s)` (Question 2 → d)
- Extractor location: `src/preview/` module per kind (Question 7 → b)
- Classification basis: artifact `kind` (Question 1 → a)

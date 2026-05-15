# Grouped Tool Output Design

**Date:** 2026-05-15
**Author:** Marius
**Status:** Draft — design approved, ready for plan

## Problem

Three read-heavy tools — `symbols` (search mode), `references`, `grep` —
return flat lists of located items. When results cluster in a few files, the
flat shape forces the LLM to re-establish file context on every entry, and
every entry repeats its `file` field verbatim. The `complete_text` Python
debug session (May 2026) showed the symptom most starkly: one function plus
four parameters returned as five rows, each repeating the same path.

Fix C (single-file `file` hoist for `symbols`) addressed the trivial case.
This design generalizes the principle to multi-file results across the three
tools and unifies the rendering.

## Goals

1. Cut token cost of multi-result responses by grouping per file in the
   LLM-facing text rendering.
2. Restructure JSON output of `references` and `grep` to a file-grouped
   shape that matches the rendered text.
3. Leave `symbols` JSON untouched (load-bearing for tests + onboarding;
   Fix C established the precedent).
4. Preserve item-level truncation behavior; never lose the "N hits across M
   files" global signal.

## Non-goals

- `call_graph`, `semantic_search`, `tree`, `symbols(overview)` — out of
  scope. Their existing shapes already serve their semantics (tree, rank,
  hierarchy).
- Configurable grouping criterion (by kind, by directory). YAGNI until a
  second use case appears.
- Backwards-compatible dual emission of `references[]` and `file_groups[]`.
  Pure duplication; defeats the compactness goal.

## The three settled questions

The Snow Lion audit (2026-05-15) flagged three loose stones in the initial
design. Resolutions below are part of the spec, not pending decisions.

### 1. `symbols` JSON asymmetry

**Decision:** `symbols` JSON is a legacy outlier. New tools and the two
restructured tools (`references`, `grep`) use the `file_groups[]` shape.
`symbols` JSON stays flat indefinitely; migration is deferred until the
cost of inconsistency exceeds the cost of the migration.

**Why:** `symbols` JSON is referenced in tests, the onboarding prompt, and
the per-project system prompt builder. Fix C demonstrated the blast
radius. The win from restructuring it would be small (text rendering
already captures it via `format_compact`); the cost is high.

**Marker:** A comment in `src/tools/symbol/symbols.rs` above the result
assembly notes this is the legacy flat shape and points to the
`file_groups[]` convention used by other tools.

### 2. `file_group` module honesty

**Decision:** The shared module groups *by file only*. No `file_key`
parameter, no `group_by` configurability. Module name: `file_group`. If
future requirements demand grouping by kind or directory, extract a new
abstraction *then*, not preemptively.

**Why:** A parameterized `file_key` advertises flexibility that no caller
needs. The Snow Lion's heuristic 5 — abstractions are justified by two
concrete implementations, not one anticipated future — applies. The
parameter would calcify into the API and demand maintenance forever for
a single use case.

### 3. Truncation seam

**Decision:** Truncation moves into the grouping module. A new helper
`file_group::cap_grouped` enacts a single policy:

1. Compute the un-truncated `total` and `files` count first.
2. Fill the budget with one hit per file, in count-desc order, until the
   budget is exhausted *or* every file has contributed one hit.
3. Fill the remaining budget by descending into the hottest files first.
4. Return both the truncated groups and an overflow summary referencing
   the un-truncated totals.

**Why:** A "one hit per file before any file's second hit" policy gives
the LLM a maximally diverse picture under tight budgets — useful when a
hot file would otherwise drown the long tail. Centralizing the policy
also satisfies heuristic 3: a future change to truncation semantics
touches one file, not three.

**Header invariant:** "N hits in M files" derives from the un-truncated
totals, never from the visible group count. Under truncation the user
sees "5 of 47 hits in 12 files" — not "5 hits in 4 files" with a hidden
overflow tail.

## JSON shape

### `references` and `grep` — new

```json
{
  "file_groups": [
    {
      "file": "src/foo.rs",
      "count": 3,
      "items": [
        { "line": 42, "column": 10, "context": "let x = foo();" },
        { "line": 55, "column": 14, "context": "    foo();" },
        { "line": 89, "column": 6,  "context": "use foo;" }
      ]
    },
    {
      "file": "src/bar.rs",
      "count": 1,
      "items": [ { "line": 123, "column": 0, "context": "// foo" } ]
    }
  ],
  "total": 4,
  "files": 2,
  "overflow": { ... }
}
```

- Array preserves count-desc sort (ties: path asc). Dict would lose order.
- `items` is a generic key; field shape inside is tool-specific.
- Per-item `file` field dropped — implied by the group.
- `total` and `files` always reflect the un-truncated reality.
- `overflow` block (existing `OutputGuard` shape) unchanged.

### `symbols(search)` — unchanged

Flat `symbols[]` array, optional top-level `file` (Fix C hoist).

## Text rendering (`format_compact`)

### `symbols(search)`

```
8 hits in 2 files

src/tools/symbol/tests.rs (6)
  Function  collect_matching_keeps_class_method_descendants    6077
  Function  collect_matching_kind_filter_none_returns_all...   1901
  Function  collect_matching_matches_name_path                 1489
  Function  collect_matching_skips_function_children_when…     6011
  Function  collect_matching_slash_pattern_precision           1724
  Function  collect_matching_with_kind_filter_class_only       1844
src/symbol/query.rs (2)
  Function  collect_matching          53-100
  Function  collect_matching_symbols  636-648
```

Single-file results suppress the "N hits in M files" header (one file
header above the rows is enough signal).

### `references`

```
4 references in 2 files

src/foo.rs (3)
   42  let x = foo();
   55      foo();
   89  use foo;
src/bar.rs (1)
  123  // foo
```

Line numbers right-aligned to the group's max width; context trimmed of
leading whitespace.

### `grep` simple mode

```
5 matches in 2 files

src/foo.rs (3)
   42: let x = foo()
   55:     foo()
   89: use foo
src/bar.rs (2)
  123: // foo
  145: // also foo
```

### `grep` context mode (`context_lines > 0`)

Existing per-block rendering preserved. Block header gains a leading
`--- src/foo.rs:42 ---` separator instead of inline `file:line` rows.
Less aggressive grouping because context blocks are already
visually self-bounded.

### Overflow line

When truncation kicks in:

```
… 23 more hits in 4 files (use offset=50 for next page)
```

Both numbers come from un-truncated totals.

## Architecture

### New module: `src/tools/file_group.rs`

Surface:

```rust
pub struct FileGroup<'a> {
    pub file: &'a str,
    pub items: Vec<&'a Value>,
}

/// Group a flat list of items by their `file` field.
/// Sort: count desc, ties by path asc. Stable within each group.
pub fn group_by_file(items: &[Value]) -> Vec<FileGroup<'_>>;

/// Render groups to text using a per-tool item renderer.
/// `noun` is the plural item word for the header ("hits", "references", "matches").
pub fn render_grouped(
    groups: &[FileGroup],
    total: usize,
    files: usize,
    noun: &str,
    render_item: impl Fn(&Value) -> String,
) -> String;

/// Build the JSON `file_groups` shape for tools that opt into it.
pub fn groups_to_json(groups: &[FileGroup]) -> Value;

/// Single-policy truncation: round-robin one hit per file, then fill
/// hottest files first. Returns (visible_items, total, files).
pub fn cap_grouped(items: Vec<Value>, budget: usize) -> (Vec<Value>, usize, usize);
```

### Per-tool wiring

| Tool | JSON | format_compact |
|---|---|---|
| `symbols(search)` | unchanged flat | `group_by_file` + `render_grouped` |
| `references` | `groups_to_json` (new `file_groups[]`) | `render_grouped` |
| `grep` simple | `groups_to_json` | `render_grouped` |
| `grep` context | unchanged | preserve existing block renderer, header tweak only |

### Coupling check

- `file_group` consumed by 3 tool modules.
- Three distinct responsibilities (data transform, text render, JSON build,
  capping) intentionally co-located: they share the `FileGroup` type and
  always change together when the grouping convention itself changes.
  Splitting now would invent boundaries with no traffic across them.
- `OutputGuard::cap_items` is bypassed for grouped tools — `cap_grouped`
  replaces it on the truncation seam. `OutputGuard` retains its role for
  non-grouped tools.

## Tests

| Module | Test | Asserts |
|---|---|---|
| `file_group` | `groups_sorted_by_count_desc` | hits {1,5,2} → order [5,2,1] |
| `file_group` | `groups_tie_break_by_path_asc` | equal counts → alphabetical |
| `file_group` | `cap_grouped_round_robin_first` | budget=3 across 4 files of {5,2,1,1} → one hit per top-3 |
| `file_group` | `cap_grouped_fills_hot_after_breadth` | budget=10 across {6,3,1} → hot file gets the remainder |
| `file_group` | `header_uses_untruncated_totals` | snapshot |
| `symbols` | `format_compact_multi_file_groups` | snapshot |
| `symbols` | `format_compact_single_file_no_summary_header` | one file → no "N hits in 1 files" line |
| `references` | `json_shape_uses_file_groups` | new shape, counts correct |
| `references` | `format_compact_groups_results` | snapshot |
| `grep` | `simple_mode_json_uses_file_groups` | new shape |
| `grep` | `simple_mode_format_groups_results` | snapshot |
| `grep` | `context_mode_keeps_per_block_rendering` | regression: unchanged for context_lines>0 |

Snapshots inline (no `insta` dep added).

## Migration

### Breaking changes

- `references` JSON: `references[]` → `file_groups[]`. Update callers.
- `grep` JSON: `matches[]` → `file_groups[]`. Update callers.

### Caller sweep

Before implementation, grep the codebase for:

- `result["references"]` / `result.references` / `references[]`
- `result["matches"]` / `result.matches` / `matches[]`

Count hits per surface (`tests/`, `src/`, `docs/`, prompts). The number
sets the upper bound on the migration effort; the plan adjusts step 5 of
the implementation order accordingly.

### Prompt surfaces (CLAUDE.md "Prompt Surface Consistency")

Three surfaces to check:

- `src/prompts/server_instructions.md` — live on next `/mcp`.
- `src/prompts/onboarding_prompt.md` — requires `ONBOARDING_VERSION` bump.
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — same.

`ONBOARDING_VERSION` bump triggers system-prompt regeneration across all
onboarded projects. Bump iff the textual references to the JSON shapes
exist in `onboarding_prompt.md` or `builders.rs`. `server_instructions.md`
edits do not require a bump.

Run `cargo test prompt_surfaces_reference_only_real_tools` to catch
stale token references at build time.

## Implementation order

1. `src/tools/file_group.rs` — pure functions, fast feedback.
2. `file_group` unit tests.
3. Caller sweep (grep, count, list).
4. `symbols(search)` `format_compact` wiring + tests.
5. `references` JSON + `format_compact` + tests + caller updates.
6. `grep` JSON + `format_compact` + tests + caller updates.
7. Prompt surface sweep + version bump if required.
8. `cargo fmt && cargo clippy -- -D warnings && cargo test`.
9. Live MCP verification (`cargo build --release` + `/mcp` reconnect).

## Out of scope (parked)

- `call_graph` regrouping — already a tree.
- `semantic_search` regrouping — fights rank.
- `tree`, `symbols(overview)` — already file-anchored.
- `group_by_kind`, `group_by_directory` — YAGNI.
- Migrating `symbols` JSON to `file_groups[]` — deferred legacy outlier.

## Risk register

| Risk | Mitigation |
|---|---|
| Caller sweep misses a JSON consumer | Run sweep BEFORE implementation; include `tests/` and downstream plugin repos in the grep |
| Prompt surfaces drift between server_instructions and onboarding_prompt | `prompt_surfaces_reference_only_real_tools` test catches stale tool names; manual review for shape descriptions |
| Snapshot tests calcify and obstruct future renderer tweaks | Accept; cost is low (small string diffs) and benefit is high (regression coverage) |
| `cap_grouped` policy surprises a user expecting "first N by score" | Document in the tool's `long_docs` that truncation prefers file diversity |

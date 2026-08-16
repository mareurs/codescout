---
id: ec267997e2048d87
kind: bug
status: fixed
title: docs/manual/src/tools/semantic-search.md documents the retired legacy sqlite-vec interface, not the current Qdrant stack
tags:
- docs
- semantic-search
- drift
closed: 2026-08-14
opened: 2026-08-13
owner: marius
related: []
severity: medium
---

# BUG: `docs/manual/src/tools/semantic-search.md` documents the retired legacy sqlite-vec interface, not the current Qdrant stack

## Summary

Noticed while fixing `long_docs()` on `SemanticSearch` (task-8 of the
`2026-08-13-worktree-semantic-search` plan). The Tool Reference manual page for
`semantic_search` still describes the pre-Phase-7 legacy interface: a `score`
field, a `language` field, a `detail_level` parameter, and an `offset`
parameter. None of these exist on the current Qdrant-backed tool.

## Symptom (Effect)

`docs/manual/src/tools/semantic-search.md`'s example output block
(`## semantic_search` → `**Output (compact, default):**`) shows:

```json
{
  "results": [
    { "file_path": "...", "language": "rust", "content": "...", "start_line": 42,
      "end_line": 68, "score": 0.91, "source": "project" }
  ],
  "total": 2
}
```

and documents a `detail_level` parameter (`"full"` vs compact truncation) and
an `offset` parameter for pagination. The page's Tips section also says
"Scores above 0.85 are typically a strong match... below 0.6 usually indicate
the concept is not well represented."

## Reproduction

Read `src/tools/semantic/semantic_search.rs`:
- `format_search_result_item` (lines 677-694) emits only `file_path`,
  `start_line`, `end_line`, `content`, and `source` (only when not
  `"project"`). No `score`, no `language`.
- `input_schema` (lines 441-456) — not yet re-verified field-by-field in this
  session, but the `call()` body (lines 457-666) never reads `detail_level` or
  `offset`; the comment at the top of `call()` reads: "Phase 7 (narrow): stack
  is the only retrieval backend for code search. The legacy sqlite-vec +
  tantivy path is gone."

## Environment

codescout repo, `experiments` branch, commit at time of finding:
run `git rev-parse HEAD` in the codescout repo — not captured at finding time
(discovered via code reading, not a live repro).

## Root cause

Unknown — see Hypotheses tried. Most likely: this manual page was written
against the pre-Phase-7 legacy sqlite-vec + tantivy retrieval backend and
never updated when that backend was removed and replaced by the Qdrant-backed
retrieval stack (see `docs/trackers/2026-05-07-legacy-retrieval-removal.md`,
L-01..L-11, referenced directly in `semantic_search.rs`'s `call()` body).
Not measured against a specific commit — inferred from reading
`src/tools/semantic/semantic_search.rs` on 2026-08-13, not measured at runtime.

## Evidence

### `format_search_result_item` never emits `score` or `language`

`src/tools/semantic/semantic_search.rs:677-694`:

```rust
pub(crate) fn format_search_result_item(
    file_path: &str,
    start_line: usize,
    end_line: usize,
    source: &str,
    content: String,
) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("file_path".into(), json!(file_path));
    map.insert("start_line".into(), json!(start_line));
    map.insert("end_line".into(), json!(end_line));
    if source != "project" {
        map.insert("source".into(), json!(source));
    }
    map.insert("content".into(), json!(content));
    Value::Object(map)
}
```

### The tool's own `long_docs()` (fixed in this session) now says the same

`src/tools/semantic/semantic_search.rs`'s `long_docs()` was corrected in the
`2026-08-13-worktree-semantic-search` task-8 documentation pass to read "There
is no `score` field" — the manual page under `docs/manual/src/tools/` was not
in scope for that pass and was left stale.

## Hypotheses tried

1. **Hypothesis:** the manual page is simply aspirational (describes a
   near-future interface). **Test:** grepped for `detail_level` and `offset`
   handling in `call()`'s body — neither appears. **Verdict:** rejected —
   the page describes a *past*, removed interface (per the Phase-7 comment),
   not a future one.

## Fix

Fixed 2026-08-14 on `experiments`. `docs/manual/src/tools/semantic-search.md`
rewritten field-by-field against the live code: `SemanticSearch::input_schema`,
`format_search_result_item`, `search_response`, `apply_worktree_plan_notes`
(`src/tools/semantic/semantic_search.rs`) and both `input_schema`s plus the status
response construction in `src/tools/semantic/index.rs`.

The bug's Resume said to check the `index` / `index_project` / `index_status`
sections "for the same class of drift". They had it worse than the
`semantic_search` section did.

### `semantic_search`

| Documented | Live | → |
|---|---|---|
| 6 params | 8 | added `mode`, `project_id` |
| `"project": "frontend"` in the workspace example | no such param | `project_id` |
| `"score": 0.91` | not emitted | removed, with the reason |
| `"language": "rust"` | not emitted | removed |
| `"source": "project"` | `"stack"`; omitted when it would be `"project"` | corrected |
| `{results, total}` | `{results, total, truncated, truncated_hint?}` | documented |
| — | 4 worktree state fields | documented |

### `index`

| Documented | Live |
|---|---|
| "Indexing runs synchronously" | **asynchronous** — returns `{status: "started"}` immediately |
| flat `{files_indexed, total_files, total_chunks, …}` | those live in `status`'s nested `indexing` block |
| `drift_summary` on build | not emitted |
| actions `build` / `status` | also `cancel` — undocumented |
| — | `already_running` response — undocumented |

### `index_status`

Every documented field name was wrong, and both documented parameters do not exist
(`IndexStatus::input_schema` declares no properties):

| Documented | Live |
|---|---|
| `threshold`, `path` params | **no params at all** |
| `indexed_files` | `file_count` |
| `total_chunks` | `chunk_count` |
| `model` | not emitted |
| `last_updated` | `indexed_at` |
| `stale: true` | `git_sync: {status: "behind", behind_commits: N}` |
| `drift: […]` | not emitted |

Also newly documented because it is a real trap: `indexed: false` has **two** causes
— an empty index and an unreachable Qdrant — distinguishable only by `message`.

### The intro

Said vectors live in a SQLite database at `.codescout/embeddings.db`. Replaced with
Qdrant (`:6334`, collection `code_chunks`), the hybrid dense + sparse-SPLADE + RRF
shape, and the real default model `local:AllMiniLML6V2Q` (in-process, no server —
the page previously said the default "works with any OpenAI-compatible endpoint or a
local Ollama server", implying a server is required). A callout now marks the
sqlite-vec path retired and names what still reads it.

### Verified, not assumed

`index_project` / `index_status` are still registered — the page's claim held, and
`src/tools/semantic/mod.rs:6` exports both. That is the one section that needed no
correction, which is why it was worth checking rather than rewriting on suspicion.

Also not assumed: `project` really is absent as a parameter. `memory` accepts it as
an alias for `project_id` (`memory_write_accepts_project_alias_for_project_id`), and
`PATH_PARAM_ALIASES` exists for path params — so an alias layer was plausible. There
is none for this tool: `semantic_search.rs` reads `input.get("project_id")` at lines
539 and 591 and nowhere reads `"project"`, and the live MCP tool schema confirms it.

Not fixed here, filed separately: `docs/issues/2026-08-14-drift-detection-enabled-is-a-dead-config-key.md`
— the drift config key has no reader in `src/` and is still documented as working on
two *other* manual pages, one of which cites the same non-existent `threshold`
parameter. Out of this bug's stated file scope, and its fix is a real decision
(remove / mark reserved / reimplement on Qdrant).
## Tests added

None. Docs-only content change: the page backs no `include_str!`'d constant and no
test asserts on it — checked before editing, per the reconnaissance rule about
files that back embedded constants.

Gate: **3707 passed / 0 failed / 44 ignored**, unchanged from before the edit, which
is the expected outcome for a docs-only change and confirms nothing asserted on this
page. Every path and symbol newly cited in the page was existence-checked
(`docs/trackers/2026-05-07-legacy-retrieval-removal.md`, three `concepts/` pages,
`scripts/retrieval-stack.sh`, three `src/` files — all present).

**Worth noting what has no guard.** This page drifted this far because nothing
connects a tool's `input_schema` to its manual page. `prompt_surfaces_reference_only_real_tools`
gates *tool names* across the three prompt surfaces, but no gate compares a
documented parameter table against the schema it describes — which is why
`threshold`, `path`, `project`, `score`, `language`, `model`, `indexed_files`,
`last_updated`, `drift` and `drift_summary` could all be documented simultaneously
without a single test failing. A schema-vs-docs gate is the durable fix for this bug
class; it is not in this change.
## Workarounds

Readers should treat `src/tools/semantic/semantic_search.rs`'s `long_docs()`
(surfaced via the tool's own extended description) as the accurate source
until this page is rewritten.

## Resume

N/A — fixed and verified, including the `index` / `index_project` / `index_status`
sweep the original Resume asked for.

If picking up the class rather than the instance: the missing guard named under
*Tests added* (documented parameter table vs `input_schema`) is what would stop this
recurring, and `docs/issues/2026-08-14-drift-detection-enabled-is-a-dead-config-key.md`
is the sibling instance still open.
## References

- `src/tools/semantic/semantic_search.rs` (long_docs, format_search_result_item, search_response)
- `docs/manual/src/tools/semantic-search.md`
- `docs/trackers/2026-05-07-legacy-retrieval-removal.md`
- Found during: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-report.md`

---
id: ec267997e2048d87
kind: bug
status: open
title: docs/manual/src/tools/semantic-search.md documents the retired legacy sqlite-vec interface, not the current Qdrant stack
tags:
- docs
- semantic-search
- drift
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

Not implemented in this pass — out of scope for the docs-only task that found
it. Plan: rewrite `docs/manual/src/tools/semantic-search.md`'s `## semantic_search`
section (parameters table, output examples, Tips) against the current
`input_schema()` and `format_search_result_item`/`search_response` output
shape; drop `score`, `language`, `detail_level`, `offset` unless a future pass
reintroduces them for real. Cross-check `## index` and `## index_project` /
`## index_status` sections in the same file too — not audited in this pass.

## Tests added

N/A — not fixed yet.

## Workarounds

Readers should treat `src/tools/semantic/semantic_search.rs`'s `long_docs()`
(surfaced via the tool's own extended description) as the accurate source
until this page is rewritten.

## Resume

Rewrite `docs/manual/src/tools/semantic-search.md`'s `semantic_search` section
against `src/tools/semantic/semantic_search.rs::input_schema` (lines 441-456)
and `format_search_result_item`/`search_response` (lines 677-726) field-by-field;
also check the `index`/`index_project`/`index_status` sections in the same
file for the same class of drift before closing this bug.

## References

- `src/tools/semantic/semantic_search.rs` (long_docs, format_search_result_item, search_response)
- `docs/manual/src/tools/semantic-search.md`
- `docs/trackers/2026-05-07-legacy-retrieval-removal.md`
- Found during: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-report.md`


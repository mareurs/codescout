---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: ["docs/issues/archive/2026-05-09-read-file-json-path-array-elements.md", "docs/issues/archive/2026-05-17-read-file-jsonpath-negative-slice.md"]
tags: [symbols, hint, json_path, output-buffer, agent-confusion]
kind: bug
---

# BUG: `symbols(include_body=true)` hint references a `@tool_*` buffer that does not exist when the response stays inline

## Summary

`format_search_symbols` elides any body larger than 500 bytes and replaces it with the hint `use json_path="$.symbols[0].body" to extract`. That hint is only actionable when `call_content` has buffered the response into a `@tool_*` ref. For single-symbol queries whose total JSON is under the 10 KB inline threshold, the response is returned as plain text via the `OutputForm::Text` path and **no buffer is created** — so the body is unreachable and the hint sends the agent down a dead end (it naturally applies `json_path` to the source-file path, which `read_file` correctly rejects).

## Symptom (Effect)

```
● symbols (MCP)(include_body: true, name: "test_aggregate_query_tier_decomposition", path: "tests/test_nugget_cascade.py")
  ⎿  tests/test_nugget_cascade.py (1)
       Function  383-404  test_aggregate_query_tier_decomposition
           (22-line body — use json_path="$.symbols[0].body" to extract)

● read_file (MCP)(json_path: "$.symbols[0].body", path: "tests/test_nugget_cascade.py")
  ⎿  {
       "ok": false,
       "error": "json_path parameter is only supported for JSON files",
       "hint": "For Markdown files use read_markdown, for TOML/YAML use toml_key"
     }
```

Body content is unreachable through the suggested path. No `output_id` / `@tool_*` ref was emitted alongside the symbols response.

## Reproduction

```
git rev-parse HEAD
# d8086be2076881622da1d37a9151e780b9e64f37 (branch: experiments)

# Any source file with a function whose body is > 500 bytes but where the
# total JSON response stays under ~10 KB will trigger it:
mcp call codescout symbols '{"name": "format_search_symbols", "path": "src/tools/symbol/display.rs", "include_body": true}'
```

Response shows `(77-line body — use json_path=...)` with no `output_id` field.

## Environment

- Branch: `experiments` @ `d8086be2`
- Rust toolchain: stable
- MCP transport: stdio
- Project: codescout

## Root cause

Two paths inside `Tool::call_content` (`src/tools/core/types.rs:423-468`):

1. **Buffered (`exceeds_inline_limit`, JSON > 10 KB):** stores the response in a `@tool_*` buffer and emits `{output_id, summary, hint}`. The outer `hint` correctly cites the ref: `read_file("@tool_xxx", json_path="$.symbols[0].body")`.
2. **Inline (`OutputForm::Text`, JSON ≤ 10 KB):** returns only `format_compact(&val)` as text. **No buffer is created. No `output_id` is emitted.**

`format_search_symbols` (`src/tools/symbol/display.rs:155-170`) elides any body > 500 bytes (`INLINE_BODY_LIMIT`) and unconditionally appends the json_path hint. In path 2 the hint references a buffer that does not exist. `read_file` correctly rejects `json_path` on non-JSON files (`src/tools/read_file.rs`).

Mechanism, in one line: *the inner compact formatter assumes the outer wrapper has buffered the response, but for sub-10 KB single-symbol queries the wrapper has not.*

## Evidence

### display.rs:155-170

```rust
if let Some(body) = item["body"].as_str() {
    const INLINE_BODY_LIMIT: usize = 500;
    if body.len() <= INLINE_BODY_LIMIT {
        for line in body.lines() {
            row.push_str("\n      ");
            row.push_str(line);
        }
    } else {
        let line_count = body.lines().count();
        row.push_str(&format!(
            "\n      ({line_count}-line body — use json_path=\"$.symbols[0].body\" to extract)"
        ));
    }
}
```

### types.rs:423-468 (the two paths)

```rust
if exceeds_inline_limit(&json) {
    // ... store in buffer, emit {output_id, summary, hint}
}
// Small output — return format_compact text only. No buffer ref.
if form == OutputForm::Text {
    if let Some(text) = self.format_compact(&val) {
        return Ok(vec![Content::text(text)]);
    }
}
```

### read_file rejection

```
json_path parameter is only supported for JSON files
```
(`src/tools/read_file.rs:54-165`)

## Hypotheses tried

1. **Hypothesis:** the hint was correct but the agent passed the wrong path. **Test:** reread the `Symbols` response — observed no `output_id` field at all. **Verdict:** rejected — the hint is path-agnostic and offers no buffer to query.
2. **Hypothesis:** the response should have been buffered. **Test:** check `exceeds_inline_limit` threshold (10 KB) vs typical single-symbol JSON size (~3-4 KB for an 80-line body). **Verdict:** confirmed — typical single-symbol body responses never exceed 10 KB and so never trigger the buffered path.
3. **Hypothesis:** prior fixes to `$.symbols[0].body` (docs/issues/archive/2026-05-09, 2026-05-17) addressed this. **Test:** read both archived files — they fix array indexing and negative slices *inside the buffered path*. **Verdict:** rejected — neither touches the unbuffered hint case.

## Fix

Drop `INLINE_BODY_LIMIT` in `format_search_symbols` — always inline the body in compact text form.

- **Path 2 (inline, ≤ 10 KB total):** body is fully visible in the text response; agent does not need `json_path` at all.
- **Path 1 (buffered, > 10 KB total):** the body is truncated by `truncate_compact` (soft 2 KB, hard 3 KB, appends `… (truncated)` marker per `src/tools/core/types.rs:263-277`), and the outer `read_file("@tool_xxx", json_path="$.symbols[N].body")` hint is present for full retrieval.

The misleading inner json_path hint is removed; the outer call_content hint is the single source of truth for buffered retrieval.

Implementation lives at `src/tools/symbol/display.rs:155-170` (commit `a336965d` on `experiments`).

## Tests added

- `src/tools/symbol/tests.rs::symbols_with_long_body_inlines_full_content` (rewritten from `symbols_with_long_body_shows_hint_not_truncated_body`) — asserts a > 500-byte body is fully inlined.
- `src/tools/symbol/tests.rs::symbols_inline_path_makes_body_reachable_without_buffer` — three-query sandwich at the `Symbols::call_content` level: invoke with `include_body=true`, assert the returned compact text contains the full body and that no `@tool_*` ref is required to access it.

## Workarounds

Until the fix lands: when `symbols(include_body=true)` returns the `(N-line body — use json_path=...)` line and **no `output_id` is present**, fall back to `read_file(path=<source>, start_line=N, end_line=M)` using the line range printed alongside the symbol. The body is reachable; just not through the suggested hint.

## Resume

Apply the patch to `src/tools/symbol/display.rs:155-170` (remove the elision branch). Update the corresponding test in `src/tools/symbol/tests.rs:4382-4421`. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`. Then cherry-pick to master per the Standard Ship Sequence in CLAUDE.md.

## References

- `src/tools/symbol/display.rs:155-170` — the misleading elision
- `src/tools/core/types.rs:423-468` — the two output paths
- `src/tools/core/types.rs:263-277` — `truncate_compact` behavior
- `src/tools/read_file.rs:54-165` — `json_path` JSON-only enforcement
- `docs/issues/archive/2026-05-09-read-file-json-path-array-elements.md`
- `docs/issues/archive/2026-05-17-read-file-jsonpath-negative-slice.md`

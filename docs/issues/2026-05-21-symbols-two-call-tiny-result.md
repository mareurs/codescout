---
status: investigating
opened: 2026-05-21
closed:
severity: low
owner: marius
related: []
tags: [symbols, token-efficiency, progressive-disclosure]
kind: bug
---

# BUG: symbols(name=...) forces two calls for a tiny result

## Summary
`symbols(name=X, path=Y)` returns location-only in exploring mode. Agents then
re-call with `include_body=true` to read a 10-line class. Two MCP round-trips
where one would do — pure token + latency waste for the common single-symbol
lookup.

## Symptom (Effect)
Observed user-reported trace (verbatim, abridged):

```
● symbols(name: "DocumentChunk", path: "src/mrv/models.py")
  ⎿  src/mrv/models.py (1)
       Class  15-25  DocumentChunk

● symbols(include_body: true, name: "DocumentChunk", path: "src/mrv/models.py")
  ⎿  src/mrv/models.py (1)
       Class  15-25  DocumentChunk
           class DocumentChunk(BaseModel):
               """A semantically meaningful piece of a source document."""
               ...
```

Same query, only difference is the `include_body` flag. The second call is
unnecessary — the matched symbol is 11 lines.

## Reproduction
`git rev-parse HEAD` → see commit at fix time. Any project:

```
symbols(name="<small class or function>", path="<file>")
# observe: location only
symbols(name="<same>", path="<same>", include_body=true)
# observe: location + 10-30 line body
```

## Environment
codescout @ master, exploring mode (default `detail_level`), any LSP-backed
language. Reproducible regardless of host (Claude Code / Copilot / Gemini).

## Root cause
`src/tools/symbol/symbols.rs:219` — `include_body` defaults to
`guard.should_include_body()`, which is `false` in `OutputMode::Exploring`.
The flag is plumbed through `collect_matching` / `symbol_to_json`, so the
body is omitted at extract time. No "auto-inline when result is cheap"
heuristic exists; the existing `BODY_CAP = 5` only *strips* bodies when
explicitly requested, never *adds* them when small.

## Evidence
### Code path
`src/tools/symbol/symbols.rs:219`:
```rust
let include_body = optional_bool_param(&input, "include_body")
    .unwrap_or_else(|| guard.should_include_body());
```

### Default behavior
`OutputGuard::should_include_body()` returns false in Exploring mode (the
default `detail_level`).

## Hypotheses tried
1. **Hypothesis:** schema default already enables `include_body`.
   **Test:** read input_schema at `symbols.rs:113`. **Verdict:** rejected —
   schema sets `"default": false`.
2. **Hypothesis:** `format_search_symbols` strips the body for display.
   **Test:** read display path. **Verdict:** rejected — display only renders
   what's present; the body is absent from the JSON itself.

## Fix
Post-collection auto-inline. After matches are finalized (after `cap_items`,
before per-file hoisting):

- Only when `include_body` was NOT explicitly passed AND not already true.
- Only when `matches.len() <= AUTO_INLINE_MAX_MATCHES` (= 2).
- Only when `Σ (end_line − start_line + 1) <= AUTO_INLINE_MAX_LINES` (= 40).
- Read each file once; slice lines; insert `body` field.

Symmetric to the existing `BODY_CAP = 5` (which strips bodies past 5 on
explicit-true). New inverse cap *attaches* bodies up to 2 on default-false.

Commit SHA: TBD.

## Tests added
TBD — added in same commit as fix. Will assert:
- 1-symbol, 11-line match → body present without `include_body=true`.
- 1-symbol, 200-line match → body absent (over LOC cap).
- 3-symbol match → bodies absent (over match cap).
- Explicit `include_body=false` → body absent (caller intent honored).

## Workarounds
Always pass `include_body=true` when the symbol is known to be small.

## Resume
Implement the post-collection hydration block in
`src/tools/symbol/symbols.rs` between the `cap_items` call and the
`shared_file` hoisting. Add unit tests in `src/tools/symbol/tests.rs`
near the existing `symbols_with_body` / `symbols_no_body` cluster.

## References
- `src/tools/symbol/symbols.rs:219` — default include_body decision.
- `src/tools/symbol/symbols.rs:522-535` — existing BODY_CAP strip logic.
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — design principles this fix follows.

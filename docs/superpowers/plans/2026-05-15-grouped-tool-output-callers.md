# Grouped tool output — caller sweep results

Generated: 2026-05-15

## `references[]` (JSON consumers — read sites)

### Production read sites

- `src/tools/symbol/display.rs:391` — `result["references"]` access in compact display formatting
- `src/tools/symbol/display.rs:401` — `result["references"].as_array()` pattern match in display logic

### Test/spec read sites

- `src/tools/symbol/tests.rs:1384` — `result["references"].as_array()` in test assertion
- `docs/superpowers/specs/2026-05-15-grouped-tool-output-design.md:282` — spec reference to migration path
- `docs/superpowers/specs/2026-05-15-grouped-tool-output-design.md:289` — spec docstring listing JSON shapes
- `docs/superpowers/plans/2026-03-01-rich-tool-output.md:151,153` — plan documentation with example code
- `docs/superpowers/plans/2026-03-01-show-the-data-impl.md:322,332` — plan documentation with example code
- `docs/superpowers/plans/2026-03-01-tool-output-buffer-impl.md:361` — plan documentation with example code
- `docs/superpowers/plans/2026-05-15-grouped-tool-output.md:557,575,790,792,943` — plan cross-references and step descriptions

## `matches[]` (JSON consumers — read sites)

### Production read sites

- `src/symbol/query.rs:390` — `matches[0].end_line` indexing in symbol query logic (NOT JSON consumer; local vector)
- `src/tools/read_file.rs:425` — `matches[0]` indexing in read_file logic (NOT JSON consumer; local vector)
- `src/tools/edit_file.rs` (multiple lines in tests.rs and grep.rs):
  - `src/tools/grep.rs:225` — `matches[0].get()` in context detection (NOT JSON consumer; local vector)
  - Multiple test assertions reading `result["matches"].as_array()` in `src/tools/edit_file/tests.rs`:
    - Line 901, 921, 946, 1241, 1473, 1500, 1884, 1890, 1894, 1921, 1923, 1952, 1978, 2002, 2016, 2063, 2095, 2101, 2172, 2174, 2175, 2201, 2204, 2208, 2211, 2246, 2252, 2354, 2360, 2361, 2391, 2459, 2466

### Semantic JSON consumer (grep tool)

- `tests/integration.rs:66` — `search_result["matches"].as_array()` in integration test

### Test/spec (non-JSON, local vectors)

- `src/tools/symbol/tests.rs:2239–2307` — References test fixture (semantic search result struct, NOT JSON array; local Rust struct)
- `src/prompts/mod.rs:832` — `result.matches("### Symbol Navigation Patterns").count()` (string method, not JSON)
- `tests/bug_regression.rs` — Multiple `.matches()` string method calls (lines 116, 175, 241, 246, 306, 311, 399, 400, 463, 464)
- `tests/symbol_lsp.rs:1554` — `result.matches("use std::fmt;").count()` (string method, not JSON)
- Docs/plans:
  - `docs/superpowers/plans/2026-02-28-physical-position-access-impl.md:235–339` — plan examples with JSON access patterns
  - `docs/superpowers/plans/2026-03-02-symbol-range-redesign-impl.md:365` — plan example (string method)
  - `docs/superpowers/plans/2026-03-25-query-shape-detection.md:335,347,419` — plan examples
  - `docs/superpowers/plans/2026-05-05-read-file-source-hint-gate.md:286` — plan example
  - `docs/superpowers/plans/2026-05-15-grouped-tool-output.md:557,579,964,1078` — plan cross-references

## Prompt-surface mentions

### `src/prompts/server_instructions.md`
No hits — no references to `references[]` or `matches[]` JSON shapes.

### `src/prompts/onboarding_prompt.md`
No hits — no references to `references[]` or `matches[]` JSON shapes.

### `src/prompts/builders.rs`
No hits — no references to `references[]` or `matches[]` JSON shapes.

## Migration size estimate

### Breaking JSON read sites

**`references[]` JSON consumers:**
- Production: 2 sites in `src/tools/symbol/display.rs` (compact formatting)
- Tests: 1 site in `src/tools/symbol/tests.rs`
- Total: 3 production+test consumers

**`matches[]` JSON consumers:**
- Production: 1 site in `tests/integration.rs::grep` (semantic search result)
- Tests: **27 sites** in `src/tools/edit_file/tests.rs` (edit_file test suite is extensive)
- Total: 28 production+test consumers

### Code locations requiring updates in Tasks 7–8

| Category | Count | Location |
|----------|-------|----------|
| `references[]` JSON access | 3 | `src/tools/symbol/` (display.rs + tests.rs) |
| `matches[]` JSON access | 28 | `src/tools/edit_file/tests.rs` (27 test cases) + `tests/integration.rs` (1) |
| **Total breaking changes** | **31** | Across 4 files |

### Non-breaking occurrences (coincidental matches)

The grep results include many false positives that are **not** JSON consumers:
- `src/symbol/query.rs`, `src/tools/read_file.rs`, `src/tools/grep.rs` — local vector/struct indexing
- `src/prompts/mod.rs`, `tests/` suite — string `.matches()` method calls (not JSON)
- Plan/spec documentation — prose descriptions and example code blocks

These should be **left alone** during the migration.

## Prompt-surface update decision

**ONBOARDING_VERSION bump required?** **No**

Rationale: Neither `server_instructions.md`, `onboarding_prompt.md`, nor `builders.rs` contain any documentation of the `references[]` or `matches[]` JSON shapes. The prompt surfaces do not reference the old JSON schema by name, so there is no stale documentation to fix. The migration is purely internal (tool implementation + test updates).

## Filtering notes

**Actual JSON consumers (break in Tasks 7–8):**
- `result["references"].as_array()` or `result["references"]` direct access
- `result["matches"].as_array()` or `result["matches"]` direct access
- Test assertions that unpack these arrays into individual items

**Coincidental matches (leave alone):**
- Local variables named `matches` or `references` in Rust code
- String method `.matches()` on `&str` (Rust standard library)
- English prose like "the results match" or "list references"
- Parameter names in function signatures

Total non-JSON occurrences filtered out: ~40+ lines across code, tests, and docs.


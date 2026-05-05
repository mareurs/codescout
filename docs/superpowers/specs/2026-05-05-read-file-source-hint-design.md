# read_file source-range hint gate

**Date:** 2026-05-05
**Status:** approved

## Problem

Agents consistently use `read_file(path, start_line, end_line)` on source files when they should use `symbols(name, include_body=true)`. The dominant pattern observed in traces:

```
symbols(name="ONBOARDING_VERSION", path="src/tools/onboarding.rs")
  → result includes start_line: 19
→ read_file(path="src/tools/onboarding.rs", start_line=19, end_line=30)  ← silent
→ edit_file(...)
```

The agent treats `start_line`/`end_line` from `symbols` output as coordinates to fetch via `read_file`, bypassing `symbols(include_body=true)` which would do both steps in one.

**Current state:** `read_full_file` (no range) has a hint for source files. `read_with_line_range` (the dominant path) returns raw content with **no hint**.

## Decision

- **Detection:** tree-sitter AST (`extract_symbols_from_source`) — synchronous, ~1ms, zero extra I/O
- **Matching:** strict containment only — symbol body `[s, e]` contains read range OR read range contains symbol body (both 0-indexed internally). Partial overlaps ignored.
- **Gate strength:** soft block (`RecoverableError`) + `force: true` escape hatch
- **Escape param:** `force: bool` — mirrors `acknowledge_risk` in `run_command`

## Data flow

`read_with_line_range` already holds `text` and `resolved`. After validation, before content extraction:

1. If `force == true` → skip gate entirely, proceed as before
2. If `detect_file_type(path) != Source` → skip gate
3. Call `extract_symbols_from_source(text, detect_language(resolved), resolved)` — tree-sitter parse
4. Flatten symbol tree recursively (top-level + all `.children`)
5. Convert: `s0 = start - 1`, `e0 = end - 1` (1-indexed → 0-indexed)
6. Filter: keep symbols where `sym.start_line <= s0 && e0 <= sym.end_line` OR `s0 <= sym.start_line && sym.end_line <= e0`
7. On parse error → return `vec![]`, fail open (no hint)
8. If matches non-empty → `RecoverableError` (see message shape below)
9. If empty → pass through, no change

## Error message shape

Single match:
```
"source range overlaps named symbol 'impl Tool for ReadFile/call'"

hint: "Use symbols(name='impl Tool for ReadFile/call', include_body=true) to read
       the body directly. Pass force=true to read the raw line range anyway."
```

Multiple matches (cap at 3 names, then `and N more`):
```
"source range overlaps: 'fn read_with_line_range', 'fn read_full_file'"
```

## Schema changes

`ReadFile::input_schema` — new optional param:
```json
"force": {
  "type": "boolean",
  "description": "Skip source-symbol hint and read the raw line range."
}
```

`ReadFile::description` — append:
```
Source files: a start_line+end_line range overlapping a named symbol is redirected
to symbols(include_body=true); pass force=true to bypass.
```

## Implementation units

All changes in `src/tools/read_file.rs`.

### 1. `flatten_symbols`

```rust
fn flatten_symbols<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for sym in syms {
        out.push(sym);
        flatten_symbols(&sym.children, out);
    }
}
```

### 2. `find_symbols_for_range`

```rust
fn find_symbols_for_range(text: &str, resolved: &std::path::Path, start: u64, end: u64) -> Vec<String>
```

- Calls `crate::ast::parser::extract_symbols_from_source(text, crate::ast::detect_language(resolved), resolved)`
- Flattens via `flatten_symbols`
- `s0 = (start - 1) as u32`, `e0 = (end - 1) as u32`
- Strict containment: `sym.start_line <= s0 && e0 <= sym.end_line` OR `s0 <= sym.start_line && sym.end_line <= e0`
- Returns `sym.name_path.clone()` for each match
- On `Err` → returns `vec![]`

### 3. `read_with_line_range` modification

```rust
let force = input["force"].as_bool().unwrap_or(false);
// ... existing validation ...
if !force {
    if detect_file_type(path) == FileSummaryType::Source {
        let matches = find_symbols_for_range(text, resolved, start, end);
        if !matches.is_empty() {
            // build message + hint, return RecoverableError
        }
    }
}
```

### 4. Schema + description update

Add `force` to `input_schema`. Append one sentence to `description`.

## Tests

Location: `src/tools/edit_file/tests.rs` (existing read_file test suite).

| Test | Setup | Expected |
|---|---|---|
| `read_file_source_range_blocked_when_symbol_overlaps` | Rust fixture, range spans a `fn` body | `RecoverableError` with symbol name |
| `read_file_source_range_force_bypasses_gate` | Same range, `force: true` | success + content |
| `read_file_source_range_not_blocked_for_imports` | Lines 1–5 (`use` stmts, no symbol spans them) | success |
| `read_file_source_range_non_source_not_blocked` | TOML file with line range | success |

## Legitimate `read_file` uses unaffected

- Buffer refs (`@tool_*`, `@cmd_*`, `@file_*`) — bypass gate entirely (handled earlier in `call`)
- Non-source files (TOML, JSON, YAML, config) — `detect_file_type != Source`, gate skipped
- Import blocks at top of file — `use`/`import` statements at lines 1–N have no symbol body spanning them in tree-sitter output → no match → gate silent
- Full-file reads (no `start_line`/`end_line`) — handled by `read_full_file`, not `read_with_line_range`
- `force: true` — any legitimate cross-symbol read that happens to contain a body

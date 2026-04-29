---
title: LSP tools (hover / goto_definition) high error rate
status: draft
owner: marius
created: 2026-04-29
related: [src/tools/symbol/hover.rs, src/tools/symbol/goto_definition.rs]
---

# LSP tools error-rate fixes

## Background

Aggregated 61,282 tool calls across 77 codescout-instrumented projects
(`~/.codescout/usage.db` + per-project DBs) on 2026-04-29. Two LSP tools have
strikingly bad error rates:

| tool | calls | err% |
|---|---|---|
| `hover` | 35 | **65.7%** |
| `goto_definition` | 6 | 33.3% |
| `edit_markdown` | 1197 | 22.1% (separate issue) |
| `edit_file` | 2535 | 18.4% (separate issue) |
| `find_references` | 75 | 17.3% (separate issue) |

Tool descriptions and examples already exist; LLMs simply abandon these tools
after a few failures. Low usage is downstream of brokenness, not redundancy.

## Hover error breakdown (23 errors)

| count | error |
|---|---|
| 7 | `Mux connection lost` |
| 3 | `Failed to spawn mux process` |
| 5 | `no hover info at <path>:<line>` |
| 8 | `identifier '<X>' not found on line <N>` |
| 1 | `missing 'line' parameter` |

`goto_definition` (2 errors) shows the same patterns: `no definition found`,
`identifier '<X>' not found on line N`.

## Three root causes

### 1. Misclassification — empty LSP result returned as `error`

Both tools call `RecoverableError::with_hint(...)` when the LSP returns
no hover text / no definition. This is a *successful empty result*, not a
tool failure.

- `src/tools/symbol/hover.rs:138` — `None => Err(RecoverableError…)`
- `src/tools/symbol/goto_definition.rs:101` — `if definitions.is_empty()
  { return Err(RecoverableError…) }`

**Fix:** return `Ok(json!({"content": null, "hint": "no hover info at …"}))`
or analogous `definitions: []` shape. Drops `hover` err rate from 66% → ~50%
and `goto_definition` from 33% → 17% with no behavioral change for callers
that already treat empty results as terminal.

### 2. Brittle `line + identifier` parameter shape (~35% of hover errs)

The current contract is `(path, line, identifier?)`. The LLM:

1. Reads code in some earlier turn.
2. Builds a tool call with `line=N` and `identifier="X"` from that earlier
   reading.
3. The file has shifted (or it never re-verified the line), so identifier X
   is no longer on line N → `identifier '<X>' not found on line N`.

This is the worst of both worlds: the tool requires *both* a positional
(line) and a content (identifier) anchor, and demands they agree. Better
contracts:

- **`path:line:col`** — LSP-native. No identifier name to mismatch. Forces
  the caller to actually compute a column, which usually means re-reading.
- **`symbol="MyStruct/method"`** — looked up via the symbol index; robust
  to file edits.

Either alone is more durable than the current pair. Worth keeping
`identifier` as a fallback hint, but accepting a column directly should be
the primary path.

### 3. LSP supervisor flakiness (~43% of hover errs, all of `Mux connection
lost` / `Failed to spawn mux process`)

`Mux connection lost` (7) and `Failed to spawn mux process` (3) are
infrastructure errors in the LSP mux supervisor. Separate from UX —
tracking here so the count is preserved, but the fix lives in
`src/lsp/mux.rs` and `src/lsp/client.rs`. Likely causes: idle-timeout race,
mux crash on second client reconnect after Kotlin LSP cold start.

## Action plan (future session)

1. **Misclassification fix** — straightforward two-file change:
   - `hover.rs`: `None` arm returns `Ok(json!({"content": null, "location": …}))` plus a hint field.
   - `goto_definition.rs`: empty result returns `Ok(json!({"definitions": [], "from": …, "hint": …}))`.
   - Update callers' assertions in `src/tools/symbol/tests.rs` if any.

2. **Param-shape fix** — accept `col` (or `column`) on both tools; make
   `identifier` strictly optional and used only when neither `col` nor a
   resolvable cursor exists. Update tool descriptions + 3 prompt surfaces.

3. **LSP supervisor reliability** — separate investigation; `Mux connection
   lost` should at minimum trigger an automatic single retry with a
   newly-spawned mux before surfacing as an error.

## Out of scope / future tracker candidates

- `edit_markdown` 22% / `edit_file` 18% / `find_references` 17% — distinct
  failure modes; sample the error_msg column from `usage.db` before fixing.
- `register_library` (1 call ever) and `list_libraries` (2) — tool-surface
  consolidation, not error-rate work.

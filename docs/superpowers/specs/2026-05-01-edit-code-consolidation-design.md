# Design: `edit_code` — Symbol Mutation Tool Consolidation

**Date:** 2026-05-01
**Status:** Approved

## Goal

Consolidate four symbol-mutation tools (`rename_symbol`, `remove_symbol`, `replace_symbol`,
`insert_code`) into a single `edit_code` tool. Fewer tool names to remember, no behavioral
regressions, schema complexity is manageable for LLM consumers.

## Schema

**Name:** `edit_code`

**Required params:** `symbol`, `path`, `action`

```json
{
  "type": "object",
  "required": ["symbol", "path", "action"],
  "properties": {
    "symbol":   { "type": "string" },
    "path":     { "type": "string" },
    "action":   { "type": "string", "enum": ["rename", "remove", "replace", "insert"] },
    "new_name": { "type": "string", "description": "rename only" },
    "body":     { "type": "string", "description": "replace: new body; insert: code to inject" },
    "position": { "type": "string", "enum": ["before", "after"], "description": "insert only, default 'after'" }
  }
}
```

**Description string:**
> Mutate a symbol in the codebase. action='replace': overwrite the symbol body. action='insert': inject code adjacent to a symbol. action='remove': delete the symbol. action='rename': rename across the entire codebase via LSP (also sweeps textual occurrences in comments/strings).

## Dispatch

Match on `action`, delegate to private async methods:

- `"rename"`  → `do_rename(ctx, symbol, path, new_name)`
- `"remove"`  → `do_remove(ctx, symbol, path)`
- `"replace"` → `do_replace(ctx, symbol, path, body)`
- `"insert"`  → `do_insert(ctx, symbol, path, body, position)`

Each method carries the exact logic from its predecessor tool — no behavioral changes.

## Runtime Validation

Before dispatch, validate action-specific required params:

| action    | requires    | error on missing                         |
|-----------|-------------|------------------------------------------|
| `replace` | `body`      | `"action 'replace' requires 'body'"`    |
| `insert`  | `body`      | `"action 'insert' requires 'body'"`     |
| `rename`  | `new_name`  | `"action 'rename' requires 'new_name'"` |
| unknown   | —           | `"unknown action '...'"`                |

All validation errors route as `RecoverableError`.

## Error Handling

- Bad input / missing required param / unknown action → `RecoverableError`
- LSP unavailable for `rename` → `RecoverableError` (same availability gate as current `rename_symbol`)
- Genuine failures (file not found, AST parse error) → `anyhow::bail!` → fatal

## Output

- `replace`, `insert`, `remove` → `json!("ok")`
- `rename` → rich output: modified files list + textual occurrence sweep report

No behavioral regressions from predecessor tools.

## File Changes

| Change | Detail |
|--------|--------|
| New | `src/tools/symbol/edit_code.rs` — struct `EditCode`, all dispatch logic |
| Delete | `src/tools/symbol/rename_symbol.rs` |
| Delete | `src/tools/symbol/remove_symbol.rs` |
| Delete | `src/tools/symbol/replace_symbol.rs` |
| Delete | `src/tools/symbol/insert_code.rs` |
| Update | `src/tools/symbol/mod.rs` — remove old exports, add `EditCode` |
| Update | `src/server.rs` — drop 4 registrations, add 1 `EditCode`; tool count 29 → 26 |
| Update | `src/prompts/server_instructions.md` — replace old tool names with `edit_code` |
| Update | `src/prompts/onboarding_prompt.md` — replace old tool names with `edit_code` |
| Update | `src/prompts/builders.rs` — replace old tool names with `edit_code` |
| Update | `src/tools/onboarding.rs` — bump `ONBOARDING_VERSION` |

## Verification

- `cargo fmt && cargo clippy -- -D warnings && cargo test` must pass
- Test `prompt_surfaces_reference_only_real_tools` catches any stale tool name refs
- `cargo build --release` + `/mcp` restart for live MCP verification

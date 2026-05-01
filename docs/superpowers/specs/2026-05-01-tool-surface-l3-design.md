# Tool Surface Compression (L3)

**Status:** Draft
**Date:** 2026-05-01
**Blocks:** Code graph + blast radius (A) — spec to be written after L3 lands; tracked in `docs/socraticode-borrow-tracker.md`.
**Tracker:** `docs/socraticode-borrow-tracker.md`

## Goal

Reduce codescout's MCP tool count from 25 to 19 by merging tools with overlapping responsibilities, before introducing the `call_graph` tool. Cleaner prompt surface, less LLM attention spent on tool disambiguation, no semantic loss.

## Non-Goals

- New capabilities. Every kept tool retains its current capability; only the surface changes.
- Backwards-compatible aliases. This is a hard cutover.
- Designing `call_graph`. Reserved for spec A. L3 ships `call_graph` as a placeholder name only — no implementation.
- Migrating user-facing skills/agents in third-party plugins beyond `codescout-companion`.

## Final Tool Surface (19 tools)

| # | Tool | Replaces | Notes |
|---|------|----------|-------|
| 1 | `symbols(path?, name?, name_path?, kind?, include_body?, depth?)` | `find_symbol` + `list_symbols` | Polymorphic by inputs: path-only → file overview; name → search; both → scoped search. |
| 2 | `symbol_at(path, line, fields=[def, hover])` | `goto_definition` + `hover` | Fields default `[def, hover]`; both LSP single-shot at position. |
| 3 | `references(symbol)` | `find_references` (renamed) | All LSP references, undifferentiated. Kind filter deferred to spec A. |
| 4 | `call_graph` | (new placeholder) | Schema deferred to spec A. Registered with stub implementation returning "not yet implemented" so tests can reference it. |
| 5 | `search_pattern(regex, path?, glob?)` | unchanged | Regex search. |
| 6 | `semantic_search(query)` | unchanged | Embedding search. |
| 7 | `tree(path?, glob?, recursive?, max_depth?)` | `list_dir` + `find_file` | Polymorphic: glob present → file search; absent → directory listing. |
| 8 | `read_file` | unchanged | |
| 9 | `create_file` | unchanged | |
| 10 | `edit_file` | unchanged | |
| 11 | `replace_symbol` | unchanged | |
| 12 | `insert_code` | unchanged | |
| 13 | `rename_symbol` | unchanged | |
| 14 | `remove_symbol` | unchanged | |
| 15 | `run_command` | unchanged | |
| 16 | `workspace(action=activate|status|list_projects)` | `activate_project` + `project_status` | Action-param style; matches `memory` pattern. |
| 17 | `library(action=list|register)` | `list_libraries` + `register_library` | Action-param style. |
| 18 | `index(action=build|status)` | `index_project` + `index_status` | Action-param style. `remove` action deferred. |
| 19 | `memory(action=...)` | unchanged | Already action-param style. |
| — | `onboarding` | unchanged | |

**Removed (folded):** `find_symbol`, `list_symbols`, `find_references`, `goto_definition`, `hover`, `list_dir`, `find_file`, `activate_project`, `project_status`, `list_libraries`, `register_library`, `index_project`, `index_status`.

**Net:** 25 − 13 + 6 + 1 placeholder = 19.

## Per-Tool Schema Decisions

### `symbols`

Superset of current `find_symbol`. When `path` provided without `name`/`name_path`, behavior is the current `list_symbols` (file-scoped overview, top-level cap of 100). When `name` provided, behavior is current `find_symbol`. `kind` filter retained from `find_symbol`. `depth` controls hierarchical descent for file overviews.

### `symbol_at`

```
symbol_at(path: string, line: integer, fields?: ["def" | "hover"])
```

Default `fields = ["def", "hover"]`. Each field corresponds to one LSP request (`textDocument/definition`, `textDocument/hover`). Output keyed by field name. `type` and `doc` rejected during schema review — LSP hover content already includes both, parsing it apart is fragile across language servers.

### `references`

```
references(symbol: string)
```

Identical behavior to current `find_references`. Pure rename.

### `call_graph` (placeholder)

```
call_graph(symbol: string, direction: "callers" | "callees" | "both", max_depth?: integer)
```

Registered in the tool registry but its `call()` returns `RecoverableError("not yet implemented — spec at docs/superpowers/specs/<A>")`. This lets:
- Prompt surfaces reference the final name today.
- The `prompt_surfaces_reference_only_real_tools` test pass.
- Spec A focus on internals (LSP recursion, sqlite cache schema) without surface bikeshedding.

### `tree`

```
tree(path?: string, glob?: string, recursive?: boolean, max_depth?: integer)
```

When `glob` provided → behaves like current `find_file`. When absent → like current `list_dir`. `recursive` and `max_depth` apply to listing mode.

### `workspace`, `library`, `index`

All three follow the `memory(action=...)` pattern:

```
workspace(action: "activate" | "status" | "list_projects", ...action-specific args)
library(action: "list" | "register", ...)
index(action: "build" | "status", ...)
```

Action-specific args nested or top-level — to be settled during implementation; not a design-level decision.

## Migration: Hard Cutover

- Single PR / commit series on `experiments`.
- Bump `Cargo.toml` minor version (codescout is pre-1.0).
- `CHANGELOG.md` entry under "Breaking changes" listing every renamed tool.
- Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` (auto-refreshes generated system prompts on next session).
- No aliases. No deprecation period. Old names removed in same commit.

## Surfaces to Update (mandatory checklist)

1. **Tool registration** in `src/server.rs::CodeScoutServer::from_parts` — replace 13 old `Arc::new(...)` lines with 6 new ones (+ `call_graph` stub).
2. **`Tool` impl files** — rename / merge structs in `src/tools/*.rs`.
3. **Test `server_registers_all_tools`** — update tool name set.
4. **Test `prompt_surfaces_reference_only_real_tools`** — passes automatically once prompts updated; no allowlist changes expected.
5. **`src/util/path_security.rs::check_tool_access`** — update match arms for renamed write tools (`replace_symbol` etc unchanged; nothing in this layer is being merged but the gate enumerations must list final names).
6. **Prompt surfaces (all three):**
   - `src/prompts/server_instructions.md`
   - `src/prompts/onboarding_prompt.md`
   - `build_system_prompt_draft()` in `src/prompts/builders.rs`
7. **`src/tools/onboarding.rs`** — bump `ONBOARDING_VERSION`.
8. **`CLAUDE.md`** — update tool count ("25 tools" → "19 tools") and any tool-name mentions.
9. **`README.md`** — feature/tool list.
10. **`docs/ARCHITECTURE.md`** — tool taxonomy section.
11. **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — examples referencing renamed tools.
12. **`docs/manual/` (mdbook)** — every page mentioning renamed tools.
13. **`codescout-companion` plugin** at `../claude-plugins/codescout-companion/`:
    - `hooks/semantic-tool-router.sh` — error-message tool-name suggestions.
    - `hooks/pre-tool-guard.sh` — error-message tool-name suggestions.
    - `hooks/session-start.sh` and `hooks/subagent-guidance.sh` — injected guidance text.
    - `skills/` — any shipped skill prompts referencing tool names.
    - Companion's own version bump.
14. **Memory templates** — `architecture`, `onboarding`, etc. that mention specific tool names.
15. **`docs/socraticode-borrow-tracker.md`** — mark L3 ✅, unblock A.

## Testing Strategy

- **Unit tests:** rename/merge as needed. No new behavioral tests — every kept capability already has coverage; we're consolidating, not adding.
- **Integration test:** new `server_tool_count_matches_l3` asserting registry length == 19 (+ `call_graph` stub if counted) — tripwire against accidental tool re-introduction.
- **Regression test:** `prompt_surfaces_reference_only_real_tools` is the canonical guard against stale name references; rely on it.
- **Manual verification (mandatory before cherry-pick to master):** `cargo build --release` + `/mcp` restart + run each new tool name once via the live server.

## Risks

| Risk | Mitigation |
|------|------------|
| External user skills/agents reference old names | Pre-1.0 prerogative; `CHANGELOG.md` entry; `ONBOARDING_VERSION` bump auto-refreshes generated prompts. |
| Action-param tools harder for LLM to pick | Mitigated by clear action enums in schemas; `memory` already proves the pattern works. |
| `tree` polymorphism (glob vs listing) confusing | Schema description must call out the two modes explicitly; example in `server_instructions.md`. |
| `call_graph` stub breaks LLM expectations | Stub returns `RecoverableError` with explicit "not yet implemented" message and link to spec A. |

## Out of Scope (deferred)

Tracked in `docs/socraticode-borrow-tracker.md` § Deferred decisions:

- `references` kind filter (`call|noncall`) — adds tree-sitter classifier, defer to A.
- `flow(from, to)` path-search tool — separate addition.
- `index(action=remove)` — destructive op, separate review.
- Considering further L4 compression after A lands.

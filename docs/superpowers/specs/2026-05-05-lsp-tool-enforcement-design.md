# LSP Tool Enforcement — Design Spec

**Date:** 2026-05-05  
**Branch:** experiments  
**Status:** approved

## Problem

Three LSP tool usage failure modes observed in real sessions:

1. **`references` rarely used before edits** — blast-radius checks skipped; callers discovered only after breakage
2. **`symbol_at` invisible** — LLMs re-run `semantic_search` when they already have a path+line, wasting tokens
3. **`grep` used instead of `references`** for caller tracing — matches comments, strings, partial names; misses real usages

Root cause: all three tools have weak or absent presence across the three prompt surfaces. `symbol_at` is near-zero everywhere. `references` appears only defensively (as a `grep` replacement). No surface states the expected LSP workflow sequence.

## Scope

Three prompt surfaces + light tool-level enforcement in `edit_code`.  
No session-state tracking. No hard gates. One-line hints only.

---

## Changes

### 1. `src/prompts/server_instructions.md`

#### 1a. New Iron Law #8

```
8. **REFERENCES BEFORE EDITING.** Before `edit_code(action="rename"|"replace")`,
   run `references(symbol, path)` to get the concrete call-site list.
   `call_graph` gives transitive reach; `references` gives the actual locations.
   Skip only when you already ran references for this symbol in this session.
```

#### 1b. Extend Iron Law #7 decision tree

Add one bullet to the existing `grep` decision tree:

```
- "I have a path + line number from tool output" → `symbol_at(path, line)` — type sig + hover docs, no re-search needed
```

#### 1c. New `### LSP Workflow` section

Insert after `### Symbol Navigation Patterns`:

```markdown
### LSP Workflow — Standard Sequence

For any symbol change, in order:
1. `symbols(name=X)` — locate the symbol, get its defining file + line
2. `symbol_at(path, line)` — inspect type signature + docs (when you need to understand what it IS)
3. `references(symbol, path)` — enumerate all call sites before touching anything
4. `call_graph(symbol, path, direction="callers", max_depth=3)` — transitive blast radius for renames/structural changes
5. `edit_code(...)` — make the change
```

#### 1d. Two new anti-pattern rows

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `semantic_search("X")` when you already have path+line for X | `symbol_at(path, line)` | Re-searching wastes tokens; you already have the location |
| `grep("fn_name", path=dir)` to count usages after locating the symbol | `references(symbol, path)` | LSP finds actual usages; grep matches comments, strings, partial names |

---

### 2. `src/prompts/builders.rs` — `build_system_prompt_draft`

Both the single-project and multi-project navigation strategy blocks gain two new steps after the existing `symbols(name=, include_body=true)` step:

```
4b. `symbol_at(path, line)` — hover + type sig when you have an exact location from prior tool output; skip re-searching
4c. `references(symbol, path)` — all call sites before any edit
```

Single-project block (lines ~148–163 in builders.rs):  
Insert after the `symbols(name=..., include_body=true)` push and before the `call_graph` push.

Multi-project per-project subsections:  
Same two insertions after the `symbols("<root>")` step.

---

### 3. `src/tools/symbol/mod.rs` — `edit_code` tool

For `action = "rename"` or `action = "replace"`, return a richer response instead of `json!("ok")`:

```rust
json!({
    "ok": true,
    "hint": format!("verify callers: references(\"{}\", \"{}\")", symbol, path)
})
```

For `action = "remove"` or `action = "insert"`: keep `json!("ok")` unchanged.

**Rationale for rename+replace only:** these are the two actions that displace existing call sites. Remove is self-evident; insert doesn't affect callers.

**Rationale for always firing (not gated on visibility):** private helpers can still have wide blast radius across modules within a crate. A single-line hint is cheaper than a missed caller. No LSP query needed — purely stateless.
## What Is Not Changing

- `onboarding_prompt.md` — Phase 2 checklist already references `call_graph`; no LSP workflow gap there
- Hard gates / `RecoverableError` on `edit_code` — rejected (roundtrip cost, breaks known-blast-radius cases)
- Session-state tracking — rejected (complexity, stateless server is a feature)

---

## Prompt Surface Note

`server_instructions.md` is injected fresh at every MCP session start — no `ONBOARDING_VERSION` bump needed.  
`builders.rs` changes affect the stored per-project system prompt — bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs`.

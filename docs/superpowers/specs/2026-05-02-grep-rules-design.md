# Spec: Grep Rules & Search Guidance Gaps

**Date:** 2026-05-02  
**Status:** Approved

## Problem

Three gaps in codescout's LLM guidance cause recurring tool misuse:

1. No rule distinguishing when `grep` is appropriate vs when `symbols`/`references` should be used. The habit of reaching for `grep` on code files slips through because it "feels natural" for string searches.
2. No reminder that `edit_code` is a deferred tool — its schema must be loaded via `ToolSearch` before the first call each session.
3. Search routing guidance is purely negative ("don't use grep for callers") with no positive example for `semantic_search` on conceptual questions.

## Scope

Changes to `src/prompts/server_instructions.md` only. Three targeted edits:
- Add Iron Law #7 (grep rule)
- Add one row to the Anti-Patterns table (`edit_code` schema loading)
- Update Search Routing section (one qualifier + one positive bullet)

No changes to: companion plugin session-start hook, `.codescout/system-prompt.md`, CLAUDE.md.

## Changes

### 1. Iron Law #7 — Grep is for data files and string literals

Added after Iron Law #6 in `## Iron Laws`:

```
7. **`grep` IS FOR DATA FILES AND STRING LITERALS, NOT CODE STRUCTURE.**
   Use `symbols`, `references`, or `semantic_search` for code.
   Decision tree:
   - "What does symbol X look like?" → `symbols(name=X, include_body=true)`
   - "What's in this file/dir?" → `symbols(path=...)`
   - "How does X work / what calls Y?" → `semantic_search` or `references(symbol, path)`
   - "Find a string literal in JSONL/YAML/config" → `grep` ✓

   `grep` on code gives raw text you must interpret; `symbols` gives structured
   output (signature, body, line range) in fewer tokens with zero ambiguity.
```

**Rationale:** Iron Laws are the highest-visibility, most compression-resilient location. This rule applies to all codescout users, not just this project.

### 2. Anti-Patterns row — `edit_code` schema loading

New row appended to the Anti-Patterns table:

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| Call `edit_code(...)` without loading schema | `ToolSearch("select:mcp__codescout__edit_code")` before first call each session | Schema is deferred — fails with "missing 'action' parameter" until loaded |

**Rationale:** This is a session-start failure mode, not a structural principle — Anti-Patterns table is the right home. Iron Law #2 already says "use `edit_code`"; adding schema loading there would bloat it.

### 3. Search Routing — qualifier + positive semantic_search bullet

Two edits to `### Search Routing`:

- `"Know a text pattern" → grep(pattern)` → `"Know a text pattern in data/config files" → grep(pattern)` (adds qualifier)
- New bullet: `"How does X work?" or concept questions → semantic_search(query) first — faster and more relevant than grep; drill with symbols after`

**Rationale:** Positive guidance ("do this") is more actionable than negative guidance alone. The existing bullet was misleading — it implied `grep` is valid for any text pattern including code.

## What does NOT change

- Companion plugin session-start hook — Iron Law covers all users; no need to duplicate
- `.codescout/system-prompt.md` — Search Tips / Navigation Strategy already adequate; Iron Laws are higher-visibility
- ONBOARDING_VERSION bump — Iron Laws change is significant enough to warrant a bump (handled in implementation plan)

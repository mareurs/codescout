---
id: '079f8b10a0d3e14a'
kind: bug
status: open
title: 'BUG: references gives a silent false zero for a symbol used only in its own file while the reference index warms'
tags:
- cluster/lazy-warmup-bills-the-first-caller
opened: 2026-09-24
owner: marius
related:
- '74268cc1175dd4b0'
- '7bdeb054a5ab2f46'
severity: medium
---

# BUG: `references` gives a silent false zero for a symbol used only in its own file while the reference index warms

## Summary

During rust-analyzer's warm-up, `references` can return `0 references` in total, not even the definition, for a symbol that has references. The false-zero guard `corroborate_zero_references` stays silent when every real reference is in the definition file, because it only scans **other** files for the identifier. The caller gets a bare, confident zero. For a file-local helper, that reads as "unused, safe to delete".

## Symptom (Effect)

Live, 2026-09-24, on a binary containing `e26da0b2`. A cold rust-analyzer started at 16:43:02Z, and four parallel subagents called `references`:

```
references(symbol="CodeScoutServer/live_ledger", path="src/server.rs", limit=5)
→ 0 references            # 16:43:20.004Z, no warning, no completeness_warning
```

A few minutes later, on the warm server, the identical call returned `src/server.rs (4)`: lines 197, 582, 714 and 1260.

The three sibling calls in the same window also returned `0 references`. Each of those symbols appears in other files, so each carried the guard's warning (`warning: LSP returned 0 references outside the definition file, but \`adopt\` appears as a whole word in 3+ other source file(s) …`). Only the file-local symbol was silent.

## Reproduction

Right after `/mcp` (a cold rust-analyzer), call `references` on a symbol whose every use is inside its definition file, within the first ~20 s. Compare with the same call once warm. Not yet reduced to a unit test.

## Environment

Linux, rust-analyzer 1.97.1 (toolchain path in `~/.rustup`), codescout release built 19:40:39 local from `9fb229d0`, stdio MCP, `experiments`.

## Root cause

**Guard scope, measured once, inferred from the guard's design.** `corroborate_zero_references` (`src/tools/symbol/references.rs`) was written for `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md`. That bug's model is that a stale index returns only the definition, so a missing **caller elsewhere** is the case to detect, and the text scan excludes the definition file by design (its test asserts that). Two premises fail here:

1. The cold index returned **nothing**, not even the definition.
2. For a file-local symbol there is no other file to find it in.

So the guard has no input it can fire on.

measured 2026-09-24: cold `references(CodeScoutServer/live_ledger)` gave 0 with no warning; warm gave 4. The guard's exclusion of the definition file is inferred from the archived bug's Tests added section, not re-read in code this session.

## Evidence

This session's live-check transcripts (process-local, not retained). The durable part is the call, the timestamps and both counts above.

## Hypotheses tried

None yet. One discriminator worth running first: does a warming reference index return `[]`, or a list without the definition? The archived bug's model assumes the latter.

## Fix

Not started. A direction to test, not a decision: when `references` returns 0 in total, **including the definition**, that contradicts the LSP contract under `includeDeclaration`. That makes it a self-evident "index not ready" state, which could be retried or warned about without any text scan. Whether codescout asks with `includeDeclaration: true` needs reading first.

## Tests added

N/A: not fixed.

## Workarounds

Corroborate a `references` zero with `grep` over the definition file, or `call_graph(direction="callers")`, especially early in a session.

## Resume

Read `References::call` and `LspClient::references` for the `includeDeclaration` flag and the current guard condition. Then decide between "total zero means not ready" and extending the scan to the definition file (excluding the definition line).

## References

- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` (`74268cc1175dd4b0`): the guard this gap is in
- `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md` (`7bdeb054a5ab2f46`): the live check that surfaced it
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: the rule a silent zero violates

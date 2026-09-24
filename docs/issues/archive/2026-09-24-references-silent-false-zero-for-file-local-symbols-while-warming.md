---
id: 7a481ad70c6a9308
kind: bug
status: fixed
title: 'BUG: references gives a silent false zero for a symbol used only in its own file while the reference index warms'
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: 2026-09-24
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

**Measured 2026-09-24 at the component boundary.** A scripted LSP client polled `textDocument/references` (`includeDeclaration: true`, as `LspClient::references` sends) against a cold rust-analyzer 1.97.1 on this repo, for `live_ledger` (`src/server.rs:713`, single file) and `GuideLedger` (`src/tools/guide_ledger.rs:49`, 13 files):

| time | answer, for both symbols |
|---|---|
| 0.02 s | `[]`: 0 locations, declaration **absent** |
| 0.84 s | `null` (crate-graph swap) |
| 1.16 / 1.39 s | `[]` again |
| 1.55 s | `null` (second swap) |
| 1.71 → ~10.3 s | `ERROR -32801 content modified` (cache priming) |
| 10.48 / 11.09 s | 38 locations in 1 file / 108 in 13 files, declaration **present** |

That gives three not-ready states:
- **`-32801`** is already retried by `request`.
- **`null`** was read as `Ok(vec![])` by `LspClient::references`' own `if result.is_null()`, the same shortcut `document_symbols` had.
- **`[]`** is a successful empty list.

Both `null` and `[]` contradict `includeDeclaration`, and neither reaches `corroborate_zero_references` in a way it can use: the guard scans only OTHER files, so for a file-local symbol there is nothing to find and the zero went out bare. The archived guard's model ("a stale index returns only the definition") was one of four observed states, not the only one.

## Evidence

This session's live-check transcripts (process-local, not retained). The durable part is the call, the timestamps and both counts above.

## Hypotheses tried

Resolved by the boundary probe above: both. A cold rust-analyzer returns `[]` AND `null` (and then `-32801`); it never returns a list without the definition in this run.

## Fix

**FIXED in `1ea1d36b` (2026-09-24).**

- **`References::call` (`src/tools/symbol/references.rs`):** when the server returns **no locations at all**, counted before any filtering, the zero carries a `completeness_warning`: *"LSP returned no locations at all — not even the symbol's own declaration, which this request asks it to include — so the reference index is not ready yet …"*. It needs no text scan, which is what makes it reach a file-local symbol. It defers to the call-hierarchy warning when that one already fired.
- **`LspClient::references` (`src/lsp/client.rs`)** re-asks `null` and `[]` within the not-answered budget (5 s cold, 1 s warm). **On exhaustion the empty answer is kept, not made an error.** A server that ignores `includeDeclaration` answers `[]` for a genuinely unused symbol, and an error there would break the commonest `references` question. The warning above covers that zero instead.
- The retry loop is now one helper, `request_until_answered(method, params, not_answered)`. It is shared by `document_symbols` (`null`) and `references` (`null` or `[]`), and `not_answered_budget()` holds the one budget.

**STANDING: the warning can also fire for a server that ignores `includeDeclaration`** when a symbol is genuinely unused. That is accepted: with the flag sent, such a zero is suspicious by the request's own terms, and the text says to re-run or corroborate rather than asserting the index is broken.

## Tests added

All observed RED against the pre-fix code; all gate-runnable.

- `tools::symbol::tests::references_warns_when_the_answer_omits_even_the_declaration`: a file-local `helper`, and the mock answers `[]`. Pre-fix: no `completeness_warning`.
- `tools::symbol::tests::references_on_an_unused_file_local_symbol_that_returns_its_declaration_stays_bare`: the over-warning control. Only the declaration comes back, and there must be no warning.
- `lsp::client::tests::references_waits_out_an_empty_answer_during_warm_up` and `..._a_null_answer_during_warm_up`: a scripted Unix-socket peer answers `[]` (resp. `null`) twice, then one location. The location must come back after exactly 3 requests. Pre-fix: 0 locations.
- `lsp::client::tests::references_returns_a_persistent_empty_answer_as_empty_not_as_an_error`: persistent `[]` on a warm client must return an empty `Ok` after re-asking. Pre-fix: never re-asked.

**Mutations** (`scripts/mutation-probe.sh`, isolated worktree, one per site, after the refactor so the verdicts measure the shipped bytes): no-declaration warning never fires, predicate drops `[]`, predicate drops `null`, retry takes every answer, retry gives up after one attempt, `document_symbols` exhaustion returns the old `Ok(vec![])`, and `references` exhaustion becomes an error. **7/7 KILLED.** The first run of the last one was refused by the probe's exactly-once guard (the literal also matched `dispatch_lsp_message`) and re-run on a widened literal.

## Workarounds

None needed after `1ea1d36b`. On an older binary: corroborate a `references` zero with `grep` over the definition file, or `call_graph(direction="callers")`.

## Resume

Nothing left. **Live check owed to the next release rebuild.** Right after `/mcp`, while rust-analyzer is cold, call `references` on a file-local symbol (e.g. `CodeScoutServer/live_ledger` in `src/server.rs`). It should either wait out the warm-up and return the real locations, or return `0` WITH the new no-declaration warning. It should never return a bare zero.

## References

- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` (`74268cc1175dd4b0`): the guard this gap is in
- `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md` (`7bdeb054a5ab2f46`): the live check that surfaced it
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: the rule a silent zero violates

## Fix provenance

- **SHA:** `1ea1d36b` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `cd2cc97d9a09c6e3fc59d6da272ff6912abcd92f` — content hash of the diff; survives rebase and cherry-pick.

`fix(references): a zero with no declaration is never bare; null and [] answers are re-asked during warm-up`

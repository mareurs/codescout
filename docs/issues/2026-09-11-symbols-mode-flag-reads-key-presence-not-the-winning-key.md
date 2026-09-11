---
id: fedf989bfc42a718
kind: bug
status: open
title: symbols reads its exact-vs-substring mode off key presence, not off the key that supplied the pattern
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
topic: tool parameter surface
closed: null
opened: 2026-09-11
owner: marius
related: []
severity: high
---

# BUG: `symbols` reads its exact-vs-substring MODE off which keys are PRESENT, not off the key that supplied the pattern

## Summary

`symbols` advertised four name-ish parameters for two concepts (`name`/`query` =
substring, `symbol`/`name_path` = exact name-path). The pattern was resolved by a
precedence chain, but the *mode* was resolved by a separate key-presence test — so a
key that LOST the precedence race, contributing no value at all, still flipped the
matching mode. Two user-visible consequences, both returning a plausible answer rather
than an error: the regex refusal is suppressed, and the `kind` filter is silently
discarded.

## Symptom (Effect)

Against the live MCP binary, four calls, two pairs:

```
symbols(query="Tool|Doc")                    -> RecoverableError: "pattern looks like a
                                                regex (found '|') — symbols searches
                                                symbol names, not text"        [correct]
symbols(query="Tool|Doc", symbol="x")        -> {"symbols": [], "total": 0}    [WRONG]

symbols(name="OutputGuard", kind="function") -> the Function                   [correct]
symbols(name="OutputGuard", kind="function",
        name_path="zzz")                     -> the Struct                     [WRONG]
```

Neither wrong case is an error. Both are shapes a caller reads as an answer: "no such
symbol" and "here is your symbol".

## Reproduction

At `d5a1fea2` on `experiments`. Either through the MCP surface as above, or in-tree:

```
cargo test --lib -- a_losing_symbol_key --nocapture
```

against `src/tools/symbol/symbols.rs` with the mode line reverted to the key-presence
read. Observed RED, verbatim:

```
thread 'tools::symbol::tests::a_losing_symbol_key_cannot_suppress_the_regex_refusal'
panicked at src/tools/symbol/tests.rs:8006:10:
`name` supplied the pattern, so this is a substring search and the regex alternation
must be refused; a bare `symbol` key that lost the precedence race must not flip the
mode: Object {"symbols": Array [], "total": Number(0)}

thread 'tools::symbol::tests::a_losing_symbol_key_cannot_discard_the_kind_filter'
panicked at src/tools/symbol/tests.rs:8038:5:
assertion `left == right` failed: ... Got: Object {"symbols": Array [], "total": Number(0)}
  left: []
 right: [("Widget_new", "Function")]
```

## Environment

Linux, Rust, codescout `experiments` at `d5a1fea2`; MCP over stdio; project `codescout`.

## Root cause

`src/tools/symbol/symbols.rs`, `impl Tool for Symbols::call` as of `d5a1fea2`:

- the pattern came from a four-key precedence chain, `query` -> `symbol` -> `name` ->
  `name_path` (`:189-193`);
- the mode came from a two-key PRESENCE test, `input["symbol"].is_string() ||
  input["name_path"].is_string()` (`:226`);
- that flag then gated three separate behaviours: the regex refusal (`:230`), the
  `kind` filter, which it dropped outright (`:251`), and exact-vs-substring matching
  (`:278-290`).

Two independent resolutions of one decision. Nothing kept them in agreement, and the
disagreement is reachable by any input naming two of the four keys.

Measured 2026-09-11: the four live calls quoted under *Symptom*, plus the two-test RED
above reproduced by mutating `:226` back after the fix.

**A second, independent mode disagreement in the same tool**, found while fixing this
one and fixed with it: `search_project_symbols` re-implemented the name predicate
inline for the LSP `workspace/symbol` fast path (`:642-646`, comment *"Mirror the
predicate above"*) and that copy was unconditionally the SUBSTRING branch. So the
exact-name-path mode never reached the project-wide LSP path at all. Measured live
2026-09-11 before the fix: `symbols(symbol="Tool")` — documented as an exact name-path
lookup — returned `MECHANISM_TOOLS`, `fetch_tools`, `coverage_by_tool`, `CS_WRITE_TOOLS`
and five more substring hits, none of them named `Tool`.

## Evidence

### The four live calls

Quoted verbatim under *Symptom*. Run against the release binary before any change.

### The predicate divergence

`symbols(symbol="Tool")`, live, pre-fix — 19 matches across 9 files, every one a
substring hit from the Python LSP:

```
scripts/probe_guide_section_use.py   MECHANISM_TOOLS, is_mechanism_tool, tool_uses, want_tool
scripts/probe_tool_surface.py        fetch_tools, tools, tool, tools
scripts/file-provenance.py           CS_WRITE_TOOLS, NATIVE_WRITE_TOOLS
```

`symbol_name_matches(sym, "Tool")` — the predicate the path-restricted and tree-sitter
branches use — rejects every one of these.

## Hypotheses tried

1. **Hypothesis:** the two symptoms are separate bugs (one in the regex guard, one in
   the `kind` plumbing).
   **Test:** trace both to their gating expression.
   **Verdict:** rejected. Both are `is_name_path` at `:226`; a per-symptom fix would
   have left the third consumer (exact-vs-substring matching, `:278-290`) wrong.
2. **Hypothesis:** the inline LSP predicate at `:642-646` is in step with the outer one
   and only needs to be kept that way.
   **Verdict:** rejected — it was already out of step, in a direction nothing tested.
   Confirmed by the live `symbols(symbol="Tool")` call above.

## Fix

Collapse the surface to the two canonical names and resolve pattern-and-mode in one
step, so the mode cannot disagree with the value that produced it.

- `src/tools/symbol/symbols.rs` — `param_aliases()` declares `("query","name")` and
  `("name_path","symbol")`; `query`/`name_path` deleted from `input_schema()`;
  `call()` resolves `(pattern, is_name_path)` from a single `name`-then-`symbol` chain;
  `kind_filter` applies in both modes; the inline LSP predicate now calls the caller's
  `name_ok` rather than re-implementing one branch of it.
- `src/server.rs` — `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL` + `EXPECTED_ALIAS_PAIRS` gain
  the two pairs; `description_declares_an_alias` widened to the parenthetical form the
  collapse just removed; `TOOL_SURFACE_CHAR_BUDGET` ratcheted 55_355 -> 55_093.

SHA: *(recorded at archive)*
patch-id: *(recorded at archive)*

## Tests added

`src/tools/symbol/tests.rs`:

- `a_losing_symbol_key_cannot_suppress_the_regex_refusal`
- `a_losing_symbol_key_cannot_discard_the_kind_filter`
- `kind_filters_an_exact_name_path_lookup_in_both_directions`
- `the_two_modes_return_different_symbol_sets_on_this_fixture` (the control: without
  it, a mutation collapsing both modes to one leaves the two above green)
- `a_raw_alias_key_reaching_call_directly_is_not_a_name_argument`

Each of the two production sites was mutated separately and produced a RED naming a
different test — `:226`'s mutation reds the two `a_losing_symbol_key_*` tests, the
`kind_filter` mutation reds `kind_filters_an_exact_*` and nothing else.

The deleted `kind_filter_skipped_when_using_name_path` asserted the dropped behaviour
against its own re-implementation of it, never against `Symbols::call`, so it was
evidence of nothing in either direction.

## Workarounds

Send exactly one name-ish parameter per call. Any two of `name`/`query`/`symbol`/
`name_path` in one call had undefined-in-practice behaviour.

## Resume

N/A — fixed.

## References

- `src/tools/symbol/symbols.rs`
- `src/server.rs` (`EXPECTED_ALIAS_PAIRS`, `TOOL_SURFACE_CHAR_BUDGET`)
- `src/tools/core/param_alias.rs` — the alias mechanism this rides on
- `docs/superpowers/specs/2026-09-10-parameter-alias-collapse-design.md`

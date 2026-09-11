---
id: '144891dcdf9bef63'
kind: bug
status: open
title: the MCP smoke scripts call symbols with a parameter it does not have, and a tool that no longer exists
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
topic: tool parameter surface
closed: null
opened: 2026-09-11
owner: marius
related:
- '64d5aa926c9e2f32'
severity: low
unverified: the symbols-parameter half is fixed but unrun; the get_symbols_overview half is not fixed at all
---

# BUG: the MCP smoke scripts call `symbols` with a parameter it does not have, and `get_symbols_overview`, a tool that no longer exists

## Summary

`tests/mcp-smoke-rust.sh` and `tests/mcp-smoke-kotlin.sh` drive the live MCP server as a
manual smoke suite. Every keyed `symbols` call in both sent `pattern`, which `symbols`
has never accepted since the `pattern` -> `query` rename
(`docs/manual/src/tools/api-redesign.md` § *Parameter renames*), and three calls target
`get_symbols_overview`, a tool that was folded into `symbols` and is no longer in the
registry. Neither mistake errors in a way the scripts notice: a `symbols` call with no
recognised name argument dispatches to the path OVERVIEW, which returns symbols, so the
script's `assert_symbols_found` passes while the search path it claims to exercise never
runs.

## Symptom (Effect)

`tests/mcp-smoke-rust.sh:243` before the fix:

```
test_symbols_name_path() {
    call symbols '{"pattern": "impl Tool for Symbols/call", "relative_path": "src/tools/symbol.rs"}'
    if assert_symbols_found && assert_contains "call"; then
        pass 1 "symbols with name_path pattern finds method"
```

Three things wrong in one call and none of them errors: `pattern` is not a `symbols`
parameter, so the call is an overview, not a name-path lookup; `src/tools/symbol.rs` has
not existed since the `src/tools/` regrouping; and `assert_contains "call"` is satisfied
by almost any overview of almost any Rust file.

`get_symbols_overview` at `tests/mcp-smoke-rust.sh:190`, `:344` and
`tests/mcp-smoke-kotlin.sh:138`, `:259` names a tool the server does not register.

## Reproduction

Not run by `cargo test` — these are manual scripts against a live server. Read them:

```
grep -n 'call symbols\|call get_symbols_overview' tests/mcp-smoke-*.sh
```

and compare each payload's keys against `Symbols::input_schema()` in
`src/tools/symbol/symbols.rs`.

## Environment

codescout `experiments`; scripts invoked by hand, outside every gate.

## Root cause

Both scripts were written against an older tool surface and nothing re-reads them: they
are outside `cargo test`, outside clippy, and outside `audit_doc_refs` (which checks
markdown, not shell). A tool rename or a parameter rename updates the code, the schema
and the prompt surfaces, all of which are gated — and leaves these two files untouched,
because no gate names them.

Inferred from `tests/mcp-smoke-rust.sh` and `src/tools/symbol/symbols.rs` — not measured
against a live run, because the Kotlin script needs a project this checkout does not
contain.

## Hypotheses tried

1. **Hypothesis:** the scripts fail loudly today, so the staleness is self-announcing.
   **Verdict:** rejected for the `symbols` half — a no-name-argument call returns the
   path overview, which satisfies `assert_symbols_found`. Not checked for the
   `get_symbols_overview` half, which presumably does error on an unknown tool name;
   that half is loud and has simply never been run.

## Fix

Partial, 2026-09-11: every `symbols` call in both scripts moved from `pattern` to the
canonical `name` (or `symbol`, for the name-path case), and `test_symbols_name_path`'s
dead `src/tools/symbol.rs` path corrected to `src/tools/symbol/symbols.rs`.

**Not fixed:** the five `get_symbols_overview` calls, and the broader question of whether
these scripts should exist outside any gate at all. Left deliberately — fixing them
blind, without a live run against a Kotlin project this checkout does not have, would
replace a stale script with an unverified one.

## Tests added

None, and that is the defect rather than an omission: a shell script outside `cargo test`
has no gate to add a regression to. The mechanizable shape is a check that every tool
name and parameter key appearing in `tests/mcp-smoke-*.sh` exists in the live registry —
the same shape as `prompt_surfaces_reference_only_real_tools`, over a file set that gate
does not cover.

## Workarounds

Read the payload keys against `Symbols::input_schema()` before trusting a smoke pass.

## Resume

Decide whether the smoke scripts are maintained or retired. If maintained: fix the five
`get_symbols_overview` calls in `tests/mcp-smoke-rust.sh` (`:190`, `:344`) and
`tests/mcp-smoke-kotlin.sh` (`:138`, `:259`), then add a registry-vs-script name check so
the next rename cannot silently orphan them again.

## References

- `tests/mcp-smoke-rust.sh`
- `tests/mcp-smoke-kotlin.sh`
- `src/tools/symbol/symbols.rs`
- `docs/manual/src/tools/api-redesign.md` § Parameter renames

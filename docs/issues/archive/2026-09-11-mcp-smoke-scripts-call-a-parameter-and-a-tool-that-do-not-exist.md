---
id: cb19a7d83a727403
kind: bug
status: fixed
title: the MCP smoke scripts call symbols with a parameter it does not have, and a tool that no longer exists
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
topic: tool parameter surface
claimed_at: '2026-09-11T14:40:00Z'
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
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

**Complete, 2026-09-11, commit `9406f3c46ca2e84ce54531c8d9d85e7fd43c1784` (patch-id `dcb565e474519f12b7de1d0ed53b452b77e316b9`).** Builds on the earlier partial fix (`pattern` -> `name`/`symbol`, `src/tools/symbol.rs` -> `src/tools/symbol/symbols.rs`) already landed today.

Decided: the scripts are maintained, not retired — someone had already invested in the partial fix, and a mechanized gate (below) makes maintaining them cheap going forward.

Fixed all five `get_symbols_overview` calls (`symbols(path=...)`, no name/symbol argument, which is what triggers the overview branch — confirmed by reading `Symbols::call()` rather than assumed). Also fixed `test_blocked_error_has_hints`'s assertion, which checked for the literal string `"get_symbols_overview"` in a blocked-read hint that no longer contains it (`src/tools/read_file.rs`'s `outline_hint` now says `symbols(path)` / `symbols(name=...)`) — a third stale reference this file's own § Root cause didn't enumerate.

**The mechanized gate this bug's own § Tests added called for immediately found two MORE dead tool names neither this file nor the partial fix had named:** `search_for_pattern` (renamed to `grep` — 5 call sites across both scripts) and `find_file` (folded into `tree`'s `glob` parameter — 3 call sites). Both are exactly the same staleness class as `get_symbols_overview`, just not yet noticed because, like the parameter-rename half, neither errors loudly on a live server (a nonexistent tool name presumably does error — per this file's own Hypothesis 1, untested — but nobody had run these scripts since the rename to find out).

Added `tests/mcp_smoke_scripts_reference_real_tools.rs`: a static, no-live-server gate that cross-references every `call <name>` in both scripts against `fn name(&self) -> &str { "<literal>" }` bodies under `src/tools/`. Mutation-verified the honest way — it was RED (5 real offenders) before this commit's script fixes and GREEN after, rather than a synthetic mutation against a clean state.
## Tests added

`tests/mcp_smoke_scripts_reference_real_tools.rs` — the shape this section originally
asked for, minus the live registry: cross-references `call <name>` against `src/tools/`'s
`name()` literals statically instead, since no integration test in this crate constructs
a live `CodeScoutServer` (that machinery is private to `src/server.rs`'s own `#[cfg(test)]`
module) and this bug did not need to change that to get real coverage. See § Fix.

## Workarounds

Read the payload keys against `Symbols::input_schema()` before trusting a smoke pass.

## Resume

Done — see § Fix. Nothing left to resume.
## References

- `tests/mcp-smoke-rust.sh`
- `tests/mcp-smoke-kotlin.sh`
- `src/tools/symbol/symbols.rs`
- `docs/manual/src/tools/api-redesign.md` § Parameter renames

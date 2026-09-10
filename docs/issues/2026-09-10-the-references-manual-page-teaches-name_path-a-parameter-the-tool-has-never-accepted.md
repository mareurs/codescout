---
id: '57ecbac8f925d7b8'
kind: bug
status: open
title: 'BUG: the `references` manual page teaches `name_path`, a parameter the tool has never accepted'
owners:
- marius
tags:
- cluster/unclassified
closed: null
opened: 2026-09-10
related: []
severity: low
---

## Summary

`docs/manual/src/tools/symbol-navigation.md`'s `## references` section (lines 228-312) documents
`name_path` as the required symbol-identifying parameter for the `references` tool, in both the
parameter table and both JSON call examples. The tool has never accepted that key: its actual
required parameter is `symbol`. A caller following the manual verbatim sends a request missing the
tool's one `required` field.

## Symptom (Effect)

The manual's parameter table (`symbol-navigation.md:236`) reads:

```
| `name_path` | string | yes | — | Symbol identifier, e.g. `"MyStruct/my_method"` |
| `relative_path` | string | yes | — | File that contains the symbol definition |
```

and both example calls send `"name_path": "AuthService/authenticate_user"` /
`"name_path": "Logger/log"` as the identifying key (lines 249-250, 289-290). The tool's real
`input_schema()` (`src/tools/symbol/references.rs:229-246`) declares `"required": ["symbol"]` and a
`properties` object with keys `symbol`, `path`, `detail_level`, `offset`, `limit`, `scope` — no
`name_path` key anywhere, required or optional, and no alias maps it: `param_aliases()`
(`references.rs:248-250`) returns only `crate::fs::PATH_PARAM_ALIAS_MAP` (the `path`-family
aliases), and `call()`'s `require_str_param(&input, "symbol")` is an exact-key match with no
fallback (`src/tools/core/params.rs:162-167`).

## Reproduction

At `df0bb780dd2459db8b0a95062e6f2d07f3213366` (branch `experiments`): call the MCP tool
`references` with the manual's own example payload —

```json
{"tool": "references", "arguments": {"name_path": "AuthService/authenticate_user", "path": "src/auth/service.rs"}}
```

— and `require_str_param(&input, "symbol")` fails with a missing-required-parameter error, because
`symbol` was never sent.

## Environment

codescout repo, branch `experiments`, `df0bb780dd2459db8b0a95062e6f2d07f3213366`. Documentation-only
defect; no runtime environment variable affects it.

## Root cause

Verified via `git log -S'name_path' --all -- src/tools/symbol/references.rs src/tools/symbol/find_references.rs` — covering the file's full rename lineage back to `92f28d51`/`4a104036` ("extract FindReferences to symbol/find_references.rs", Apr 22 2026) — and by reading the schema at the commit immediately *before* that extraction (`92f28d51^:src/tools/symbol/mod.rs`), where `FindReferences`'s schema already reads `"required": ["symbol", "path"]`: **zero commits, at any point in this tool's recorded history, ever added or removed `name_path` as its schema key.** The manual's `name_path` claim is therefore not a decayed truth (the tool renaming out from under a once-accurate sentence) — the tool never had that name.

The likelier explanation is conflation with a *different, still-current* tool: `symbols` (the `find_symbol`+`list_symbols` merge, `47af2676`) documents `name_path` as a genuine, live alias of its own `symbol` parameter (`"description": "...(alias of symbol)"`), later formalized as a real `param_aliases()` pair by the parameter-alias-collapse plan itself (`34cad9d9`, `("name_path", "symbol")`). `docs/manual/src/tools/symbol-navigation.md`'s `## references` section most likely echoed or copied that neighboring tool's parameter name during authoring, rather than describing a `references`-specific rename. This is an ordinary authoring/conflation error with a findable author and commit in principle, not a statement that was true when written and later falsified by a code change — see the retag note under Hypotheses tried.
## Evidence

`src/tools/symbol/references.rs:229-246` (`input_schema()`):
```rust
json!({
    "type": "object",
    "required": ["symbol"],
    "properties": {
        "symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
        "path": { "type": "string", "description": "File containing the symbol" },
        "detail_level": { "type": "string", "description": "'full' for bodies (default: compact)" },
        "offset": { "type": "integer", "description": "Pagination offset" },
        "limit": { "type": "integer", "description": "Max results (default 50)" },
        "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'", "default": "project" }
    }
})
```

`src/tools/symbol/references.rs:248-250` (`param_aliases()`):
```rust
fn param_aliases(&self) -> crate::tools::param_alias::AliasMap {
    crate::fs::PATH_PARAM_ALIAS_MAP
}
```

`docs/manual/src/tools/symbol-navigation.md:236-250` (the stale doc):
```
| `name_path` | string | yes | — | Symbol identifier, e.g. `"MyStruct/my_method"` |
| `relative_path` | string | yes | — | File that contains the symbol definition |
...
{
  "tool": "references",
  "arguments": {
    "name_path": "AuthService/authenticate_user",
    "relative_path": "src/auth/service.rs"
  }
}
```

## Hypotheses tried

1. **Hypothesis:** `name_path` might be accepted via some generic/framework-level alias fallback
   not visible in `references.rs` itself. **Test:** read `src/tools/core/param_alias.rs` in full,
   including its test module, to check for an implicit default alias list. **Verdict:** rejected —
   `AliasMap` is purely a per-tool-declared `&'static [(&str, &str)]` list with no implicit
   defaults; a tool that declares nothing (or declares only the path-family map, as `references`
   does) gets no other aliasing. **Evidence link:** Evidence section above, `param_aliases()` body.
2. **Hypothesis:** this is in scope for the parameter-alias-collapse plan's Task 6 and should be
   fixed as part of that commit. **Test:** re-read `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md`
   Task 6's Step 2/3 (grep patterns `relative_path` and `"file"\s*:` only). **Verdict:** rejected —
   the plan's scope is explicitly the *path*-family collapse (`relative_path`/`file`/`file_path` →
   `path`); it does not mention `name_path`/`symbol`, and the ADR amendment this plan implements
   also does not cover it. Filing this as a separate bug rather than folding an unrelated rename
   into that commit, per this repo's Bug Tracking policy ("capture on notice, not at task end").
3. **Hypothesis:** `references` itself underwent a `name_path` → `symbol` rename, making this a
   `cluster/doc-contradicted-by-code` (IC-11) instance — true when written, later falsified by a
   code change. **Test:** `git log -S'name_path' --all` over every historical path/filename this
   tool has had, plus the schema at the commit immediately before its extraction into its own file
   (`92f28d51^:src/tools/symbol/mod.rs`). **Verdict:** refuted — zero matching commits; the
   required key has been `symbol` since the earliest recorded schema. Retagged from
   `cluster/doc-contradicted-by-code` to `cluster/unclassified` (the escape hatch documented in
   `docs/trackers/issue-clusters.md`, used when a bug is real but forcing it into a promoted class
   would corrupt that class's count) — this is an authoring/conflation error, which IC-11's own
   admission test explicitly excludes ("an ordinary authoring error with an author to find... does
   not belong here").

## Fix

*Plan first, implementation second.* Replace `name_path` with `symbol` in
`docs/manual/src/tools/symbol-navigation.md`'s `## references` section: the parameter table row
(line ~236), both JSON call examples (lines ~249-250, ~289-290), and the "Tips" prose (lines ~296-298,
which explains `name_path` semantics and should be retitled to `symbol`). Leave `relative_path` in
this same section for whichever session executes the parameter-alias-collapse plan's Task 6 to
replace with `path` — that is a separate, in-flight change on `experiments` as of this filing.

Not yet fixed. No commit exists for this bug at filing time.

## Tests added

N/A — documentation-only fix; no test asserts on manual prose content. Nothing in
`tests/cli_doc.rs` or elsewhere currently checks that `symbol-navigation.md`'s parameter tables
match each tool's live `input_schema()`.

## Workarounds

Callers of `references` should send `symbol` (not `name_path`) and `path` (not `relative_path`)
regardless of what the current manual page says, per `src/tools/symbol/references.rs:229-246`.

## Resume

Fix `docs/manual/src/tools/symbol-navigation.md`'s `## references` section (`name_path` → `symbol`,
lines ~236, ~249-250, ~289-290, ~296-298) in a session not already mid-flight on the
parameter-alias-collapse plan's Task 6 commit, to keep the two unrelated renames in separate
commits. Consider also asking whether `audit_doc_refs` or a dedicated schema-vs-manual checker
should be extended to catch a manual's parameter table diverging from a tool's live
`input_schema()` — this bug would have been caught mechanically rather than by incidental reading.

## References

- `docs/manual/src/tools/symbol-navigation.md` (the stale doc, `## references` section)
- `src/tools/symbol/references.rs:229-250` (the live schema and alias declaration)
- `src/tools/core/param_alias.rs` (confirms no implicit alias fallback)
- `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md` Task 6 (the unrelated,
  concurrently-executing plan whose scope this bug is explicitly outside of)

---
id: '7b468a9f8c201641'
kind: bug
status: open
title: ListFunctions and ListDocs implement Tool, are guarded by 15 tests, and no agent can reach either
tags:
- cluster/declared-not-wired
- tools
- dead-code
- mcp-registry
opened: 2026-09-01
owner: marius
severity: low
---

# BUG: `ListFunctions` and `ListDocs` implement `Tool`, are guarded by 15 tests, and no agent can reach either

## Summary

`src/tools/ast.rs` defines two `impl Tool` types — `ListFunctions` and `ListDocs` — that the
MCP server never registers. `CodeScoutServer::new`'s `tools: Vec<Arc<dyn Tool>>` in
`src/server.rs` names 23 tools inline plus `PeerTool`, `ProbeTool` and the librarian set;
neither of these is among them, and no `match` arm delegates to them the way
`Workspace/call` delegates to `ActivateProject` or `Library/call` to `RegisterLibrary`.

So the capability is declared, schema and all, and **no agent can ever call it.**

## Symptom (Effect)

None visible, which is the point. Nothing errors, nothing warns, and the suite is green —
because the tests reach the tools directly.

## Evidence

Text sweep across `src/**`, `tests/**` and `crates/**` for both identifiers: **25 hits in 4
files, and every non-definition hit is test code.**

| site | count | kind |
|---|---:|---|
| `src/tools/ast.rs` definitions + `impl Tool` | 4 | declaration |
| `src/tools/ast.rs` inline `#[cfg(test)]` call sites | 12 | test |
| `tests/integration.rs` (`workflow_analyze_ast`) | 5 | test |
| `tests/e2e/harness.rs` (`run_list_functions`) | 2 | test |
| `src/tools/symbol/tests.rs` | 2 | test fixture *string*, not a call |

**Production call sites: zero. Registration sites: zero.** Fifteen tests guard two tools
nothing can reach.

The contrast inside the same sweep is what makes this a finding rather than a guess. Seven
other unregistered `impl Tool` types — `ActivateProject`, `ProjectStatus`, `ListLibraries`,
`RegisterLibrary`, `IndexProject`, `IndexStatus` — are all reached in production by an
explicit named delegation (`"activate" => ActivateProject.call(…)`,
`"register" => RegisterLibrary.call(…)`, `"status" => IndexStatus.call(…)`). They are the
consolidated-tool pattern working correctly. These two have no such arm.

`GetUsageStats` is a third candidate and is **not** claimed here: it is re-exported at
`src/tools/mod.rs:29` and the sweep for it was not run to completion. Recorded so the next
reader knows the difference between "checked and clear" and "not checked".

## Why the test suite cannot catch this

This is `OB-7` / `issue-clusters:IC-3` in its purest form, and the entry predicted this exact
shape before the instance was found: *"a unit test constructs its own inputs, so it exercises
the matcher with a selector production never emits, and passes. The test is not weak; it is
scoped to the half that works."*

`ListFunctions.call(json!({…}), &ctx)` works perfectly. It is the *registration* that is
absent, and no test that constructs the tool directly can observe that.

The compiler cannot see it either: both are `pub` in a library crate, so `dead_code` is
exempt by construction regardless of callers.

## How it was found

By running `OB-7`'s prescribed reachability check against a real population, seeded from the
**dispatch** side — enumerate every `impl Tool for X`, diff against the registry — which is
the probe `cluster-promotion-session-log:F-1` named as the one that would either demonstrate
the check's false-positive mode or produce its first genuine negative control. It produced
the latter, plus this.

## Fix

Not taken — this is a **product decision, not a repair**, and the two options differ in what
they preserve:

1. **Register them.** They become reachable, and 15 existing tests become meaningful. But
   `symbols` already covers "list the functions in this file" and `include_docs` covers
   docstrings, so this adds two tools to a surface with a documented description-byte cap.
2. **Delete both, and their 15 tests.** Honest if `symbols` genuinely subsumes them. Note
   the tests are not evidence *for* keeping them — a test of an unreachable tool is exactly
   what this class produces.

Whoever decides should check whether `tests/e2e/harness.rs`'s `run_list_functions` is load-
bearing for the e2e lane's coverage of the AST path, since deleting it may remove the only
non-`symbols` exercise of that code.

## Resume

Decide (1) or (2). Then re-run the same probe against the **remaining** unregistered types —
`GetUsageStats` is unresolved above — and consider whether the registry-vs-`impl Tool` diff
is worth a standing test, since it is a set difference over two lists the code already holds.


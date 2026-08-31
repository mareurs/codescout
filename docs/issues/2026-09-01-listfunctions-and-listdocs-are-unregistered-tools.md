---
id: '7b468a9f8c201641'
kind: bug
status: fixed
title: ListFunctions and ListDocs implement Tool, are guarded by 13 tests, and no agent can reach either
tags:
- cluster/declared-not-wired
- tools
- dead-code
- mcp-registry
closed: 2026-09-01
opened: 2026-09-01
owner: marius
severity: low
---

# BUG: `ListFunctions` and `ListDocs` implement `Tool`, are guarded by 13 tests, and no agent can reach either

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

**Production call sites: zero. Registration sites: zero.**

**The test count, published as its derivation rather than as a figure — this file said "fifteen"
and fifteen does not survive a count.** Corrected 2026-09-01 after a peer failed to
reconstruct it and got 18 by a different rule. Both were answering different questions, and
neither number is the one the claim needs:

```
symbols(path="src/tools/ast.rs", name_path="tests", depth=1)   → 19 functions
  minus project_ctx_with_file (a fixture helper, not a test)   → 18 tests in the module
grep '\b(ListFunctions|ListDocs)\b' with enclosing-symbol annotation
  → 12 of those 18 call the tools by name
+ tests/integration.rs::workflow_analyze_ast                    → 13 tests total
+ tests/e2e/harness.rs::run_list_functions                      → 1 e2e path, reached
                                                                  via run_single's
                                                                  "list_functions" arm
                                                                  (a helper, not a test)
```

So: **13 tests call these two tools, plus one e2e harness path.** The peer's **18** counts every
test in the module, including four formatter tests, `symbols_include_docs_returns_docstrings`,
and `list_functions_omits_source_field` — which despite its name contains neither identifier
and so is not guarding these tools either. My **15** double-counted: it swept in
`src/tools/symbol/tests.rs::symbols_overview_directory_mode`, which contains the *string*
`"ListFunctions"` in a JSON fixture and never calls the tool, and counted the e2e helper as a
test.

**The near-miss is why it survived, and it is the third instance of that shape in one night.**
15-against-18 is plausible enough that no reader stops; an implausible figure would have been
caught immediately. Same property as `OB-4`'s 2/3-accurate liveness marker, and same as the
20%-versus-25% units error corrected in `cluster-promotion-session-log:F-2` an hour earlier. A
ratio or a count that is merely *close* spends trust it never earned.

The contrast inside the same sweep is what makes this a finding rather than a guess. Seven
other unregistered `impl Tool` types — `ActivateProject`, `ProjectStatus`, `ListLibraries`,
`RegisterLibrary`, `IndexProject`, `IndexStatus` — are all reached in production by an
explicit named delegation (`"activate" => ActivateProject.call(…)`,
`"register" => RegisterLibrary.call(…)`, `"status" => IndexStatus.call(…)`). They are the
consolidated-tool pattern working correctly. These two have no such arm.

**`GetUsageStats` — RESOLVED 2026-09-01, and it IS a third instance.** This paragraph previously
read *"a third candidate and **not** claimed here — the sweep was not run to completion"*. The
sweep has now been run. (It also cited `src/tools/mod.rs:29`; the re-export is at **`:28`**.)
Three layers, all green, all unreachable:

1. **`impl Tool for GetUsageStats`** (`src/tools/usage.rs:9`) — **never registered.** Established
   *positively*, by reading `CodeScoutServer::new`'s whole registry (`src/server.rs:322-362`):
   21 unconditional entries plus `PeerTool`, `ProbeTool` and the librarian block. `GetUsageStats`
   appears in **none**, and a `GetUsageStats` grep over `src/server.rs` returns zero. Its only
   constructions are four test bodies (`:143`, `:165`, `:192`, `:213`).
2. **`Tool::pinnable()`'s `"get_usage_stats"` arm** (`src/tools/core/types.rs:754`) — a **matcher
   that can never match**, since `self.name()` is only ever evaluated for *registered* tools and
   none returns that string. That is `IC-3`'s third family sitting inside its first.
3. **`src/server.rs:3106` asserts the behaviour of both, and the assertion is VACUOUS for this
   name.** The `pinnable` set is built from `server.tools` — the registry — so
   `assert!(!pinnable.contains("get_usage_stats"))` passes because the tool is **absent**, not
   because `pinnable()` excludes it. Delete the arm in (2) and the test still passes. **The same
   loop line is live for its two neighbours** (`"workspace"` and `"get_guide"` are both
   registered, so removing their arms would fail it) and vacuous for the third — which is
   exactly why it reads as covered. `CLAUDE.md`'s monotone law, precisely: `!contains` is
   monotone under **removal**, so it cannot distinguish *correctly excluded* from *not there at
   all*.

**Tests: 6**, and here the count has one defensible value rather than four. `mod tests` holds 7
functions less the `ctx_with_project` fixture; **all six** exercise the tool or its formatter by
name — four construct `GetUsageStats`, two call `format_get_usage_stats`, whose sole production
caller is `format_compact` at `:55`, on the unreachable impl. The units coincide *because* every
test in the module touches the dead code; for `ListFunctions`/`ListDocs` they did not, which is
the whole reason that population needed its unit stated.

**The probe nearly returned a false method, and the positive control caught it.** A first pass
grepped `Arc::new\((Grep|GetUsageStats|…)` and found `Grep`, `Onboarding`, `Workspace` but **not
`GetGuide`** — which is demonstrably live. `GetGuide` is registered as
`Arc::new(crate::tools::guide::GetGuide::new())`: qualified path plus a constructor call. So
*"absent from `Arc::new(ShortName)`"* is **not** evidence of non-registration, and had the
control not been in the pattern the sweep would have produced a confident wrong answer by the
same method. Reading the registry in full is what the finding rests on.

## Why the test suite cannot catch this

This is `OB-7` / `issue-clusters:IC-3` in its purest form, and the entry predicted this exact
shape before the instance was found: *"a unit test constructs its own inputs, so it exercises
the matcher with a selector production never emits, and passes. The test is not weak; it is
scoped to the half that works."*

`ListFunctions.call(json!({…}), &ctx)` works perfectly. It is the *registration* that is
absent, and no test that constructs the tool directly can observe that.

The absence was established with a positive control rather than as a bare zero, which is what
makes it a finding: `src/server.rs:326` shows what registration looks like (`Arc::new(Grep)`
inside `CodeScoutServer::new`'s `Vec<Arc<dyn Tool>>`), and `ListFunctions|ListDocs` returns
**0 matches in that same file**. Knowing the shape of a present registration first is what
lets the zero mean *absent* rather than *not searched for*.

The compiler cannot see it either: both are `pub` in a library crate, so `dead_code` is
exempt by construction regardless of callers.

## How it was found

By running `OB-7`'s prescribed reachability check against a real population, seeded from the
**dispatch** side — enumerate every `impl Tool for X`, diff against the registry — which is
the probe `cluster-promotion-session-log:F-1` named as the one that would either demonstrate
the check's false-positive mode or produce its first genuine negative control. It produced
the latter, plus this.

## Fix

**Taken: option (2), delete both — plus a third instance and a standing guard.**

Fixed at `0f28fc28` on `experiments` (patch-id
`467338e4428601351a0801348f2f8419b853c33d`). 18 files, 536 insertions, 1003 deletions.

- **`ListFunctions` / `ListDocs` deleted**, with `src/tools/ast.rs` and its `pub mod ast;`
  declaration. `symbols(path=…)` already returns functions with 1-indexed lines and
  `include_docs=true` returns docstrings, so the capability was subsumed rather than lost.
  `symbols_include_docs_returns_docstrings` was relocated to `src/tools/symbol/tests.rs`
  with its fixture — it covers the *replacement* and only lived in that file because the
  helper did.
- **`GetUsageStats` resolved and deleted.** This file recorded it as *checked-and-unresolved*;
  it is a third instance of the identical shape — `impl Tool` at `src/tools/usage.rs:9`,
  re-exported at `src/tools/mod.rs`, registered nowhere, zero callers outside its own six
  inline tests.
- **The e2e question in the old text is answered: `run_list_functions` WAS load-bearing.**
  `[list_functions_signatures]` fans out across all five language e2e binaries, and because
  `prime_lsp` warms the LSP before every other scenario it was the lane's only exercise of
  `codescout::ast::extract_symbols` — live code reached from ~30 call sites. The runner was
  **retargeted, not deleted**: it now calls `extract_symbols` directly and is renamed
  `tree_sitter_signatures`. Assertion, fixture and five-language fan-out unchanged.
- **A vacuous assertion fell out of the deletion.** `pinnable` in `src/server.rs`'s test is
  built from `server.tools` — the live registry — so
  `assert!(!pinnable.contains("get_usage_stats"))` could never fail: the tool was never in
  the set, and the assertion passed for the wrong reason. Removed. That is `IC-16`
  (`cluster/assertion-that-cannot-fail`), a third live instance of that class.

**Collateral the tools' absence would otherwise have left wrong**, several of which no cargo
gate reports: `src/fs/mod.rs`'s unsupported-file-type hint recommended `list_functions` to
users; `tests/mcp-smoke-{rust,kotlin}.sh` *call* the tool over MCP and would have broken at
runtime; `CLAUDE.md` cited `src/tools/ast.rs:10-11` as live line refs, which `audit_doc_refs`
gates; `docs/manual/src/tools/ast.md` claimed both were *"still registered for backward
compatibility"*, which was never true.
## Resume

Nothing outstanding. Both follow-ups this section asked for are done.

**The registry-vs-`impl Tool` diff shipped** as `tests/tool_reachability.rs`, so the question
*"is any other tool unreachable?"* is now a test rather than a probe someone has to remember
to run. It diffs every `impl … Tool for X` against the types reachable by `Arc::new(X)`
registration or a `=> X.call(` delegation arm, tolerating the seven genuine delegation-only
types behind a two-way tripwire.

Two things in it were established by measurement rather than argument, and both are the
reason to trust it:

- **It was verified against this bug.** Restoring the real pre-deletion `src/tools/ast.rs`
  and re-running makes the guard name `ListDocs` and `ListFunctions`. It would have caught
  this.
- **Its first draft would NOT have.** That draft accepted any `X.call(`, which is textually
  what these tools' own tests did — so it marked all three reachable and passed green on the
  corpus containing the defect. Requiring the match-arm `=>` is what separates a delegation
  from a test call, and `the_scan_discriminates` pins that exact case.

Also closed: `IndexVerify` is declared `impl crate::tools::Tool for IndexVerify`, a qualified
path that a bare `impl Tool for` scan misses — and it is a *seventh* delegation-only type
beyond the six usually listed. `the_scan_finds_qualified_impls` fails the moment that
tolerance is simplified away.

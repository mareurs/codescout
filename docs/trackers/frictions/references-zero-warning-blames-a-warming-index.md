---
kind: tracker
status: active
title: "Evidence — references blames a warming index when the name only matches same-named locals"
tags:
  - frictions
  - references
---

# Evidence — `references` blames a warming index when the name only matches same-named locals

Ledger entry: FR-1 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`.

## What the agent sees

On a successful `references` call with no hit outside the definition file, the output carries this warning:

> LSP returned 0 references outside the definition file, but `<sym>` appears as a whole word in N+ other source file(s) (e.g. …) — the reference index may still be warming after a reindex. Re-run, or corroborate with grep / call_graph(direction='callers') before treating this symbol as unused.

The text is built in `src/tools/symbol/references.rs` at the `format!` that contains "may still be warming" (line 493 when read on 2026-11-11). The trigger is a whole-word text scan of up to N source files. The scan cannot tell a call site from a same-named local, a field, a doc comment or a test name.

## Scale

- 18 of 248 successful `references` calls carry this warning (7.3%).
- Sibling warnings on the same tool: "LSP returned no locations at all" (5 rows, accurate: the answer lacks even its own declaration) and a worktree banner (10 rows).
- 81 of the 248 success rows have an empty `output_json`, so the true count may be higher.

## Verification

The `lsp` miner grepped the current tree for 12 of the 18 flagged symbols. Row ids: 188858, 194033, 199628, 202760, 235033, 283050, 282854, 259140, 264544, 219842, 278717, 241188.
In all 12 cases the callers were in the definition file, or absent. The "other files" hits were same-named locals, doc comments, tests or fields. Example: a `let dirty_files_arc = ctx` local at `src/semantic/index.rs:339`.

## Why it matters

- The LSP answer was right. The explanation was wrong.
- The warning tells the agent to re-run or to grep. Both cost calls and teach the agent to distrust `references`.
- This bears on the visibility question: an agent that gets this warning once has a reason to avoid `references` afterwards. That link is a hypothesis; no data shows it.
- ADR `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, clause 3: claim only what is proven, offer the rest as a thing to check. "May still be warming" names a cause that nothing checked.

## Live reproduction (2026-11-11, this session)

`references(symbol="diff_tree_to_tree", path="src/git/mod.rs")` returned warning A, naming `src/librarian/tools/doctor.rs` as the other file. `doctor.rs:8362` calls the `git2` method `repo.diff_tree_to_tree(...)`, a different symbol. The crate function `crate::git::diff_tree_to_tree` has callers only in its own tests (`src/git/mod.rs:116,128,151,162`). So the LSP answer ("no callers outside the definition file") was right and the stated cause ("index may still be warming") was false. This is the 13th checked case.

## Idea (hypothesis, not tested)

Say what was proven: "N other files contain the name as a whole word; the LSP reports none. Check for same-named locals or comments before you treat the symbol as unused." Keep "warming" only when the LSP also returned no declaration (warning B), because that case proves it.

## Not verified

- 6 of the 18 flagged symbols were not checked.
- The grep ran on the current tree, not the tree at call time.
- Whether agents distrust `references` after this warning.

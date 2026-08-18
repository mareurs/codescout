---
kind: bug
status: fixed
title: 'BUG: symbols(name=X) resolves a name path that edit_code(symbol=X) rejects as not-found, so a name you just looked up is not a name you can edit with'
tags:
- symbols
- edit-code
- name-resolution
- tool-friction
closed: 2026-08-15
opened: 2026-08-08
owner: marius
related: []
severity: low
---

# BUG: `symbols` and `edit_code` disagree on the same name path

## Summary

`symbols(name="tests/RecordingStore/project_index_stats", include_body=true)` resolves and
returns the method. `edit_code(symbol="tests/RecordingStore/project_index_stats", ...)`
against the same file returns `symbol not found` and suggests
`tests/impl CodeVectorStore for RecordingStore/project_index_stats` instead.

So the name a reader obtains from the discovery tool is not necessarily a name the edit
tool accepts. The workflow the Iron Laws prescribe — look it up with `symbols`, then edit
it with `edit_code` — has a gap between step 1 and step 2.

## Symptom (Effect)

```
symbols(name="RecordingStore/project_index_stats", include_body=true)
  → src/retrieval/sync.rs (1)
      Method  473-475  tests/RecordingStore/project_index_stats
```

Same session, same file:

```
edit_code(symbol="tests/RecordingStore/project_index_stats", path="src/retrieval/sync.rs", ...)
  → symbol not found: tests/RecordingStore/project_index_stats — did you mean
    'tests/impl CodeVectorStore for RecordingStore/project_index_stats', ...
```

The suggestion is correct and the retry with it succeeds. The cost is a wasted call, and
`symbols`' own output is what led to the wrong name.

## Reproduction

```
1. Pick a trait-impl method inside a #[cfg(test)] module — e.g.
   src/retrieval/sync.rs, `impl CodeVectorStore for RecordingStore`.
2. symbols(name="<Struct>/<method>", include_body=true)  → resolves, prints a
   container-qualified path in its result line.
3. edit_code(symbol=<that same string>, path=..., action="insert", ...)  → not found.
```

Commit: `6b97db0b`.

## Environment

codescout `experiments`, Linux, MCP stdio.

## Root cause

**Measured 2026-08-15** (the text below replaces an explicitly-unverified
hypothesis, which this file was right to label as one).

The original guess — *"`symbols` matches a `Struct/method` form while `edit_code`
requires `impl Trait for Struct/method`; whether one is too lax or the other too
strict is the open question"* — framed it as one tool being wrong. Neither is. There
are **three** name-path producers and they disagree by construction:

| Producer | Impl method reads as |
|---|---|
| `crate::ast::extract_symbols` (`src/ast/parser.rs:247`) | `Type/method` |
| LSP `workspace/symbol` (what `symbols` searches) | container-dependent |
| LSP `documentSymbol` (what `edit_code` resolves against) | `impl Trait for Type/method` |

The AST extractor takes only the **implementing type** for an `impl_item` and
merges the methods up a level, with this comment:

```rust
// Don't create a symbol for impl blocks; merge methods at current level
// This matches how LSP reports symbols (methods under the type)
```

**That comment is false, and is the root cause.** `documentSymbol` nests methods
under an `impl Trait for Type` node, so the two forms differ — and because the
comment asserted they agreed, nobody reconciled them.

**Why the existing matcher did not save it.** `symbol_name_matches` already
tolerates `RecordingStore/project_index_stats` →
`impl CodeVectorStore for RecordingStore/project_index_stats` by suffix-at-word-
boundary. What bit was the *printed* form `tests/RecordingStore/project_index_stats`:
the module prefix sits **in front of** the elided qualifier, so the query is not a
suffix of the candidate and every whole-string rule misses. The un-prefixed form the
caller originally queried with would have worked; copying what the tool printed is
what broke it.
## Evidence

Noted twice on 2026-08-08 while adding a trait method to five implementors. First
occurrence cost one failed call on `tests/InMemoryCodeStore/project_index_stats` (the
hint named `tests/impl CodeVectorStore for InMemoryCodeStore/project_index_stats`); the
second repeated it for `RecordingStore` despite knowing the pattern, because the
*printed* form from `symbols` is the one in front of you.

Also observed: an earlier `edit_code` call with the fully-qualified form returned
`0 matches` and a later identical call succeeded, suggesting the LSP index was still
warming. That is a separate effect and should not be conflated with the naming
asymmetry.

## Hypotheses tried

None — filed on notice, not investigated.

## Fix

Shipped in `b2bc8edb`. Fixed at the **matcher**, not the producers.

`segments_match_eliding_qualifiers` (`src/symbol/query.rs`) compares the two paths
segment by segment, which puts the elision back inside a single segment where the
existing boundary rule already applies. Changing the AST to emit
`impl Trait for Type` instead would have been the "make them agree" fix, but
`Type/method` is the better name to *show a reader* — the divergence is worth
keeping, it just has to be reconciled somewhere.

The false comment in `src/ast/parser.rs` is corrected in place and now points at
the reconciliation, so the next person to change the naming finds the constraint
before they trip over it.

Deliberately strict in three ways, each pinned by a negative in the test:

1. **Same arity** — `A/b` must not match `X/A/b`; that is the suffix rule's job,
   and floating arity would let a bare leaf match anything ending in it.
2. **Boundary-anchored** — `SemanticSearch` matches `impl Tool for SemanticSearch`
   but not `FooSemanticSearch`.
3. **No generic tolerance** — `Catalog` still does not match `impl Catalog<T>`,
   preserving behaviour `symbol_name_matches_suffix_at_word_boundary` already
   asserted.
## Tests added

`symbol_name_matches_qualifier_elided_in_a_middle_segment` — the exact repro plus
the three strictness negatives above, and a `:`-boundary case
(`outer/Book/search_text` → `outer/impl Searchable for crate::models::Book/search_text`).

**Mutation-verified rather than assumed.** Stubbing the new branch to `if false &&
…` fails on exactly the repro assertion and leaves the other three
`symbol_name_matches_*` tests passing — so the branch is load-bearing *and* does
not mask the rules it sits behind. Restored after.

Gate: `cargo test --workspace` → 3810 passed / 0 failed / 50 ignored; clippy
`--workspace --all-targets -D warnings` clean.
## Workarounds

Read the `did you mean` list — it is correct. For trait-impl methods, reach for
`impl <Trait> for <Struct>/<method>` first.

## Resume

Closed, together with the duplicate-`name_path` bug it was linked to — the two are
the same seam seen from opposite ends.

This file did one thing exactly right and it is worth copying: it **labelled its
own root cause as a hypothesis** (*"inferred from two tool responses in one
session — the resolution code was not read"*). That label is why the section could
be replaced without argument. An unlabelled guess in the same slot would have been
read as measurement by every later session, and the false comment in
`src/ast/parser.rs` is the standing demonstration of what that costs.
## References

- `src/retrieval/sync.rs` — the file the two calls disagreed about
- Observed while implementing `project_has_chunks` across five `CodeVectorStore` impls

## Fix provenance

- **SHA:** `6b97db0b` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `d999b16d4f408723f8f6cff91d7df70c0241e1cb` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep d999b16d4f40 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.

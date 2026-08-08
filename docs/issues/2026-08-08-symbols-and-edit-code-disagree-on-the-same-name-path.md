---
kind: bug
status: open
title: 'BUG: symbols(name=X) resolves a name path that edit_code(symbol=X) rejects as not-found, so a name you just looked up is not a name you can edit with'
tags:
- symbols
- edit-code
- name-resolution
- tool-friction
closed: null
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

Unknown — not investigated. The two tools evidently apply different name-path
normalisation for trait-impl methods: `symbols` matches a `Struct/method` form (and
*prints* `tests/Struct/method`), while `edit_code` requires the fully-qualified
`impl Trait for Struct/method`. Whether `symbols` is too lax or `edit_code` too strict is
the open question; the asymmetry is what bites either way.

*Inferred from two tool responses in one session — the resolution code was not read. This
is a hypothesis about the mechanism, not a measurement of it.*

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

Not yet implemented. The cheap direction is to make `edit_code` accept whatever `symbols`
resolves and prints: if `symbols` can disambiguate `Struct/method` to exactly one symbol,
`edit_code` given the same string can too. If ambiguity is the concern, the existing
`ambiguous name_path` error already covers it — that error fired correctly in this same
session for a bare `FakeEmbedder`, so the machinery exists.

## Tests added

None. A regression test would assert that any name path `symbols` returns is accepted by
`edit_code` for the same file — which is the invariant worth pinning regardless of which
side is changed.

## Workarounds

Read the `did you mean` list — it is correct. For trait-impl methods, reach for
`impl <Trait> for <Struct>/<method>` first.

## Resume

Find where `edit_code` resolves `symbol` to a name path and compare it with `symbols`'
matcher. Start from the error string `symbol not found:` and its `did you mean` hint —
whatever builds that suggestion list already knows the mapping, so the fix may be as
small as consulting it before failing.

## References

- `src/retrieval/sync.rs` — the file the two calls disagreed about
- Observed while implementing `project_has_chunks` across five `CodeVectorStore` impls

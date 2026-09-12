---
id: '8ace06843584dd1e'
kind: bug
status: open
title: 'BUG: the tree-sitter fallback is gated on matches.is_empty(), so a warm LSP returns strictly fewer symbols than a cold one'
tags:
- cluster/selector-narrower-than-its-population
---

# BUG: the tree-sitter fallback is gated on `matches.is_empty()`, so a warm LSP returns strictly fewer symbols than a cold one

## Summary

`search_project_symbols` (`src/tools/symbol/symbols.rs`) serves project-scope `symbols`
searches from LSP `workspace/symbol` first, and falls back to a tree-sitter walk **only
when the LSP produced nothing at all** (`if matches.is_empty()`). The fallback's
granularity is the whole call, not the file. So any source file the LSP does not index is
reachable **only while the LSP is failing** — one in-workspace hit is enough to suppress
the walk that would have found it.

The observable consequence is inverted from what a reader expects: a **cold or broken**
rust-analyzer returns MORE results than a warm, working one, silently, with no marker on
either answer.

## Symptom (Effect)

Measured 2026-09-12 on this checkout. `pub trait Tool` exists at
`tests/fixtures/edit-eval-rust/src/bug054_repro.rs:1`.

```
symbols(symbol="Tool")   -> 2 matches     (warm rust-analyzer)
symbols(symbol="Tool")   -> 3 matches     (same call, earlier, cold rust-analyzer)
```

Neither answer is marked. The 2-result answer is the one a caller gets under normal
operation, and it is the incomplete one.

## Reproduction

1. `symbols(symbol="Tool")` against this repo with rust-analyzer warm — 2 matches, and
   `tests/fixtures/edit-eval-rust/src/bug054_repro.rs` is absent.
2. Confirm the symbol is really there: `grep -rn "Tool" tests/fixtures/edit-eval-rust/src/`
   -> `bug054_repro.rs:1:pub trait Tool`.
3. Confirm the file is reachable by the walk — it is not gitignored, and
   `symbols(path="tests/fixtures/edit-eval-rust/src/bug054_repro.rs")` lists its `Tool`.

The cold-LSP arm was observed by a peer session before the warm arm, which is how the
discrepancy surfaced at all — the two readings were minutes apart on one unchanged tree.

## Environment

Linux, branch `experiments`, rust-analyzer 1.97.1. Not feature-gated.

## Root cause

Two independent narrowings compose, and only the second is documented at its site.

**1. The LSP's population is the cargo workspace, not the walk.**
`tests/fixtures/edit-eval-rust/` is its own cargo project (own `Cargo.toml` + `Cargo.lock`),
and the outer manifest declares `members = [".", "crates/codescout-embed"]`. It is not a
member, so `workspace/symbol` never indexes it. Meanwhile the accepted-files walk in the
same function **does** accept it (`ignore::WalkBuilder` + `ast::detect_language`), so the
file is inside the population the tool presents itself as searching.

**2. The fallback is all-or-nothing.** After the LSP arm, the tree-sitter walk runs under
`if matches.is_empty()`. That predicate asks "did we find anything?", never "did we search
everywhere?". One hit from an indexed crate therefore suppresses the only mechanism that
covers the unindexed ones.

The `in_walk` filter directly above is the tell that the two populations were already
known to differ: it exists to drop LSP hits the walk did *not* accept (gitignored build
output). Nothing does the converse — admit walk-visible files the LSP did not return.

## Evidence

- `src/tools/symbol/symbols.rs`, `search_project_symbols` — `if matches.is_empty()` gating
  the tree-sitter walk; `in_walk` handling only the opposite direction.
- `Cargo.toml` — `members = [".", "crates/codescout-embed"]`.
- `tests/fixtures/edit-eval-rust/Cargo.toml` — separate project.
- `tests/fixtures/edit-eval-rust/src/bug054_repro.rs:1` — the unreachable `pub trait Tool`.

### Four calls on one unchanged tree, all consistent with the gate and with nothing else

Contributed by the peer session that first hit the symptom (sessionId
`b0b9bc40-5358-4a44-b342-a2a71dc50fad`), against server pid 218699 confirmed to be the
current on-disk binary by walking its own process tree. Reproduced here because it is a
better evidence set than the one this file was opened with — rows 3 and 4 are positive
controls that the summary's two rows cannot supply:

| call | result |
|---|---|
| `symbols(symbol="Tool")` | 2 — fixture absent |
| `symbols(path=".../bug054_repro.rs")` | file overview shows `Interface 1-3 Tool` |
| `symbols(symbol="Tool", path="tests/fixtures/edit-eval-rust")` | **FINDS it** |
| `symbols(symbol="ReadMarkdown")` | **finds the fixture, project-wide, exact mode** |

Rows 3 and 4 are the load-bearing ones, and they are what make this a mechanism rather
than a story about a flaky index. Both produce an **empty LSP result** — row 3 because the
scope contains no workspace member, row 4 because no in-workspace symbol is named
`ReadMarkdown` — so `matches.is_empty()` fires and the fallback walks. The fixture is
found in exactly the cases where the LSP contributes nothing, and lost in exactly the
cases where it contributes something. Row 4 also independently kills the "exact mode is a
different backend" reading: exact mode reaches the fixture fine, when the LSP is empty.

**The failure is invisible precisely when the system is healthy.** The tool's coverage is
*inversely* related to how well the LSP is working — a warm, correct index silently
returns a subset of what a broken one returns. That is the property that makes this worth
fixing rather than documenting: every mechanism a reader would use to gain confidence in
the answer (LSP up, index warm, no errors logged) is a mechanism that makes the answer
smaller.

**A duplicate was filed on the symptom and withdrawn.**
`08d1ef6387a9e569` ("project-wide exact symbol lookup drops a colliding member", tagged
`cluster/capped-result-presented-as-complete`) was opened ~20 minutes before this file,
uncommitted, and deleted by its author in favour of this one — same observation, wrong
root-cause class. Recorded so a reader who saw it in a `find` before it went does not
spend time reconciling two records.
## Hypotheses tried

1. **Hypothesis:** exact and substring lookups are served by different backends, which
   would explain one finding the file and the other not. **Test:** read
   `search_project_symbols` — both modes go through one `name_ok` predicate on one LSP
   path. **Verdict:** rejected. The mode is not the variable; LSP warmth is.
2. **Hypothesis:** the fixture is gitignored or language-undetectable, so the walk never
   accepts it. **Test:** `symbols(path=...)` on the file returns its symbols, and it is
   not in `.gitignore`. **Verdict:** rejected — the walk accepts it; only the LSP does not.

## Fix

Not fixed, and this is a design decision rather than a cleanup, which is why it is filed
rather than patched in passing:

- **Per-file union** — run the tree-sitter walk over `accepted_files` the LSP did not
  return symbols *for*, instead of over everything-or-nothing. Correct, and the most
  expensive: it walks on every call rather than only on LSP failure.
- **Walk only the gap** — compute the set of accepted files belonging to no LSP-indexed
  project and tree-sitter those. Cheaper, needs a way to ask which roots the LSP covers,
  which `workspace/symbol` does not expose.
- **Declare the scope instead of widening it** — keep today's behaviour and have the
  response say which files were LSP-served vs walked, per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. Cheapest and honest, but
  leaves the result set LSP-state-dependent.

Note the third option alone does not remove the inversion (cold > warm); it only stops it
being silent.

## Tests added

None — this is the initial filing.

## Workarounds

Scope the call to the file or directory (`symbols(path=...)`), which takes the
path-restricted branch and does not consult `workspace/symbol` at all. Do not treat a
project-wide `symbols` count over a multi-cargo-project tree as complete.

## Resume

Decide between the three Fix shapes. Before implementing, confirm the inversion directly:
restart the LSP (or point at a cold workspace), run `symbols(symbol="Tool")`, then run it
again once warm, and require the two counts to differ — that observation is the whole bug,
and it is load/timing dependent, so it must be produced rather than assumed.

## References

- `src/tools/symbol/symbols.rs` — `search_project_symbols`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- Surfaced 2026-09-12 by a peer session's measurement during unrelated `symbols` work; the
  mechanism was read out of the source and confirmed against the manifests here.

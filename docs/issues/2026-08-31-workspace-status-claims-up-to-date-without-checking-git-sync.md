---
status: open
opened: 2026-08-31
closed:
severity: medium
owner: marius
related: []
tags: [workspace, index, staleness, misleading-report]
kind: bug
---

# BUG: workspace(status) reports index.status "up_to_date" on chunks > 0 alone, never consulting git_sync

## Summary

`workspace(action="status")` reports `index.status: "up_to_date"` whenever the Qdrant
collection holds any chunks at all. It never consults `git_sync` / `behind_commits`, so an
index 286 commits behind HEAD and missing 70 files reports as up to date. The honest
predicate already exists, with the same vocabulary, and is simply not read.

## Symptom (Effect)

Two surfaces, called seconds apart on the same server process, disagree:

```
workspace(action="status")
  "index": { "status": "up_to_date", "files": 1685, "chunks": 50942,
             "hint": "Call index(action='status') for full Qdrant collection details." }

index(action="verify")          # immediately after
  "verdict": "stale",
  "expected_files": 1753, "stored_files": 1685, "missing_count": 70,
  "git_sync": { "status": "behind", "behind_commits": 286,
                "last_indexed_commit": "14aab5ff", "head_commit": "2f434fba" }
```

Note also that `workspace(action="activate")` on the same process reported the weaker and
honest `"index": { "status": "indexed" }`. So three surfaces use three words for one state,
and only the `status` one asserts currency it has not checked.

## Reproduction

At `2f434fba` (`git rev-parse HEAD`), on a machine whose semantic index was built at an
older commit:

1. `git merge --ff-only origin/experiments`  (any pull that adds indexable files)
2. `cargo rb && /mcp`
3. `workspace(action="activate", path=<repo>)` → `index.status: "indexed"`
4. `workspace(action="status")` → `index.status: "up_to_date"`  ← **false**
5. `index(action="verify")` → `verdict: "stale"`, `behind_commits: 286`

Observed live 2026-08-31 during a laptop→desktop catalog resume.

## Environment

Arch Linux (zen 7.1.9), codescout 0.15.0 at `2f434fba`, release build via `cargo rb`
(`server-stack,local-embed`), Qdrant backend (`embedding_backend: remote-http`,
`CodeRankEmbed`), MCP stdio transport, project `codescout`, branch `experiments`.

## Root cause

`src/tools/config/mod.rs:555-562` — the value is hardcoded on a chunk-count guard:

```rust
match qdrant_stats {
    Some((chunks, files)) if chunks > 0 => {
        result["index"] = json!({
            "status": "up_to_date",
            "files": files,
            "chunks": chunks,
            ...
```

`chunks > 0` is the *only* input. Nothing in `ProjectStatus/call` reads
`behind_commits`, `last_indexed_commit` or `head_commit`, so "up_to_date" is a statement
about non-emptiness wearing the name of a statement about currency. The `_ =>` arm emits
`not_indexed`, making the field binary where three states exist.

The correct predicate is already implemented and already uses this exact vocabulary —
`src/retrieval/index_state.rs:282-293` documents and returns
`{ status: "up_to_date" | "behind", behind_commits, last_indexed_commit, head_commit }`.
The `"behind"` arm is unreachable from `workspace(status)`.

*measured 2026-08-31: the two tool calls quoted under Symptom, run in sequence against
pid 1941149; code read at `src/tools/config/mod.rs:538-575` and
`src/retrieval/index_state.rs:282-293`.*

## Evidence

### The guard, read at source

`src/tools/config/mod.rs:555-562` as quoted under Root cause. `grep '"up_to_date"'` over
`src/**/*.rs` returns 7 matches in 3 files; only `index_state.rs` pairs the token with a
`"behind"` alternative.

### Why this is load-bearing, not cosmetic

`docs/conventions/cross-machine-catalog-resume.md` § *Why a clone is never enough* states
the semantic index does not travel with `git pull` and that every missing layer is silent.
Step 4 of that sequence is `index(action="build")`. A session that orients with
`workspace(action="status")` — which the activation-bootstrap guide recommends, and whose
own `hint` invites you to call `index(action='status')` rather than `verify` — is told the
index is current and has no reason to run step 4. In this session the gap was 286 commits
and 70 files, all of them the just-pulled work.

## Hypotheses tried

1. **Hypothesis:** `index.status` in `workspace(status)` means "a usable index exists", not
   "current with HEAD", making `up_to_date` a naming issue rather than a wrong value.
   **Test:** compare the vocabulary across surfaces and against the module that computes
   staleness. **Verdict:** rejected as an excuse, though it explains the intent —
   `workspace(activate)` already emits `"indexed"` for exactly that meaning, and
   `index_state.rs` reserves `"up_to_date"` for the git-relative sense with `"behind"` as
   its complement. The token is spent elsewhere on the stronger claim.
   **Evidence:** *The guard, read at source*.

## Fix

Not yet implemented.

Plan: in `ProjectStatus/call` (`src/tools/config/mod.rs`, the `chunks > 0` arm), read the
git-sync state from `src/retrieval/index_state.rs` and emit its `status` verbatim
(`"up_to_date" | "behind"`), carrying `behind_commits` / `last_indexed_commit` /
`head_commit` alongside `files` / `chunks`. When behind, the `hint` should name
`index(action='build')` rather than `index(action='status')`. Prefer reusing the existing
function over recomputing the comparison, so the two surfaces cannot drift apart again —
this bug is what a second implementation of one predicate looks like.

Consider whether `not_indexed` should stay binary against a populated-but-behind index, or
whether the third state deserves its own word here as it does in `index_state.rs`.

## Tests added

None yet — the fix is not written. The regression test should assert the `"behind"` arm is
reachable from `workspace(status)`: build an index, move HEAD, then assert
`status == "behind"` and `behind_commits > 0`. A test that only checks the
populated-and-current case is monotone under exactly this defect and would have passed
throughout (see CLAUDE.md § *Testing Discipline* — the assertion must be able to fail in
the direction of the bug).

## Workarounds

Use `index(action="verify")` rather than `workspace(action="status")` whenever currency
matters — it is the only surface that compares against HEAD and walks for coverage.
`index(action="status")` also carries a correct `git_sync` block, and additionally warns
that `file_count` / `chunk_count` are what the store holds rather than proof it holds
everything eligible (`coverage: "unchecked"`).

## Resume

Patch the `chunks > 0` arm in `ProjectStatus/call` (`src/tools/config/mod.rs:555-562`) to
call into `src/retrieval/index_state.rs`'s git-sync computation instead of hardcoding
`"up_to_date"`; check `references("git_sync_status", "src/retrieval/index_state.rs")` first
to reuse the existing entry point rather than adding a second one. Then add the
`"behind"`-arm test described under *Tests added* and confirm it fails before the fix.

## References

- `src/tools/config/mod.rs:538-575` — `ProjectStatus/call`, the offending arm
- `src/tools/config/mod.rs:1198` — `format_project_status`, which already switches on
  `"up_to_date" | "behind"` and so is ready for the corrected value
- `src/retrieval/index_state.rs:282-293` — the correct predicate and its documented shape
- `docs/conventions/cross-machine-catalog-resume.md` — the sequence whose step 4 this
  report can cause a session to skip
- `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` — the class this
  belongs to: a plausible value standing in for a check that never ran
- `docs/issues/archive/` — `74e1309bf8a4d0ba` is the inverse defect on the same field
  (`workspace(activate)` reporting `not_indexed` for a fully indexed project, fixed)

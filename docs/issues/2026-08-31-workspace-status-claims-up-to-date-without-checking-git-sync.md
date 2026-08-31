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

### Second reproduction — the other machine, at near-minimum gap

Observed 2026-08-31 12:52 on the laptop (the other half of the same resume), pid 3247619,
release build from `aa7d5039`, after `git merge --ff-only` took HEAD to `43c2f81a`. Same
disagreement, at a gap ~70× smaller:

```
workspace(action="status")  "index": { "status": "up_to_date", "files": 1754, "chunks": 53894 }
index(action="status")      git_sync: { "status": "behind", "behind_commits": 4,
                                        "last_indexed_commit": "aa7d5039",
                                        "head_commit": "43c2f81a" }
index(action="verify")      verdict "stale", expected_files 1757, stored_files 1754,
                            missing_count 3
```

**What this adds: the field is not inaccurate *at* a large gap, it is independent *of* the
gap.** The first reproduction — 286 commits, 70 files — is equally consistent with a
tolerance that is merely too loose, and that reading points at a fix (tighten the
threshold) which would leave this case untouched. Four commits and three files report the
identical `"up_to_date"`, because `chunks > 0` is satisfied by 53894 either way. There is
no threshold to tighten; the value is not a function of the gap at any magnitude, which is
what the root cause predicts and what a second datapoint at the opposite end of the range
is needed to show.

Note also that `index(action="verify")`'s own `hint` reads *"Index is 4 commit(s) behind
HEAD; the 3 missing file(s) are explained by that. Run index(action='build') to catch up"*
— the honest sentence, naming the remedy, emitted by the same process that had just
answered `up_to_date`.

The three missing files were exactly the three this pull added:

```
docs/issues/2026-08-31-workspace-status-claims-up-to-date-without-checking-git-sync.md
docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md
docs/trackers/archive/provenance-subsystem-recovered-entries.md
```

So this report was itself absent from the semantic index whose currency it disputes, while
`workspace(status)` reported that index current.

> **Retracted, same day, `83b5651d` → this commit.** That paragraph first continued: *"a
> session that oriented with `status` and then ran `semantic_search` for prior art on the
> defect would have found none, and been given no reason to doubt the zero."* The zero is
> real and the mechanism is wrong, which is the worse of the two ways to be wrong here.
> `semantic_search` would have returned nothing **either way**: five probes, twenty-five
> results, zero markdown among them — including phrases lifted verbatim from files that
> exist only as markdown. It is documented as concept-level *code* exploration
> (`src/prompts/source.md:136`); documents are the librarian's lane, and
> `artifact(action="find")` located this report without difficulty throughout.
>
> Keep the distinction the retraction exposes, because this report's own title depends on
> it: **"the index" names two stores.** The semantic index (Qdrant, `index(action=…)`) is
> what `workspace(status)` misreports. The librarian **catalog** is a different store with
> a different repair (`librarian(action="reindex")`), and in this window it was *also*
> stale — `artifact(action="find")` reported `unindexed_files: 3` and could match none of
> them until reindexed. Two layers arrived stale from one `git pull`, only one of them is
> this bug, and the surface that would have told you about either is the one under report.
> The prior-art hazard is therefore real but belongs to the catalog layer, and is already
> documented at `docs/conventions/cross-machine-catalog-resume.md`.

*Measured against a closing window: the background reindex was already running, so
`behind_commits: 4` is a fact about 12:52 and not about whenever this is read. The
`up_to_date` claim is not — it is reproducible at any non-zero chunk count. Both stores
were brought level afterwards; `index(action="verify")` now returns `verdict: "complete"`,
1757/1757, level with HEAD.*
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

**Make the fixture's gap exactly one commit, and say on the fixture line that the `1` is
load-bearing.** The second reproduction above shows the defect is gap-independent, so a
fixture built on a comfortable margin — 50 commits, 20 files — passes under the correct fix
*and* under a wrong one that merely thresholds on staleness, and nothing distinguishes
them. A one-commit, one-file gap is the only fixture a threshold-shaped fix fails. This is
the monotone-assertion rule applied to setup rather than to the assertion: the gap size is
the part of the arrangement that makes the test able to tell, and a later tidy-up that
rounds it up to "a clearly stale index" leaves the test green and no longer
discriminating (CLAUDE.md § *Testing Discipline*, the fixture-annotation clause).
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

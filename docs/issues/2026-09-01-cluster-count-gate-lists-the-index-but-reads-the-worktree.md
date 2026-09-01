---
id: '68481127b199baa0'
kind: bug
status: open
title: The cluster-count gate scopes its file list to the index but reads content from the working tree
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
topic: shared-checkout gate correctness
opened: 2026-09-01
owner: marius
related: []
severity: medium
---

# BUG: the cluster-count gate scopes its file LIST to the index but reads CONTENT from the working tree

## Summary

`tests/issue_clusters.rs`'s count gate derives its population from `git ls-files` (the index) but
reads each file's bytes with `std::fs::read_to_string` (the working tree). On a checkout shared by
six sessions, any peer's half-written bug file changes the count for the duration of the write —
so the gate reds another session's build for a reason that has already ceased to be true by the
time anyone looks.

The module header argues explicitly for **tracked files only**, on the grounds that *"gating on the
working tree would let one session's unfinished bug file red another session's build, which is the
very class `IC-1` describes."* That bound is enforced on the file **list** and not on the file
**content**, so the exposure it names is reintroduced one layer down.

## Symptom (Effect)

Observed 2026-09-01 mid-way through an unrelated archive move:

```
---- every_index_count_matches_the_corpus stdout ----
the Index table's `n` column disagrees with the corpus:
  cluster/gate-keyed-on-unobservable-event — table says 22, corpus has 21

---- every_bare_n_in_a_class_field_matches_the_corpus stdout ----
  cluster/gate-keyed-on-unobservable-event — **Members:** states a bare n=22, corpus has 21
```

A re-run minutes later, with no change to the ledger or to any file carrying that slug:

```
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The failing slug was unrelated to the change under test, which carried
`cluster/addressing-without-an-escape-hatch` and read `declared 30 / actual 30` throughout.

## Reproduction

Two sessions on one checkout.

1. Session A begins an `artifact(action="move")` on a bug file, or any write that briefly leaves a
   tagged file absent or partially written.
2. Session B runs `cargo test --workspace --test issue_clusters` inside that window.
3. B's `actual_counts` reads A's mid-write bytes and reports a count one short.
4. B's gate reds, naming a cluster B never touched.

The window is small and the failure is not reproducible on demand, which is what makes it
expensive: the honest reading of a single red run is unavailable to the session that gets it.

## Environment

codescout `experiments`, Linux, six live Claude Code sessions in one checkout
(`/home/marius/work/claude/codescout`). Peers were committing continuously —
`a34deabb docs(issues): archive the subagent-guide bug` landed inside the same minute.

## Root cause

The two halves of the corpus derivation read different worlds.

- `tracked_all_bug_files()` (`tests/issue_clusters.rs`) shells `git ls-files docs/issues` — the
  **index**.
- `actual_counts()` then does `std::fs::read_to_string(repo_root().join(&rel))` per entry — the
  **working tree**.

*Measured 2026-09-01, not inferred from the code alone:* a hand re-derivation over the identical
population, parsing only frontmatter, returned **22** — matching both HEAD's ledger and the
worktree's — while the gate in the same window returned 21. The disagreement is the window, not
the parse.

The comparison target is read from disk too (`declared_counts` / `bare_n_claims` both
`read_to_string` the ledger), so a peer editing `issue-clusters.md` moves the *other* side of the
same comparison independently. Two worktree-scoped reads either side of one equality is the shape.

## Evidence

`tests/issue_clusters.rs` module header, stating the bound the implementation then only half
enforces:

```
//! - **Tracked files only.** An untracked file is a peer session's in-flight work on a
//!   shared checkout. Gating on the working tree would let one session's unfinished bug
//!   file red another session's build, which is the very class `IC-1` describes.
```

`actual_counts`, reading the other world:

```rust
for rel in tracked_all_bug_files() {          // index
    let Ok(content) = std::fs::read_to_string(repo_root().join(&rel)) else {  // worktree
```

## Hypotheses tried

1. **Hypothesis:** the archive move under test dropped the count.
   **Test:** the moved file carries `cluster/addressing-without-an-escape-hatch`; checked that
   class's numbers with the move staged.
   **Verdict:** rejected — `declared 30 / actual 30`, and the count population deliberately spans
   `archive/`, so archiving moves no number.

2. **Hypothesis:** a peer had edited the ledger's `22`.
   **Test:** compared `git show HEAD:docs/trackers/issue-clusters.md` against the worktree copy for
   that slug.
   **Verdict:** rejected — both declare 22, byte-identical rows.

3. **Hypothesis:** the gate's own parse is stricter than the documented re-derivation.
   **Test:** hand-computed the count over `git ls-files docs/issues`, frontmatter-only, matching
   `cluster_tags`' shape.
   **Verdict:** rejected — 22, agreeing with the ledger and disagreeing with the gate.

4. **Hypothesis:** transient peer state.
   **Test:** re-ran the unchanged gate.
   **Verdict:** confirmed — 18/18 green, nothing altered in between.

## Fix

Not implemented. The obvious repair is to read content from the same world as the list —
`git show :<path>` per entry, or one `git grep --cached` pass — which is what
`scripts/pre-commit-ledger-counts.py` already does (*"This reads the INDEX and nothing else"*).
That sibling instrument got it right, which makes this a divergence between two implementations of
one rule rather than an unsolved problem.

Two things to decide rather than assume:

- **The ledger side too.** `declared_counts` reads `issue-clusters.md` from disk. Fixing only the
  corpus side leaves one worktree read in the comparison, which is the same defect at half
  amplitude.
- **Whether an index read is right for a local `cargo test`.** A developer editing a bug file and
  running the gate expects to see their own edit. Index-scoping makes the gate authoritative about
  *what would be committed*, which is the question a pre-commit gate asks and not necessarily the
  one an interactive run asks.

## Tests added

None — this is a report. A regression test is awkward by construction: it would have to write a
file, read it mid-write, and assert a specific transient. The tractable shape is a unit test over
a seeded temp repo asserting that `actual_counts` returns the **index** value when index and
worktree deliberately disagree, which needs no race at all.

## Workarounds

Re-run the gate. A single red run on this checkout is not evidence about the ledger until it
survives a second run — and per this repo's own `R-3`, a result that changes on re-run with no
input change is evidence about the instrument, not about the corpus.

## Resume

Decide the two questions under *Fix*, then mirror `scripts/pre-commit-ledger-counts.py`'s
index-only read into `tests/issue_clusters.rs` — both `actual_counts` and `declared_counts`.
Positive control before trusting the change: seed a temp repo where index and worktree disagree by
one tagged file and assert the gate reports the index count.

## References

- `tests/issue_clusters.rs` — module header, `tracked_all_bug_files`, `actual_counts`,
  `declared_counts`.
- `scripts/pre-commit-ledger-counts.py` — the sibling that reads the index and says so.
- `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md` — the other
  half of this checkout's gate/shared-state friction.


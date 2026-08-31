---
id: '6ff4394bb3b18d86'
kind: bug
status: fixed
title: 'BUG: reindex_cli is test-only and carries a broken copy of a DELETE that was deliberately removed for causing data loss'
tags:
- librarian
- catalog
- dead-code
- latent-data-loss
- cli-parity
- cluster/declared-not-wired
closed: 2026-08-30
opened: 2026-08-30
owner: marius
severity: low
---

# BUG: `reindex_cli` is test-only and carries a broken copy of a DELETE removed for causing data loss

## Summary

`src/librarian/mod.rs:392-398` (`reindex_cli`) contains:

```rust
if force {
    for root in &roots {
        cat.conn.execute(
            "DELETE FROM artifact WHERE abs_path LIKE ?1",
            rusqlite::params![format!("{}/", RepoPath::from(&root.path))],
        )?;
    }
}
```

Three things are wrong with it, and they cancel out into "harmless today, dangerous
tomorrow":

1. The `LIKE` pattern has **no `%`**, so it matches only an `abs_path` exactly equal to
   `<root>/`. No artifact row has that. The DELETE removes **zero rows**.
2. `force` is used for *nothing else* — `index_repo` is called without it — so
   `--force` is behaviourally identical to no flag.
3. Restoring the `%` would re-create precisely the destructive pre-walk DELETE that was
   **deliberately removed** from the real path in `d482ca8a`.

## Symptom (Effect)

None observable. That is the finding: the code reads as a working force-reindex and does
nothing, and the shape that would make it "work" is a documented data-loss bug.

## Reproduction

```
$ sqlite3 :memory: "CREATE TABLE t(p TEXT);
   INSERT INTO t VALUES('/r/docs/a.md'),('/r/docs/b.md'),('/r/');
   SELECT COUNT(*) FROM t WHERE p LIKE '/r/';    -- 1  (the literal row only)
   SELECT COUNT(*) FROM t WHERE p LIKE '/r/%';   -- 3"
```

`LIKE` with no wildcard is equality. Artifact `abs_path`s are files, never `<root>/`.

## Environment

`experiments` @ `1857fca3`, linux, codescout 0.15.0.

## Root cause

**Divergence between two implementations of one operation, where only one received the
fix.** `src/librarian/tools/reindex.rs:154-164` records the history in its own comment:

> *previously, `force=true` issued `DELETE FROM artifact WHERE abs_path LIKE <root>/%`
> here, before the re-walk. That was destructive: `artifact_augmentation` is declared
> `ON DELETE CASCADE`, so the DELETE cascade-wiped augmentations. \[…\] Removed
> 2026-05-17 per bug-tracker #7.*

And `force_wipes_then_reindexes` records the full arc: destructive DELETE → removed in
`d482ca8a` (leaving `force` a **no-op pending proper plumbing**) → task #31 plumbed
`force_rewalk` through `index_repo_sync`.

The MCP path completed that arc. `reindex_cli` never left stage one — it still holds the
DELETE, and its copy was never correct in the first place.

## Evidence

**It is test-only, which is the whole of the severity argument.**

```
src/librarian/mod.rs:351   #[cfg(test)]
src/librarian/mod.rs:352   pub(crate) async fn reindex_cli(env, repo, force) -> Result<()>
```

Confirmed against the shipped binary rather than inferred: `codescout --help` lists **no**
`reindex` subcommand (`grep -c reindex` → 0), and there is no `librarian` subcommand
(`exit=2`). So despite the name, nothing user-facing reaches this function.

**`force` reaches nothing else.** `src/librarian/mod.rs:452` calls
`indexer::index_repo(&cat, &rules, &root.path, &ignore, embedding, store, &project_id)` —
no force argument exists on that signature. `index_repo` in turn hardcodes
`index_repo_sync(cat, rules, abs_root, ignore, want, false, false)`
(`src/librarian/indexer.rs:666`), so `force_rewalk` and `force_embed` are unconditionally
`false` on this path.

**The block is not exercised even by tests.** The only caller,
`reindex_cli_indexes_repo`, passes `force: false` twice.

## Hypotheses tried

1. **Hypothesis:** the missing `%` is a deliberate neutering.
   **Test:** searched for a comment or test asserting the no-op.
   **Verdict:** nothing found. `reindex.rs` documents removing its DELETE outright rather
   than neutering it, so a silent neuter here would be inconsistent with the sibling's
   recorded decision. Undetermined, and it does not change the remedy.

## Fix

**Taken: (a)** — deleted the block and the `force` parameter. `9f743091` on `experiments`,
patch-id `92db5adf65b7a748`.

One fact decided it that this record did not have when it was written. **Both call sites of
`index_repo` are test-only** — `indexer.rs:1624` sits inside `#[cfg(test)] mod tests`
(which opens at `indexer.rs:707`), and `mod.rs:452` is inside `reindex_cli`, itself
`#[cfg(test)]` — while `index_repo` is *public API* via `lib.rs:39 pub mod librarian` →
`librarian/mod.rs:24 pub mod indexer`.

So (b) was not the neutral "make it a correct reference implementation" it reads as. It
meant a **semver-breaking signature change to public API, serving only test callers**, to
build a reference implementation that nothing references — the real path bypasses
`index_repo` entirely and calls `index_repo_sync` directly (`tools/reindex.rs:311`) with
its own force values. (a) touches only `#[cfg(test)] pub(crate)` code and has zero API
impact.

**The comment is the actual remedy, not the deletion.** A comment now stands where the
block was, recording why there is no force step, that `d482ca8a` removed the same thing
from the MCP path, and that forced re-walk lives in `index_repo_sync`'s `force_rewalk` /
`force_embed`. This mirrors what `tools/reindex.rs` already does, and it is what makes the
decision survive the next reader — the danger was never what the code did.
## Tests added

`reindex_cli_never_wipes_augmentations_under_the_root` (`src/librarian/mod.rs`).

**This section's original claim — "if (a) is taken there is nothing to assert; the deletion
is the change" — was wrong, and the way it was wrong is worth keeping.** A deletion leaves
no artefact to assert on, true; but the *invariant the deletion establishes* is assertable,
and asserting it is the only thing that stops the code coming back. "Nothing to assert" is
what a removed guard always looks like from the inside.

The test indexes one spec, attaches an augmentation to it, reindexes, and asserts the
augmentation survives. Mutation matrix — which is the result, not the green tick:

| mutation | outcome |
|---|---|
| re-add the DELETE **with** `%` (the "typo fix") | test **FAILS** |
| re-add the DELETE **without** `%` (the shipped code) | test **passes** |

So it discriminates on the *hazard*, not on the presence of a DELETE — and it confirms at
runtime this record's central claim, that the shipped statement was genuinely inert.

Two things stayed green under the failing mutation, and both are the reason the assertion
is where it is: the sibling `reindex_cli_indexes_repo`, and this test's own
`COUNT(*) FROM artifact == 1`. The count cannot see the wipe, because `id =
sha256(abs_path)` means the re-walk re-inserts the same row. Only the augmentation
discriminates, because `artifact_augmentation` is `ON DELETE CASCADE` and `Catalog::open`
sets `PRAGMA foreign_keys = ON` — verified at `catalog/mod.rs:422`, since the test would be
vacuous if it did not.

The reproduction was run before the fix, per CLAUDE.md: `LIKE '/r/'` → 1, `LIKE '/r/%'` → 3.
## Workarounds

None needed — nothing reaches it.

## Resume

Done — nothing outstanding. Fixed at `9f743091` (patch-id `92db5adf65b7a748`), gate green
(fmt; clippy `--workspace --all-targets --features local-embed`; 4837/0 full; 3362/0 lean).

**One finding surfaced by the fix and deliberately not actioned here:** `index_repo` is
`pub` on a `pub mod` path and has no production caller at all — dead public API rather than
a defect, and out of scope for a bug file. If it is ever tidied, note that removing it is a
semver-breaking change for library consumers even though nothing in this repo calls it.
## References

- `src/librarian/mod.rs:392-398` — the block
- `src/librarian/mod.rs:351` — the `#[cfg(test)]` that bounds the severity
- `src/librarian/tools/reindex.rs:154-164` — the sibling's comment recording the removal
- `src/librarian/tools/reindex.rs` `force_wipes_then_reindexes` — the three-stage history
- `d482ca8a` — the commit that removed the destructive DELETE from the real path
- `src/librarian/indexer.rs:666` — `index_repo` hardcoding `false, false`
- Found while investigating
  `docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md`
  (`open-issue-work-queue:BL-56`) as a candidate cause; **acquitted** there, twice over.

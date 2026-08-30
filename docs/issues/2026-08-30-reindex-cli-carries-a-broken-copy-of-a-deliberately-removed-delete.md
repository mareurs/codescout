---
id: d5c8f2e8ca951ddc
kind: bug
status: open
title: 'BUG: reindex_cli is test-only and carries a broken copy of a DELETE that was deliberately removed for causing data loss'
tags:
- librarian
- catalog
- dead-code
- latent-data-loss
- cli-parity
closed: null
opened: 2026-08-30
owner: marius
severity: low
unverified: 'Severity is low ONLY because the function is #[cfg(test)] and no `reindex` subcommand exists — verified against `codescout --help`. If either fact changes, re-rate: the block is one character from being the data-loss path `d482ca8a` removed.'
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

Two candidates, and the choice is a real one rather than a formality.

- **a. Delete the block and the `force` parameter.** Smallest, and it matches what
  `reindex.rs` actually did — that path removed its DELETE rather than repairing it.
  Costs the ability to say `reindex_cli(env, repo, true)` in a test.
- **b. Plumb `force` properly**, mirroring the MCP path: add `force_rewalk` / `force_embed`
  to `index_repo` and pass them through. Makes `reindex_cli` a correct reference
  implementation of the same operation. Blast radius is small — `index_repo` has exactly
  **two** call sites (`indexer.rs:1624`, `mod.rs:452`).

**Not (c): restore the `%`.** That is the data-loss path by construction.

Deliberately not applied in this record: the function is test-only, so neither option is
urgent, and picking between them is a judgement about whether `reindex_cli` is meant to
survive at all.

## Tests added

None. The testable claim if (b) is taken is the one the MCP side already pins —
`force=true` on an existing-unchanged file yields `updated=1, added=0, unchanged=0`
(`force_wipes_then_reindexes`). If (a) is taken there is nothing to assert; the deletion
is the change.

## Workarounds

None needed — nothing reaches it.

## Resume

Decide (a) vs (b). (a) if `reindex_cli` is vestigial and should go; (b) if a CLI reindex
is wanted later, since the plumbing is the part that would otherwise be re-derived.

**Whichever is chosen, do not leave the block as-is.** Its danger is not what it does, it
is what a future reader does to it: the pattern reads as an obvious missing-`%` typo, and
"fixing" the typo restores a cascade-delete of every augmentation under the root, with the
`ON DELETE CASCADE` doing the damage silently.

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


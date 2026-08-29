---
status: open
opened: 2026-08-29
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md
tags:
  - librarian
  - catalog
  - tooling
  - silent-divergence
kind: bug
---

# BUG: `edit_markdown`'s frontmatter write never touches the catalog, so `find(kind="bug", status=…)` reports the pre-edit status indefinitely

## Summary

`edit_markdown(path, frontmatter={set: {status: "fixed"}})` — the exact call
`get_guide("tracker-conventions")` and this tool's own `long_docs` recommend for
flipping a bug's status — writes the file and leaves the catalog row untouched.
The documented triage query reads the **catalog**, so a bug closed this way keeps
appearing as `open` until someone runs an unrelated reindex.

## Symptom (Effect)

File on disk, after the edit:

```yaml
status: fixed
closed: 2026-08-29
```

Catalog, immediately after, for the same artifact:

```json
{ "id": "fcf24087722a7b1e", "kind": "bug", "status": "open",
  "abs_path": "docs/issues/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md" }
```

Nothing errors. `edit_markdown` returns `{"status": "ok"}`.

## Reproduction

Any catalogued bug file that is neither augmented nor a ledger:

```
edit_markdown(path="docs/issues/<any-open-bug>.md",
              frontmatter={set: {status: "fixed", closed: "2026-08-29"}})
  → {"status": "ok"}

artifact(action="find", filter={"rel_path": {"contains": "<slug>"}})
  → status: "open"        ← stale
```

`artifact(action="update", id=…, patch={"status": "fixed"})` reconciles it, as
does `librarian(action="reindex")`.

Observed 2026-08-29 at commit `28bb6e8a`.

## Environment

Linux, branch `experiments`. Independent of feature flags.

## Root cause

**Observed at runtime** (the sequence above), and the mechanism traced in the
code: `edit_markdown` has exactly **one** librarian interaction, and it is a
guard, not a write —

`src/tools/markdown/edit_markdown.rs:1341-1346`:

```rust
// Reject librarian-managed artifacts — use artifact(action="update") instead.
// Passing the resolved path also catches augmented artifacts with no
// frontmatter id, where a direct write desynchronises file from params.
crate::util::librarian_guard::guard_not_librarian_managed(
```

Grepping `catalog|reindex|upsert|librarian` across the whole file returns that
one hit. There is no catalog write path.

**The guard is why this is silent rather than loud.** It rejects *augmented*
artifacts and *ledgers* — a plain bug file is neither, so it passes through
correctly, which is intended: `CLAUDE.md` and `get_guide("tracker-conventions")`
both document editing bug files with `edit_markdown`, and
`librarian_guard`'s own pinned test
(`a_catalogued_but_unaugmented_file_stays_directly_editable`) argues that
guarding on catalog membership would wrongly refuse `docs/RELEASE.md`,
`CONTRIBUTING.md` and every ADR.

So the guard is right and the gap is real: catalog **membership** is precisely
the population that is (a) allowed through and (b) has a row that now lies. The
divergence window is unbounded — nothing schedules a reindex.

## Evidence

Two independent instances the same day, found by two sessions that were not
coordinating on this:

1. `docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md`
   — this session. File `fixed`, catalog `open`.
2. The capped-get bug (`0349c912ab8950e3`,
   `docs/issues/2026-08-28-capped-get-body-round-trips-into-truncating-write.md`)
   — reported independently by session `codescout-97` while assembling a
   resume-queue status report: *"the catalog row still says status=open while the
   file on disk says status: fixed / closed: 2026-08-29 … so it keeps surfacing
   in the kind=bug triage query."*

The second is the consequence that matters: the divergence does not merely sit
there, it **feeds a status report**. `CLAUDE.md`'s verify-open cadence and the
activation bootstrap both prescribe the catalog query as the way to answer
"what's open?".

## Hypotheses tried

1. **Hypothesis:** the catalog updates lazily and would have caught up.
   **Test:** queried immediately, then again after the `artifact(update)` that
   fixed it; and the peer's instance had persisted for hours.
   **Verdict:** rejected — nothing reconciles without an explicit reindex or an
   `artifact` write.

2. **Hypothesis:** `edit_markdown` should have been refused by the librarian
   guard, making this user error rather than a defect.
   **Test:** read `guard_not_librarian_managed`'s contract and its pinned test.
   **Verdict:** rejected. Guarding on catalog membership is the behaviour that
   test exists to forbid, and both `CLAUDE.md` and the tracker-conventions guide
   explicitly document `edit_markdown` as the way to edit bug files.

## Fix

Options, cheapest first.

1. **Sync the row on write.** After a successful frontmatter mutation, if the
   resolved path matches a catalog row, upsert the indexed fields (`status`,
   `title`, `tags`, `time_scope`). Narrow: only when `frontmatter_changed` is
   true, only for paths already in the catalog — it does not add rows, so it
   cannot turn arbitrary markdown into an artifact.
2. **Refuse the specific keys.** Let `edit_markdown` write any frontmatter key
   *except* the catalog-indexed ones, and point those at `artifact(update)`. Loud
   instead of silent, but it contradicts the tool's own `long_docs` row 3d,
   which advertises `frontmatter={set: {status: "fixed"}}` as a headline use.
3. **Report the drift.** Add a `doctor` check comparing each row's `status`
   against its file's frontmatter. Catches this and any other desync route, but
   only when someone runs `doctor`.

(1) and (3) are complementary; (1) alone leaves other write routes unguarded, (3)
alone leaves the window open until someone looks. Not applied in this record.

## Tests added

None — this record is the finding. A regression test for option (1) should
assert on the **catalog**, not the file: edit a catalogued bug's status via
`edit_markdown`, then `artifact(find)` it and assert the returned `status`
matches. Asserting the file's bytes would pass today and prove nothing, since
the file was never the broken half.

## Workarounds

Flip status through the catalog instead:

```
artifact(action="update", id="<id>", patch={"status": "fixed"})
```

It writes both. Use `edit_markdown` for body prose and non-indexed frontmatter
keys (`unverified:`, `closed:`) where the divergence does not matter. Or run
`librarian(action="reindex")` afterwards.

## Resume

Decide between Fix (1) and Fix (3) — they are complementary, so likely both.
Implement (1) in `src/tools/markdown/edit_markdown.rs` at the
`frontmatter_changed` branch (`:1356`). Confirm the population first: count
catalogued artifacts whose file frontmatter `status` differs from their catalog
row, to size the existing drift rather than assuming these two are all of it.

## References

- `src/tools/markdown/edit_markdown.rs:1341-1346` — the only librarian
  interaction, a guard
- `src/tools/markdown/edit_markdown.rs:1237` — `long_docs` row 3d advertising the
  affected call
- `src/util/librarian_guard.rs` —
  `a_catalogued_but_unaugmented_file_stays_directly_editable`, why the guard
  correctly lets bug files through

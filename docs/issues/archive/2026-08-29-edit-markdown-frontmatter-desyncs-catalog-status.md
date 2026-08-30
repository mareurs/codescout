---
status: fixed
opened: 2026-08-29
closed: 2026-08-30
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
unverified: "Liveness caveat CLEARED 2026-08-30 13:2x: re-verified after the rebuild this field asked for, on a fresh server (PID 1899149, /proc/<pid>/exe live, binary inode 6442149 built 13:25:19). This very frontmatter write is the probe — it sets a CATALOG COLUMN (owners), and the catalog reflected it with no reindex. Remaining and unchanged: the server-side install at src/server.rs:374 is covered by no test; deleting that line leaves all 8 green."
owners: ["marius"]
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

**Third instance — 2026-08-30, this bug reproducing on itself.** Closing this
record was done with the very call it describes:
`edit_markdown(frontmatter={set: {status: "fixed", closed: "2026-08-30", …}})`.
Disk flipped to `fixed`; `artifact(action="get")` kept returning `status: "open"`.
The running server is executing an *unlinked* binary: `/proc/803849/exe` reports
`… (deleted)`, the process started 11:10:11, and the rebuild at 11:39:59 replaced
the file on disk (inode 6397097) without touching the running image. The code
answering these calls therefore predates 11:10:11 — two builds behind the fix
commit at 11:46:13, not one. *(Corrected: this first read "the running server was
built 11:39:59", which described the file on disk rather than the process. An
`mtime` on the binary says nothing about a server that started before it. The
conclusion is unchanged and stronger.)* The prediction was written down before the
call, and the reproduction cost nothing — the status had to be flipped anyway.

**It also sharpened the description: the desync is field-selective, and the split
follows exactly the filterable/non-filterable line.** `src/librarian/tools/get.rs:335`
serves `status` from `row.status`, a catalog column. `get.rs:525-533` re-parses
`extra` from the file on every call, with the comment *"not in the catalog and not
filterable via find — so the file is the only source"*. One `get` payload therefore
mixes two epochs: `"status": "open"` came back alongside `"closed": "2026-08-30"`
and an `unverified` note saying the bug was fixed.

That self-contradiction is the reader's **only** tell — and it is an accident of
this file happening to carry `extra` keys. A bug with none returns a stale
`status` that looks perfectly self-consistent, which is precisely the shape of the
two instances above.
**And this state is BL-45's own condition, which makes the recursion exact.** A
server running a deleted binary is precisely what `guard_stale_binary`
(`22f8b8d5`, patch-id `fd2c453b…`) refuses to let re-index. It would fire here —
except it is not in the running image either, for exactly the reason the hook is
not. Two correct, committed, gate-green fixes, both absent from the process that
would enforce them. Anything measured about catalog or index behaviour in this
window is a measurement of the *old* code, and nothing in any tool response says
so.
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

**Shipped 2026-08-30 — option (1). `518549d6` on `experiments`, patch-id
`c424f89f8aeb67eaa692eeda4a9812a13820041c`.**

Not as a direct call. `edit_markdown` is core and the catalog sits behind
`#[cfg(feature = "librarian")]`, so calling it directly compiles locally and fails
CI's lean lane. It goes through an installed hook mirroring `librarian_guard`'s
oracle, in the opposite direction: a `CatalogFrontmatterSync` trait and
process-wide slot in `src/util/librarian_sync.rs`, implemented by
`CatalogFrontmatterSyncer` in `src/librarian/adapter.rs`, installed at
`src/server.rs:374`. The syncer looks the row up and **never creates one**, so it
cannot promote arbitrary markdown into an artifact — the narrowness option (1)
promised.

Option (3), the `doctor` drift check, is **not** shipped and remains the
complementary half: (1) closes the `edit_markdown` route only, and says nothing
about any other writer.
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

Closed. Two things are genuinely outstanding; neither blocks the archive.

1. **The fix is not live in the server that closed this file.** The process (PID 803849, started 11:10:11) runs an
   unlinked binary — `/proc/<pid>/exe` reports `(deleted)` — so its code predates
   11:10:11; the 11:39:59 rebuild replaced the file, not the image, and the fix
   landed 11:46:13. Re-verify after the next
   `cargo rb`: one `edit_markdown` frontmatter write, then `artifact(action="get")`
   on the same id, and check that `status` moved.
2. **The server-side install is covered by no test.** The chain has four links —
   `edit_markdown` calls the hook, the slot holds it, the syncer updates the row,
   and `server.rs:374` installs the syncer. Three are mutation-checked. The fourth
   is not: deleting the install line leaves all 8 tests green, which was run and
   confirmed rather than assumed. Closing it needs a test that builds a real server
   and inspects the process-wide slot — and the identical gap exists for
   `librarian_guard`'s own install, so it is a shared shape, not a one-off.

Both are recorded in the `unverified:` frontmatter key, so a triage query reaches
them without anyone opening this file.
### Re-verified live 2026-08-30, after the rebuild this section asked for

Item 1 is **closed**. The operator rebuilt and reconnected; the new server (PID 1899149)
runs a live image — `/proc/<pid>/exe` resolves rather than reporting `(deleted)`, binary
inode 6442149, built 13:25:19, comfortably after `518549d6` at 11:46:13.

The probe was a frontmatter write on **this file**, chosen so it could not be answered by
the fresh half of the payload: it sets `owners`, a **catalog column**, not an `extra` key.

| | before | after |
|---|---|---|
| `owners` | `[]` | `["marius"]` |
| `updated_at` | `1788080569757` | `1788085622663` |

`updated_at` is the stronger of the two and was not part of the prediction. It is the
catalog **row's** own timestamp: under the bug this morning it stayed frozen at
`1788018155906` across a status flip, because nothing wrote the row. Its advancing here
proves a write occurred rather than a re-read.

**This narrows item 2 rather than closing it.** The four-link chain's first link —
`server.rs:374` actually installing the syncer — is now confirmed *empirically*: a real
server, a real `edit_markdown`, a real catalog column moving. What remains is exactly a
**regression** gap: nothing automated would notice if that install were deleted, and the
mutation proving so (all 8 tests stay green without it) still stands. The honest statement
is no longer "we do not know whether the install works" but "it works, and no test guards
it."
## References

- `src/tools/markdown/edit_markdown.rs:1341-1346` — the only librarian
  interaction, a guard
- `src/tools/markdown/edit_markdown.rs:1237` — `long_docs` row 3d advertising the
  affected call
- `src/util/librarian_guard.rs` —
  `a_catalogued_but_unaugmented_file_stays_directly_editable`, why the guard
  correctly lets bug files through

- `src/util/librarian_sync.rs` — the trait and process-wide slot (the fix)
- `src/librarian/adapter.rs` — `CatalogFrontmatterSyncer`, the catalog-side impl
- `tests/edit_markdown_catalog_sync.rs` — the end-to-end wiring test, in its own
  binary because the slot is process-wide and the unit-test binary contests it
- `src/librarian/tools/get.rs:335` and `:525-533` — why the desync is
  field-selective rather than total
- `open-issue-work-queue:BL-48`

---
status: open
opened: 2026-08-17
closed:
severity: high
owner: marius
related: []
tags: [librarian, ledger, entry-ids, link-scan, silent-corruption]
kind: bug
---

# BUG: a ledger's id counter can fall behind its own history, and the archived-definer tie-break turns the reissue into a silent wrong edge

## Summary

`allocate_entry_id` derives the next entry id from two sources — the ledger
file's own body (`body_max`) and a machine-local `entry_reservation` row
(`reserved_max`). Both can understate the ledger's real history at the same
time: the reservation is dropped by any operation that re-keys the artifact,
and `body_max` *decreases* whenever entries are compacted out to an archive
companion. When both understate, the allocator reissues an id that an archived
entry already defines — and because the resolver prefers the sole **active**
definer, every historical citation of that id silently re-points to the new,
unrelated entry. No dangling count moves; no ambiguity is reported.

## Symptom (Effect)

Not observed in the wild, but **reproduced under test 2026-08-17**. A ledger whose
history runs to `HY-10`, with its entries compacted into an archived companion and its
reservation gone, allocates **`HY-1`**:

```
---- librarian::catalog::augmentation::tests::allocate_entry_id_never_reissues_an_id_the_archive_still_defines stdout ----
thread '...' panicked at src/librarian/catalog/augmentation.rs:2102:9:
reissued HY-1 — HY-1..HY-10 are still defined by the archived companion, and the
resolver binds each token to its sole ACTIVE definer, so this re-points history
silently instead of dangling
```

The damaging part is what does *not* appear. Because the archived definer is
`active: false` and the reissued one is active, the resolver returns an `Edge` rather
than `Ambiguous` or `Dangling`:

```
before:  HY-1 (archived companion)  <- 6 citations resolve here
after:   HY-1 (archived companion)      0 citations resolve here
         HY-1 (live ledger, reissued)   6 citations resolve here, silently
link_scan: dangling +0, ambiguous +0
```

So no metric moves, and no sweep can find it after the fact.
## Reproduction

Commit `66487591`, branch `experiments`. The allocator is compiled in but not
live in a running MCP session until `cargo rb` + `/mcp`.

1. A ledger `docs/trackers/foo.md` declares `entry_prefix: HY` and holds
   `## HY-1 …` through `## HY-10 …`. Its `entry_reservation` row reads 10.
2. A hygiene sweep compacts HY-1…HY-8 into
   `docs/trackers/archive/foo-archived-entries.md`, keeping their headings
   (the ladder `get_guide("tracker-conventions")` § *Compaction and archival*
   mandates). The live body's max is now 10 — still fine.
3. Any ONE of these removes the reservation:
   - `artifact(action="move", …)` on the ledger — see Root cause;
   - a fresh clone, or any second machine (the table is machine-local);
   - a worktree session (a shadow row is a different `artifact_id`; see the
     sibling bug `docs/issues/2026-08-17-prose-ledger-worktree-id-collision.md`).
4. Now compact the remaining HY-9/HY-10 out too, or archive the whole file and
   start a successor at the same path. `body_max` is `None`.
5. `append_entry(id_prefix="HY")` returns **HY-1**.
6. `link_scan` binds every pre-existing `HY-1` citation to the new entry.

## Environment

Linux, `experiments` @ `66487591`. Affects every ledger declaring
`entry_prefix` — five today per `docs/TAXONOMY.md`.

## Root cause

Two independent mechanisms, both required, and each individually harmless —
which is why neither was caught when the allocator shipped.

**1. The reservation is not migrated by any re-keying path.**
`entry_reservation` is declared with
`artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE`
(`src/librarian/catalog/mod.rs`). `graft_rows` re-points exactly four tables —
`events`, `artifact_observation`, `artifact_link`, `event_edges` — plus
augmentation, then `DELETE FROM artifact WHERE id=?1`
(`src/librarian/catalog/graft.rs:51-86`). `entry_reservation` is not in that
list, so the delete cascades it away. Since `artifact(action="move")` is
implemented over the same graft, **archiving a ledger resets its counter**, and
archiving is a ledger's normal end state.

*Measured 2026-08-17:* `grep entry_reservation src/**/*.rs` → 4 matches in 2
files (`catalog/mod.rs` = the DDL, `catalog/augmentation.rs` = the allocator).
No migration path, no fork path, no merge path references it.

**2. `body_max` is not monotonic.** The allocator reads exactly one file — the
row's own `abs_path` — and scans it for line-anchored `## PREFIX-N` headings
and `| PREFIX-N |` row cells
(`src/librarian/catalog/augmentation.rs:726-828`, `body_claimed_indices`). It
never reads the ledger's archive companion. Compaction is *defined* as moving
entries out of that one file, so it lowers `body_max` by design.

The allocator's own doc comment states the safety argument that fails here:

> The reservation high-water mark is a row in `entry_reservation`, which is
> machine-local — and that is safe precisely because it is re-derivable: the
> allocator reads the committed body every time, so losing the table costs at
> most a re-issue of an id whose entry was never written.

The unstated premise is that the live body still contains **every id ever
issued**. Compaction and whole-file archival both violate it, and both are
mandated procedures in this repo (`archive-cadence-policy` surface 2,
promote-or-die; the hygiene skill's D10 distill-then-archive).

**3. Why it is silent rather than loud.** `resolve.rs:200-206`:

```rust
_ => {
    let active: Vec<_> = definers.iter().filter(|d| d.active).collect();
    if active.len() == 1 {
        Some(Outcome::Edge { dst_id: active[0].artifact_id.clone() })
    } else { Ambiguous { … } }
}
```

The archived definer is `active: false` (`status ∈ {archived, superseded}`,
`resolve.rs:37`), so the reissued live entry is the *sole* active definer and
wins outright — `Edge`, not `Ambiguous`, not `Dangling`. Pinned by
`archived_tie_break_resolves_to_sole_active_definer`
(`src/librarian/tools/link_scan/resolve.rs:316`).

That tie-break is correct for its stated purpose — "archive-flow residue must
not turn every archived tracker into an ambiguity generator" (`resolve.rs:17-19`)
— and it is precisely what converts this reissue from a reported ambiguity into
an unreported wrong edge. The defect is the *combination*, not the tie-break.

*Status of this analysis:* **measured 2026-08-17** — `cargo test --lib -- --ignored
allocate_entry_id_never_reissues` reproduces the reissue (output in § Symptom). The
three mechanisms above are each read from the cited lines; their *combination* is what
the test exercises end to end.

## Evidence

### The reservation lookup is keyed on a value that changes

`src/librarian/catalog/augmentation.rs`, inside `allocate_entry_id`:

```rust
let reserved_max: Option<u64> = tx
    .query_row(
        "SELECT max_allocated FROM entry_reservation WHERE artifact_id = ?1 AND prefix = ?2",
        rusqlite::params![artifact_id, id_prefix],
        |r| r.get::<_, i64>(0),
    )
    .optional()?
    .map(|v| v.max(0) as u64);

let next = body_max
    .map_or(0, |m| m + 1)
    .max(reserved_max.map_or(0, |m| m + 1))
    .max(1);
```

`artifact_id` is `sha256(abs_path)`. Both inputs to `next` are therefore
path-dependent, and `.max(1)` is the floor that yields `HY-1` when both are
absent.

### A vestigial second counter reads as authoritative

`docs/trackers/tracker-hygiene-log.md`'s augmentation params carry:

```json
{ "__allocated": { "HY": 10 } }
```

*Measured 2026-08-17:* `grep __allocated` over `*.rs`, `*.md`, `*.json`
including hidden paths → **0 matches**. Nothing writes it, nothing reads it,
no doc prescribes it. It is correct today by coincidence and will not track
HY-11. It is also the one place a reader *would* look for a committed
high-water mark — this session read it as the counter before checking.

## Hypotheses tried

1. **Hypothesis:** the reservation survives a move, because `move` grafts history.
   **Test:** read `GraftReport`'s fields and `graft_rows`' body.
   **Verdict:** rejected — the report enumerates events / observations / links /
   event_edges / entries; `entry_reservation` appears in neither, and step 6
   deletes the source row with `ON DELETE CASCADE` in force.
2. **Hypothesis:** a reissued id shows up as `Ambiguous`, so `link_scan` reports it.
   **Test:** read the multi-definer arm of `resolve`.
   **Verdict:** rejected — the sole-active tie-break returns `Edge`. This is the
   finding that raised severity from medium to high.
3. **Hypothesis:** the `__allocated` params key is the committed high-water mark.
   **Test:** `grep __allocated` across `*.rs`/`*.md`/`*.json`, hidden included.
   **Verdict:** rejected — 0 matches; it is hand-written DB-only residue.

## Fix

Not implemented. Two candidates, and the first is strictly better:

**A. Commit the high-water mark to frontmatter** (recommended). Store
`entry_high_water: 100` beside the existing `entry_prefix` declaration, written
by the allocator in the same transaction. Then
`next = max(frontmatter_high_water, body_max, reserved_max) + 1`. This makes the
counter survive clone, move, compaction and archival for the same reason
`entry_prefix` does — it is committed, so it is a fact about the ledger rather
than about one machine's database. `entry_reservation` degrades to what it is
genuinely good at: an in-transaction race guard between concurrent sessions on
one machine.

**B. Also scan the archive companion.** Cheaper to write, worse: it needs a
convention linking a ledger to its archive file, it re-reads a second file on
every allocation, and it still fails for a ledger whose entries were archived
into a differently-named file. Fixes the compaction trigger only, not the
clone/move ones.

Either way, add `entry_reservation` to `repoint_history`'s table list so a move
stops silently resetting the counter.

## Tests added

`allocate_entry_id_never_reissues_an_id_the_archive_still_defines` —
`src/librarian/catalog/augmentation.rs:2039`. **Red on purpose**, carrying
`#[ignore = "red until the high-water mark is committed — …"]` so the default gate stays
green; run it with `cargo test -- --ignored`. Delete the `#[ignore]` attribute when the
fix lands — that is the whole ceremony.

It is placed immediately after
`allocate_entry_id_does_not_reissue_when_the_body_has_not_caught_up`, which is its exact
counterpart: there the body lags the reservation and the reservation saves us; here both
lag the ledger's real history.

The fixture also seeds the archived companion artifact even though the allocator never
opens it, for two reasons — it states in-place *why* a low id is wrong, and it gives
candidate Fix B something to read without rewriting the test.

Still wanted, and not yet written: a resolver-side companion asserting that a live
definer and an archived definer of the same token do not silently produce an `Edge` for
a number the ledger has already retired. That one needs a policy decision first — the
tie-break is correct for its stated purpose, so the assertion has to name a narrower
condition than "two definers".
## Workarounds

- **Do not compact a ledger's entries out of its live body and archive the file
  in the same sweep.** Either operation alone is safe while the reservation
  survives; together on a machine that has lost the reservation they reissue.
- Before the first `append_entry` on a ledger in a fresh clone or after a move,
  check the returned `body_max` / `reserved_max` in the response against the
  ledger's real maximum including its archive companion. Both fields are
  returned by `AllocateOutcome` precisely so the caller can notice this.
- Prefer citing entry ids in the qualified `<file-stem>:HY-1` form for entries
  that have been archived: the qualifier pins the file, so a reissue in the live
  ledger cannot capture a citation aimed at the archive.

## Resume

The red test exists and the premise is now measured, so the next action is the design
call, not more investigation: **choose Fix A or Fix B** in § Fix. A is recommended —
commit `entry_high_water` to frontmatter beside `entry_prefix`, making the counter a
fact about the ledger rather than about one machine's database.

Whichever is chosen, `entry_reservation` must also join `repoint_history`'s table list
in `src/librarian/catalog/graft.rs`, or `artifact(move)` keeps silently resetting the
counter — that part is independent of A-vs-B and can land first.
## References

- `src/librarian/catalog/augmentation.rs` — `allocate_entry_id`, `ENTRY_PREFIX_KEY`,
  `body_claimed_indices`
- `src/librarian/catalog/graft.rs:51-86` — `graft_rows`, the table list that omits
  `entry_reservation`
- `src/librarian/catalog/mod.rs` — `entry_reservation` DDL
- `src/librarian/tools/link_scan/resolve.rs:17-21,37,200-206,316` — the
  archived-definer tie-break
- `docs/issues/2026-08-17-prose-ledger-worktree-id-collision.md` — sibling defect,
  same counter, different trigger
- `docs/trackers/tracker-hygiene-log.md` — HY-10 (ledger vs tracker), HY-11
- `docs/trackers/archive-cadence-policy.md` — surface 2 (promote-or-die) and
  surface 3 (archive destination), the two procedures that trigger this

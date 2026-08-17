---
status: fixed
opened: 2026-08-17
closed: 2026-08-17
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
     sibling bug `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md`).
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

**Fix A, implemented 2026-08-17 in `0364c23a` (`experiments`).** Promotion to `master` is
a fast-forward — `git rev-list --left-right --count master...experiments` reported `0` on
the left — so this SHA *is* the master SHA once promoted, and there is no second one to
record later.

The high-water mark is now committed to the ledger's
own frontmatter, one key per declared namespace:

```yaml
entry_prefix: HY
entry_high_water_HY: 11
```

`allocate_entry_id` takes `next` as the max of **three** inputs — the committed mark,
the body maximum, and the machine-local reservation — so no single source can walk the
counter backwards. Each is unreliable in a different way and none is ever wrong in the
*high* direction, which is what makes the max safe.

Where the change lives:

- `src/librarian/catalog/augmentation.rs` — `ENTRY_HIGH_WATER_PREFIX`,
  `entry_high_water_key`, the third input folded into `next`, and the write-back.
  `AllocateOutcome` gained `frontmatter_max` so a caller can see which input governed.
- `src/librarian/frontmatter.rs` — `upsert_int_line`, a new surgical primitive.
  `replace_scalar_line` *declines* on a missing key and
  `rewrite_frontmatter_normalizing` reformats hand-authored blocks (30 changed lines,
  BL-34); a caller that must persist committed state can accept neither. It inserts
  above the closing `---` rather than after `entry_prefix`, because that key's value may
  be a block sequence and a line spliced after it would land *inside* the sequence.
- `src/librarian/catalog/graft.rs` — `repoint_history` folds `entry_reservation` rows
  instead of letting the cascade drop them, merging per prefix with **`MAX`, not
  last-writer-wins**: folding a lower mark over a higher one would reintroduce this very
  bug through the move path.

**Write ordering is the durability argument.** The frontmatter write happens *before*
`tx.commit()`, and a failure to write fails the whole allocation:

- write fails → the transaction rolls back, no id was handed out. Refusing is correct —
  an id whose committed mark did not advance is precisely the reissue, so a silent
  fallback would reintroduce it.
- write succeeds, commit fails → frontmatter runs ahead of the database. Safe, because
  `next` takes the max: the next call reads the higher mark and skips an integer.

Concurrency on one machine is still handled by the enclosing `IMMEDIATE` transaction — a
second session blocks on the write lock and cannot interleave with the file write.

**Fix B (also scan the archive companion) was not taken.** It fixes only the compaction
trigger, not the clone or move ones, needs a convention linking a ledger to its
companion, and re-reads a second file on every allocation.

**One contract changed deliberately.** A prose reservation is no longer a pure read: it
writes exactly one frontmatter line. The tool's `next_step` hint and the test formerly
named `…_writes_nothing` (now `…_writes_no_entry`) were both updated to say so. The
intent that mattered is intact — the *entry* is still the caller's to write.
## Tests added

- `allocate_entry_id_never_reissues_an_id_the_archive_still_defines` —
  `src/librarian/catalog/augmentation.rs`. Written red first (output in § Symptom), now
  green; the `#[ignore]` is gone. It sits immediately after
  `allocate_entry_id_does_not_reissue_when_the_body_has_not_caught_up`, its exact
  counterpart: there the body lags the reservation and the reservation saves us, here
  both lag the real history.

  **One fixture decision the test turns on:** it compacts through
  `frontmatter::replace_body`, not by hand-writing the compacted file. Real compaction
  is a body edit and a body edit preserves the frontmatter block byte for byte;
  hand-writing it would erase the mark and silently test nothing. The first draft did
  exactly that and would have passed for the wrong reason.

- `graft_folds_entry_reservations_taking_the_higher_mark` and
  `graft_folds_entry_reservations_when_the_source_is_ahead` —
  `src/librarian/catalog/graft.rs`. The pair pins `MAX` rather than either extreme:
  with the source ahead, `max` and a plain overwrite agree, so only the
  source-behind direction discriminates. **Mutation-verified** — replacing
  `MAX(entry_reservation.max_allocated, excluded.max_allocated)` with
  `excluded.max_allocated` fails exactly the discriminating test (`Some(5)` vs
  `Some(12)`) and leaves the other green.

- Six `upsert_int_line_*` tests — `src/librarian/frontmatter.rs`. The load-bearing two:
  `…_renders_the_value_bare_not_quoted` (why this is not `replace_scalar_line`, whose
  round-trip check would quote `11` and make it read back as a string) and
  `…_does_not_insert_inside_a_block_sequence` (why the anchor is the closing delimiter
  and not `entry_prefix`).

- `omitting_entry_collection_reserves_an_id_and_writes_no_entry` —
  `src/librarian/tools/append_entry.rs`, updated. Now asserts exact file equality
  against the original plus the one spliced line, so any extra or reordered byte fails
  — which is what would catch a normalizing rewrite creeping back in — and asserts the
  second reservation *splices* the line rather than appending a second one.

Still wanted, deliberately not written: a resolver-side test asserting that a live and
an archived definer of the same token do not silently produce an `Edge`. That would
contradict `archived_tie_break_resolves_to_sole_active_definer`, which is correct for
its stated purpose, so the assertion needs a narrower condition than "two definers" —
a policy decision, not a test-writing one.
## Workarounds

No longer needed. Historical, for anyone on a build before this fix:

- do not compact a ledger's entries out of its live body and archive the file in the
  same sweep — either alone was safe while the reservation survived;
- check `body_max` / `reserved_max` in the `append_entry` response against the ledger's
  real maximum *including* its archive companion before the first allocation in a fresh
  clone or after a move.

One piece of advice outlives the fix: prefer the qualified `<file-stem>:HY-1` citation
form for entries that have been archived. The qualifier pins the file, so nothing in the
live ledger can capture a citation aimed at the archive.
## Resume

N/A — fixed and verified on `experiments`.
## References

- `src/librarian/catalog/augmentation.rs` — `allocate_entry_id`, `ENTRY_PREFIX_KEY`,
  `body_claimed_indices`
- `src/librarian/catalog/graft.rs:51-86` — `graft_rows`, the table list that omits
  `entry_reservation`
- `src/librarian/catalog/mod.rs` — `entry_reservation` DDL
- `src/librarian/tools/link_scan/resolve.rs:17-21,37,200-206,316` — the
  archived-definer tie-break
- `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md` — sibling defect,
  same counter, different trigger
- `docs/trackers/tracker-hygiene-log.md` — HY-10 (ledger vs tracker), HY-11
- `docs/trackers/archive-cadence-policy.md` — surface 2 (promote-or-die) and
  surface 3 (archive destination), the two procedures that trigger this

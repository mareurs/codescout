---
kind: bug
status: open
tags:
- cluster/authorship-unrecoverable-after-the-fact
opened: 2026-09-06
closed:
severity: medium
owner: marius
related:
- docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md
---

# BUG: `audit_host_id_previous` is single-valued, so a catalog transported twice loses the chain back to its first identity

## Summary

`resolve_host_id`'s transport check writes the id it replaced into a single
`catalog_meta` key, `audit_host_id_previous`. A catalog transported a **second** time overwrites
that key with the id from the *first* transport, and the link back to the original host is gone
— along with `doctor`'s ability to name the shard holding the oldest stranded rows.

Found while answering *"we are moving this work to the desktop — does it make sense to repair?"*,
which is exactly the second hop. It is a limitation of the fix at
`d4f0bafb`, noticed by its own author reasoning about the next move rather than by anyone
hitting it.

## Symptom (Effect)

Not yet observed. Derived from the code and from a concrete impending scenario.

Laptop today, after `d4f0bafb` fired:

```
audit_host_id          = archlinux-d9b5c3
audit_host_id_previous = ripper-65e654      <- the workstation, correct
```

Transport this catalog to the workstation and open it there:

```
audit_host_id          = ripper-<newhex>    <- re-minted, correct
audit_host_id_previous = archlinux-d9b5c3   <- OVERWRITES ripper-65e654
```

`ripper-65e654-202609.jsonl` still exists, still holds 27,297 rows, and is now referenced by
nothing. `doctor`'s `host_previous_stranded_rows` will report the `archlinux-d9b5c3` shard and
be silent about the larger, older one.

## Reproduction

```
sqlite3 <catalog> "UPDATE catalog_meta SET value='a-aaaaaa' WHERE key='audit_host_id';"
# open on a host whose name sanitizes to `b`  -> previous = a-aaaaaa
sqlite3 <catalog> "UPDATE catalog_meta SET value='c-cccccc' WHERE key='audit_host_id';"
# open on a host whose name sanitizes to `d`  -> previous = c-cccccc, and `a-aaaaaa` is gone
```

## Environment

`experiments` @ `72363d8d`. Applies to every host running `d4f0bafb` or later; the defect is in
the fix, not in the code it fixed.

## Root cause

`src/librarian/catalog/audit/host.rs`:

```rust
pub(crate) const PREV_HOST_META_KEY: &str = "audit_host_id_previous";
...
gc::set_meta(conn, PREV_HOST_META_KEY, &stored)?;
```

`set_meta` is a replace. The key models *"the id before this one"* when what the shards actually
require is *"every id this catalog has ever written under"* — the audit directory accumulates one
file per identity per month and never forgets, so a single-valued pointer into an append-only
population is structurally short by however many hops have happened.

**Why it was not caught:** the fix was reasoned about, tested and verified against exactly one
transport, because exactly one had happened. Every test seeds a clean catalog and one foreign
id, so the second hop is not merely untested — it is unrepresentable in the fixtures as written.
Verified 2026-09-06 by reading `resolve_host_id_for` and all six of its tests.

## Evidence

### The key is written unconditionally on every re-mint

`resolve_host_id_for` has one `set_meta(PREV_HOST_META_KEY, …)` call, on the re-mint path, with
no read-modify-write and no append.

### Nothing else records the chain

`catalog_audit` has no host column (`seq, at_ms, tbl, op, row_id, actor, verb, payload`), so the
per-row host is not recoverable from the trail either — see the parent bug's § *Fix*. The
`catalog_meta` key is the only record that a given shard belongs to this catalog's lineage.

## Hypotheses tried

1. **Hypothesis:** the shard filenames on disk are a sufficient substitute, so the key does not
   need to be a list.
   **Verdict:** rejected. `.codescout/audit/` is shared and git-tracked, so it accumulates shards
   from hosts this catalog has never been — a clone carries every collaborator's files. The
   directory tells you which shards *exist*, never which are *this lineage's*. That distinction
   is the whole value of the key.

## Fix

Not implemented. Make the value a list rather than a scalar — append on re-mint, dedupe, and
have `doctor` sum stranded rows across every id in it rather than the one.

Two things to decide rather than assume:

- **Ordering and cap.** Newest-first is the useful read order. A cap is probably wrong: the
  population is bounded by how many machines have held the catalog, which is small, and a
  silently-dropped oldest entry reproduces this bug one layer down.
- **Migration.** Existing catalogs hold a scalar. Read a bare string as a one-element list
  rather than requiring a migration — the value is host ids, which never contain the separator
  a list form would use.

**Do not** solve it by stamping the host per audit row. That is a much larger change, it does
not help any row already written, and the parent bug establishes that machine attribution of
existing rows is unrecoverable regardless.

## Tests added

None — filed at notice, and the fix is not written.

The test this needs is the one whose absence hid it: **two** re-mints in sequence, asserting the
first predecessor survives the second. Every existing test seeds one foreign id, so the whole
suite is monotone under this defect — it cannot fail whatever the second hop does, which is why
six passing tests and a live production verification all missed it.

## Workarounds

Read `audit_host_id_previous` **before** transporting a catalog onto a third machine, and record
it somewhere outside `catalog_meta` if the chain matters. On this laptop today that value is
`ripper-65e654`, and the shard it names holds 27,297 rows.

Cheaper still: **do not transport the catalog again.** The git repo carries the code and the
shards; a machine that already has its own catalog does not need another one's.

## Resume

Decide list-vs-scalar and write the two-hop test first — it reds against `d4f0bafb` as it
stands, which is the cheapest possible confirmation that the defect is real before any fix is
designed.

## References

- `docs/issues/archive/2026-09-04-a-transported-catalog-carries-its-host-identity.md` — the
  parent bug, whose fix introduced this key.
- `src/librarian/catalog/audit/host.rs` — `PREV_HOST_META_KEY`, `resolve_host_id_for`.

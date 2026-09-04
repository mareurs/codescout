---
id: d0e2dbc4a4085d0c
kind: bug
status: open
title: 'BUG: audit_host_id lives in the catalog, so a transported catalog makes the receiving host write the sender''s audit lines into a committed merge=union shard'
owners:
- marius
tags:
- cluster/authorship-unrecoverable-after-the-fact
- librarian
- audit
- cross-machine
- catalog
topic: audit host identity survives catalog transport
opened: 2026-09-04
related:
- docs/conventions/cross-machine-catalog-resume.md
- docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md
severity: high
unverified: 'Whether the several-hundred-entry audit_open_gaps list is caused by cross-host sequence interleaving is NOT established -- gaps have other documented causes (prune markers, rolled-back transactions burning seq). The host-identity adoption itself IS established at the bytes: catalog_meta.audit_host_id = ripper-65e654 on a host whose every candidate_name() source yields archlinux. Also not established: whether any OTHER host has already merged rows into this shard, which would make the mixing bidirectional rather than one-way.'
---

## Summary

`resolve_host_id` persists the audit host id in **`catalog_meta`** and mints one only when
that key is absent. The catalog is machine-local *and transportable* — this repo ships a
spec and a plan for integrating one host's catalog into another's. After such a transport
the receiving host reads the sender's `audit_host_id` and adopts it silently, so two
machines write audit rows under one identity, into one monthly shard file, which is
**tracked in git and declared `merge=union`**.

`shard_file_name`'s own doc comment states the invariant this loses:

> `<host>-<YYYYMM>.jsonl`. One file per host per month: month bounds the file size, and
> **host keeps two machines off each other's lines entirely.**

## Observed live

Session `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, 2026-09-04, on `experiments` at
`0b20709c`.

This machine:

```
hostname       archlinux
/etc/machine-id 44ac14c3d011437f9060f3c2c13ff674
CODESCOUT_AUDIT_HOST / COMPUTERNAME / HOSTNAME   unset
```

The catalog:

```
sqlite> SELECT value FROM catalog_meta WHERE key='audit_host_id';
ripper-65e654
```

`candidate_name()` tries `CODESCOUT_AUDIT_HOST`, `COMPUTERNAME`, `HOSTNAME`, then
`/etc/hostname` — all of which yield `archlinux` here — so a fresh mint on this host would
be `archlinux-<suffix>` and could never be `ripper-65e654`. The id did not originate here.

`~/.local/share/librarian/catalog.db` was replaced wholesale earlier the same day: a
backup named `catalog.db.bak-preworkstation-20260904-094517` sits beside it, and the file
grew 7.4 MB → 124 MB at 09:45. The id arrived with the bytes.

**The contamination is already in the tracked file.** `.codescout/audit/ripper-65e654-202609.jsonl`
holds 20,011 rows, **every one** stamped `"host": "ripper-65e654"` — including 58 rows
whose `actor` is `codescout:cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, this session, on this
laptop. A `librarian(action="reindex")` this afternoon appended 5,566 lines to it. Those
lines are **not committed** — see § *Workarounds*.

## Root cause

`src/librarian/catalog/audit/host.rs`:

```rust
pub(crate) const HOST_META_KEY: &str = "audit_host_id";

pub(crate) fn resolve_host_id(conn: &Connection) -> Result<String> {
    if let Some(existing) = gc::get_meta(conn, HOST_META_KEY)? {   // ← catalog, not machine
        if !existing.trim().is_empty() {
            return Ok(existing.trim().to_string());
        }
    }
    let id = mint_host_id(&candidate_name());
    gc::set_meta(conn, HOST_META_KEY, &id)?;
    Ok(id)
}
```

The function is correct for its stated contract — *"The stable id for this catalog's
machine: read from `catalog_meta` if already minted"*. The defect is in the premise, and
the doc comment names it precisely: **"this catalog's machine"** assumes a catalog belongs
to one machine for life. It does not, and this repo's own
`docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md`
is the design that breaks the assumption.

`the_host_id_is_resolved_once_and_then_persisted` pins the persistence, correctly. Nothing
checks that the persisted id belongs to the host reading it — the check that would catch
this cannot be written from the id alone, because the id is *deliberately* not derived
from anything verifiable (the comment says the readable prefix "is a courtesy, not the
correctness").

## Why this matters more than a mislabelled row

- **`seq` is per-host.** Two machines minting sequence numbers under one host id interleave
  them. `catalog_meta` here holds an `audit_open_gaps` list running to several hundred
  entries against `audit_written_through_seq = 104649`, which is the shape that
  interleaving would produce. **Not established** as caused by this — see § *Unverified*.
- **The shard is committed and `merge=union`** (`.gitattributes:10`). So the wrong
  attribution does not stay local: it is published, and a union merge folds both machines'
  lines into one stream with nothing marking the seam. `read_shards` keys on the host
  segment of the filename, so it cannot separate them afterwards.
- **It defeats the feature's whole purpose.** `516da1df` committed the first shard with the
  message *"commit this host's audit shard so a clone can answer for its history"*. A clone
  can now answer for two hosts' history as though it were one host's.
- **It is silent in both directions.** The receiving host reports a plausible id and a
  green export; the sending host sees its own shard grow with rows it did not write.

## Suggested direction (not a plan — reproduce first)

1. **Bind the id to the machine, not the catalog.** Keep persisting it, but persist the
   machine fingerprint alongside (`/etc/machine-id`, or the `candidate_name()` result) and
   re-mint when the stored fingerprint does not match the running host. That makes the
   transport case self-healing and leaves the normal case untouched.
2. **Or refuse rather than adopt** — a loud error at open naming both ids beats a silent
   re-identification, and the catalog-integration flow can then clear the key deliberately.
3. **Whichever ships, the integration path must clear or re-mint `audit_host_id`.** That is
   the one-line half, and it is worth doing even before (1): it is the step the 09:45
   transport was missing.
4. **The already-mixed rows are a separate question.** Rows written by this laptop under
   `ripper-65e654` cannot be re-attributed from the data — `actor` carries a sessionId, so
   *session* attribution survives, and that is the recovery route rather than the host
   field. Do not rewrite the committed shard; decide whether the mixed month is worth
   annotating.

## Workarounds

The 5,566 exported lines are **left uncommitted deliberately**. Committing them would
publish this laptop's rows under the workstation's host id into a `merge=union` file, which
is the irreversible half of the defect. They are not lost — the rows are in the catalog and
`audit_exported_through_seq` governs re-export once the identity is fixed.

## Unverified

Whether the large `audit_open_gaps` list is caused by cross-host sequence interleaving is
**not established** — gaps have other documented causes (a prune leaves a marker, and
rolled-back transactions burn sequence numbers). It is stated above as a shape consistent
with interleaving, not as a consequence proven here.

## Resume

Not started. Reproduce by reading `catalog_meta.audit_host_id` on any host whose catalog
was transported, and comparing it to `mint_host_id(candidate_name())` for that host.

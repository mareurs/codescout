---
id: 1f70ca0cd916e7a0
kind: bug
status: fixed
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
closed: 2026-09-06
opened: 2026-09-04
related:
- docs/conventions/cross-machine-catalog-resume.md
- docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md
severity: high
unverified: 'The audit_open_gaps question is UNCHANGED and still not established: whether the several-hundred-entry gap list was caused by cross-host sequence interleaving was never proven, and gaps have other documented causes (prune markers, rolled-back transactions burning seq). The fix stops future interleaving; it does not explain the existing gaps and no measurement here attributes them. Also still unestablished: whether any OTHER host merged rows into this shard, which would make the mixing bidirectional. AND NEW, created by the fix: the ~5,566 rows already exported under ripper-65e654 stay counted as exported by the per-repo watermark and will never re-emit under the corrected id without a deliberate rollback -- doctor reports them as host_previous_stranded_rows, but nothing remediates them and no one has decided whether to.'
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


## Fix

**Fixed at `d4f0bafb` on `experiments`, patch-id
`ec44ea726129a1117fe2c034c9bcc3c18ae11de6`.** Direction 1 above, with one simplification and
one addition that the direction did not anticipate.

**Reproduced before the plan was read**, as this file's own Resume demanded:
`catalog_meta.audit_host_id` = `ripper-65e654` on a host where `CODESCOUT_AUDIT_HOST` and
`COMPUTERNAME` are unset and `HOSTNAME` and `/etc/hostname` both read `archlinux`. The shard
had been written two minutes earlier, so this was live accumulation, not a historical
artifact.

### The simplification: no new state, which is what makes it reach the affected catalogs

Direction 1 proposed persisting a machine fingerprint alongside the id. Correct, but a new
meta key only protects catalogs minted **after** it exists — and the population that matters
is exactly the ones minted before, this machine's among them. A remedy that cannot fix the
instance that motivated it is not a remedy.

`mint_host_id` writes `sanitize(candidate) + "-" + <6 lowercase hex>`, so the sanitized machine
name *at mint time* is already inside the id. `minted_name()` recovers it and `foreign_mint()`
compares it against this host's — `ripper` vs `archlinux` — with no new key and no migration.

**Two cases deliberately do not re-mint**, because neither can discriminate:

- an id that does not parse as `<name>-<6 lowercase hex>` — an unrecognised format is not
  evidence of transport, and re-minting would discard an identity the code does not
  understand;
- a host whose name sanitizes to the `host` fallback — every machine that cannot name itself
  produces that string, so re-minting there churns a fresh shard on every open, which is this
  defect with its sign flipped.

### Direction 2 rejected, and the reason generalises

*"Refuse rather than adopt — a loud error at open"* is the better **report** and the worse
**behaviour**. The catalogs this fires on are already in the mismatched state, so refusing
bricks every tool call on that host until a human intervenes. Stated as a rule, because it is
not specific to this bug: **a guard that refuses on an invalid state is only safe if the state
is reachable but not yet reached.** Once the system is already in it, the same guard is a
denial of service — and the asymmetry is that the party proposing it is reasoning about a clean
system while the party implementing it is standing in the dirty one.

### The addition the direction did not anticipate: the fix would have hidden the loss

Raised by `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b` while the change was in flight, and it changed
the shape of the fix rather than adding a field to it.

After re-minting, the identity is correct **and** the export is complete by the watermark's own
accounting — because the watermark is per-repo and never rolls back (see the corrected
§ *Workarounds*). So a report answering only *"does the stored id match this host?"* would read
clean while thousands of rows sat in another machine's shard, owed by no counter anywhere.
**That is the fix making the symptom invisible while leaving the loss — worse than the bug,
which at least left a wrong-looking filename in `git status`.**

So `doctor` reports the stranded population, not the identity match:

| field | |
|---|---|
| `audit_health.host_previous` | the id that was replaced, from `audit_host_id_previous` |
| `audit_health.host_previous_stranded_rows` | total rows sitting in that host's shards |
| `audit_health.host_previous_shards` | per-file line counts |
| `audit_health.host_previous_hint` | says outright that `unexported_rows` will **not** count them |

The hint naming the contradiction is the part that makes it a report rather than one more
number — every other figure in that block says the export is complete, and it is, by the only
definition those figures use.

## Tests added

Six, in `src/librarian/catalog/audit/host.rs`; 17/17 in the module.

- `a_transported_catalog_is_re_minted_and_names_what_it_replaced` — the motivating case, end to
  end, including that the replaced id is preserved rather than dropped.
- `an_id_minted_here_is_returned_unchanged_and_records_no_previous` — the control. Without it
  every other assertion is satisfied by a function that re-mints unconditionally.
- `a_catalog_that_cannot_be_judged_is_left_exactly_as_found` — both no-re-mint cases, as
  separate assertions rather than a loop, because they are separate guards.
- `a_blank_stored_id_mints_without_claiming_a_predecessor` — pins the ordering of the emptiness
  check, which if reordered would write a `host_previous` of `""` that `doctor` would report as
  a real predecessor.
- `foreign_mint_distinguishes_cannot_tell_from_all_is_well` and
  `minted_name_accepts_only_the_shape_mint_host_id_emits` — the pure halves.

**Deterministic without touching the environment.** `resolve_host_id_for` takes the machine
name, so every branch is an ordinary call rather than a race on a process-global env var under
a parallel runner — this repo already carries bug files about load-sensitive flakes, and
`candidate_name()` reads three env vars.

**Mutation-verified, with the runs bracketed by `git rev-parse HEAD` and the file's mtime**
so a peer's commit could not silently revert the mutation and manufacture a false survivor
(`WINDOW_CLEAN=yes` both times). Disabling the fallback guard killed two tests; widening the
suffix test from `== 6` to `>= 4` killed the shape test. No survivors.
## Workarounds

The exported lines sitting uncommitted in `.codescout/audit/ripper-65e654-202609.jsonl` are
**left uncommitted deliberately**. Committing them would publish this laptop's rows under the
workstation's host id into a `merge=union` file, which is the irreversible half of the defect.

**CORRECTED 2026-09-06, and the corrected sentence is the one an operator reads first.** This
section used to end: *"They are not lost — the rows are in the catalog and
`audit_exported_through_seq` governs re-export once the identity is fixed."* The watermark does
**not** govern that. It is per-**repo**, keyed on the repo path (`shard::watermark_key`,
`src/librarian/catalog/audit/shard.rs:146-154`), and the host id is absent from the key
entirely. Fixing the identity does not roll it back: every row already emitted under
`ripper-65e654` stays counted as exported and will never re-emit under the corrected id on its
own. Re-verified at those lines by the original reporter rather than accepted from the
correction.

What actually survives, and what does not:

- **Not recovered by the fix:** the *machine* attribution of the already-exported rows. Nothing
  automatic restores it — and after the fix ships, every counter reports the export as
  complete, because by the watermark's accounting it is. That is precisely why `doctor` reports
  `audit_health.host_previous_stranded_rows` and not merely whether the identity matches: a
  check that answered only the identity question would read clean while thousands of rows sat
  in another machine's shard with nothing owed by any number in the report.
- **Not lost at all:** *session* attribution. Audit rows carry `actor`, which is a sessionId, so
  **who** wrote a row is recoverable even where **which machine** is not. That was always the
  recovery route; the host field was never the thing carrying it.
- **Recoverable, but only on purpose:** re-emitting the stranded rows under the corrected id
  means rolling the per-repo watermark back below the contaminated range and re-exporting. That
  has a real cost — it re-emits *everything* after that point, not only the contaminated rows —
  so it is an operator decision, and the fix deliberately does not make it unasked.
## Unverified

Whether the large `audit_open_gaps` list is caused by cross-host sequence interleaving is
**not established** — gaps have other documented causes (a prune leaves a marker, and
rolled-back transactions burn sequence numbers). It is stated above as a shape consistent
with interleaving, not as a consequence proven here.

## Resume

Fixed and archived. Two things are genuinely outstanding and are recorded in `unverified:`
rather than left to be rediscovered:

1. **The `audit_open_gaps` question is untouched.** Whether the several-hundred-entry gap list
   was caused by cross-host interleaving was never established, and the fix does not answer it
   — it stops *future* interleaving. Gaps have other documented causes (prune markers,
   rolled-back transactions burning `seq`), and no measurement here separates them.
2. **The stranded rows are reported but not remediated.** `doctor` now names them
   (`audit_health.host_previous_stranded_rows`); nothing re-attributes them, and whether the
   watermark rollback is worth its cost is an operator decision nobody has made.

**On this host specifically:** the re-mint has not happened yet at time of archiving. It fires
on the next `resolve_host_id` call against the live catalog — any `reindex`, `audit_log` or
`doctor`. After it, `.codescout/audit/` will hold `ripper-65e654-202609.jsonl` (frozen, this
laptop's rows mixed into the workstation's) alongside a new `archlinux-<hex>-202609.jsonl`, and
`doctor` will report the first as stranded. That gap is permanent unless someone acts on (2).

**Do not commit the uncommitted `ripper-65e654-202609.jsonl` lines** — see § *Workarounds*.
That advice survives the fix unchanged.

---
id: ea6be3de65e9e674
kind: spec
status: draft
title: Re-keying entry_cite.src_slug — design
owners:
- marius
tags:
- librarian
- entry-graph
- graft
- design
topic: A catalog primitive that moves entry_cite rows from one artifact slug to another
---

# Re-keying `entry_cite.src_slug` — design

Status: draft for operator review. **Design only. No code was written, and no live catalog was read or written.**
Origin: `docs/issues/archive/2026-09-17-graft-onto-a-destination-whose-slug-has-durable-citations-fails-at-commit.md`
(fixed by refusal, `1d76c5b0`), whose last paragraph says a tool that re-keys `src_slug` "would be the real fix for the
both-cited case; not built — a product decision." This document is the input to that decision.

Evidence labels used below: **[ran]** I executed it (sqlite3 CLI 3.53.4 against a scratch in-memory schema copied from
`src/librarian/catalog/mod.rs:195-210`; the rusqlite-bundled version was not tested). **[read]** I read the lines cited and did
not execute them. **[unverified]** stated as such.

## 1. What is true today

### 1.1 The schema (read)

`src/librarian/catalog/mod.rs:195-210`:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS ux_artifact_slug ON artifact(slug);
CREATE TABLE IF NOT EXISTS entry_cite (
  src_slug  TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local TEXT NOT NULL,
  dst_ref   TEXT NOT NULL,          -- free TEXT, NO foreign key
  rel       TEXT NOT NULL,
  origin    TEXT NOT NULL DEFAULT 'write',
  created_at INTEGER NOT NULL,
  PRIMARY KEY (src_slug, src_local, dst_ref, rel));
```

Two columns carry a slug, and they behave differently:

| column | holds | integrity |
|---|---|---|
| `src_slug` | the citing artifact's slug | FK to `artifact(slug)`, `ON DELETE CASCADE`, **no `ON UPDATE`** (so `NO ACTION`) |
| `dst_ref` | either a 16-hex artifact id, or `<slug>:<local>` of the cited entry | **none** — nothing stops it dangling |

`dst_ref` is told apart by shape: `gc.rs:462-475` (`apply_rehome`) relies on "a slug never equals an id", and
`artifact.rs:268-325` (`ensure_slug`) refuses to mint a 16-hex slug for exactly that reason (`looks_like_artifact_id`,
`artifact.rs:404-408`). `resolve_cite_ref` (`augmentation.rs:2389-2460`) splits a cite on the first `:`, so a slug may not
contain `:` either. Any new slug-writing code inherits both constraints.

### 1.2 Who writes `entry_cite` (read, by grep over `src/`)

| writer | file:line | origin | keyed by |
|---|---|---|---|
| `append_entry(cites=…)` | `augmentation.rs:972-986` (`ensure_slug`, then `entry_cite::insert_with`) | `write` | the ledger's slug |
| `link_scan(write=true)` | `link_scan/mod.rs:834-850` (`prune_scan_rows`, then `insert_with`) | `scan` | the scanned artifact's slug |
| `rekey_prefix` | `rekey.rs:621-675` (`UPDATE OR IGNORE … SET src_local` / `SET dst_ref`) | both | moves `src_local` and the token inside `dst_ref` |
| `apply_rehome` | `gc.rs:470` (`UPDATE entry_cite SET dst_ref`) | both | rewrites a hex-id `dst_ref` |

The other `INSERT INTO entry_cite` hits (`migrate_v6.rs:635`, `gc.rs:595`, `link_scan/mod.rs:2134`, `audit/*.rs`) are inside
`#[cfg(test)]` modules.

**No production code updates `src_slug`.** `grep -E 'SET\s+src_slug' src/` returns 0 matches. [ran]

The committed audit shards agree for the one host that exports them (`.codescout/audit/*.jsonl`, Sept 2026, host
`ripper-65e654` only): 29,022 `entry_cite` inserts, 27,785 deletes, **0 updates**, 110 distinct citing slugs. [ran: `grep` over
the shards.] Blind spots: one host, one month, no `origin` on insert rows, and the delete count is mostly scan re-derivation
churn. It shows scale and the absence of any past re-key, not demand.

### 1.3 Who writes `artifact.slug` (read)

Exactly two places:

1. `ensure_slug` — `artifact.rs:321`, mint-if-NULL, reached from `create.rs:442`, `update.rs:819`, `adapter.rs:160`,
   `indexer.rs:583` (all via `upsert_and_mint_slug`, `artifact.rs:336`) and from `append_entry` (`augmentation.rs:972`).
2. `graft_rows` — `graft.rs:197-204`, the slug handover (below).

Slugs are documented as immutable once non-null (`artifact.rs:344-346`; test message at `artifact.rs:873`: "a re-mint
would silently re-key entry_cite.src_slug"). Any rename tool is a deliberate exception to that stated position.

### 1.4 What `graft` and `move` do with a slug (read)

`graft_rows` (`graft.rs:129-220`), inside one `IMMEDIATE` transaction:

- Step 0 (`graft.rs:160`) calls `refuse_if_handover_would_strand_citations` (`graft.rs:56-107`). It refuses only when **both
  rows hold differing slugs and `into`'s slug has outgoing `entry_cite` rows** — the count at `graft.rs:72` is
  `WHERE src_slug = ?1` and nothing else.
- Step 6 (`graft.rs:197-204`): `defer_foreign_keys = ON`; `UPDATE artifact SET slug=NULL` on `from`; `UPDATE artifact SET
  slug=<from's> ` on `into`; `DELETE` `from`. So **`from`'s slug always survives and `into`'s slug is always the displaced
  one.** `from`'s outgoing rows follow the slug for free because they are keyed by it (`entry_citations_carried`,
  `graft.rs:210`).

`doc(move)` does not change a slug **on the paths I traced**: `mv.rs:312` seeds the destination with `artifact::upsert` (which has no slug column,
`artifact.rs:137-187`), then `mv.rs:379` calls `graft_rows`, which carries the old slug onto the slug-less new row. The same
holds for `reseat_one` (`merge_worktree.rs:513-514`) and `doctor` `reseat_worktree` (`doctor.rs:2477`). `merge_one` never calls
`graft_rows` (module header, `merge_worktree.rs:1-15`).

**So a slug changes in exactly one reachable case: a `doc(action="graft")` between two rows that both already hold differing
slugs.** The question "a move that changes a slug" has no live instance; extending `move` would be a wall in an empty field.

### 1.5 The scenario that actually reaches it (read, not executed)

A bare `git mv` followed by a reindex. The librarian guide says the row keeps pointing at the vanished path and "the next
reindex mints a fresh id for the new one — with no graft". The indexer mints a slug for that new row
(`indexer.rs:583`, `upsert_and_mint_slug`), and `ensure_slug` dedups against slugs already taken (`artifact.rs:299-321`), while
the old row still holds the title-derived base. The new row therefore gets `<base>-2`. If a session then appends a citing entry
to the new row before someone grafts old into new, both rows hold slugs and `into` has citations: the refusal fires, and the
refusal text itself says "No tool re-keys `entry_cite.src_slug`" (`graft.rs:96`).

## 2. What is broken or silent (findings)

**F-A. Both-cited graft is a dead end.** The refusal is correct and loses nothing, but offers no way out when both sides are
cited (`graft.rs` remedy text; test `graft_refusal_names_only_remedies_that_exist`, `graft.rs:836-895`). This is the stated
problem.

**F-B. The graft dry-run is not a faithful preview of the refusal.** `tools/graft.rs:49-85` builds its preview from
`artifact::get`, `links::outgoing/incoming`, `observations`, `events` and never calls `refuse_if_handover_would_strand_citations`
(it is private to `catalog/graft.rs`) and never runs the transaction. A refused pair therefore previews as fine, and `force=true`
then refuses. `would_move` also has no `entry_cite` field. Contrast `rekey_prefix`, whose comment at `rekey.rs:745-752` and tool
hint (`tools/rekey_prefix.rs:73`) promise "an Ok preview means the apply would also succeed".

**F-C. Inbound citations to the displaced slug dangle silently.** The refusal counts `src_slug = into_slug` only
(`graft.rs:72`). Rows in *other* ledgers whose `dst_ref` is `<into_slug>:<local>` have no FK, so they survive the overwrite and
point at a slug no row holds. The control test that pins "no children, still succeeds" (`graft.rs:811-829`) seeds no inbound
row. No `doctor` check looks for this: `grep dst_ref src/librarian/tools/doctor*.rs` returns 0 matches, and
`doctor/scope.rs:500` says an unresolvable `dst_ref` is skipped ("fail closed"). [read]
Whether this matters in practice is unmeasured (section 8).

**F-D. Entry renumbering is not applied to citations.** When both sides carry the same `entry_collection`,
`fold_entries` (`graft.rs:346-420`) renumbers colliding incoming ids and records `report.remap`. Nothing applies that map to
`entry_cite.src_local` or to inbound `dst_ref` tokens; `graft_rows` touches `entry_cite` only through the slug.
After a renumber, `from`'s carried rows can attach to a *different* entry with the old id. [read; no test run — characterize
before relying on it, test 9 below.]

**F-E (adjacent, not in scope).** `artifact::upsert`'s pre-clean `DELETE FROM artifact WHERE abs_path=? AND id != ?`
(`artifact.rs:152`) cascades a row's outgoing citations when an id changes at a path. `mv.rs:266-275` already documents the
mechanism for events/links. Noted so nobody mistakes it for a slug problem.

## 3. Named change scenarios

| # | scenario | needs |
|---|---|---|
| S1 | Graft two established, cited rows (bare `git mv` + reindex + new citations, then graft). Today: refused, no remedy. | move `into`'s rows onto the surviving slug, atomically with the handover, plus inbound `dst_ref` |
| S2 | Rename one slug to a free name (e.g. drop a `-2`; `artifact.rs:342-368` measured 173 of 4104 unslugged rows needing a suffix at the 50-char cap, max chain depth 10). No one has asked for this. | move the artifact's own rows and inbound `dst_ref` to a free slug |
| S3 | Hosts disagree about a slug. | not solvable by a re-key tool; see 5.5 |

S1 and S2 are the same row operation — "move every row keyed by slug X to slug Y" — differing only in the precondition on Y
(held by the graft's surviving row, or free). That is two concretes for one primitive, which is the only justification for
extracting one.

## 4. Options

### Option A — a new `doc(action="rekey_slug")`

A standalone action: `id`, `to`, `force`. Rename-only is safe (`to` free). A *merge* variant (`to` held by another row) is
**unsafe as a standalone step**: after it, the rows of row P are keyed by a slug held by row Q, so `doc get` on Q shows P's
citations until a graft completes. A crash, a refusal or an abandoned session leaves them mis-attributed with no marker.
Composition also fails the other way: re-keying `from`'s rows onto `into`'s slug does not unblock the graft, because step 6
always makes `from`'s slug survive, so `into`'s slug (now holding all the rows) is displaced and the refusal fires again.
- Gains: discoverable; solves S2; composable with `doc(event_create)`.
- Costs: does not solve S1 on its own; one more `doc` action (schema floor, description budget, 3 prompt surfaces — section 7);
  contradicts "slug is immutable" unless that is amended; a sanctioned way to change a name agents type into `cites=`.
- Confidence it is the right *first* build: low.

### Option B — extend `graft` (not `move`)

Add `citations: "refuse" | "merge"` to `doc(action="graft")`, default `refuse` (today's behavior). `merge` re-keys, inside the
existing `IMMEDIATE` transaction and the existing deferred-FK handover, the displaced slug's rows onto the surviving slug, and
rewrites inbound `dst_ref` the same way. Because it lives in the same transaction as the handover there is no mis-attributed
interim state.
- Gains: solves S1; one atomic unit; lets `remap` (F-D) be applied to `src_local`/`dst_ref` in the same pass; the dry-run can be
  made faithful (F-B) because `graft_rows` already produces a `GraftReport`.
- Costs: graft gains a policy knob whose semantics are an assertion about the two rows (section 6.3); `graft_rows`'s signature
  changes for 4 other callers (`mv.rs:379`, `merge_worktree.rs:514`, `doctor.rs:2477`, one test at `worktree.rs:603`); it does
  nothing for S2.
- Not for `move`: section 1.4 shows `move` cannot reach a slug change, so there is nothing for it to opt into.

### Option C — do nothing

Keep the refusal. Cost: S1 is a documented dead end ("Leave the rows separate"), F-B/F-C/F-D stay. Benefit: no new surface,
and the refusal "loses nothing". **This is the right answer if section 8's measurement returns zero both-cited pairs**, and the
cheap parts of the fix (F-B, F-C) are still worth doing because they change no behavior that anyone depends on.

### Option D — one shared primitive, graft first, standalone rename only on demand (recommended)

`catalog/rekey_slug.rs` (new) holds `rekey_slug_rows(tx, from_slug, to_slug, mode)`. The first caller is `graft_rows` under
`citations: "merge"` (Option B). A standalone `doc(action="rekey_slug")` is a thin wrapper that can be added when S2 shows
real demand, with no second implementation to drift. This is Option B with the part that could be reused named, not an
abstraction in an empty field: the second caller is a known, deferred concrete (S2), and the first has an evidenced blocker (S1).

## 5. Failure modes

### 5.1 Partial write

Everything this operation touches is catalog-side: `slug` is not in frontmatter (`grep slug` over `frontmatter.rs` and
`augmentation_sidecar.rs`: 0 matches), not in a committed sidecar, and prose citations use the file **stem**, not the slug
(`link_scan/resolve.rs:274-277`). So, unlike `rekey_prefix`, there is no external half and no post-commit publish step to
fail between (compare `rekey.rs:779-800`, whose error says "the catalog moved but its committed sidecar did not"). The only
partial-write risk is a multi-transaction caller; `graft_rows` is one `IMMEDIATE` transaction.

### 5.2 FK checked at COMMIT, and why a rolled-back preview is not enough

Measured [ran]:

- Renaming `artifact.slug` with children and no deferral fails immediately: `FOREIGN KEY constraint failed`.
- With `PRAGMA defer_foreign_keys=ON`, updating the parent then the children in one transaction commits cleanly.
- With deferral on and the children **not** re-keyed, the failure moves to `COMMIT` and names nothing — the exact character of
  the bug filed 2026-09-17.
- **`PRAGMA foreign_key_check` run inside the transaction, before commit, reports the pending violation** (`entry_cite|1|artifact|0`)
  and reports nothing once the children are re-keyed.

Consequence for dry-run: a preview that only runs the statements and drops the transaction would pass cases that apply fails,
because a dropped transaction never reaches the deferred check. The preview must run `foreign_key_check` before dropping and
fail the same way apply would. `rekey_prefix` does not need this (`UPDATE … SET src_local` touches no FK); this operation does.

### 5.3 Dry run: one code path

Follow `rekey.rs:46-51` and `rekey.rs:749-752`: a `Mode { Preview, Apply }` flag, the same statements, the same refusals, and
`drop(tx)` instead of `commit()` for Preview. `graft_rows` writes only catalog rows (no file, no vector store), so unlike
`rekey_body` a rolled-back run is a faithful preview here. [read: `graft.rs:129-220` has no I/O.] This also fixes F-B.
One caveat: the audit triggers (`audit/mod.rs:236-250`) fire inside the transaction, and their rows are rolled back with it;
I did not check that `audit_ctx` stamping behaves identically in a rolled-back run. [unverified]

### 5.4 Citation collisions

Two distinct kinds, and only one is an error:

1. **Primary-key collision.** Re-keying rows from slug X to a *held* slug Y can duplicate `(src_slug, src_local, dst_ref, rel)`.
   [ran] `UPDATE OR IGNORE … SET src_slug` leaves the colliding row behind under X (1 of 2 moved in the fixture), so the leftover
   must be deleted explicitly, as `drop_superseded` does (`rekey.rs:130-141`) — and for the same reason: here the later
   `DELETE`/cascade is not guaranteed to run. A dropped duplicate is lossless (same citer, cited, rel). Report it as
   `duplicates_merged`.
2. **Local-id collision.** Rows `(Y, T-5, …)` and `(X, T-5, …)` have different PKs when `dst_ref` differs, so [ran] they merge
   into one `T-5` carrying both targets — no error, and wrong if the two `T-5`s are different entries. The catalog cannot tell
   "the same ledger moved" from "two unrelated ledgers with colliding ids". Only the caller knows, which is why `merge` must be an
   explicit assertion and must **refuse** a local-id collision that has no `remap` (listing the colliding locals) unless the
   caller says the namespaces are one (open question Q3). Prose ledgers have no `remap` at all: `fold_entries` only touches params
   rows (`graft.rs:440-515`).

A third, for a *rename* (S2): `dst_ref` has no FK, so a deleted former holder of the target slug can leave dangling `<to>:*`
rows. Re-using that slug would silently re-point them at a new artifact. Count them in the preview and refuse by default (Q6).

### 5.5 Cross-host catalog drift

Slugs and `entry_cite` are machine-local and not committed. `mint_missing_slugs` (`artifact.rs:449-472`) orders by `abs_path` so
the same corpus mints the same slugs; its doc comment makes that determinism the point. A re-key on host A therefore makes A's
slugs a *non-function of the corpus*: host B, or a fresh clone, mints from scratch and disagrees. Only the audit shards cross
hosts, and their `entry_cite` rows embed the slug in `row_id` (`audit/mod.rs:62-65`) and attribute it with a **live** join on
`artifact.slug` (`audit/shard.rs:282-287` and its `entry_cite` arm), so after a re-key old shard rows are unattributable to a
path. I did not test two hosts. [unverified]

What this implies: the tool is local-only and cannot cause drift that is not already possible, but it cannot repair it, and an
agent that typed `cites=["old-name:F-1"]` on host A gets "no such entry (slug or local id not found)" on a host that never saw the
rename (`augmentation.rs:2454`). The root cause is that the slug is a catalog-local name used as a cross-session address.
Making it a committed fact is out of scope (Q7), but it is the thing that would remove S3.

### 5.6 Other

- **Concurrency.** `IMMEDIATE`, as `graft_rows` and `rekey_prefix_rows` already do; a second codescout on the same WAL catalog
  is the case `audit/mod.rs:177-185` already calls out. The preview's counts can be stale by apply time; apply recomputes and
  reports what it did rather than trusting the preview.
- **Worktree sessions.** A slug is catalog-global, so a worktree session must not fork a shadow to rename it. Mirror `move` and
  `delete`: refuse a main-root target from a worktree (`mv.rs:194`, `delete.rs:77`; `resolve_write_target` forks instead,
  `worktree.rs:75-89`, which is right for per-ledger content and wrong here). `graft` itself currently has no such guard.
  [read; whether graft needs one is Q8.]
- **Shape.** Any caller-supplied slug must match `^[a-z0-9]+(-[a-z0-9]+)*$`, not be 16 lowercase hex, contain no `:`, and fit a
  stated cap. Today the guards live in `ensure_slug` and are not reusable (`looks_like_artifact_id` is private,
  `artifact.rs:404`). This is a parser over a namespace (CLAUDE.md § *Parsers Over a Namespace*): it owes an escape or a refusal,
  and a disambiguator. The refusal is the escape.
- **Scan rows.** Unlike `rekey_prefix`, which drops `origin='scan'` rows because the prose still says the old token
  (`rekey.rs:18-27`), a slug re-key leaves the prose unchanged (stems), so scan rows are **rewritten, not dropped**: the next
  `link_scan(write=true)` re-derives the same triple under the new slug. Dropping them would lose edges until a rescan for no
  reason.

## 6. Recommendation (Option D)

**Decision:** extract `rekey_slug_rows` as the single place that writes `entry_cite.src_slug` and the slug prefix of
`dst_ref`; call it first from `graft` under an explicit `citations: "merge"`; make graft's dry-run run the real transaction; do
not add a standalone action or touch `move` until measurement or a request says so.

### 6.1 Contract

`doc(action="graft", from_id, into_id, citations?="refuse"|"merge", force?)`

- `force` omitted or false: **Preview.** Runs the real path inside the transaction, runs `PRAGMA foreign_key_check`, drops the
  transaction. Returns the same refusal an apply would, or the full report. Every refusal precedes the first write, so an Ok
  preview is a statement that apply would succeed (the property `rekey_prefix` already advertises, absent from graft today).
- `citations` omitted or `"refuse"`: current behavior, plus the refusal also counts **inbound** rows (`dst_ref LIKE
  '<displaced>:%'`, re-derived in Rust as `rekey.rs:644-655` does, since `_` is a LIKE wildcard) so F-C is no longer silent.
  This changes one case that succeeds today (Q4).
- `citations: "merge"`: after the handover statements and before commit, inside the same transaction:
  1. outgoing: rows keyed by the displaced slug get `src_slug` = surviving slug;
  2. inbound: `dst_ref` `<displaced>:<local>` becomes `<surviving>:<local>`;
  3. `remap` from `fold_entries` is applied to `src_local` (outgoing) and to `<local>` (inbound) for the renumbered entries;
  4. PK duplicates are dropped explicitly and counted;
  5. a local-id collision with no `remap` entry refuses, listing the locals, **before any write** — unless Q3 resolves to
     "assert same ledger".
- The report gains `citations: { strategy, displaced_slug, surviving_slug, outgoing_rekeyed, inbound_rewritten,
  duplicates_merged, local_collisions[], remap_applied }`, in both Preview and Apply. `entry_citations_carried` stays.
- The response states, at the moment the reader can act, what the tool did **not** do: it changes no file, and a citing file that
  quotes the displaced slug in prose or a note is not rewritten (prose normally uses stems).

### 6.2 Transaction order (the load-bearing part)

Inside `graft_rows`' existing `IMMEDIATE` transaction, after `repoint_history`/`merge_augmentation` (so `remap` exists) and
using the `defer_foreign_keys = ON` already set at `graft.rs:197`:

1. handover as today (`from.slug = NULL`, `into.slug = <from's>`, delete `from`);
2. `rekey_slug_rows(displaced → surviving)`;
3. `PRAGMA foreign_key_check` must be empty, else return the error naming table and parent (a `LibrarianRecoverableError`,
   not the bare commit failure);
4. Apply: `commit()`. Preview: `drop(tx)`.

The deferral is what makes step 1 legal and step 2 repair it; ordering step 2 before step 1 fails the immediate check [ran].

### 6.3 What `merge` asserts

`merge` means "these two rows are one ledger; collisions on the same local id are the same entry". It is correct for S1 (same
file moved) and wrong for two unrelated ledgers. The default stays `refuse` so the assertion is never implied. Whether the
assertion should be a separate explicit flag, and what to do for prose ledgers, are Q2/Q3.

### 6.4 The standalone rename (deferred)

If built: `doc(action="rekey_slug", id, to, force)`, rename only (`to` free, shape-checked, adoption of dangling `<to>:*` rows
refused), worktree main-root targets refused, Preview/Apply as above, and a `doc(event_create)` note carrying
`{from, to}` so a later reader can find the old name. It reuses `rekey_slug_rows` unchanged. The `doc` description is
about 1,630 bytes of a 1,800 cap (`artifact.rs:106-110`; the 1,638 is `wc -c` of that source line including indentation and quotes, and whether the cap counts bytes or characters was not checked), so the action blurb must fit in about 150
bytes, and `artifact.rs:279` (the `force` description) names the dry-run actions by name.

## 7. Tests that would prove it

Write each red first against the current code where one exists. Names are suggestions; homes are `src/librarian/catalog/graft.rs`
(`#[cfg(test)]`) and the new `rekey_slug.rs`.

Primitive:

1. `outgoing_rows_of_every_origin_follow_the_slug` — `write` and `scan` rows both re-keyed. Kills: filtering on `origin`.
2. `inbound_dst_ref_is_rewritten_on_an_exact_slug_boundary` — `old:T-1` moves; `old-extra:T-1`, `old2:T-1` and a 16-hex id do not.
   Kills: `LIKE 'old%'`, a missing `:`.
3. `a_clean_rekey_leaves_no_foreign_key_violation` — `pragma_foreign_key_check` count is 0 after apply (pattern at
   `migrate_v6.rs:643`). Kills: deleting the child `UPDATE`.
4. `preview_fails_where_apply_would_at_commit` — a fixture whose handover would strand rows: Preview returns the named error,
   not Ok. Kills: dropping the in-transaction `foreign_key_check`. [Section 5.2 is why.]
5. `preview_changes_nothing` — full-table snapshot of `artifact` and `entry_cite` (every column, `created_at` included) is
   byte-equal before and after.
6. `pk_duplicates_are_dropped_and_counted_not_left_behind` — [ran] shows `OR IGNORE` leaves 1 row; assert 0 remain under the
   displaced slug and `duplicates_merged` equals the count. Kills: omitting the cleanup.
7. `a_local_id_collision_with_no_remap_refuses_and_names_the_locals` — nothing written (snapshot equal).
8. `dst_ref_shape_discriminator_survives` — after a re-key, no `dst_ref` that was `<slug>:<local>` has become 16-hex and vice versa
   (the `gc.rs:462` invariant).

Graft integration:

9. `graft_remap_is_applied_to_citations` — build a params collision (`fold_entries` renumbers `T-5` to `T-9`), assert carried
   rows and inbound `dst_ref` follow it. **Characterize the current behavior first** and check it is wrong before fixing (F-D is
   read, not run).
10. `graft_preview_refuses_exactly_where_apply_refuses` — today red (F-B). Both-cited pair, default `citations`.
11. `graft_default_is_unchanged_for_cited_destination` — the three existing tests (`graft.rs:737-895`) stay green untouched.
12. `graft_refuses_when_only_inbound_rows_would_dangle` — F-C; and a control that a destination with neither kind still succeeds
    (`graft.rs:811-829`).
13. `both_cited_graft_with_merge_keeps_every_citation` — S1 end to end through `doc(graft)`: preview, then apply, counts equal,
    `doc get include_links` shows both sets under the surviving slug.

Guards:

14. `src_slug_is_rewritten_only_in_rekey_slug` — a source-grep DRY gate in the style of
    `like_escape_idiom_is_not_inlined_outside_helper`: no `SET src_slug` outside the helper. Today it passes vacuously (0 matches),
    which is its point: it fixes the invariant before there is a writer to violate it. It should also pin that `artifact.slug` is
    written only by `ensure_slug` and the graft handover.
15. Mutation probe, `./scripts/mutation-probe.sh` (`docs/PROBES.md`): at minimum the four mutants named in 2, 3, 4, 6. A new test
    that no mutant kills is decoration.
16. Schema floor: `graft:citations` adds one action/key pair, so the floor at `artifact.rs:618` moves 99 to 100. **Measure it from
    `sweep`'s own `checked`; do not assume** (the comment above it records exactly that error being made once).

## 8. Measure before building

Run read-only against the live catalog. I did **not**; the SQL below was checked only on a toy schema (syntax and join shape).

```sql
-- S1 candidates: a suffixed slug and its base both carrying outgoing citations.
WITH s AS (SELECT a.id, a.slug, a.abs_path, a.missing_since,
                  (SELECT COUNT(*) FROM entry_cite c WHERE c.src_slug = a.slug) AS n
           FROM artifact a WHERE a.slug IS NOT NULL)
SELECT x.slug AS suffixed, x.n AS suffixed_rows, y.slug AS base, y.n AS base_rows, y.missing_since
FROM s x JOIN s y ON x.slug GLOB y.slug || '-[0-9]*' AND x.slug <> y.slug
WHERE x.n > 0 AND y.n > 0;

-- F-C: inbound slug-form dst_ref whose slug no row holds.
SELECT COUNT(*) AS dangling, group_concat(DISTINCT substr(dst_ref, 1, instr(dst_ref, ':') - 1)) AS slugs
FROM entry_cite c
WHERE instr(c.dst_ref, ':') > 0
  AND NOT EXISTS (SELECT 1 FROM artifact a WHERE a.slug = substr(c.dst_ref, 1, instr(c.dst_ref, ':') - 1));
```

Decision rule: first query empty **and** second near zero, so Option C plus F-B/F-C. First non-empty, so Option D.

## 9. Open questions for the operator

- **Q1.** Is S1 real? Run section 8. If the first query is empty, is "do nothing plus F-B/F-C" acceptable?
- **Q2.** In a both-cited graft, `from`'s slug always survives today. Should graft take `keep: "from" | "into"`? It changes which
  artifact's *name* persists, not which row survives. Keeping the default is the conservative answer.
- **Q3.** What does `merge` assert, and how for prose ledgers? Options: (a) refuse on any local-id collision lacking a `remap`
  (safe; S1 will usually collide wholesale and so usually refuses); (b) treat collisions as the same entry on an explicit
  `same_ledger=true`; (c) only `merge` when `into` has no augmentation and no own entries. S1's typical shape decides this, so it
  needs the section 8 data.
- **Q4.** Extending the refusal to inbound rows changes one currently-successful case (F-C). Acceptable, or warn-only for a release?
- **Q5.** Is a standalone rename wanted, given slugs are documented immutable (`artifact.rs:344-346`, test message `:873`)? If yes,
  those comments are amended in the same change.
- **Q6.** Rename onto a slug that dangling `<to>:*` rows already name: refuse (recommended), or adopt?
- **Q7.** Should the slug become a committed, cross-host fact (frontmatter or sidecar)? It is the root of S3 and out of scope here.
- **Q8.** Should `doc(graft)` refuse a main-root target from a worktree session, as `move`/`delete` do? Today it does not.
- **Q9.** Record `{from, to}` slugs in an event, knowing events are catalog-local too?

## 10. Unverified

- F-D (renumber not applied to citations) and S1's `-2` suffix chain are read, not executed.
- The rusqlite-bundled SQLite version was not compared with the 3.53.4 CLI used for the FK experiments.
- Audit behavior of a rolled-back preview (`audit_ctx` stamping) and the exact payload the `UPDATE` audit trigger would record
  for a `src_slug` change.
- Cross-host behavior (5.5) from reading only; two catalogs were not compared.
- The live catalog was neither read nor written. The shard counts in 1.2 are from committed files in the worktree.
- `artifact::upsert`'s `ON CONFLICT(id) DO UPDATE` omits `slug` (`artifact.rs:137-187`), so it leaves an existing slug alone. If a catalog row already existed at `move`'s destination id, that row's slug would survive the seed and `move` would meet the same refusal as `graft`. `move` refuses an existing destination *file* (`mv.rs:213-243`) and I found no check of a catalog row there, so a stale row with no file (for example one already stamped `missing_since`) is the only way I can see to reach it. Not executed.
- `merge_one`'s `DELETE FROM artifact WHERE id=shadow_id` would cascade any `entry_cite` keyed by a shadow's slug.
  `append_entry(cites=…)` from a worktree is refused (`append_entry.rs:404-410`), but a reindex can mint a shadow slug
  (`indexer.rs:583`) and a worktree `link_scan(write=true)` could then write scan rows under it. Scan rows are re-derivable;
  I did not check whether anything does this.

## 11. Decision record

**Decision:** one primitive owns every write to `entry_cite.src_slug`; `graft` is its first caller behind an explicit
`citations: "merge"`; graft's preview runs the real transaction plus `foreign_key_check`.
**Context:** the 2026-09-17 refusal made the both-cited graft safe and unrecoverable; `src_slug` has no `ON UPDATE`; `dst_ref` has no FK; no code updates `src_slug`.
**Alternatives considered:** standalone `rekey_slug` merge (rejected: mis-attributed interim state, and it cannot unblock a graft whose handover always keeps `from`'s slug); standalone rename first (deferred: no demonstrated demand, contradicts stated slug immutability); extend `move` (rejected: `move` cannot reach a slug change, section 1.4); do nothing (acceptable only if section 8 is empty).
**Consequences:**
- now easier: S1 resolves atomically; F-B, F-C, F-D stop being silent; there is one auditable writer of `src_slug`.
- now harder: `graft_rows` gains a mode and a policy argument across 5 callers; `merge` carries an assertion callers can get wrong; the slug-immutability comments need a qualifier.
**Change scenarios absorbed:** S1 now; S2 by adding a thin action later; a future decision to commit slugs (Q7) changes one writer.
**Revisit-when:** section 8 returns zero; a second slug-writing feature appears; slugs become committed.
**Confidence:** medium on the shape (primitive in graft first), low on whether it should be built at all before section 8 is run.

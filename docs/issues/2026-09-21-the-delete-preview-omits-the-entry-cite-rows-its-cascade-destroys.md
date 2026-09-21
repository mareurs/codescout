---
id: '0e18a4933944a359'
kind: bug
status: open
title: 'BUG: the delete dry-run preview omits the entry_cite rows its own cascade destroys'
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-09-21
owner: marius
related:
- docs/issues/2026-09-21-the-delete-preview-calls-an-untracked-file-git-restorable.md
severity: high
---

# BUG: the delete dry-run preview omits the entry_cite rows its own cascade destroys

## Summary

`doc(action="delete")` without `force` returns a `cascades` object that enumerates five
relations — `augmentation`, `links_out`, `links_in`, `observations`, `has_events`. It does
**not** name `entry_cite`, which `ON DELETE CASCADE` removes on the same call. The preview
exists to let a caller see an irreversible blast radius before authorising it, and the one
relation whose `origin='write'` rows nothing can rebuild is the one it does not mention.

## Symptom (Effect)

Measured 2026-09-21, tree `d155a8f6bb938b769a0a6fd0242ffc7e60a25f77`, on a throwaway artifact
created and deleted inside this session. One `entry_cite` row existed at preview time:

```
sqlite3 -readonly ~/.local/share/librarian/catalog.db \
  "select src_slug,src_local,dst_ref,rel,origin from entry_cite where src_slug like 'scratch-571eb3d6%'"
scratch-delete-probe-ledger-571eb3d6|ZZZ-1|61e8179f432a60c3|cites|write
```

The dry run, on that same artifact, one call later:

```
{"dry_run": true, "deleted": false, "id": "17b231e872f20cd1",
 "would_delete_abs_path": ".../docs/scratch-571eb3d6-ledger.md",
 "cascades": {"augmentation": true, "links_out": 0, "links_in": 0,
              "observations": 0, "has_events": false},
 "recoverable": "the file is git-tracked and restorable; the augmentation, events, links
                 and observations are catalog-only and are not",
 "hint": "re-run with force=true to apply: ..."}
```

No `entry_cite` key, and no `entry_cite` in the `recoverable` sentence either. After
`force=true`:

```
sqlite3 -readonly ... "select count(*) from entry_cite where src_slug='scratch-delete-probe-ledger-571eb3d6'"
0
```

**1 → 0, previewed as nothing.** The success response
(`{id, deleted_abs_path, deleted, vectors_deleted}`) does not report it either, so the
destruction is announced at no point, before or after.

## Reproduction

Tree `d155a8f6bb938b769a0a6fd0242ffc7e60a25f77`, branch `experiments`, 2026-09-21T16:20Z.
Reproduced here from scratch, not relayed:

1. `doc(action="create", kind="note", rel_path="docs/scratch-<x>-target.md", …)` → id A.
2. `doc(action="create", kind="tracker", rel_path="docs/scratch-<x>-ledger.md", …)` → id B.
3. `doc(action="augment", id=B, augment={prompt:…, entry_collection:"rows", params:{rows:[]}})`
   — `cites` at write time requires an `entry_collection`; a prose ledger refuses it with
   *"`cites` is not supported on a prose ledger"*.
4. `doc(action="append_entry", id=B, entry_collection="rows", id_prefix="ZZZ", entry={…}, cites=[A])`
   → one `entry_cite` row, `origin='write'`.
5. Confirm the row by SQL (above).
6. `doc(action="delete", id=B)` → the preview above. No `entry_cite`.
7. `doc(action="delete", id=B, force=true)`, then re-run the SQL → `0`.

Both scratch artifacts were deleted afterwards and never committed; `git status --short`
returns the tree to its pre-probe state.

## Environment

codescout MCP over stdio, profile `~/.claude-kat`, branch `experiments`, Linux.
Catalog at `~/.local/share/librarian/catalog.db`, sqlite backend, Qdrant artifact store
reachable (`vectors_deleted: true`). Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`.

## Root cause

**Measured** (above) and **read at the bytes**:

- `src/librarian/tools/delete.rs:90` — the dry-run branch imports exactly
  `{augmentation, events, links, observations}`. `entry_cite` is not imported, so the
  enumeration below could not name it.
- `src/librarian/tools/delete.rs:96-102` — the `cascades` object, five hand-written keys.
- `src/librarian/catalog/mod.rs:200-208` — `entry_cite.src_slug TEXT NOT NULL REFERENCES
  artifact(slug) ON DELETE CASCADE`. Dropping the `artifact` row drops every outgoing
  `entry_cite` row keyed by that slug.

The population is a **hand-enumerated list**, not a derived one: nothing ties the five keys to
the set of `ON DELETE CASCADE` relations, so a table added after the preview was written is
excluded with no count to report and nothing to mark. `entry_cite` is exactly that table —
introduced by the v9 entry-graph migration, later than the delete gate's enumeration.

**Severity turns on recoverability, and one half is genuinely unrecoverable.**
`src/librarian/tools/link_scan/mod.rs:833` writes `origin = ORIGIN_SCAN`, and its prune is
scoped to scan rows (`entry_cite::prune_scan_rows`, `src/librarian/catalog/entry_cite.rs:69`).
So a write-mode `link_scan` re-derives `origin='scan'` rows from prose and **nothing rebuilds
`origin='write'` rows**, which is the independently-recorded finding of
`docs/issues/archive/2026-09-17-graft-cascade-deletes-the-source-ledgers-outgoing-entry-citations.md`.
The probe row above was `origin='write'` with no prose counterpart at all
(`append_entry` returned `section_written: false`), so it was unrecoverable by any means.

## Evidence

### The suite already asserts the cascade the preview omits

`src/librarian/catalog/entry_cite.rs:296` —
`cascade_delete_removes_entry_cite_when_artifact_deleted`. The codebase knows the cascade
fires; only the preview does not say so.

### The `cascades` fix is a two-line change, because the accessors already match

`src/librarian/catalog/entry_cite.rs` exposes `outgoing` (:81) and `incoming` (:106) — the
same shape as the `links::outgoing` / `links::incoming` already called at
`src/librarian/tools/delete.rs:98-99`. Nothing had to be written for this to be previewable.

### The preview's prose makes the stronger, and false, claim

`src/librarian/tools/delete.rs:103-104` reads *"the augmentation, events, links and
observations are catalog-only and are not [restorable]"*. That sentence purports to
**enumerate** the unrecoverable casualties, so the absence of `entry_cite` is not merely a
missing field — it is an assertion that the four named are the whole set.

### The existing test's own comment enshrines the wrong total

`src/librarian/tools/delete.rs:389` asserts `cascades.augmentation == true` with the message
*"it is the casualty `reindex` cannot rebuild — a preview that omits it hides the only
irreversible part"* (and the module doc at :373 repeats it). `origin='write'` `entry_cite`
rows are a second irreversible part, so the test documents a false uniqueness. This is
`CLAUDE.md` § *Testing Discipline*'s **closed-population** law: the assertion was exact when
the augmentation *was* the only such casualty, and a later table silently widened its subject
with no edit to the test, the code it guards, or its fixture.

### Two halves, and only one cascades

`dst_ref` carries **no** foreign key (`src/librarian/catalog/mod.rs:203`). So *incoming*
entry-grain citations — rows whose `dst_ref` names the deleted artifact or one of its entries
— are **not** cascaded; they are left dangling. That half is **inferred from the schema, not
measured**: the probe's incoming row shared a source with the deleted artifact, so its removal
does not discriminate. `entry_cite::incoming` would preview it.

## Hypotheses tried

1. **Hypothesis** — the preview omits `entry_cite` because nothing cascades it, so the
   omission is correct. **Test** — count rows before and after a forced delete.
   **Verdict** rejected: 1 → 0.
2. **Hypothesis** — the omission is cosmetic because `link_scan` re-derives the rows.
   **Test** — read `link_scan`'s materialize path and prune scope.
   **Verdict** rejected for `origin='write'` rows: `src/librarian/tools/link_scan/mod.rs:833`
   writes only `ORIGIN_SCAN`, and `prune_scan_rows` touches only scan rows.
3. **Hypothesis** — the post-delete response reports what was removed, so the caller learns
   it after the fact. **Verdict** rejected: the success payload is
   `{id, deleted_abs_path, deleted, vectors_deleted}`.

## Fix

Not fixed. The change is to add `entry_cite` to the enumeration at
`src/librarian/tools/delete.rs:96-102` — `entry_cite_out: entry_cite::outgoing(&cat, &slug)?.len()`
and `entry_cite_in: entry_cite::incoming(&cat, …)?.len()` — and to extend the `recoverable`
sentence to distinguish `origin='scan'` rows (re-derivable by a write-mode `link_scan`) from
`origin='write'` rows (not re-derivable by anything).

Note the keying difference that makes this slightly more than a copy of the `links` lines:
`artifact_link` is keyed by artifact **id**, `entry_cite.src_slug` by the artifact's **slug**,
which is nullable. An artifact with a NULL slug holds no outgoing rows and the FK never bites
— so a correct preview reads the slug first rather than the id.

The deeper repair is to stop hand-enumerating: derive the `cascades` keys from the set of
relations declared `ON DELETE CASCADE` against `artifact`, so the next table added is
previewed by construction.

## Tests added

None — status is `open`. A regression test must **not** be a second aggregate: per
`CLAUDE.md` § *Testing Discipline*, assert the named member (`cascades.entry_cite_out == 1`
on a fixture holding exactly one `origin='write'` row) and observe the red before the fix,
not a `cascades.len()` bound. The existing
`delete_without_force_is_a_dry_run_and_destroys_nothing`
(`src/librarian/tools/delete.rs:376`) and its *"the only irreversible part"* comment both need
correcting in the same change, or the suite keeps asserting the false uniqueness.

## Workarounds

Before any `doc(action="delete")` on an artifact that might hold entry-grain citations, read
them yourself — the preview will not:

```
sqlite3 -readonly ~/.local/share/librarian/catalog.db \
  "select src_slug,src_local,dst_ref,origin from entry_cite
   where src_slug=(select slug from artifact where id='<id>')
      or dst_ref like '%<id>%'"
```

Prefer `doc(action="move")` where the intent is relocation. (`move` grafts, but see the
archived graft bug below — its own `entry_cite` cascade was a separate, now-fixed defect.)

## Resume

Add the two `entry_cite` counts at `src/librarian/tools/delete.rs:96`, reading the artifact's
`slug` (not `id`) from the row already fetched at the top of the dry-run block. Then fix the
`recoverable` string at :103 and the two test comments at :373 and :389. Check whether
`doc(action="graft")`'s and `doc(action="move")`'s own previews — if any — share the
enumeration, so the fix lands once per site rather than once per verb
(`CLAUDE.md` § *Testing Discipline*: mutate once per guarded SITE).

## References

- `src/librarian/tools/delete.rs` — the dry-run gate (:89), the import (:90), `cascades`
  (:96-102), `recoverable` (:103), the test and its comment (:373, :389).
- `src/librarian/catalog/mod.rs:200-208` — the `entry_cite` schema and its cascade.
- `src/librarian/catalog/entry_cite.rs` — `outgoing` (:81), `incoming` (:106),
  `prune_scan_rows` (:69), `cascade_delete_removes_entry_cite_when_artifact_deleted` (:296).
- `src/librarian/tools/link_scan/mod.rs:833` — `origin = ORIGIN_SCAN`, and the comment at
  :838 confirming a scan must not clobber `origin='write'` rows.
- `docs/issues/archive/2026-09-17-graft-cascade-deletes-the-source-ledgers-outgoing-entry-citations.md`
  — the sibling: there the cascade itself was the defect, here the **preview** is. Same
  table, same irreversibility argument, different call site.
- `docs/issues/2026-09-21-the-delete-preview-calls-an-untracked-file-git-restorable.md` — the
  other defect in this same preview, filed separately (see below).

## Cluster

`cluster/selector-narrower-than-its-population` (`IC-18`). The class claim is *"a selector …
is narrower than the population its name or its caller's intent implies. It runs to completion
over a subset and returns a well-formed answer, and because the excluded members were never
examined there is no count to report and nothing to mark"* — and the class's own sharpening
from `probe-enumerates-wire-fields-so-a-new-one-counts-as-zero` is this instance exactly: the
excluded member was introduced **after** the selector. Here there is not even a zero; the key
is simply absent, which reads as *no such casualty exists* rather than *never looked at*.

**Two classes checked and rejected, stated so the fit is falsifiable.**
`cluster/blast-radius-exceeds-visibility` (`IC-1`) is the tag the archived graft sibling
carries, but `IC-1` was split on 2026-09-01 down to `n=3` and its post-split claim is
specifically about *peer* listings scoped narrower than filesystem sharing; the graft file
post-dates that split and arguably inherited a pre-split tag.
`cluster/floor-published-under-the-name-of-a-total` (`IC-20`) requires the true value to be
**unknowable** because the walk stopped — here it is one SQL query away, so the remedy is a
wider enumeration rather than a rename.

## Why this is filed separately from the `recoverable` defect

Both live in the same `json!` block, seven lines apart, and both mislead the same caller at
the same moment — but they are different mechanisms with different repairs. This one is an
**omission** from a hand-enumerated list; its repair adds a field. The other is a **false
assertion** about state nothing queried; its repair either asks git or withdraws the claim.
Merging them would give one file two root causes and one cluster tag for two classes, and
would let fixing the cheap half read as closing the file. They cross-reference instead.

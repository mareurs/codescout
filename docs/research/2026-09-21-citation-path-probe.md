---
id: b2b08f9bf0a8c4db
kind: research
status: draft
title: 'Citation path probe: does an adjudication''s citation of a derived row resolve?'
tags:
- probe
- librarian
- citations
- adr-evidence
topic: tracker state durability and adjudication
---

**Valid:** dated 2026-09-21

Measurement probe, not an implementation. It exercises the one claim
[the recoverability ADR](../adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md)
rates as *"read from tool documentation and the module inventory, **not exercised end-to-end
against a derived row bearing a new id**"*:

> An adjudication written as a prose-ledger entry can cite a derived row bearing an allocated id,
> and that citation resolves to a queryable `cites` / `entry_cite` edge.

**Headline: the claim FAILS as written, in the direction the ADR needs.** A prose adjudication
cannot carry a write-time `cites` at all — the tool refuses it by name — and the surface that
*does* accept one (`resolve_cite_ref`) cannot see a prose entry as a target. The resolution path
that remains is the scanner, and its report-only form names a resolved citation at **artifact
grain only**. Entry grain exists on the write path and in an aggregate count; it is not reachable
by any read this probe was permitted to make.

Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`. Two throwaway artifacts were created, used and
deleted; neither was committed. Paths, fenced because they no longer exist and an unfenced
backticked path-shaped token reds `Audit Doc Refs`:

```
docs/research/2026-09-21-citation-probe-scratch.md          aad7cbb1c2e6ce1e   slug citation-path-probe-scratch
docs/research/2026-09-21-citation-probe-adjudications.md    acbb087a1aa4f850   slug citation-path-probe-adjudications-scratch
```

The first declared `entry_prefix: [DRV, ADJ]` and an augmentation with
`entry_collection: "derived"`; the second declared `entry_prefix: ADX` and no augmentation.

**The `DRV-N` / `ADJ-N` / `ADX-N` tokens in this file are inert, and the 16-hex ids are elided
outside fenced blocks.** The ledgers are deleted, so no artifact defines or declares those
prefixes; `link_scan` reports a prefix with no definer anywhere as prose noise rather than
dangling, and `doctor`'s `cited_prefix_with_no_definer` requires `MIN_FILES = 2`
(`src/librarian/tools/doctor.rs:5001`), which one file cannot meet. The artifact ids are a
different parser: written in full in prose they are real citations naming artifacts that no
longer exist, which `audit_doc_refs` rates `high` and CI reds on — measured on this very file,
see § *Incidental tool observations*. Full ids survive only inside fenced blocks, where
`code_block` caps them. Do not try to resolve any of it.

## Verdict table

| # | Property / control | Observed | Verdict |
|---|---|---|---|
| P1 | dual-home allocation | `append_entry` with `entry_collection` + `title`/`body`/`anchor_heading` + `index_row`/`index_after_line` wrote a params row, a `## DRV-N — <title>` section and an index-table row in one call, same id. Twice (DRV-1, DRV-2), plus DRV-3. | **HOLDS** |
| P2 | prose adjudication entry | `append_entry` omitting `entry_collection` with the three section fields wrote `## ADJ-1 — retraction of the close on DRV-1` itself. Response: `section_written: true`, `reserved: false`. | **HOLDS** |
| P3-A | write-time `cites` from the adjudication | Refused: *"append_entry: `cites` is not supported on a prose ledger"*. Structural, not a corpus accident — `src/librarian/tools/append_entry.rs:194-201`, pinned by `a_prose_ledger_refuses_cites` at `:949`. | **FAILS** |
| P3-A' | write-time `cites` *toward* a prose entry, from a params row | Refused: *"cite `citation-path-probe-scratch:ADJ-1` — no such entry"*, with the slug provably present in the catalog. `resolve_cite_ref` validates `<slug>:<local>` only against the destination's augmentation `entry_collection` (`src/librarian/catalog/augmentation.rs:1944-2014`). Both directions between the two species are therefore closed at write time. | **FAILS** |
| P3-A'' | write-time `cites` between two params rows | DRV-3 cited `citation-path-probe-scratch:DRV-1` and it materialized. So the write path works — just never with a prose entry at either end. | **HOLDS** |
| P3-B | report-only resolution of a prose citation | Cross-file: ADX-1's prose citation of `DRV-1` appears in `link_scan(write=false)`'s `edges_missing` as `src_id acbb087…f850 → dst_id aad7cbb…ce1e`. Intra-ledger: ADJ-1's citation of `DRV-1` appears in no finding array and moved `counts.entry_edges.attributed` 2049 → 2050. | **HOLDS** (resolution), see P4 for grain |
| P4 | the edge names the ENTRY | **Split by path.** Write path: `doc(get, include_links=true)` returned `entry_links.outgoing[{src_local: "DRV-3", dst_ref: "citation-path-probe-scratch:DRV-1"}]` — entry grain both ends. Scanner report-only: the sole per-edge listing is `edges_missing`, whose rows carry `src_id`/`dst_id`/`src`/`dst` and **no entry**; entry grain is a count (`counts.entry_edges.derived`). `doc(action="graph", depth=2)` returned `nodes:[self], edges:[]` — it reads `links::outgoing` only and is blind to `entry_cite` (`src/librarian/tools/graph.rs:46`). | **Mixed — see below** |
| C1 | dangling citation produces no edge and no partial write | 7 refused calls (1 prose-`cites` guard, 6 unresolvable refs). Next successful ids were DRV-3, DRV-4 and ADJ-1..ADJ-3 in unbroken sequence; `json_array_length(params,'$.derived')` = 4 with last id `DRV-4`. No id consumed, no params row, no `entry_cite` row. | **HOLDS** |
| C2 | a shapeless heading defines no token | A section whose heading is a source path, a dash and a title with **no** `PREFIX-N` token (exact text in § *Calls made*) was written for the reserved id ADJ-2. `link_scan` reported `{src_id: aad7cbb…ce1e, raw: "ADJ-2", kind: "EntryToken"}` in `dangling`. The ADR's diagnosis of the legibility Verdicts section is **correct**. | **HOLDS** |
| C2' | dangling bucket is not silent on this file | `DRV-9`, never allocated, also reported dangling. `dangling_by_source` gave the scratch file exactly `2`. | **HOLDS** |
| C3 | wrong-neighbour discrimination | The only `entry_cite` rows touching either artifact, read straight from the catalog, are `DRV-3 → citation-path-probe-scratch:DRV-1` and `DRV-3 → aad7cbb…ce1e`. `DRV-2` appears in no row. | **HOLDS** (write path) |
| C4 | durability precondition | Frontmatter carries `entry_high_water_ADJ: 3` and **no `entry_high_water_DRV`** after four DRV allocations. **Precondition, not property** — see below. | **FAILS for derived rows** |

## Every ref syntax tried — the reusable part

Write-time `cites` on an `append_entry` that supplied `entry_collection` (the only path that
accepts `cites` at all). Refusals wrote nothing and consumed no id.

| Ref written | Resolved? | Stored `dst_ref` | Note |
|---|---|---|---|
| `citation-path-probe-scratch:DRV-1` | **yes** | `citation-path-probe-scratch:DRV-1` | `<slug>:<local>`, local is a **params row**. Entry grain. |
| `aad7cbb…ce1e` | **yes** | `aad7cbb…ce1e` | 16-hex artifact id that exists. Artifact grain. |
| the scratch artifact's own rel_path | **yes** | `aad7cbb…ce1e` | rel_path, unique suffix match. Collapses onto the hex form — three refs produced **two** rows. |
| `citation-path-probe-scratch:ADJ-1` | no | — | *"no such entry"*. Local is a **prose** entry with a valid defining heading. The load-bearing refusal. |
| `citation-path-probe-scratch:DRV-999` | no | — | *"no such entry"*. Unknown local. |
| `2026-09-21-citation-probe-scratch:DRV-1` | no | — | *"no such entry"*. The qualifier is `artifact.slug`, **not** the file stem — the opposite of `link_scan`'s prose grammar, where the qualifier *is* the file stem. |
| `DRV-1` | no | — | *"did not resolve"*. A bare token has no colon, so it falls to the rel_path branch. |
| a 16-hex literal naming no artifact (`0123…cdef`, deliberately elided — written in full it is indistinguishable from a real citation and reds `Audit Doc Refs` at `high`, which is this repo's § *Parsers Over a Namespace* holding about the document that reports it) | no | — | *"did not resolve"*. Hex-shaped but no such artifact; falls through to the rel_path branch, so the error never mentions the id form. |

Three of the four refusals share one message — *"no such entry (slug or local id not found)"* —
for three different causes (unknown slug, unknown local, local-exists-but-is-prose). The message
cannot discriminate them, and this probe needed an out-of-band catalog read to tell the first
from the third.

In **prose** (`link_scan`), a bare `DRV-1` resolved, because `DRV` had exactly one definer. That
is the documented grammar and it behaved as documented.

## What contradicts the ADR

Prominently, because the ADR asked to be amended rather than defended.

**1. The joining citation cannot be written at write time, in either direction.** The ADR's
§ *Decision* says the two stores are *"joined by an explicit citation from the adjudication to the
derived row it judges"*. An adjudication is a prose-ledger entry, and `cites` is refused on a
prose ledger by name. Reversing the arrow does not help: a params row citing
`<slug>:<prose-entry-id>` is refused too, because `resolve_cite_ref` looks the local up in the
destination's `entry_collection`. So the join is available only through prose text plus
`link_scan` — which the ADR does list, but as one of *"two more pieces in place"* rather than as
the sole surviving mechanism.

**2. § *The mechanism this needs already exists* is true of the wrong allocator.** The ADR cites
`src/librarian/tools/append_entry.rs:294-324` for the three-input allocation and concludes *"id
allocation already survives a catalog loss, because its durable input is frontmatter"*. Those
lines sit **inside the prose branch** (`if a.entry_collection.is_none()`, opening at `:183`). The
params allocator is a different function — `src/librarian/catalog/augmentation.rs:770-772` —
and computes `params_next.max(body_max + 1)`. It never reads `entry_high_water_<PREFIX>` and never
writes one. Measured: after DRV-1..DRV-4, frontmatter held `entry_high_water_ADJ: 3` and no DRV
key at all. **Derived rows are exactly the species the ADR says must carry ledger ids, and they
are the species the durable mark does not cover.**

What *does* travel for a derived row is the body claim. The dual-home write (P1) puts the id in a
`## DRV-N — <title>` heading and an index row, both in git, and `body_max` reads them — so
allocation after a catalog loss would resume correctly *provided the section was written*. The
probe measured the failure of that proviso directly: a params-only append (DRV-4, no section, no
index row) came back with the tool's own diagnosis —

```
snapshot_hint: "... Entry rows live in the catalog, which is machine-local and git-ignored —
                a row absent from the body is in no repo."
undefined_in_body: "`DRV-4` has no `## DRV-4 — <title>` heading in the body, so any citation
                    of it would resolve to nothing — an index row does not define a token."
```

— and `DRV-4` appears nowhere in the committed body. Note the second line's parenthesis: for
*citability* the index row buys nothing, while for *allocation* it counts. The two surfaces are
not interchangeable and the ADR treats id-bearing-ness as one property.

**3. The consequence *"a consumer can ask which derived rows carry a judgment"* is not served by
any read tried here.** From the cited side, `doc(get, include_links=true)`'s
`entry_links.incoming` items are built as `{src: "<slug>:<local>", rel}` —
`src/librarian/tools/get.rs:405-409` drops `dst_ref`. So the derived ledger's own view showed two
incoming rows, both reading `citation-path-probe-scratch:DRV-3`, indistinguishable from each
other and naming neither `DRV-1` nor the artifact. The question is answerable from the *citing*
side's `outgoing`, or by SQL — not from the row that carries the judgment.

## P4, stated plainly

**Entry grain exists; artifact grain is what a permitted read returns.**

- Materialized by `cites` on a params append: entry grain on both ends, and
  `doc(get, include_links=true).entry_links.outgoing` shows it. `DRV-1` vs `DRV-2` discriminated
  (C3).
- Derived by the scanner in report-only mode: the resolved edge is named **only** at artifact
  grain (`edges_missing`). Entry grain appears as `counts.entry_edges.{attributed, derived}` —
  an aggregate over 1771 artifacts, which cannot verify a claim about a member. Materializing the
  entry-grain row needs `link_scan(write=true)`, forbidden to this probe because it prunes
  project-wide on a shared checkout.
- `doc(action="graph")`: no entry-grain edges at any depth, ever. It is not a slug or scan
  artefact; the traversal reads `links` only.

So the ADR's design is **not** artifact-grain-only — the mechanism reaches entry grain. But every
route to it runs through a params row, and a prose adjudication is not one.

## Calls made, with the fields that mattered

```
doc(create, rel_path=…scratch.md, kind=tracker, extra={entry_prefix:[DRV,ADJ]},
    augment={entry_collection:"derived", params:{derived:[]}, prompt:…})
  -> id aad7cbb1c2e6ce1e

doc(append_entry, id=…, entry_collection="derived", id_prefix="DRV",
    entry={target:"src/probe/alpha.rs",…}, title=…, body=…, anchor_heading="## End marker",
    index_row="| {id} | … |", index_after_line="|---|---|---|")
  -> {id: "DRV-1", section_written: true}          (then DRV-2 the same way)

doc(append_entry, id=…, id_prefix="ADJ", cites=["citation-path-probe-scratch:DRV-1"], …)
  -> REFUSED "cites is not supported on a prose ledger"

doc(append_entry, id=…, id_prefix="ADJ", title=…, body=<prose citing DRV-1>,
    anchor_heading="## End marker")
  -> {id: "ADJ-1", reserved: false, section_written: true,
      body_max: null, reserved_max: null, frontmatter_max: null}

doc(append_entry, id=…, id_prefix="ADJ")                       # reserve only
  -> {id: "ADJ-2", reserved: true, section_written: false,
      body_max: 1, reserved_max: 1, frontmatter_max: 1}
doc(update, id=…, patch={body_edits:[{heading:"## End marker", action:"insert_before",
    content:"## src/probe/gamma.rs — Verdict/close ✅ CLOSED 2026-09-21\n…"}]})

doc(append_entry, id=…, id_prefix="ADJ", title=…, body=<prose citing ADJ-2 and DRV-9>, …)
  -> {id: "ADJ-3", body_max: 1, reserved_max: 2, frontmatter_max: 2}
     # body_max stays 1: the shapeless heading claims nothing, and only the committed
     # mark and the local reservation stopped ADJ-2 being reissued.

doc(append_entry, id=…, entry_collection="derived", id_prefix="DRV",
    cites=["citation-path-probe-scratch:DRV-1", "aad7cbb1c2e6ce1e",
           "docs/research/2026-09-21-citation-probe-scratch.md"], + section + index_row)
  -> {id: "DRV-3", section_written: true}          # 3 refs -> 2 entry_cite rows

doc(append_entry, id=…, entry_collection="derived", id_prefix="DRV", entry={…})
  -> {id: "DRV-4", section_written: false, snapshot_missing: ["DRV-4"],
      snapshot_hint: …, undefined_in_body: …}
librarian(link_scan, write=false, findings_limit=1000)   x3 (baseline, +ADJ, +ADX)
sqlite3 -readonly ~/.local/share/librarian/catalog.db  (slug, entry_cite rows, params count)
```

`link_scan` deltas across the three report-only runs:

| | baseline | after ADJ-1/ADJ-3 | after ADX-1 |
|---|---|---|---|
| `artifacts_scanned` | 1770 | 1770 | 1771 |
| `entry_edges.attributed` | 2049 | 2050 | 2051 |
| `entry_edges.derived` | 2994 | 2995 | 2996 |
| `edges_desired` | 2928 | 2928 | 2929 |
| `edges_missing` | 192 | 192 | 193 |
| `dangling` | 681 | 683 | 683 |
| `entry_edges.written` | 0 | 0 | 0 |

`written: 0` on every run is the receipt that no scanner-derived edge was ever materialized.

## Incidental tool observations

Noticed while running the probe; neither is what the probe was for, and neither has a bug file
because this brief's write boundary allowed only this document.

- **`doc(action="delete")`'s dry-run cascade omits `entry_cite`.** Both dry runs reported
  `links_out: 0, links_in: 0` for the scratch artifact while two `entry_cite` rows keyed on its
  slug existed. `links_*` counts artifact-grain `links` only, so the field that looks like "what
  citations am I about to drop?" is silent about the entry-grain ones. The two rows *were* gone
  after the forced delete, so the cascade works; it is the preview that under-reports.
- **The dry run's `recoverable` line is a constant, not a check.** It read *"the file is
  git-tracked and restorable"* for two files that were untracked and therefore not restorable at
  all. The sentence is right for the common case and unconditionally asserted.
- **Three distinct `resolve_cite_ref` failures share one message.** Recorded in the ref-syntax
  table above; it cost this probe an out-of-band catalog read to tell "unknown slug" from
  "local exists but is prose".
- **This document tripped `Audit Doc Refs` at `high` on its own probe input** — the fake 16-hex
  id, written as a literal in a table cell, is indistinguishable from a real citation. Caught by
  running `librarian(action="audit_doc_refs", emit_tracker=false)` against this file before
  committing, not by re-reading it. Zero `high` after the elision; the two remaining `med`
  findings are fenced fixture paths.

## What this probe did NOT establish

- **No scanner-derived edge was materialized.** Every `entry_cite` row observed had
  `origin = 'write'`. `link_scan(write=true)` was out of bounds, so the report-only arm's
  resolution is evidence that the scanner *would* derive an edge, not that it does.
- **The intra-ledger resolution rests on an aggregate and an absence.** ADJ-1's citation of
  `DRV-1` is a self-cite at file grain, so it is named nowhere; the evidence is `attributed`
  +1 and non-membership in `dangling`/`ambiguous`. The cross-file case (ADX-1) is the one with
  positive naming.
- **C4 is a precondition, not the property.** Reading the frontmatter establishes that the ADJ
  namespace carries a committed mark and the DRV namespace does not. It does not test survival:
  that needs an isolated catalog, a rebuild, and a re-allocation, none of which are safe on a
  shared catalog.
- **The count deltas above are not controlled for peers.** This is a shared checkout with other
  live sessions, and the three runs are minutes apart. Per-source arrays
  (`dangling_by_source` = 2 for the scratch file) and direct SQL are the readings that do not
  depend on project totals holding still; treat the delta table as corroboration only.
- **Nothing about key→id stability under a real migration**, which is the ADR's other
  *"reasoned, not tested"* item. Untouched.
- **Nothing cross-repo, nothing from a worktree, no archived-definer tie-break, no
  `rekey_prefix` over a dual-homed row.** All of those have their own guards and this probe
  tripped none of them.
- **One corpus, one pair of artifacts, one prefix per namespace.** Where a number is quoted it is
  a reading from this run, not a rate.

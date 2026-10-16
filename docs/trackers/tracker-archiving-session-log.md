---
id: '65a9afa30d1a58e3'
kind: tracker
status: active
title: Session Log — Tracker Archiving
tags:
- session-log
- tracker-archiving
topic: Tracker archiving surface exploration and design
entry_prefix:
- F
- W
snapshot_anchor: '| ID | Date | Severity | Category | Status | Title |'
entry_high_water_F: 2
entry_high_water_W: 2
---

# Session Log — Tracker Archiving

Adapted from [the session-log template](../templates/session-log.md). Scope: explore a technique that both shrinks ongoing ledgers and retires completed trackers while preserving their history. No archive implementation has been selected.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-10-08 | med | documentation | promoted-to-bug-tracker | Taxonomy still prescribes quarterly companions after the archive policy narrowed the rule |
| F-2 | 2026-10-08 | med | documentation | promoted-to-bug-tracker | The serving retirement recipe permits dropping the headings that define citable entries |
| W-2 | 2026-10-09 | — | architectural | validated | Augmentation shape sidecars cannot serve as the durable archived-row payload |

## Wins

Win entries use W-N and the same heading format. This log's headings define the citable entries.

## Status vocabulary

Frictions: open, mitigated, fixed-verified, wontfix-false-alarm, promoted-to-bug-tracker, pinned-as-eval-baseline.
Wins: validated, promoted-to-permanent-docs, archived.
Every entry declares Status and Valid; preserve defining headings during archival.

## F-1 — Taxonomy still prescribes quarterly companions after the archive policy narrowed the rule

**Observed:** 2026-10-08. Read docs/TAXONOMY.md, Archive cadence, and doc(get) for the ratified archive-cadence-policy sections. The taxonomy prescribes time-partitioned entry archives; the ratified amendment prescribes one stable companion per ledger.

**Expected:** One entry-level destination rule across the live instruction surfaces.

**Got:** A future archive pass can select a different companion recipe depending on which instruction surface it enters.

**Severity:** med — an archive operator must reconcile contradictory recipes before writing.

**Status:** promoted-to-bug-tracker

**Valid:** dated 2026-10-08

**Rests on:** the two live instruction surfaces read during this scout; no new archive operation was executed.

**Fix idea / Pointer:** [Dedicated issue](../issues/2026-10-08-taxonomy-archive-cadence-conflicts-with-ratified-policy.md).

## F-2 — The serving retirement recipe permits dropping the headings that define citable entries

**Observed:** 2026-10-08. Read D10 step 4 in the serving tracker-hygiene skill, the tracker-conventions archive ladder, and the citation resolver. D10 specifies tables plus an outcomes digest, with prose only in git; the guide requires archived definition headings to survive.

**Expected:** Compaction preserves indexed definitions and full archived evidence.

**Got:** The documentation permits a retirement whose remaining body no longer defines its entry tokens. The runtime consequence is inferred from resolve and its existing test, not newly measured by applying a retirement.

**Severity:** med — an archive operator must reconcile contradictory recipes before writing.

**Status:** promoted-to-bug-tracker

**Valid:** dated 2026-10-08

**Rests on:** the two live instruction surfaces read during this scout; no new archive operation was executed.

**Fix idea / Pointer:** [Dedicated issue](../issues/2026-10-08-tracker-hygiene-compaction-drops-citable-entry-headings.md).

## Design brief — 2026-10-09

The user selected hybrid explicit and automatic invocation, covering both shrinking ongoing ledgers and retiring completed trackers. They suggested a codescout guide/prompt with configurable cleanup aggressiveness, and asked to build the consistency scaffolding first.

A [draft scaffolding design](../superpowers/specs/2026-10-09-tracker-archiving-scaffolding-design.md) separates selection policy from archive preservation. It proposes ledger-family metadata, a preview/apply service, conserved section/row transfer, recoverable writes and in-place retirement for the first slice. The proposal is awaiting design review; code implementation has not started.

## W-1 — Scout distinguishes parsed entry text from raw bytes before choosing an archive payload

**Observed:** 2026-10-09, before writing the scaffolding draft. Read entry_sections in src/librarian/tools/link_scan/extract.rs. Its section text is assembled with lines().join("\n"), which is a parsed representation rather than an original byte slice.

**Pattern:** Reuse the shared fence-aware section boundaries, but derive raw source byte slices for conservation. Do not use EntrySection.text as the preserved evidence payload.

**Counterfactual:** An archive executor built from that text field would normalize line endings while presenting the transfer as byte-preserving. This is inferred from the function body; an archive executor has not been applied.

**Impact:** The draft now requires LF/CRLF payload fixtures and exact preimage comparisons.

**Promote-when:** The archive executor's tests confirm raw payload preservation across LF and CRLF fixtures.

**Status:** validated

**Valid:** dated 2026-10-09

**Rests on:** the inspected shared entry_sections implementation and its lines().join("\n") projection; code-inspected, not a new runtime archive measurement.

## W-2 — Augmentation shape sidecars cannot serve as the durable archived-row payload

**Observed:** The draft promises to preserve complete structured entry rows and keep archived rows filterable. `src/librarian/augmentation_sidecar.rs` explicitly excludes params: `impl AugmentationSidecar/to_row` reconstructs `params: "{}"`, while `write_through` projects shape fields only. `AugmentationRow` holds params as catalog JSON. This is an existing deliberate storage contract, not a newly discovered implementation bug.

**Pattern:** Separate committed archive evidence from machine-local catalog projections. Name the durable representation and fresh-catalog restore path for archived rows before supporting augmented entry transfer.

**Counterfactual:** Reusing only the existing augmentation sidecar could pass a same-catalog row-equality check while leaving archived structured fields absent after catalog reconstruction. The source inspection prevents promising that the existing sidecar carries those fields.

**Evidence boundary:** Code-inspected storage semantics and a design gap. No archive implementation or runtime catalog-loss experiment exists yet; no successful recovery is claimed.

**Status:** validated

**Valid:** dated 2026-10-09

**Promote-when:** An isolated archive fixture rebuilds a fresh catalog and observes every transferred structured field and the archived entry-filter result from committed payloads.

## Template for new entries

Append frictions through doc(action="append_entry", id_prefix="F", index_row="| {id} | <date> | <severity> | <category> | <status> | <title> |") with this tracker's id, this heading as anchor, title and body together. Use id_prefix="W" for wins. F-N and W-N rows use index_row with the declared snapshot_anchor. Do not allocate identifiers by hand.

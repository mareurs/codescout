---
id: d66562ed420391a8
kind: adr
status: draft
title: Tracker state splits by recoverability, not by shape
tags:
- adr
- librarian
- trackers
- augmentation
- deep-agent
topic: tracker state durability and adjudication
---

**Valid:** dated 2026-09-21

## Decision

A tracker's mutable state splits by **recoverability**, not by shape.

- **Derived state** — rows a tool recomputes mechanically (measurements, scan results, counts) — stays in catalog `params`: machine-local, gitignored, disposable, re-derivable on demand.
- **Adjudications** — irreproducible judgments made by a party about a specific thing (a verdict, a retraction, a sufficiency report) — live in the artifact **body**, as id-bearing prose-ledger entries, which are in git.
- The two are joined by an **explicit citation** from the adjudication to the derived row it judges. Derived rows therefore carry ledger ids (`PREFIX-N`), not only natural keys.

## Context

`params` does not travel, by design: `src/librarian/augmentation_sidecar.rs:10-13` states it is "live state that churns, and committing it would recreate the params-vs-body drift class BL-29 / BL-40 / BL-42 closed." That reasoning is correct and is not being reversed here.

But the decision it produced sorted state by **churn**, and the property that matters is **recoverability**. Those come apart, and inside one `params` blob today sit two species with opposite cost-if-lost:

| | derived state | adjudication |
|---|---|---|
| written by | a scan, mechanically | a party, by judgment |
| reproducible | yes — re-run the scan | **no, ever** |
| cost if lost | one scan run | irreplaceable |
| durability today | gitignored, wholesale-replaceable | **identical** |

Three measured incidents, one mechanism:

1. **T-N, 2026-08-16.** One `doc(action="augment", merge=true, params={observations:[…]})` took the ledger from 19 entries to 1. Those were `verdict` judgments — adjudications in the disposable store.
2. **legibility-backlog, 2026-06-13.** Retiring the `name_collision` detector (ADR `2026-06-13-drop-name-collision-defect`, `919dbe5c`) auto-closed 19 rows. The Verdicts prose records that 7 of them "closed because the **detector was removed, not because the code was refactored**". Nothing in the collection separates those 7 from the 12 genuine relocations — same `status`, same `defects`, same `closed_at` — and `render_template.j2` files all of them under one heading. The adjudication had no structured home, so it went to prose, where no reader can join it. **Every automated reader of those params sees 19 clean closes.**
3. **legibility-backlog, observed 2026-09-21.** Catalog `params` hold 17 open / 25 closed at `last_scan_at: 2026-06-15`; the committed body, machine-rendered from those same fields, reads `Scanned 2026-08-28 · 47 open` with an empty Closed table. Filed as `787345ce75805909`. Both values are correct for their own frame and each is published under a name stating the other's.

Scale: **15 of 31 augmented trackers** in this project declare an `entry_collection`, i.e. keep their entries in `params`. The remaining 16 are prose ledgers, whose entries are body sections and have lost nothing.

**And this is why the consumer question had no answer.** A session-level *"my context was insufficient"* is an adjudication. Three candidate hosts were examined on 2026-09-21 and all three failed for what is one reason: `legibility_scan` keys rows on `friction_target` (`src/legibility/mod.rs:248-285`) and a keyless annotation has no row; the T-N ledger's `params_schema` constrains `verdict` to `enum ["legitimate","debatable","wrong-tool",null]` and validates on every merge, so a `sufficient` verdict is **refused at write time**; `pika_observations` has no rendering consumer at all. The search was for a consumer of an object that had nowhere durable to be stored.

## The mechanism — partly present, and the missing half is measured

A probe on 2026-09-21 exercised this path end to end: `docs/research/2026-09-21-citation-path-probe.md`. **Three claims in this ADR's first revision were wrong.** The Decision above is unaffected, because it rests on the three incidents rather than on the mechanism — but the mechanism it named was partly the wrong one.

**What holds.** Dual-home allocation works: one `append_entry` carrying both `entry_collection` and `index_row` writes the params row, a `## DRV-N — <title>` section and the index row under one allocated id, in one write. A prose adjudication in a second prefix namespace allocates independently. A citation of a non-existent id is refused with no partial write and no id consumed. A citation of `DRV-1` produces an edge to `DRV-1` and not to its neighbour, so the mechanism does reach entry grain.

**Durability for a derived row comes from the committed BODY, not from frontmatter — which makes `index_row` load-bearing rather than optional.** The first revision cited `append_entry.rs:294-324`; that code sits **inside the prose branch** (opens at `:183`) and reads a committed frontmatter high-water mark. Params rows take a different allocator: `augmentation.rs:770-772` computes `params_next.max(body_max + 1)`, where `body_max` is the greatest id **the committed body claims**, and it never touches `entry_high_water_*`. Measured after four `DRV` allocations: `entry_high_water_ADJ: 3`, and **no `DRV` key at all**.

So a params-only append leaves the id in no committed surface, and the id↔key binding dies with the catalog — the exact failure this design exists to prevent. Passing `index_row` puts the binding in git and feeds `body_max`, which is what stops a lost id being reissued. **`index_row` is therefore a requirement of this decision, not a convenience.** The allocator already carries a warning for the state it guards: `body_max + 1 > params_next` reports that params is missing rows the body documents.

**The write-time join does not exist, in either direction.** `cites` is refused on a prose ledger (`append_entry.rs:194-201`), and reversing the arrow fails too, because `resolve_cite_ref` validates a `<slug>:<local>` ref against the *destination's* `entry_collection` — which a prose ledger does not have. The refusal's own hint prescribes the remedy: *"Reserve the id, write the body, and cite in prose — link_scan derives the edges from the text."* **So prose + `link_scan` is the sole mechanism, not one of two.** Convenient in one respect — it is also the retrofit path for the existing Verdicts section, so new adjudications and retrofits share one code path. Costly in another: the join is **derived rather than written**, so it exists only after a scan that operates project-wide and prunes.

**Working ref syntax, since this is the reusable part.** `<slug>:<local>` resolves at entry grain when `<local>` names a **params** row. A 16-hex artifact id resolves at artifact grain; a unique `rel_path` also resolves at artifact grain and collapses onto the hex form. Refused: `<slug>:<prose-entry-id>`; `<file-stem>:<TOKEN>` — the qualifier is `artifact.slug`, **not** the file stem, which is the opposite of `link_scan`'s prose grammar; a bare token; a hex-shaped id naming nothing. In prose, a bare `DRV-1` resolves.

**Two reachability gaps, both small, both real.** `doc(action="get", include_links=true)` elides `dst_ref` from `entry_links.incoming` (`get.rs:405-409`), so from the cited side a derived row cannot name which adjudication cited it — the query § *Consequences* promised is unserved today. And `doc(action="graph")` reads `links::outgoing` only (`graph.rs:46`) and is blind to `entry_cite`, so graph traversal does not see these edges at all.

**The probe did confirm this ADR's diagnosis of the legibility Verdicts section.** A heading carrying a dash and a title but no `PREFIX-N` defines no token, and a citation of it dangles — control C2, held.

## Alternatives considered

- **Commit `params` to git.** Rejected. It churns per scan, so every scan becomes a diff; it reopens the drift class the sidecar closed; and it does not touch the real blast radius, since one `merge=true` still overwrites the collection.
- **Adjudications as catalog events** (`event_create`, `kind="verdict"`, `resolves_intent_event_id`). **Better semantics than the chosen design** — immutable, timestamped, git-anchored via `head_commit`, and the intent→verdict loop is already modelled. Rejected on durability only: the sole sidecar directory is `docs/augmentations/` (`SIDECAR_DIR`, `augmentation_sidecar.rs:31`), `catalog.db` is machine-local and gitignored, and no event export path was found. **Not established** that events cannot travel — only that no mechanism for it was located. This is the alternative that wins if events gain a sidecar.
- **A new `sufficiency` collection in `params`.** Rejected: it returns the irreplaceable thing to the disposable store. This is incident 1's shape with a new field name.
- **Keep adjudications in free prose** (status quo). Rejected: measured invisible at incident 2.
- **Cite the derived row by its natural key as a literal string** (the cheap variant of this decision, considered and declined by the repo owner on 2026-09-21). Needs no id migration and works immediately, but no gate can verify the citation — which reproduces `CLAUDE.md` § *Parsers Over a Namespace*: a string that looks like a reference and that nothing resolves. That is what the Verdicts section already has.

## Consequences

**Now easier.** An adjudication survives a catalog loss, a machine move, and a wholesale `params` write. The citation is a materialized edge, so a consumer can ask which derived rows carry a judgment instead of re-reading prose. A sufficiency annotation has a host that does not require a symbol key. And a reader who sees a closed row can reach the reason it closed.

**Now harder.** Two stores, so a writer must decide which side a row belongs on — and that decision is exactly what the current design avoids asking, so it has to be stated somewhere a writer reads. Derived rows need ids they do not have today: a migration across the 15 params-backed trackers, each of which must keep its key→id binding stable through the first allocation or every later citation is meaningless. A prose entry is heavier to write than a `params` merge, which will bias writers toward calling a judgment "derived". And an id-bearing derived row is a new citable token per row, which widens the namespace `link_scan` and `rekey_prefix` operate over.

**Added by the 2026-09-21 probe, and these are the costs a first revision could not see.** Because the write-time join does not exist, the join is derived: it materializes only when `link_scan` runs, and that scan is project-wide and **prunes**, so on a shared checkout it is not a step a session can take casually. `index_row` becomes mandatory rather than optional, so every derived-row write is a body write — heavier, and it puts derived rows into the committed diff, which is part of what committing `params` was rejected for. Two reader-side gaps need closing before the promised query works: `entry_links.incoming` elides `dst_ref`, and `doc(action="graph")` cannot see `entry_cite` edges at all.

## Change scenarios absorbed

- **A detector is retired.** (2026-06-13.) The retirement becomes an adjudication citing the rows it closed; a consumer reading closes finds the citation.
- **A catalog is lost or rebuilt on another machine.** (Measured twice: 22 augmentations gone on a 437-commit pull; incident 3.) Adjudications survive in git; derived state is re-derived, which is correct because it is derivable.
- **Someone writes `augment(merge=true, params=…)`.** (2026-08-16.) Blast radius is derived state only.
- **A sufficiency annotation needs a home.** The open question this decision was reached in service of.

## Revisit-when

- Catalog events gain a sidecar, or any other durable on-disk form → prefer the event design; its semantics are better and only durability disqualified it.
- A third species appears that is neither mechanically derived nor a party's judgment.
- A writer population is observed classifying judgments as derived to avoid the heavier write path — that is the predicted failure mode of this boundary and it should be looked for rather than assumed absent.

## Confidence

**Medium-high on the boundary**, unchanged. It rests on three measured incidents sharing one mechanism, plus the split between 15 params-backed and 16 prose-backed trackers in which only the former have lost data. The 2026-09-21 probe did not test the boundary and does not bear on it.

**Medium on the mechanism — the same rating as the first revision, for the opposite reason.** It was medium because the path had only been read; it is now exercised, and the exercise falsified three claims. Measured: allocation, refusal behaviour, neighbour discrimination, entry-grain edges on the write path, and the absence of a params high-water mark. **Not established:** no scanner-derived edge was ever materialized — `link_scan` ran report-only by design, because `write=true` prunes project-wide on a shared checkout — so the surviving mechanism's *write* half is inferred from its report rather than observed. And control C4 established the **precondition** for surviving a catalog loss, never the property; that needs an isolated catalog, which the probe deliberately did not create.

## What this does not decide

No implementation. `CLAUDE.md` § *Deep-agent observation window* defers implementation to after **2026-10-02**, with a review on **25 September**, in favour of evidence capture; this ADR is a decision record and does not authorize the migration. It also names no field for the sufficiency annotation itself — the annotation's schema stays downstream of this boundary, which is the ordering the consumer investigation established.

Nor does it decide the retrofit order. The legibility Verdicts section is the cheapest first case — it is already a prose ledger missing only its ids — but whether it or the T-N ledger goes first is an implementation question for after the window.

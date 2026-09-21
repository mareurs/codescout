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

## The mechanism this needs already exists

This is the finding that makes the decision cheap rather than a rewrite. `doc(action="append_entry")` allocates from **three** inputs (`src/librarian/tools/append_entry.rs:294-324`): `frontmatter_max` (a committed high-water mark, in git), `body_max` (ids the live body claims), and `reserved_max` (this machine's reservation table). The code's own diagnostic names the case where the committed frontmatter mark leads both others and calls it "expected … a fresh clone".

So **id allocation already survives a catalog loss**, because its durable input is frontmatter. That is precisely the property the citation target needs: an adjudication citing `LB-7` must still find `LB-7` after a machine move. Had the high-water mark lived only in `params`, a rebuild would re-mint different ids for the same keys and silently re-point every citation.

Two more pieces are in place. `link_scan` derives `rel="cites"` edges from prose citations and materializes them, so *"which closes were retracted?"* becomes a query rather than a re-reading. And `append_entry`'s doc at `:22-29` states the shape that defines a citable token: a heading missing its dash-and-title "defines no token under `link_scan`'s `def_re`, and every citation of the entry dangles."

**That last line diagnoses incident 2 exactly.** The legibility Verdicts section is already a prose ledger in every respect but one — one `###` per adjudication, authored prose, a stable subject — and it lacks only an allocated id. Its headings read `### src/lsp/manager.rs — LspManager/get_or_start ✅ CLOSED 2026-06-13`: a dash, a title, no `PREFIX-N`. So it defines no token, and the retraction it carries is unreachable by construction rather than by neglect.

## Alternatives considered

- **Commit `params` to git.** Rejected. It churns per scan, so every scan becomes a diff; it reopens the drift class the sidecar closed; and it does not touch the real blast radius, since one `merge=true` still overwrites the collection.
- **Adjudications as catalog events** (`event_create`, `kind="verdict"`, `resolves_intent_event_id`). **Better semantics than the chosen design** — immutable, timestamped, git-anchored via `head_commit`, and the intent→verdict loop is already modelled. Rejected on durability only: the sole sidecar directory is `docs/augmentations/` (`SIDECAR_DIR`, `augmentation_sidecar.rs:31`), `catalog.db` is machine-local and gitignored, and no event export path was found. **Not established** that events cannot travel — only that no mechanism for it was located. This is the alternative that wins if events gain a sidecar.
- **A new `sufficiency` collection in `params`.** Rejected: it returns the irreplaceable thing to the disposable store. This is incident 1's shape with a new field name.
- **Keep adjudications in free prose** (status quo). Rejected: measured invisible at incident 2.
- **Cite the derived row by its natural key as a literal string** (the cheap variant of this decision, considered and declined by the repo owner on 2026-09-21). Needs no id migration and works immediately, but no gate can verify the citation — which reproduces `CLAUDE.md` § *Parsers Over a Namespace*: a string that looks like a reference and that nothing resolves. That is what the Verdicts section already has.

## Consequences

**Now easier.** An adjudication survives a catalog loss, a machine move, and a wholesale `params` write. The citation is a materialized edge, so a consumer can ask which derived rows carry a judgment instead of re-reading prose. A sufficiency annotation has a host that does not require a symbol key. And a reader who sees a closed row can reach the reason it closed.

**Now harder.** Two stores, so a writer must decide which side a row belongs on — and that decision is exactly what the current design avoids asking, so it has to be stated somewhere a writer reads. Derived rows need ids they do not have today: a migration across the 15 params-backed trackers, each of which must keep its key→id binding stable through the first allocation or every later citation is meaningless. A prose entry is heavier to write than a `params` merge, which will bias writers toward calling a judgment "derived". And an id-bearing derived row is a new citable token per row, which widens the namespace `link_scan` and `rekey_prefix` operate over.

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

**Medium-high on the boundary**, which rests on three measured incidents sharing one mechanism, and on the existing split between 15 params-backed and 16 prose-backed trackers where only the former have lost data.

**Medium on the mechanism.** The allocator's durability was verified at `append_entry.rs:294-324`; the citation resolution path (`link_scan`, `entry_cite`) was read from tool documentation and the module inventory, **not exercised end-to-end against a derived row bearing a new id**. The key→id stability requirement under a first migration is reasoned, not tested.

## What this does not decide

No implementation. `CLAUDE.md` § *Deep-agent observation window* defers implementation to after **2026-10-02**, with a review on **25 September**, in favour of evidence capture; this ADR is a decision record and does not authorize the migration. It also names no field for the sufficiency annotation itself — the annotation's schema stays downstream of this boundary, which is the ordering the consumer investigation established.

Nor does it decide the retrofit order. The legibility Verdicts section is the cheapest first case — it is already a prose ledger missing only its ids — but whether it or the T-N ledger goes first is an implementation question for after the window.

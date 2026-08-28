---
id: '341871a6eb347bc4'
kind: tracker
status: active
title: Resume queue — Statement Validity Layers 3c/5b (SV-N)
owners:
- marius
tags:
- resume-queue
- statements
- validity
- librarian
- attestation
topic: statement validity
entry_high_water_SV: 6
entry_prefix: SV
---

# Resume queue — Statement Validity Layers 3c/5b (SV-N)

Buildable work left in the Statements work stream after Layers 1–2 shipped
(`docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md`).

**Spec:** `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`
(artifact `189c6c3db794d1d7`) — read its `## Sequencing` table first; it is the
authority on intent, this file is the authority on what is left.

**Sibling, not a duplicate:** `docs/trackers/statement-validity-session-log.md`
(`cf9cdcc0cd91ef1e`) holds the F/W observations *from building* Layers 1–2. This
file holds the work *not yet built*. Do not merge them.

## How to use this queue

**To act:** scan the `## SV-N` headings — each carries a `**Status:**` line, and
there is no index table to fall out of sync. Take an `open` entry, **verify its
evidence against current code before starting** — every claim here carries the
command that produced it, and each is dated. A claim that no longer reproduces is
itself the finding; correct the entry rather than working from it.

**To append:** one call, from the main checkout —

```
artifact(action="append_entry", id="<this artifact's id>", id_prefix="SV",
         anchor_heading="## Template for new entries", title=…, body=…)
```

The server allocates the id, writes `## SV-N — <title>` (the only shape
`link_scan` reads as a definition), and advances `entry_high_water_SV` in
frontmatter. Never hand-allocate.

**Deliberately unaugmented.** No `params`, no `entry_collection`, no
`render_template`. Entries are body sections. Rationale in
`docs/conventions/cross-machine-catalog-resume.md` — augmentation is
machine-local and git-ignored, so a queue meant to be picked up by another
session or another machine must carry its state in committed markdown.

## Provenance

Opened 2026-08-28 from a full-surface sweep for partially-implemented designs.
Every `**Valid:** dated 2026-08-28` line below was re-verified at the bytes that
day — the spec's own status column was written 2026-08-20/21 and several of its
claims needed re-checking, which is why they are restated here with commands.

## SV-1 — `asserted_at` never shipped, so a refutation cannot close an interval

**Status:** open
**Valid:** dated 2026-08-28
**Rests on:** spec § *Layer 1 — Bitemporal storage: two clocks, never one*

**Observed.** `grep 'asserted_at' src/**/*.rs` → **zero matches** (2026-08-28).

Bitemporal storage is designed-only. The spec's *two clocks, never one* rule —
separating when a claim was asserted from when it was true — has no
representation in the catalog, so there is nowhere to record that a claim held
over an interval and stopped holding at a point.

**Consequence:** a refutation cannot close an interval; it can only overwrite.
The spec is explicit that a refutation must close rather than reset or
overwrite (§ *A refutation closes an interval*), and that behaviour is
unreachable today.

**Next:** decide whether Layer 5b storage lands as one unit (SV-3) or whether
`asserted_at` lands alone first as the smaller, independently-useful half.

## SV-2 — `resolve_validity` has zero production callers, so default-is-decay holds on paper only

**Status:** open
**Valid:** dated 2026-08-28
**Rests on:** spec Decision 3 (absence of a `**Valid:**` declaration means decay, not exemption)

**Observed.** `resolve_validity` is defined at `src/librarian/statements.rs:214`.
Every reference to it is a test (`:432`, `:436`, `:466`) or a doc comment
(`src/librarian/tools/doctor.rs:2437-2438`). No production path calls it.

The `Default` clock the function needs — last commit touching the heading's line
range — is unimplemented. The undeclared population is routed to
`entry_cited_from_outside_but_undeclared` instead, which is a *worklist*, not the
decay semantics Decision 3 describes.

**Do not "fix" this by wiring `resolve_validity` into `entry_dated_stale`.** The
doc comment at `src/librarian/tools/doctor.rs:2437` is a deliberate carve-out: that check must not
guess an undeclared entry's age, which is exactly what `resolve_validity` would
make it do. Whatever calls `resolve_validity` first has to be a *new* consumer.

**Prior art:** `docs/issues/archive/2026-08-20-validity-spec-terminology-contradicts-decision-3.md`.

**Next:** name the first production consumer. Without one this function is dead
code with a test suite, and Decision 3 is a doc claim the code contradicts.

## SV-3 — Layer 5b storage is designed-only; only the read-only tap shipped

**Status:** open
**Valid:** dated 2026-08-28

**Observed.** `entry_attestation` and `condition_event` → **zero matches** in
`src/` (2026-08-28). What shipped 2026-08-21 is the *tap*: read-only, no new
tables, proof-carrying enforced at read.

Missing: the two tables, appraisal coalescing, and the bitemporal `asserted_at`
from SV-1.

**Why this is the load-bearing one.** The spec states it directly: *"Layer 5b is
what turns the rest from bookkeeping into a feedback loop, and its absence is
already measured at 4086 artifacts reporting `freshness: unknown`."* Layers 1–4
record and surface; nothing yet closes the loop by recording that a human or
agent actually appraised a claim and what they concluded.

**Next:** this is a spec-sized piece of work, not a task. It needs its own plan
under `docs/superpowers/plans/`. Start from spec §§ *Storage*, *The tap*,
*What proof means, by class*, and *Proof-carrying, and the counter resets on the
appraisal — not on the assertion*.

## SV-4 — Layer 3c `rests-on` edges: deliberately not building

**Status:** deferred — do not pick this up without re-measuring first
**Valid:** conditional — resolvable `**Rests on:**` declarations reach ~20 corpus-wide

Re-measured 2026-08-21: **one** resolvable declaration corpus-wide, and its edge
already exists as a `cites` edge. Most `**Rests on:**` lines in the corpus are
fenced examples inside the spec itself, `docs/templates/session-log.md`, and the
manual page — the field is parsed, and nothing consumes it yet.

Building the edge type now would add a rel with a population of one.

**Next:** re-run the count before doing anything. If it is still in single
digits, leave this closed and update the date on this entry.

## SV-5 — Layer 5a read-leak closure: retired, with a named reopen trigger

**Status:** retired 2026-08-21
**Valid:** conditional — the spec §6 event trigger fires

Closing the buffer-slice and `grep` read-attribution leaks was retired rather
than deferred: the leak measures ~4 entry-grain reads per 30h in every era, and
Layer 4's `context(anchor_id="<slug>:<local>")` names the entry in the call
input, so the growth path arrives pre-attributed.

Recorded so it is not re-raised as an oversight. Reopen only on the spec's own
trigger.

## SV-6 — Exposure is one term, not `max(reads, in-degree)`

**Status:** open — low priority
**Valid:** dated 2026-08-28

Layer 2's exposure gate is **cross-file citation in-degree only**. The spec
designed `max(reads, in-degree)`; the read counters were still leaking (SV-5)
when Layer 2 shipped, so `link_scan`'s in-degree was used alone.

This degrades by being **smaller**, not by breaking: an entry that is heavily
read but rarely cited under-reports its exposure and stays off the worklist. No
false positives are possible from this.

Two properties added beyond the design and worth preserving through any change:
the reported worklist is scoped to the active project while the metric stays
cross-repo, and rows filtered out that way are counted in
`catalog_health.entry_validity_scoped_by_project` rather than dropped silently.

**Next:** blocked on a read counter worth reading. Revisit with SV-5's trigger.

## Template for new entries

```
## SV-N — <one-line title>

**Status:** open | in-progress | done | deferred | retired
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>
**Rests on:** <ADR, decision, or principle that outlives the code>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened

Seeded with SV-1..SV-6 from a full-surface partial-implementation sweep.
`asserted_at`, `entry_attestation`, `condition_event` and `resolve_validity`'s
caller set were all re-verified at the bytes that day rather than taken from the
spec's status column.

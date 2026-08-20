---
kind: spec
status: active
title: Entry Validity + Attestation — declared decay, an entry-grain graph, and proof-carrying verification
owners: []
tags:
  - librarian
  - graph
  - trackers
  - validity
  - attestation
---

# Entry Validity + Attestation

**Goal:** give a tracker *entry* a declared decay class, populate the entry-grain
citation graph that already exists but is empty, serve entries as graph
neighbourhoods, and make verification something the system asks for rather than
something nobody ever does.

**Brainstormed 2026-08-20** with Marius. Five decisions were taken interactively and
are recorded in *Design decisions* below; the rest of this document is downstream of
them.

**Relationship to CAP-8 (the "gram").** CAP-8 proposes content-addressed entry
identity — `hash(title + description)` — and is explicitly *not* a dependency here.
This spec attaches validity to the entry as it is addressed **today**
(`<slug>:<local>`, shipped by the Stage-2 entry graph). If CAP-8 later replaces that
addressing, validity travels with it unchanged, because nothing here reads the id's
internal structure. Decoupling was decision 1: CAP-8 is a catalog-wide migration whose
own entry says it should land after CAP-7 and that two prior questions must be settled
before any code, and blocking a small measured win behind it is the failure mode this
project calls the rewrite trap.

---

## Motivation, measured

Every figure in this section was measured on **2026-08-20** against the live catalog at
`/home/marius/.local/share/librarian/catalog.db` and the project usage DB at
`.codescout/usage.db`, on `experiments` at `85e9b2da` (re-confirmed at `cdd2f9dd`).
Each is stated with the predicate that produced it, per CLAUDE.md § *Measurement —
Never State a Count Your Instrument Did Not Measure*.

### 1. Records go false, and nothing notices

Four entries were found this session whose disposition was accurate when written and
false within days, none noticed for weeks:

| Entry | Text that aged | Ledger |
|---|---|---|
| `F-3` | "after plan edit lands this turn" | `bug-fix-session-log` |
| `F-17` | "planned" | `bug-fix-session-log` |
| `F-44` | "should be updated" | `bug-fix-session-log` |
| `R-19` | "Formal sync flow … still pending a commit" | `reconnaissance-patterns` |

`R-19` is the sharpest case: it is genuinely promoted and back-cited from the served
`SKILL.md`, while its own last sentence still says the work is pending. **A disposition
field should record what *is*; intent belongs in a `Fix idea` / `Next` line where
nothing reads it as state.** Nothing in the system distinguishes a claim that is true
forever from one that was true on a Tuesday.

### 2. An artifact-level twin already exists, and two other repos use it

`SELECT time_scope, COUNT(*) FROM artifact WHERE time_scope IS NOT NULL GROUP BY 1`
returns **484** non-null rows across 4087 artifacts:

| Value | Count | Origin |
|---|---|---|
| `dated:<YYYY-MM-DD>` | 261 | hand-written (one is the literal unfilled placeholder) |
| `open-ended` | 161 | hand-written |
| `dated_snapshot` | 55 | classifier (`src/librarian/classify.rs:115,193` — two path globs) |
| `2026-Q2`, `point-in-time`, `2026-07`, four bare dates | 7 | hand-written strays |

By repo root: `work/stefanini` 160 `dated:` + 36 `open-ended`; `work/mirela` 123
`open-ended` + 99 `dated:`; `work/claude` (which contains codescout) 30
`dated_snapshot` + 2 `dated:` + 2 `open-ended` + 5 strays.

**Two unrelated repos converged independently on exactly the binary this spec
proposes** — `open-ended` (invariant) versus `dated:<date>` (temporary). Nobody specced
it. That validates the vocabulary and localises the problem: the codescout tree is the
one that never adopted its own field, not the idea that failed.

The 7 strays and the unfilled `dated:<YYYY-MM-DD>` literal are the cost of a free-text
field with no parse. The entry-level field must not repeat that.

*Cross-check:* an independent `GROUP BY value, repo` sums to the same 484
(261 + 161 + 55 + 7). Two GROUP BYs agreeing is the calibration; neither figure is
hand-arithmetic.

*Instrument note:* the first attempt at this count used
`filter={"time_scope": {"ne": null}}` and returned 0. `LeafOp::Ne` compiles to SQL `!=`
(`src/librarian/filter.rs:55`) and `x != NULL` is never true. A positive control on a
field known to be populated (`topic`) also returned 0, which is what exposed the broken
predicate. See decision 3 — this is a substrate constraint, not just an anecdote.

### 3. The entry graph is built, correct, and empty

```
entry_cite rows ............   13   (all origin='write')
artifact_link rows .........  2789
artifacts with a slug ......     2   tool-usage-patterns, open-issue-work-queue-bl-n
artifacts total ............  4087
```

This is structural, not neglect. `entry_cite`'s **only** writer is
`append_entry(cites=…)`, which requires an augmented ledger declaring an
`entry_collection` and is refused from a worktree checkout. The prose ledgers where
every failure in §1 lives never take that path. `artifact_link` has 2789 rows because a
*scanner* fills it and `entry_cite` has 13 because only a hand-passed argument does.

> **Correction to an existing source comment.** `src/librarian/tools/doctor.rs:1631`
> justifies recomputing citations from files with *"That table is materialized only by
> `link_scan(write=true)`"*. `link_scan` has **zero** references to `entry_cite`
> (`grep "entry_cite|EntryCiteRow" src/librarian/tools/link_scan/**` → 0 matches), and
> every live row carries `origin='write'`. The decision the comment defends is correct;
> its stated reason is false, and it is exactly the premise one needs to reason about
> backfilling. Filed as
> `docs/issues/2026-08-20-doctor-comment-misnames-entry-cite-writer.md`.

### 4. Verification is a first-class concept that has happened once

`SELECT kind, COUNT(*) FROM events GROUP BY kind`:

```
field_patch ....... 2507
note .................. 7
worktree_fork ......... 5
worktree_merge ........ 2
external_signal ....... 1
intent ................ 1
reviewed .............. 1     <- one, on one artifact
verdict ............... 0
status_change ......... 0
superseded_by ......... 0
```

`reviewed` is in the `events` table's `CHECK` constraint and is the sole input to
`Freshness`. Reading `src/librarian/freshness.rs:36`:

```rust
let Some(reviewed_at) = input.latest_reviewed_at else { return Freshness::Unknown; };
```

One artifact carries a `reviewed` event and zero carry `superseded_by`, so **4086 of
4087 artifacts report `freshness: unknown`** — a field surfaced on every
`artifact(action="get")` response that is structurally incapable of saying anything
else.

**2507 `field_patch` events and 1 `reviewed`.** The system records every write and has
essentially never recorded a verification. That asymmetry is not a schema gap — the
schema has both. Writing is forced by the act of working; verifying is forced by
nothing. The same mechanism explains §2: `time_scope` reached 484 because agents write
frontmatter, and `reviewed` reached 1 because nothing ever asks.

### 5. Read counts are already derivable

`.codescout/usage.db` for this project holds **29,739** `tool_calls` rows with
`input_json`, `output_json`, `session_id` and `cc_session_id` retained (`output_json`
non-null on 29,496). Of 2794 `artifact` calls, **2210 carry an `id` in `input_json`**.

*Scope caveat:* that is this repo's usage DB only. Four `usage.db` files were found
under the codescout tree alone, and each project root has its own — so 2210 is a floor
for one repo and says nothing about the corpus. It establishes feasibility, not volume.

---

## Design decisions (brainstorm 2026-08-20)

1. **Validity attaches to the ENTRY**, not to the gram and not to the edge. Ships
   without CAP-8. Aimed at the four-datapoint class in §1. Accepted limitation: it
   describes the claim, not the claim's bindings — see *Non-goals*.
2. **The field carries a class plus a prose condition, adjudicated by an agent** — not
   a machine-runnable predicate for every entry. Most conditions in this corpus
   ("after the plan edit lands", "one more cluster", "pending a commit") are not
   expressible as a shell predicate, and demanding one would produce fakes. Selection
   is syntactic and cheap; judgement is the reader's. This is the D11 shape, whose own
   confidence column reads *"low by design"* for the same reason.
3. **Absence means decay, and a doctor check finds what absence hides.** An entry with
   no `**Valid:**` line MEANS `dated <its last commit>`. Write cost for the common case
   is zero and authors only write the line to *upgrade*. This is also forced by a
   substrate fact: the filter AST has ops `eq, ne, in, nin, gt, lt, gte, lte, contains,
   prefix` and **no null/exists op** (`src/librarian/filter.rs:36-48`), and `ne`
   compiles to SQL `!=` (`:55`), so `field != NULL` is never true — absence is not
   directly queryable. A non-null default sidesteps that entirely.
4. **The graph is populated by backfill from prose, then packed entry-grain.**
   `link_scan::extract` already produces what is needed; see Layer 3.
5. **Attestation taps the Nth reader with a deferred, recorded obligation** — serve in
   full, enqueue the proof, and record the obligation when it is not discharged.

---

## Non-goals

- **Any check that answers "is this entry promoted?"** Measured 2026-08-20: a
  promotion, an eval-fixture list and a kin reference are syntactically identical —
  `(R-41 in codescout's docs/trackers/…)`, `(R-2, R-4, R-8, R-10, R-19, R-23)`,
  `(R-87 is the same law's *hit* …)`. `grep -c '<id>'` counts any mention; using it as
  a promotion predicate mislabelled three of five entries in commit `9a982ed5`, and a
  narrowed regex was also wrong. Two successive predicates disagreed with each other
  and with reading. **This direction stays human.** Layer 2 ships the *inverse* check
  instead — "cited from outside, but undeclared" — which is a real signal without
  claiming to know why.
- **CAP-8 / content-addressed identity.** Independent; see the header note.
- **Re-keying artifact ids** (`sha256(abs_path)` → stored). Out of scope for the same
  reason the Stage-2 spec put it out of scope.
- **Backfilling the `**Valid:**` field itself** across ~4000 existing entries. Decision
  3 makes that unnecessary: history reads as `dated` by default, and Layer 2 surfaces
  only the entries where the default is wrong *and* costly.

---

## Layer 1 — the `**Valid:**` field

### Grammar

A line inside the entry's section, sibling to `**Status:**`:

```
**Valid:** invariant
**Valid:** dated 2026-08-20
**Valid:** conditional — until the jsonpath plan edit lands
```

Exactly three classes. Each maps to a distinct sweep action, which is the test for
whether a fourth is earned:

| Class | Means | Artifact-level equivalent |
|---|---|---|
| `invariant` | A law. No expiry. What gets promoted. | `time_scope: open-ended` (161 artifacts) |
| `dated <date>` | True of an instant. Every measured count. | `time_scope: dated:<date>` (261 artifacts) |
| `conditional — <event>` | True until a named event fires. | none — this is the gap |

### Detection

Anchor on **line-start structure**, never on a keyword.
`get_guide("tracker-conventions")` § *Detecting these fields* records the reason:
prose and field share a vocabulary by construction, so `grep -c 'Status:'` also counts
sentences *about* Status, and both mistakes were made in one pass by one agent.

The parser matches `^\*\*Valid:\*\*\s+` at line start within the entry's section
bounds, where section bounds run from the entry's defining heading (`link_scan`'s
`def_re` shape) to the next heading at the same or higher level.

### Validation

`dated` MUST be followed by an ISO `YYYY-MM-DD`. A non-date is a `RecoverableError`
naming the three valid forms. This is the one rule the artifact-level field lacks, and
there is a live artifact carrying the literal string `dated:<YYYY-MM-DD>` to show what
its absence costs.

`conditional` MUST be followed by a separator and non-empty text. A bare `conditional`
is refused: a condition nobody named can only ever produce "go re-read this", which is
the nudge that left 34 of 61 entries status-less for three months.

### Default

An entry with no `**Valid:**` line reads as `dated <the last commit touching its
heading's line range>`. **Not the file's mtime and not the file's last commit** — a
ledger is appended to constantly, so a file-grain date would refresh every entry every
time any entry changed, which reads green in exactly the broken world this is meant to
catch.

*Implementation risk, unmeasured:* per-entry blame over ~4000 entries has an unknown
cost. Measure before choosing between (a) `git blame -L <start>,<end>` per entry,
(b) one `git log -p` pass per ledger with line attribution derived once, or (c) caching
the resolved date on the attestation row (Layer 5) and recomputing only when the file's
mtime moves. Option (c) is the likely answer but must not be assumed — it is a cache
whose invalidation is exactly the thing under test.

### Server-side stamping (new entries)

`augmentation::PendingSection` (`src/librarian/catalog/augmentation.rs:826-839`) is
written by `allocate_entry_id` (`:909`) in the **same file write and transaction** as
the `entry_high_water_<PREFIX>` mark, and `src/librarian/tools/append_entry.rs:144`
passes it. Its doc comment states the constraint that makes this safe:

> *"A caller that wrote the section afterwards would do a second read-modify-write
> outside that transaction, so a peer session allocating on the same file in between
> gets clobbered — and what gets clobbered is the peer's committed mark, walking the
> counter BACKWARDS."*

So the allocator — and only the allocator — appends `**Valid:** dated <today>` to
`PendingSection.body` when the caller supplies no class. New entries are born with a
declared class the same way they are born with a `def_re`-conformant heading: by
construction, not by convention. Callers may pass a class explicitly; they may not
format the line themselves, for the same reason CAP-5 stopped them formatting headings.

This does **not** cover hand-written entries. Decision 3 covers those.

---

## Layer 2 — three doctor checks

Shaped after the three checks CAP-7 shipped on 2026-08-19, so the surface, the
per-check JSON report and the read-only guarantee are unchanged.

| Check | Fires when | Confidence |
|---|---|---|
| `entry_conditional_past_due` | An entry declared `conditional` whose heading's last commit is older than the horizon | **low by design** — selection is syntactic, judgement is the reader's |
| `entry_dated_stale` | An entry declared (or defaulted to) `dated` past the horizon, **ranked by incoming citations** | low |
| `entry_cited_from_outside_but_undeclared` | An entry cited from a different ledger that carries no explicit `**Valid:**` line | high — purely syntactic once Layer 3 lands |

**The horizon is one configurable, in days, defaulting to 30, and it is a guess.** It
is deliberately *not* `FRESHNESS_HORIZON_DEFAULT` (`src/librarian/freshness.rs:30`),
which is a commit distance of 50 whose own doc comment says every call site passes
`topo_distance_from_head: None`, so "the constant is effectively unused in v1". Reusing
a number that has never been exercised would import an untested calibration. 30 days is
chosen because the verify-open cadence in `CLAUDE.md` already uses 14 days for
`Status: open` entries and a decay horizon should be looser than a triage one; re-tune
against the first month's output rather than defending the initial value.

**Ranking `entry_dated_stale` by incoming citations is load-bearing, not a nicety.** A
decayed fact nothing cites costs nothing; one cited from a promoted skill costs a lot.
An unranked list of every `dated` entry past a horizon is ~4000 rows and will be
ignored, which is the same outcome as not shipping it.

`entry_cited_from_outside_but_undeclared` is the check that would have surfaced `R-41`
and `R-42` — genuinely promoted, declared nowhere. It deliberately reports *"this is
load-bearing and undeclared"* and not *"this is promoted"*, per Non-goals.

Before Layer 3 lands, this third check runs against citations recomputed by
`link_scan::extract` rather than against `entry_cite`, matching what `doctor` already
does at `doctor.rs:1629` and for the reason that comment *should* have given.

**Every check reports a worklist, never a verdict.** The spec says so explicitly so no
downstream reader infers automation that is not there.

---

## Layer 3 — backfill the entry graph

### Attribution

`link_scan::extract` (`src/librarian/tools/link_scan/extract.rs:128`) returns:

```rust
pub struct DocExtract {
    pub definitions: Vec<Definition>,   // Definition { token: String, line: u32 }
    pub citations:   Vec<Citation>,     // Citation   { raw: String, kind: CitationKind, line: u32 }
    pub declared_prefixes: Vec<String>,
}
```

**Both definitions and citations carry a line number.** Entry-grain attribution is
therefore one comparison over data that already exists:

```
src_local(citation) = the definition with the greatest line ≤ citation.line
```

A citation above the first definition belongs to no entry and is dropped — it is
file-grain prose (a preamble, an index table), and `artifact_link` already covers it.

### Resolution and materialization

Destination resolution uses `resolve::resolve`
(`src/librarian/tools/link_scan/resolve.rs:213`) unchanged. Its `Outcome` (`:193-204`)
is:

```rust
pub enum Outcome {
    Edge { dst_id: String },
    SelfCite,
    Ambiguous { candidates: Vec<String>, total: usize },
    Dangling,
    CrossRepo,
}
```

**Only `Edge` becomes a row.** `Ambiguous`, `Dangling`, `CrossRepo` and `SelfCite` stay
reported and are never guessed, preserving the existing tie-break in
`DefinerRef { artifact_id, active }` (`resolve.rs:15-21`) where archived definers lose
to active ones.

`Outcome::Edge.dst_id` is an **artifact id** — file grain. The entry-grain destination
is assembled from that plus the citation's own `raw` token; this is assembly, not new
resolution logic.

Rows are written to the existing `entry_cite` table with `origin='scan'`. The `origin`
column exists today as a forward-compat placeholder that MVP only ever writes as
`'write'`, so this is the use it was reserved for. Scanner-owned rows are pruned and
re-materialized per scan; `origin='write'` rows are never touched by the scan.

### Prerequisite: slugs at scale

`entry_cite.src_slug` FKs `artifact(slug)`, and **2 of 4087 artifacts have one**
(minted lazily on first `append_entry`). The backfill needs bulk minting from
`slugify(title)` with the existing numeric-suffix dedup, honouring the existing
immutability rule: once non-null in the catalog the slug never changes, a differing
frontmatter slug does not overwrite it, and a colliding one is rejected and logged.

**This is the layer with blast radius outside this feature.** `merge_worktree` and the
worktree overlay both key on slugs, and minting 4000 of them changes what those see.
Bulk minting must be its own reviewed change with its own tests, sequenced before the
materializer.

### Sizing caveat

CAP-8 reports 6321 cross-file entry citations at 43% resolving / 33% ambiguous / 24%
dangling. **CAP-8 itself flags these as upper bounds**, contaminated because
`link_scan` has no "mention" mode: a token written to *teach* citation syntax is
extracted identically to one written to cite, and teaching examples land preferentially
in the ambiguous and dangling buckets. Sample the real resolvable yield on this corpus
before sizing the work. Sizing off the published number is precisely CLAUDE.md's
Measurement clause 1 — a proxy reported as the target.

---

## Layer 4 — serve grams as context

`librarian(action="context")` gains an entry-grain anchor. When `anchor_id` is of the
form `<slug>:<local>`:

- walk `entry_cite` outward and inward — `entry_cite::outgoing`, `::incoming`,
  `::incoming_like` all exist (`src/librarian/catalog/entry_cite.rs`), and
  `artifact(action="get", include_links=true)` already surfaces them as `entry_links`
  (`src/librarian/tools/get.rs:202-209`);
- annotate every packed node with its validity class and, where Layer 5 is live, its
  read/verify counts;
- keep the existing anchor reserve unchanged — `src/librarian/tools/context.rs:346`
  reserves half the char budget for the anchor whenever it has neighbours, added after
  `docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md`.

**Flag, never suppress.** A fired `conditional` or a refuted attestation is packed
*with its flag*, not dropped. Dropping would make the packer lie by omission, which is
the failure this whole design exists to catch — and it would make a decayed record
indistinguishable from an absent one, which is strictly less information than today.

File-grain anchors keep walking `artifact_link` exactly as now. This is an added mode,
not a replacement.

---

## Layer 5 — attestation

### Storage

A separate slug-keyed table, following the precedent the Stage-2 design established and
defended: `events` is keyed on `artifact_id` with no entry column, and
`artifact_link.dst_id` FKs `artifact(id)` which is move-fragile.

```sql
CREATE TABLE IF NOT EXISTS entry_attestation (
  src_slug         TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local        TEXT NOT NULL,
  reads            INTEGER NOT NULL DEFAULT 0,   -- session-deduped
  verifies         INTEGER NOT NULL DEFAULT 0,
  last_verified_at INTEGER,
  last_verdict     TEXT,                         -- 'held' | 'refuted' | 'inconclusive'
  obligations_open INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (src_slug, src_local)
);
```

`ON DELETE CASCADE` off `artifact(slug)` is move-stable, which `artifact(id)` is not.

### Counting reads

**Session-deduped.** `tool_calls` carries `session_id` and `cc_session_id`, so "5 reads"
means five distinct sessions. Without dedup a single agent re-reading a ledger while
working discharges the counter against itself, and the count stops meaning leverage.

A read is *an entry being served into a context* — the Layer 4 packer and an explicit
`artifact(action="get")` both count. That matches what the mechanism is for: the tap
fires on entries agents actually internalise.

### The tap

**The threshold is 5 distinct sessions since the last passing attestation**, and like
the Layer 2 horizon it is a guess with no measurement behind it. It is a *floor*, not a
period: an entry verified at read 5 arms again at read 10. Two properties make the
initial value low-stakes — a missed tap costs nothing but a louder banner (see *Making
a deferred obligation stick*), and the counter is per-entry rather than per-ledger, so
a busy ledger does not tap on every entry at once. Re-tune from the first month's
`obligations_open` distribution.

Past the threshold, `librarian(action="context")` serves the entry **in full**, with a
banner, and returns a `pending_attestations` array alongside the pack:

```
R-89 [invariant] · read 5/5 · last verified: never
…entry served in full…

pending_attestations: [
  { entry: "reconnaissance-patterns:R-89",
    proof: "counterexample search",
    due:   "before end of turn" } ]
```

This uses the channel `append_entry` already uses for `snapshot_missing` and
`undefined_in_body` — response fields whose whole purpose is to tell an agent something
it was about to miss.

### What proof means, by class

The validity class supplies the proof obligation lazily, at the one moment an agent is
already holding the entry:

| Class | Discharge |
|---|---|
| `invariant` | A counterexample search — "name a case where this fails." A tap on an invariant is a real test, because the entry claims never to expire. |
| `dated <date>` | Re-run the measurement, record the new figure. The recheck command is written **here**, once, by the reader who needed it — not demanded from every author up front. |
| `conditional — <event>` | Check whether the event fired. Binary. |

This is why decision 2 could safely decline a machine-runnable predicate at write time:
the predicate is only ever demanded from entries that have proven load-bearing.

### Proof-carrying, or it does not count

**A `reviewed` event whose payload names no instrument does not reset the counter.**
Without this the mechanism is a laundering machine: the claim acquires a verification
stamp it did not earn and reads as *more* trustworthy than before the tap — strictly
worse than no mechanism, and precisely CLAUDE.md's described failure (a cheap proxy
standing in for the target, returning a plausible value and no error). CLAUDE.md states
the general form: *"a review that reports findings without observed mutation pass/fail
counts has not verified anything."*

Required payload fields: `instrument` (what was run — command, query, or the named
reasoning step for a counterexample search), `observed` (what it returned, verbatim),
`verdict` (`held` | `refuted` | `inconclusive`). Any missing → the event is recorded as
a `note`, not a `reviewed`, and the counter does not move.

### A refutation is not a reset

`last_verdict = 'refuted'` stops the entry being served as fact and routes it to the
same worklist a fired `conditional` lands on. **Verification that fails is the
highest-value output this mechanism produces** and must never be recorded identically
to one that passes.

`inconclusive` also does not reset the counter — it records that someone looked and
could not tell, which is information, and leaves the tap armed.

### Making a deferred obligation stick

Decision 5 chose deferral over a gate. The honest weakness is that a suggestion which
evaporates at end of turn *is* the mechanism that got `reviewed` to 1. Three properties
convert it into a measurable suggestion:

1. **An un-discharged obligation is recorded** (`obligations_open`) and reported by
   `doctor`. It accumulates visibly instead of evaporating.
2. **The read counter does not reset on a missed tap.** Reader 5/5 becomes 12/5 and the
   banner gets louder on its own.
3. **Escalation becomes evidence-driven.** If deferral does not work, the
   open-obligation count will say so — and today no such number exists.

This is the move `unverified:` made on bug files, where the guide states the principle:
*"The system's scarce resource is not candour; it is legibility."* 14 of 16
terminal-but-unarchived bugs already stated their blocker in prose; the record was
merely unqueryable.

Escalating to a hard gate is a **future decision with a named trigger**, not a
follow-on assumed here: revisit if open obligations exceed discharged ones by an order
of magnitude over a month of real use.

---

## Testing

Per CLAUDE.md § *Subagent Dispatch*, every review of this work applies candidate
mutations and reports the **observed** surviving count — a coverage argument is not a
verification.

**Layer 1**
- A body with two entries and a `**Valid:**` line in the second: parsed onto the
  second, not the first. The off-by-one at a heading line is the load-bearing case.
- `**Valid:** dated notadate` → `RecoverableError` naming the three forms.
- `**Valid:** conditional` with no condition → refused.
- A line reading `the **Valid:** field is required` in prose does **not** parse as a
  declaration (structure, not keyword).
- Default: an entry with no line resolves to its *heading's* last commit, and a
  sibling entry edited later in the same file does not move it.
- Allocator: `append_entry` with no class stamps `dated <today>`; with a class, stamps
  that; the caller never formats the line.

**Layer 2**
- Each check against a fixture where the other two are clean — a fixture that trips all
  three proves none of them individually.
- `entry_dated_stale` ordering is by incoming citations, asserted on the returned order
  and not merely on membership.

**Layer 3**
- Attribution: citation on the same line as a heading, immediately before the next
  heading, and above the first definition (dropped).
- `Ambiguous` / `Dangling` / `CrossRepo` / `SelfCite` produce **no** row.
- A re-scan is idempotent: `origin='scan'` rows are replaced, `origin='write'` rows
  survive byte-identical.
- Slug bulk-mint: a frontmatter slug colliding with a stored slug is rejected and
  logged, not silently duplicated.

**Layer 4**
- An entry-grain anchor packs neighbours; a file-grain anchor's behaviour is unchanged
  (pinned against the current output).
- A fired `conditional` neighbour appears in the pack **with** its flag — assert
  presence, not absence, because the bug this guards is suppression.
- The half-budget anchor reserve still applies.

**Layer 5**
- Two reads in one session increment `reads` by 1.
- A `reviewed` event missing `instrument` / `observed` / `verdict` does not increment
  `verifies` and does not clear `obligations_open`.
- `refuted` sets `last_verdict` and does **not** reset the read counter.
- A missed obligation increments `obligations_open` and the entry's next banner shows
  the higher read count.

---

## Sequencing

| # | Layer | Ships on | Status |
|---|---|---|---|
| 1 | `**Valid:**` field, default-is-decay, allocator stamping | — | **scheduled** |
| 2 | Three doctor checks | 1 | **scheduled** |
| 3 | Slug bulk-mint, then `origin='scan'` materializer | — | designed, not scheduled |
| 4 | Entry-grain `context` anchor | 3 | designed, not scheduled |
| 5 | `entry_attestation`, taps, proof-carrying `reviewed` | 3, 4 | designed, not scheduled |

Layers 1–2 deliver the entire measured win in §1 and touch no graph. Layer 5 is what
turns the rest from bookkeeping into a feedback loop, and its absence is already
measured at 4086 artifacts reporting `freshness: unknown`.

---

## Risks

- **Layer 3's sizing rests on a contaminated figure.** CAP-8's 43%-resolve is a
  self-declared upper bound. Measure the real yield first.
- **`conditional` adjudication is agent-judged** and always will be. Every surface must
  say "worklist", never "verdict".
- **Bulk slug minting touches `merge_worktree` and the worktree overlay.** Its own
  change, its own review.
- **Per-entry blame cost is unmeasured.** Decision 3's default depends on it; three
  options are named in Layer 1 and the choice is deferred to measurement.
- **The tap could train perfunctory discharge.** Proof-carrying payloads are the
  control; if `verifies` climbs while `refuted` stays at zero, that is the tell, and it
  is worth watching from the first week rather than discovering later.

## Prior art

An external research pass was commissioned 2026-08-20 and its findings are **not yet
folded in**; this section will be reconciled against it before Layer 1 is implemented.
What follows is the *internal* prior art, read from
`memory("research/agent-memory-frameworks")` (research dated 2026-05-25, exploratory,
no spec or plan ever committed).

### An invariant this design must not break

**codescout is a passive embedder.** `Embedder` / `RemoteEmbedder::openai` are
embeddings-only; there is no generative or chat client anywhere in the tree. The
2026-05-25 pass identified this as the fork that kills otherwise-elegant designs: its
Approach B (a native distillation pipeline modelled on TencentDB Agent Memory) mapped
almost 1:1 onto librarian rows and was rejected anyway, because distillation requires
an in-server LLM and that is an identity break.

Layer 5 honours the invariant by construction. codescout never adjudicates anything: it
counts reads, serves the entry with a banner, emits an obligation, and records what
comes back. **The host agent does every judgement.** That is exactly Approach C —
*host-driven scaffolding: keep codescout passive, the host LLM does generation via new
tools* — which the pass recommended and which nothing has yet built on.

### This design is the missing axis

That pass mapped codescout against the survey taxonomy in *Memory for Autonomous LLM
Agents: Mechanisms, Evaluation, and Emerging Frontiers* (cited there as arXiv
2603.07670; **not re-verified for this spec**), whose three orthogonal axes are
temporal scope, substrate, and **control policy**. Its conclusion:

> codescout already spans ALL 3 substrate types … The ONLY real gap is **Axis 3 —
> control policy**: every write is hand-authored (prompted self-control); no automated
> consolidation or reflection.

Layers 2 and 5 are a control policy — the first one in the system. That is the frame to
evaluate this design in, and it is a stronger claim than "add a field": the substrates
have been complete for months and nothing decides *when* to act on them.

### The same field was already recommended, and never built

Approach C's third piece, verbatim from the 2026-05-25 pass:

> Add `valid_until` / `superseded_by` to `ArtifactRow` → Zep-style temporal +
> Supermemory-style forgetting, reusing the `HIDDEN_STATUSES` filter.

**87 days later nothing shipped**, and the recommendation itself went unharvested — an
instance of the exact failure §1 of this spec describes, one level up. Two differences
from what is proposed here, both deliberate:

- That proposal is **artifact-grain** (`ArtifactRow`). This spec is entry-grain, because
  §1's four failures are all *entries* inside files nobody would call stale.
- It is a **hard expiry** (`valid_until`) plus a *hiding* mechanism (`HIDDEN_STATUSES`).
  This spec declines both: a class-plus-condition instead of a timestamp (decision 2),
  and flag-never-suppress instead of hiding (Layer 4). Hiding a decayed record makes it
  indistinguishable from an absent one, which is strictly less information than today.

### External systems already named internally

Worth checking the commissioned research against, rather than rediscovering:

- **Zep / Graphiti** — temporal knowledge graph with explicit *fact validity windows*.
  The closest named prior art to Layers 1 and 4 combined.
- **Supermemory** — MCP-native, explicit forgetting/expiry, explicitly targets coding
  agents.
- **Letta / MemGPT, Mem0, Cognee, LangMem** — other points on the substrate axis; none
  noted as modelling validity rather than recency or importance.

*Benchmark caveat carried from that pass:* the headline numbers quoted for these
systems come from different benchmark families (agentic vs. LoCoMo / LongMemEval) and
are **not comparable to each other**. The same pass also records that standard memory
benchmarks are shallow — 85–94% of their questions need evidence from only two
sessions — so none of them measures the property this spec is about.
## References

- `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md` — Stage 2:
  slugs, `<slug>:<local>` ids, `entry_cite`, write-time `cites`
- `docs/trackers/capability-proposals.md` — CAP-5 (shipped), CAP-7 (shipped), CAP-8
  (the gram, proposed)
- `docs/issues/2026-08-20-doctor-comment-misnames-entry-cite-writer.md` — the comment
  correction found while writing this
- `src/librarian/freshness.rs` — the artifact-grain ancestor of Layer 5
- `src/librarian/tools/link_scan/extract.rs`, `.../resolve.rs` — the backfill substrate
- `src/librarian/catalog/augmentation.rs:826` — `PendingSection`, the prose write path
- `get_guide("tracker-conventions")` § *Required fields*, § *Detecting these fields*

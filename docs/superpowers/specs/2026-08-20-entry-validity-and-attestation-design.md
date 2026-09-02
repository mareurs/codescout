---
kind: spec
status: active
title: Statements — validity, provenance, and attestation for tracker claims
tags:
- librarian
- graph
- trackers
- validity
- attestation
- statements
- bitemporal
---

# Statements — Validity, Provenance, and Attestation

**Goal:** give every claim a tracker entry asserts — a **Statement** — a declared decay
class and a durable route to its own proof, populate the entry-grain
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
> `docs/issues/2026-08-20-doctor-comment-misnames-entry-cite-writer.md` (archived, fixed 2026-08-21 —
> by then `link_scan(write=true)` had gained a real write path to `entry_cite`, so the fix ended up
> correcting the comment in the opposite direction from what this measurement would suggest; see the
> archived bug file's 2026-08-21 correction note).

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

### 6. The read path leaks, and the leak is the progressive-disclosure buffer

Measured 2026-08-20 against this repo's `.codescout/usage.db`. Each row names what the
predicate literally counts.

| Read path | Volume | Entry-grain attributable today? |
|---|---|---|
| `artifact(get)` **with** a `heading` | 264 | yes, exactly |
| `artifact(get)` naming no heading | 457 (63% of 721) | no — reads every entry at once |
| `read_markdown` on a **ledger** | refused | already closed (see below) |
| `read_file` on an `@` handle | 669 — 317 line-range, 304 `json_path`, 48 whole | **no** — untracked |
| `grep` content on a guarded ledger | ≤373 calls name a tracker/issue path | **no** — untracked |

**The guard is already correct and already scoped.** `read_markdown` on
`docs/trackers/prompt-surface-compaction-session-log.md` — a ledger with **no**
augmentation row — is refused with the short form of the message, i.e. the
`entry_prefix` reason firing rather than the augmentation reason. All **12** ledgers in
`docs/trackers/*.md` that declare a namespace are read-guarded; the other 84 trackers
are not, correctly, because they own no `PREFIX-N` namespace and therefore carry no
Statement to count. (96 trackers total; 21 augmented, 75 plain.)

**The buffer is the leak.** `artifact(get)` on a large ledger overflows to a `@tool_*`
handle, and slices read off that handle are untracked. Observed in this session's own
transcript, on this spec's own source material:

```
artifact(get, id=01291679…)            → @tool_1c73a348    counted, no grain
read_file(@tool_1c73a348, $.body)      → @file_1c73b79a    untracked
read_file(@file_1c73b79a, lines 1-100)                     untracked, entry-grain
```

One instrumented call, three untracked reads, two handle-hops deep. The mechanism that
protects the context window is the one that destroys the read signal.

**`grep` bypasses the guard.** A `grep(mode="content")` against
`docs/trackers/capability-proposals.md` — whose `read_markdown` is refused — returned 12
matches with full entry text and line numbers. Observed directly this session.

**Consequence for the design, and it is a correction.** On
`reconnaissance-patterns.md`, the ledger holding all nine promoted entries: **53**
instrumented `artifact(get)` calls and **54 successful** direct `read_markdown` reads
(plus 6 refused). About half the historical read traffic never touched the instrument —
before counting the buffer or `grep` at all.

So the entry-grain read distribution measured from `artifact(get)` alone — 232 distinct
`(artifact, heading)` pairs ever read, maximum **3** for any single entry
(`R-90`), with everything above 3 being a navigation heading like `## Index` — is a
**floor from a leaky instrument**, not a fact about how much entries are read. An
earlier draft of this spec concluded from it that read-count was unusable. That
conclusion applied clause 3 of CLAUDE.md's Measurement rule to someone else's argument
and not to its own: a zero is evidence about the search.

**Both leaking paths already carry the grain.** A buffer slice names a `json_path` or a
line range, and the server created the handle.

> **Correction — measured 2026-08-21, and it is the premise that made 5a look free.**
> The server creates the handle but does **not** record which artifact produced it.
> `OutputBuffer::store_tool` (`src/tools/output_buffer.rs:303`) writes
> `command = <tool name>` and `source_path: None` — so an `@tool_*` minted by
> `artifact(get)` knows only that *some* call to `artifact` made it.
> `store_file` (`src/tools/output_buffer.rs:279`) then drops provenance again on every
> buffer-to-buffer hop, and does so **deliberately**: a `@`-prefixed path would be
> `stat`-ed by `get_with_refresh_flag` and evict the entry on first read. The one
> leaking case that *is* attributable today is a handle minted from a real filesystem
> path, which carries `source_path`.
>
> The `command` field does preserve the hop chain (`@file_b.command == "@tool_a"`), so
> the chain is walkable — it just terminates at the string `"artifact"`.
>
> **Two routes, neither free, not yet chosen.**
>
> - **A — carry provenance forward.** Give `BufferEntry` an artifact-identity field,
>   populate it at the librarian `get` site, propagate it across hops. Exact, and it
>   fixes the grain at the source. Buys **no history**: every read already recorded
>   stays unattributable.
> - **B — reconstruct from `usage.db`.** `tool_calls` already stores `input_json`,
>   `output_json`, `session_id` and `cc_session_id`, so the minting call's artifact id
>   and the handle it returned are both on disk, as is every later call naming that
>   handle. Needs no change to `OutputBuffer` at all and works **retroactively**.
>   Measured on this project's `.codescout/usage.db` on 2026-08-21: 2905 `artifact`
>   calls, **267** with a `@tool_` handle in `output_json`; 3398 `read_file` calls,
>   **1097** naming an `@` handle in `input_json`. (`instr()`, not `LIKE` — `_` is a
>   single-character wildcard in SQL `LIKE` and would have over-counted.)
>   Two constraints the join must respect: handle ids are a **32-bit truncated
>   timestamp** (`format!("@tool_{:08x}", …)`), and `store_tool` **deduplicates by
>   content hash**, so one handle string can name two artifacts with identical bodies.
>   Join session-scoped and recency-ordered, never on the handle string alone.
>
> **Settled by measurement, 2026-08-21 — and the answer is neither.** The grain question
> turned out to be decidable from `usage.db` without building either route. Walking the
> whole chain, same-session-scoped:
>
> | | reads | artifacts |
> |---|---:|---:|
> | hop 1 — `read_file` on an artifact's `@tool_` handle | 228 | 48 |
> | … `json_path = $.body` — whole body, **no grain** | 81 | 42 |
> | … a bare line range on the JSON response | 7 | 6 |
> | … everything else (`$.entries[*]`, `$.augmentation.*`, `$.preview.*`) | 140 | — |
> | hop 1 reads that themselves minted a `@file_` handle | 44 | — |
> | hop 2 — reads of those `@file_` handles | 42 | 21 |
> | … carrying a line range — **the entry-grain population** | **38** | ≤21 |
>
> So the leak Layer 5a exists to close is **38 reads**. Set it against the path that
> already attributes exactly — `artifact(get, heading=…)`: 273 calls (plus 106 using the
> `headings` array), 240 distinct `(artifact, heading)` pairs, **267** distinct
> `(artifact, heading, session)` triples, of which **57** name a heading carrying an
> entry token rather than a navigation heading like `## Index`. Closing the leak moves
> the entry-grain read population from 267 to roughly 305, and the per-entry maximum is
> **6 distinct sessions** either way.
>
> **The "about half never touched the instrument" claim above was true in a different
> unit than the metric consumes.** It counted *artifact*-grain traffic on one ledger — 53
> instrumented `artifact(get)` against 54 direct `read_markdown`. At *entry* grain the
> leak is 38 against 267. That is `statement-validity-session-log:W-9` recurring one
> level up: a number measured in one unit and consumed in another.
>
> **Consequence.** `max(reads, in-degree)` would pair a term that maxes at 6 with one
> already materialized at 1534 rows. Layer 5a changes no gate outcome, so it is
> **descoped rather than built** — see the Sequencing table.
>
> **What the same measurement did find.** At *artifact* grain the leaked signal is real
> and recoverable today with no code change at all: 228 reads over 48 artifacts, peak
> **26 distinct sessions** on a single artifact, against a maximum of 6 for any single
> `(artifact, heading)`. The join is sound — of 253 artifact-minted handles only **2**
> recur across sessions, and session-scoping recovers 243 of 246 traces. If exposure
> wants a read term now, artifact grain is the one that has data. §6 never considered it,
> because it assumed entry grain was free.
>
> **The era objection, raised and tested.** An all-time count is a weak instrument here:
> the tracker tooling that would *generate* entry-grain reads is recent, so a historical
> total could be measuring an era rather than a mechanism. Re-cut by era — a 30-hour
> window, the 30 hours before it, and everything older (≈16 days) — the corpus answers:
>
> | per window | last 30h | 30–60h | older (≈16d) |
> |---|---:|---:|---:|
> | all tool calls | 5210 | 4931 | 23550 |
> | `artifact` calls | 238 | 342 | 2343 |
> | `get(heading=…)` | 16 | 42 | 215 |
> | … naming an entry token | 7 | 15 | 38 |
> | `append_entry` | 22 | 25 | 90 |
> | `get(entry_filter=…)` | 1 | 5 | 74 |
> | **leaked entry-grain reads** | **4** | **4** | **30** |
> | `context(anchor_id = <entry>)` | **4** | 0 | 0 |
>
> Three readings, and the third is the one that settles it.
>
> 1. **The leak is ~4 reads per 30 hours in every era.** The descope does not rest on an
>    all-time total that a shifting distribution could invalidate; the recent window says
>    the same thing. Four reads is not a trend in either direction — it is noise, and it
>    is named as noise rather than read as one.
> 2. **The distribution IS shifting, toward writes rather than grain-reads.**
>    `append_entry` as a share of `artifact` calls: 3.8% → 7.3% → **9.2%**.
>    `get(heading=…)`: 9.2% → 12.3% → **6.7%**. `entry_filter`: 3.2% → 1.5% → **0.4%**.
>    Tracker work grew; it grew on the writing side.
> 3. **Layer 4 made the growth path pre-attributed, which retires 5a rather than
>    deferring it.** All five `anchor_id` calls ever recorded are in the table above, and
>    the four recent ones are `reconnaissance-patterns:R-3` from two sessions inside 26
>    hours — the feature's own smoke tests, by its author, against the one anchor
>    `context-performance:CTX-1` flags as far-tail. So organic entry-grain reading has
>    **not started**, and the last-30h window cannot test whether it will. But the path it
>    will arrive on names the entry **in the call input** (`<slug>:<local>`): no buffer
>    provenance, no nearest-preceding-heading attribution, no `usage.db` join. 5a's whole
>    premise was that entry-grain reads arrive through leaky paths. Layer 4 built a
>    non-leaky one and made it the ergonomic one.
>
> **Revisit on an event, not on a date.** "Re-run this in a while" is the deferral shape
> `reconnaissance-patterns:R-95` warns about — nobody re-runs it, because its function is
> to stop anyone looking. The condition that would actually reopen 5a: anchored `context`
> calls exceeding ~20 in a 30-hour window across more than three distinct anchors, from
> sessions that are not verifying Layer 4 itself. If that ever fires and those reads are
> *still* landing on `read_file` handles rather than on `anchor_id`, the leak is real and
> route B earns its cost. Until then it is a fix for traffic that does not exist, aimed at
> a path the traffic has been given a reason not to use.
 A `grep` match carries a path and a line number. Both resolve to an entry
by nearest-preceding-heading — the identical attribution Layer 3 needs for citations.
**One algorithm, three consumers**, no extra tool call and no extra context. This is why
the design does not need the obvious alternative of asking the agent to report back what
it read: that costs a round trip and context, and it can be forgotten, where
server-side attribution cannot.
## Terminology — entry vs. Statement

- An **entry** is the markdown section: a `## <ID> — <title>` heading and its body. A
  container, addressed as `<slug>:<local>`.
- A **Statement** is the *claim* that entry asserts — something that can be true or
  false. A Statement carries a validity class, a proof, and a route to re-derive that
  proof.

The distinction is load-bearing, and it is why this is not a pure rename of CAP-8's
"gram". **Not every entry is written to declare a class.** A backlog item or a proposal
typically doesn't; an observation, a measurement, or a law typically does. What makes an
entry a Statement in the strict sense is that it declares a `**Valid:**` class — see
Layer 1. **But declaring none is not an exemption:** per decision 3 below, absence still
means decay — an undeclared entry defaults to `dated <its last commit>` regardless of
what its author meant to assert. That default is silent in practice only because nothing
rests on it until the entry is cited from outside; see Layer 1 § *Default* and `doctor`'s
`entry_cited_from_outside_but_undeclared` check, which is what actually decides whether
an undeclared entry gets flagged.

"Gram" named an *identity*, which is the smaller half. CAP-8's own open decision 1
already argues the point: *"`bug-fix-session-log:F-33` is legible and `gram:a3f9c2` is
not, and this project's moat is the LLM-facing surface."* Identity is orthogonal — a
Statement can later be content-addressed without any of this changing.

## Design decisions (brainstorm 2026-08-20)

1. **Validity attaches to the ENTRY**, not to a content-hash identity and not to the
   edge. Ships without CAP-8. Aimed at the four-datapoint class in §1. Accepted
   limitation: it describes the claim, not the claim's bindings — see *Non-goals*.
2. **The field carries a class plus a prose condition, adjudicated by an agent** — not
   a machine-runnable predicate for every entry. Most conditions in this corpus
   ("after the plan edit lands", "one more cluster", "pending a commit") are not
   expressible as a shell predicate, and demanding one would produce fakes. Selection
   is syntactic and cheap; judgement is the reader's. This is the D11 shape, whose own
   confidence column reads *"low by design"* for the same reason.
   **Amended by decision 8:** the *condition* is interned as a first-class event with an
   id and a `fired_at`, so one event firing closes every Statement waiting on it. Prose
   alone gives N unjoinable strings.
3. **Absence means decay, and a doctor check finds what absence hides.** An entry with
   no `**Valid:**` line MEANS `dated <its last commit>`. Write cost for the common case
   is zero and authors only write the line to *upgrade*. This is also forced by a
   substrate fact: the filter AST has ops `eq, ne, in, nin, gt, lt, gte, lte, contains,
   prefix` and **no null/exists op** (`src/librarian/filter.rs:36-48`), and `ne`
   compiles to SQL `!=` (`:55`), so `field != NULL` is never true — absence is not
   directly queryable. A non-null default sidesteps that entirely.
4. **The graph is populated by backfill from prose, then packed entry-grain.**
   `link_scan::extract` already produces what is needed; see Layer 3.
5. **Attestation taps a Statement's Nth consumer with a deferred, recorded obligation**
   — serve in full, enqueue the proof, and record the obligation when it is not
   discharged.
6. **The unit is a Statement, not a "gram".** See *Terminology* above.
7. **A Statement's high-level route is prose that becomes an edge when it resolves.**
   `**Rests on:**` takes one durable sentence. If it names something the resolver can
   reach — an ADR path, an artifact id, another Statement's token — the scanner
   materializes a `rests-on` edge; if not it stays prose and still does its job.
   Chosen over an ADR-only reference because the arithmetic forbids it: **84 catalogued
   ADRs against ~4000 entries** means most Statements have nothing to point at, and a
   required field with no valid target pressures authors into writing thin ADRs. Chosen
   over a pure homogeneous graph because that needs Layer 3 before it does anything.
   This is CAP-8's own migration principle — *"Additive first … leave every existing
   citation working. Big-bang re-keying is the rewrite trap."*
8. **Close the read-path leaks, then trigger on `max(reads, rests-on in-degree)`.**
   Reads measure what agents consult; in-degree measures what rests on a claim. They
   are different properties and neither subsumes the other, so the trigger takes the
   larger. Closing the leaks (§6) is worth doing independently of the trigger: a
   counter that silently misses half its events is the instrument failure this project
   files bugs about.

**Two clocks, not one (folded in 2026-08-20 from the prior-art pass).** The vocabulary
in decision 2 must not collapse *valid time* into *transaction time*. §1's motivating
failure IS that collapse: `F-3` was true from when it was written until the plan edit
landed, and we learned it was false weeks after that — three dates, one field. Layer 1
therefore stores `asserted_at` separately, and refutation **closes an interval** rather
than overwriting prose. See *Bitemporal storage* in Layer 1.
---

## Layer 3c — the trigger fired

Added 2026-09-02. The Sequencing table's 3c row read `not building` on a
measurement of **1 resolvable declaration**, with an explicit revisit condition of
**~20**. Re-measured today: **62**.

### Why the old number was right, and why it decayed

Layer 1 — the `**Rests on:**` **field itself** — shipped **2026-08-20**. The
re-measurement that produced `1` was taken **2026-08-21**, one day later. Authors
had had a day. Twelve days of authoring since produced 208 declarations, 45.3% of
the 459 entries that declare a `**Valid:**` class.

So the ruling was correct when made and is stale now by exactly the mechanism this
document exists to detect. Worth stating plainly because it generalises: **a revisit
trigger with no owner is a policy, not a mechanism.** Nothing watched this one; it
was found by a measurement sweep aimed at something else. If 3c ships, its own
follow-on thresholds should be `doctor` checks rather than prose.

### The measurement

Resolver-grain — a declaration counts only if some token in it would yield
`Outcome::Edge` from `resolve::resolve`, not merely if it *looks* like a path or a
token. That distinction is load-bearing: a shape-based count over the same corpus
returns ~98–123, and 22 of those are `src/` paths that name no artifact at all.

| outcome | n |
|---|---|
| entry-token, unique definer | 35 |
| rel_path naming a catalogued artifact | 22 |
| 16-hex artifact id | 5 |
| **resolvable total** | **62** |
| entry-token **ambiguous** — reported, never guessed | 25 |
| path names no catalogued artifact | 22 |
| entry-token dangling | 1 |

**62 is a lower bound**, twice over: the probe parsed 163 declarations against the
validated corpus count of 208 (its fence handling is a naive toggle, not
[`FenceState`]), and it does not resolve **qualified** tokens
(`bug-fix-session-log:F-33`). The second omission is the larger one — most of the 25
ambiguous entry-tokens are `F-N`/`W-N`, which are per-work-stream namespaces that the
qualified form resolves by construction, and `get_guide("tracker-conventions")`
already instructs authors to write them that way.

### What did NOT change

The design in § *The high-level route* and § *Resolution and materialization* stands
unamended — `entry_cite` row, `rel='rests-on'`, `origin='scan'`, only `Outcome::Edge`
becomes a row, prune-and-rematerialize per scan. The prerequisite in § *Prerequisite:
slugs at scale* is **already discharged** (3a shipped, 4107/4107 minted), so 3c's
blast radius is now confined to a new `rel` value in a table whose primary key already
includes `rel`.

One open question the original design did not have to answer at n=1: **the 25
ambiguous tokens.** `resolve::resolve` reports and never guesses, which is correct, but
at this volume the report is the useful output — it names the entries whose author
wrote a bare `F-33` where the qualified form was required. That is a `doctor`-shaped
finding, not an edge.

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

### The high-level route — `**Rests on:**`

A second line, sibling to `**Valid:**`, carrying **one durable sentence**:

```
**Rests on:** ADR 2026-07-10 — repair-and-continue input handling
**Rests on:** verification must be event-sourced, not mtime-derived — mtime
              cannot see inside a file
```

If the sentence names something the resolver can reach — an ADR rel_path, a 16-hex
artifact id, another Statement's token — `link_scan` materializes an
`entry_cite` row with `rel='rests-on'`. If it names nothing resolvable it stays prose
and still does its job: a reader six months out can regenerate the proof from the
intent after every `path:line` in the entry has rotted.

**Why a separate relation and why it is free.** `entry_cite.rel` is `TEXT NOT NULL`
with **no CHECK constraint** and sits **inside** the primary key
`(src_slug, src_local, dst_ref, rel)`, so a `rests-on` edge coexists with a `cites`
edge between the same pair rather than replacing it. The file-grain twin already runs
nine relation types (`cites` 2743, `tracks` 11, `relates_to` 11, `implements` 8,
`references` 6, `relates` 5, `worktree_of` 3, `remediates` 1, `amends` 1), so a typed
vocabulary is established practice, not a new idea.

**What it buys beyond durability.** In-degree over `rests-on` is the exposure signal
decision 8 needs, and superseding an ADR mechanically identifies every Statement
derived from it — invalidation **by derivation** rather than by elapsed time. Nothing
in the system can answer *"what rests on this decision?"* today.

**This structure is not invented here.** Measured across all 7 codescout ADRs: three
carry the full `Decision / Revisit-when / Confidence / Sites (initial)` quartet, and
they are the **three most recent** (2026-07-10, -07-20, -07-25); the four from May and
June carry `## Decision` alone. The mapping is exact — `Decision` is the claim,
`Confidence` (*"High on the boundary — verified live"*) is proof-carrying attestation,
`Revisit-when` is `**Valid:** conditional`, and `Sites (initial)` is the rotting
instance **already labelled as rotting by its own heading**. Three authors converged on
it by practice. This layer propagates that shape down two orders of magnitude in grain,
from 84 catalogued ADRs to ~4000 entries.

It is also the third independent appearance of one law in this repo:

| Positional (rots) | Durable (survives) | Surface |
|---|---|---|
| git SHA | `patch-id` | archived bug files |
| `sha256(abs_path)` | `<slug>:<local>` | Stage-2 entry graph |
| `Sites (initial)`, `path:line` | the Decision / `**Rests on:**` | this layer |

### Bitemporal storage — two clocks, never one

A Statement has **three** dates and the `**Valid:**` line holds one. `F-3` was true from
when it was written, became false when the plan edit landed, and was discovered false
weeks after that. Collapsing those is the motivating failure, not an edge case.

Store, on the attestation row (Layer 5) rather than in prose:

| Field | Meaning | Clock |
|---|---|---|
| `asserted_at` | when the claim was first made | transaction |
| `valid_from` / `valid_until` | the interval the claim actually held | valid |
| `recorded_at` | when we learned the interval had closed | transaction |

`valid_until` is NULL while the Statement stands. **Refutation closes the interval; it
never rewrites the prose.** This is standard bitemporal modelling (Snodgrass; SQL:2011
temporal tables) and is what Zep/Graphiti applies to agent memory — every edge carries
`(t_valid, t_invalid)` plus ingestion time, and a contradiction *invalidates* rather
than deletes.

The practical payoff is that the corpus can answer *"what did we believe on
2026-06-14, and when did we find out otherwise?"* — which is exactly the question a
post-mortem asks and the question a record that overwrites itself can never answer.
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

**One gate, shared with Layer 5.** These checks and the attestation tap must not
produce work independently, or the two backlogs sum. The checks' population is
`defaulted-or-stale` **AND** `exposure ≥ threshold` — the same exposure term Layer 5
taps on. A `dated` Statement nothing reads and nothing rests on generates no work at
all, ever.

Two measured reasons this is not over-caution. **Marking is cheap and discharging is
not**: as of June 2025 more than **604,000** English Wikipedia pages carried at least
one `{{citation needed}}`, and that backlog is the steady state, not a transient.
**Alert fatigue is a cliff, not a slope**: false positives run 18–86% of
static-analysis warnings, and past a threshold developers stop reading the output
entirely — converting a false-positive problem into false negatives. A checker that
emits 4000 rows has the same effect as one that emits none, at higher cost.

---

## Layer 3 — backfill the entry graph

### Attribution — measured 2026-08-20 (Layer 0)

`link_scan::extract` (`src/librarian/tools/link_scan/extract.rs:128`) returns:

```rust
pub struct DocExtract {
    pub definitions: Vec<Definition>,   // Definition { token: String, line: u32 }
    pub citations:   Vec<Citation>,     // Citation   { raw: String, kind: CitationKind, line: u32 }
    pub declared_prefixes: Vec<String>,
}
```

**Both definitions and citations carry a line number**, so attribution is a comparison
over data that already exists. The question Layer 0 answered is *which* comparison.

#### The rule is section-bounded, NOT nearest-preceding-heading

```
WRONG:  src_local = the definition with the greatest line ≤ citation.line
RIGHT:  src_local = that definition, but ONLY IF the citation is still inside its
        section — which ends at the next heading of the SAME OR HIGHER level.
```

**Measured over all 12 declared ledgers** (246 definitions, 1427 extracted citations):

| | count |
|---|---|
| citations above the first definition — unattributed under **both** rules | 407 (28.5%) |
| the two rules **agree** | 897 |
| **naive rule attributes outside the owner's section** | **123** |
| **naive precision on attributed citations** | **897/1020 = 87.9%** |

**The 12.1% error is a tail effect and it is concentrated**: four ledgers carry 109 of
the 123 errors (89%) — `structural-debt-refactor` 45, `2026-08-16-iron-law-gate-firing-audit`
31, `reconnaissance-patterns` 17, `tracker-hygiene-log` 16.

One mechanism produces nearly all of it: **the last entry in a file absorbs every
citation in the trailing non-entry sections.** In the gate-firing audit, `GF-8` is
defined at L129 and its section ends at L133, but the naive rule attributes every
citation from L134 to end-of-file to it — an entire `## Summary`-style analysis tail,
dozens of `IL-1` / `IL-2` / `IL-3` references, all landing on one unrelated entry.

**This is the same bound Layer 1's `**Valid:**` parser already specifies.** The spec had
the rule in one place and not the other; Layer 0's contribution is that they are one
rule, and that skipping it costs 12.1%.

#### The 28.5% that attributes to nothing is correct, not lost

Citations above the first definition are index-table rows and preambles — the
hand-maintained `## Index` tables the tracker guide describes. They belong to no entry
and both rules say so. They are also same-file references, so `link_scan` already
classifies them `SelfCite` (853 project-wide) and excludes them from edges. **In-degree
must never count them**, or an entry's own index row inflates its exposure.

#### Calibration (CLAUDE.md Measurement clause 4)

The probe's extractor was calibrated against `link_scan`'s own output before being
extended: on the 11 dangling `EntryToken` citations from `link_scan`'s sample that
resolve to a readable file, the probe reproduces **11/11** `(token, line)` pairs, zero
misses. Ratio 1.0 licenses the extension.

**Honest limit on the 87.9%.** It is the *agreement rate between two algorithms*, with
section-bounded assumed correct — it is not ground truth. Ground-truth checking was a
24-row hand sample of naive attributions; exactly one of those rows fell in the
disagreement set (the `GF-8` case above), and on that one, reading the source confirms
the bounded rule. So: n=24 hand-checked, n=1 overlapping the disagreement, bounded
correct there. A larger ground-truth sample is cheap and should be run before Layer 5
resets any counter on this basis.

Reproduce with `scripts/probe_entry_attribution.py`.
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

### Sizing — measured on this corpus, not inherited from CAP-8

`librarian(action="link_scan", write=false)`, project scope, run 2026-08-20 with
`scan_truncated: false`:

```
artifacts_scanned .. 1089        edges_desired ..... 958
citations .......... 3985        edges_unchanged ... 920      (96% already materialized)
self_cites .........  853        edges_missing .....  38
ambiguous ..........  430        edges_stale .......   2
dangling ...........  556        cross_repo ........   7
```

**Resolved = 2139, derived by subtraction** — the tool reports the four minority classes
and the total but not the resolved count directly, and the five classes sum to exactly
3985. The `ambiguous` array's elements are one-per-occurrence
(`{src_id, raw, kind, line, candidates, candidates_total}`), which is what makes that
partition sound rather than a mix of deduped and undeduped counts.

Excluding self-cites (n=3132): **68.3% resolve, 13.7% ambiguous, 17.8% dangling,
0.2% cross-repo.**

**These are NOT the figures the earlier draft cited, and they are not comparable to
them.** CAP-8 reports 43% / 33% / 24% measured **umbrella-wide across 10 repos and 2
umbrellas** at 6321 citations; the above is **project scope** at 3985. Different
populations, both real. Size Layer 3 off the project-scope number when the work is
project-scoped.

CAP-8's contamination caveat still applies to both: `link_scan` has no "mention" mode,
so a token written to *teach* citation syntax is extracted identically to one written
to cite, and teaching examples land preferentially in the ambiguous and dangling
buckets.

**Two findings that make Layer 3 cheaper than the earlier draft assumed:**

- **920 of 958 file-grain edges are already materialized** (96%), with 38 missing and 2
  stale. The scan is close to a no-op at file grain; the work is entry grain.
- **Every `ambiguous` element already carries a `line`.** The attribution substrate is
  in `link_scan`'s live output, not something to build.

And the attribution algorithm has **three consumers**, not one: entry-grain citation
edges, buffer-slice read attribution, and `grep`-hit read attribution (§6). Its
precision is therefore load-bearing for both the graph and the counter — and per the
prior-art pass, nothing in the literature treats the nearest-preceding-heading
heuristic, so its error rate must be measured here rather than cited.
---

## Layer 4 — serve Statements as context

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

Two tables. The first is slug-keyed, following the precedent the Stage-2 design
established and defended: `events` is keyed on `artifact_id` with no entry column, and
`artifact_link.dst_id` FKs `artifact(id)`, which is move-fragile.

```sql
CREATE TABLE IF NOT EXISTS entry_attestation (
  src_slug           TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local          TEXT NOT NULL,
  -- bitemporal: two clocks, never one
  asserted_at        INTEGER NOT NULL,   -- transaction: when the claim was made
  valid_from         INTEGER NOT NULL,   -- valid:  interval start
  valid_until        INTEGER,            -- valid:  NULL while the Statement stands
  recorded_at        INTEGER,            -- transaction: when we learned it closed
  -- exposure
  reads              INTEGER NOT NULL DEFAULT 0,   -- session-deduped, all three paths
  -- attestation
  verifies           INTEGER NOT NULL DEFAULT 0,
  last_verified_at   INTEGER,
  last_verdict       TEXT,               -- 'held' | 'refuted' | 'inconclusive'
  obligation_state   TEXT NOT NULL DEFAULT 'none',  -- 'none' | 'open' | 'in_flight'
  obligations_missed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (src_slug, src_local)
);

CREATE TABLE IF NOT EXISTS condition_event (
  id         TEXT PRIMARY KEY,           -- interned; many Statements may reference one
  label      TEXT NOT NULL,              -- "the jsonpath plan edit lands"
  fired_at   INTEGER,                    -- NULL until it fires
  created_at INTEGER NOT NULL
);
```

`ON DELETE CASCADE` off `artifact(slug)` is move-stable, which `artifact(id)` is not.

**Why `condition_event` is a table and not prose.** A prose condition gives N unjoinable
strings: three Statements each waiting on "the plan edit lands" are three separate
conditions that must each be adjudicated separately, and the event fires against
nothing. Interning it means **one `fired_at` closes every Statement that references
it** — which is the whole value of an assumption label in an ATMS (de Kleer 1986), and
the reason `conditional` becomes a mechanism rather than a comment.

This borrows the *labelling* idea and deliberately not the machinery: ATMS label
computation is exponential in assumptions, and nobody runs a truth-maintenance system
over 4000 entries. A flat interned-event table with a `fired_at` is the 95% of the
value at none of the cost.
### Exposure — reads *and* in-degree, whichever is larger

The tap fires on `max(reads, rests-on in-degree)`. The two measure different properties
and neither subsumes the other: reads measure what agents actually consult, in-degree
measures what other Statements rest on. A claim everyone reads but nobody cites, and a
claim nobody opens but forty things derive from, are both load-bearing.

**Reads are counted from three attributed paths, session-deduped.** `tool_calls`
carries `session_id` and `cc_session_id`, so "5 reads" means five distinct sessions —
without dedup a single agent re-reading a ledger while working discharges the counter
against itself, and the count stops meaning exposure.

| Path | Attribution |
|---|---|
| `artifact(get, heading=…)` | exact, today |
| `read_file` on an `@` handle with a `json_path` or line range | **not attributable as built** — the handle records a tool name, not an artifact; see the correction in §6 for the two routes |
| `grep(mode="content")` on a ledger | the match carries a path and a line number |

All three resolve to an entry by nearest-preceding-heading — the same algorithm Layer 3
uses for citations. Closing these two leaks is worth doing independently of the trigger
(§6): about half the historical read traffic on the corpus's most-read ledger never
touched the instrument.

**A whole-artifact `artifact(get)` counts for no entry.** It has no grain, and counting
it for all 61 entries would arm an entire ledger at once — which is a mass tap, not a
signal. Under-counting a whole-read is the conservative error; the `max()` with
in-degree is what stops that under-count mattering.

**Deliberately NOT reset by successful verification in the spaced-repetition sense.**
FSRS and SM-2 *lengthen* the interval on successful retrieval, because they model
risk-of-being-forgotten. This models risk-of-being-wrong, so the sign is inverted: more
exposure means sooner, not later. Stated explicitly because a reader fluent in spaced
repetition will assume the opposite.

**Known gaming surface.** Doctorow's *Metacrap* (2001) names it: *metrics influence
results.* Once exposure gates an obligation, exposure becomes a thing to avoid — an
agent that learns "opening this section costs me a proof" will read around it. The
`max()` with in-degree is a partial defence (in-degree is not under the reader's
control), but this is a watch-item with no designed mitigation, and it belongs in the
first month's review rather than in a claim that it is handled.
### The tap

**Threshold: exposure ≥ 5 since the last passing appraisal**, where exposure is
`max(reads, rests-on in-degree)`. Like the Layer 2 horizon it is a guess with no
measurement behind it, and it is a *floor*, not a period — a Statement appraised at 5
arms again at 10. Re-tune from the first month's distribution.

> **Correction — scouted 2026-08-21, before implementing: as written, this tap can never
> fire, because BOTH of its terms are empty.** Measured against the live catalog:
>
> - **`reads` has no counter.** No table, no column, nothing increments. Layer 5a — the
>   layer that was to supply it — is retired (§6), and the leaked population it would
>   have recovered is ~4 reads per 30 hours.
> - **`rests-on in-degree` is literally zero rows.** `SELECT rel, COUNT(*) FROM
>   entry_cite GROUP BY rel` returns `cites` and nothing else, for both origins;
>   `artifact_link` carries `cites`, `tracks`, `relates_to`, `implements`, `references`,
>   `relates`, `remediates`, `amends` — **no `rests-on`**. Layer 3c, the source of that
>   term, was cancelled by measurement: one resolvable declaration corpus-wide, and its
>   edge already exists as `cites`.
>
> **Neither cancellation propagated here.** The shipped-state note under Layer 2 does say
> "`max(reads, in-degree)` is not implemented — there is one term", and that note is
> correct about the layer it corrects. It sits ~200 lines from this section, under a
> different heading, describing shipped code — while *this* section is the one an
> implementer reads to build 5b. A correction lands where the error was found, not where
> it propagates.
>
> **The term that has data is `cites` in-degree**, and the open decision is which
> **grain** of it 5b consumes, because the two rank differently and are not
> interchangeable:
>
> | | `entry_indegree` (shipped) | `entry_cite` in-degree (Layer 3b) |
> |---|---|---|
> | source | recomputed from files each run; **never reads `entry_cite`** | materialized table, 1539 `origin='scan'` rows |
> | grain | token → count of *files* citing it | entry → count of citing *entries* |
> | destinations | tokens | **654** distinct `<slug>:<local>` |
> | consumers | gates 3 shipped `doctor` checks | nothing yet |
> | multi-definer tokens | **dropped** unless exactly one definer is active | resolved, because a stem-qualified citation names one definer |
>
> That last row is the whole difference, and it is measured, not argued. Among cited
> destinations: the `F`/`W` family holds 96 distinct tokens and 442 edges, of which **at
> least 33 tokens carrying at least 339 edges (77%) have more than one definer** — the
> condition `entry_indegree` drops on. (A floor: definers are counted here only among
> *cited* slugs, so a ledger that defines `F-1` and is never cited for it does not
> appear.) For every other prefix the same figure is 7 tokens and 63 of 1056 edges (6%).
> So choosing `entry_indegree` as 5b's exposure term silently exempts most of the
> session-log corpus from ever arming the tap.
>
> **That is not a prediction about 5b — it is an observed property of shipped code.**
> `librarian(action="doctor")`, run 2026-08-21: all **32** rows of
> `entry_cited_from_outside_but_undeclared` are present in the response (a census, not a
> truncated floor — `shown` caps the combined array at 75, and this check contributes 32
> of them), and **zero** name an `F`/`W` token. Every one names another prefix — `TU-7`,
> `B-1`, `CAP-5`, `H-2`, `H-5`. So the 252 `F`/`W` Statements carrying 442 entry-grain
> edges are already invisible to all three exposure-gated checks, today, and the tap
> would inherit that exemption rather than introduce it. Fixing `entry_indegree`'s stale
> deferral therefore repairs shipped behaviour as well as unblocking 5b — which is what
> makes it a different option from "pick the other metric", not a refinement of it.
>
> **SHIPPED 2026-08-21, and the result corrects two numbers above.** `entry_indegree` is
> now keyed `(defining file, token)` and resolves a stem-qualified citation against its
> specific definer, mirroring `resolve` arm for arm — with one deliberate divergence,
> pinned by test: an unresolvable qualifier falls back to the token half rather than being
> dropped, because `call` narrows only the reported worklist and keeps the metric
> cross-repo. Measured before/after on the live catalog: `F-9` now appears on the worklist
> at 5 citing files, where **no** `F`/`W` Statement could appear at all before.
>
> Two corrections to the paragraph above, both unit errors, both worth keeping visible:
>
> - **252, not 96.** A Statement is a *(ledger, token)* pair — a `dst_ref`. Grouping by
>   the bare token string gives 96, and that is what the earlier figure counted.
> - **442 edges is not 442 exposure, and the worklist moved by 1.** `entry_cite` counts
>   entry→entry edges; `entry_indegree` counts distinct citing **files**, and the gate is
>   5 of them. Across all 252 F/W destinations `entry_cite`'s maximum distinct-citer count
>   is **4**, and none reach 5 — `F-9` clears only because non-ledger citers (specs, bug
>   files, READMEs) generate exposure here while generating no `entry_cite` row at all.
>   So the mechanism is what changed; the population it prices grows as stem-qualified
>   citation spreads, and `entry_cite` is a **floor** on citing files, never a count.
>
> This is `statement-validity-session-log:W-9` recurring inside the analysis that cites
> it: a number measured in one unit and consumed in another.
>
> **And `entry_indegree`'s own deferral is now stale.** Its doc comment names the fix —
> count a stem-qualified citation against its specific definer rather than folding it
> into the bare token — and declines it because "it needs the `Corpus`/`by_stem`
> machinery `link_scan` builds, which this function does not have." Layer 3b built
> exactly that, and `entry_cite` holds its resolved output. `reconnaissance-patterns:R-95`:
> a deferral rationale is a claim about current state, and this one has expired.
>
> **Two premises checked and holding**, recorded so the next scout does not re-check
> them: `events.kind`'s CHECK constraint already admits `'reviewed'` and `'verdict'`, so
> the appraisal event needs no migration (and `reviewed` still stands at exactly **1**
> row corpus-wide, as §4 claims); and `entry_cite.src_slug REFERENCES artifact(slug) ON
> DELETE CASCADE` proves the slug-keyed, move-stable precedent `entry_attestation` copies,
> with slugs now at 4113/4113. `asserted_at` remains absent from `src/`.
>
> **One naming defect to settle while editing:** the schema above declares
> `obligations_missed`; *Making a deferred obligation stick* says the un-discharged count
> is `obligations_open`. One concept, two names, and `doctor` is specified to report the
> second.

Past the threshold, `librarian(action="context")` serves the Statement **in full**,
with a banner, and returns a `pending_attestations` array alongside the pack:

```
R-89 [invariant] · exposure 7 (reads 3, rests-on 7) · last appraised: never
…Statement served in full…

pending_attestations: [
  { entry: "reconnaissance-patterns:R-89",
    proof: "counterexample search",
    due:   "before end of turn" } ]
```

This is RFC 5861 `stale-while-revalidate`, and the correspondence is exact: serve the
possibly-stale value immediately and in full, never block the reader, and predicate
revalidation on *an incoming request* rather than on a timer. It uses the channel
`append_entry` already uses for `snapshot_missing` and `undefined_in_body` — response
fields whose whole purpose is to tell an agent something it was about to miss.

**Coalescing is required, not optional.** RFC 5861's amplification rationale applies
directly: without it, readers 6..N each incur an obligation for the same Statement
while the first is still outstanding, and one hot Statement generates N proofs of the
same fact. `obligation_state` moves `none → open → in_flight`, and a Statement already
`open` or `in_flight` emits no new obligation.

> **SHIPPED 2026-08-21 — read-only, and with no new tables.** `librarian(action="context",
> anchor_id="<slug>:<local>")` now arms a tap when the anchor's exposure reaches the floor,
> emitting the obligation under **`must_follow`**, a `pending_attestations` array with the
> structured twin, **and** a banner inside the markdown. The banner is deliberately not
> only structured: a response field is easy to skip, and an obligation nobody sees is the
> mechanism that got `reviewed` to 1 row corpus-wide.
>
> **The register lives in the key, and that is a correction to this section's own wording.**
> The text above proposed `pending_attestations` alone. `Guidance`
> (`src/tools/core/types.rs`) already establishes the vocabulary and its reason: the three
> registers `hint` / `warning` / `must_follow` serialize under variant-named keys because
> *"agents scan JSON responses and react to the key, not the prose."* A directive filed
> under a neutral plural noun reads as data to skim — and the failure this layer exists to
> prevent is exactly a suggestion that evaporates at end of turn. So the sentence goes
> under `must_follow` and the fields stay under `pending_attestations`, mirroring how
> `RecoverableError` splices `extra` alongside its guidance.
>
> Two properties of that choice, both pinned by test. `must_follow` had been an
> **error-path** register only — success responses use `next_step` (`append_entry`,
> `refresh_stale`, `tracker_design`) — and it is safe here because the error body pairs it
> with `ok: false` and `error`, neither of which this response carries. And the directive
> is emitted under **one** register only: a response offering both `must_follow` and a
> weaker `hint` lets the reader act on the weaker one.
>
> Four decisions worth recording, because each departs from the text above:
>
> - **Exposure is `entry_cite` in-degree, not `entry_indegree`.** Measured: a full
>   `doctor` run takes **5.0s** over 4113 files, which an interactive read cannot pay. The
>   materialized graph costs nothing here — `pack_entry_anchor` already fetches
>   `incoming(anchor.reference)` to build neighbours, so the count is free. It undercounts
>   (only ledger entries produce `entry_cite` rows, so a spec or bug file citing the
>   Statement is invisible), and undercounting is the conservative direction for an
>   obligation. A gate may pay 5s; a nudge may not.
> - **The threshold is measured, not guessed.** At 5 the tap arms **5** Statements
>   corpus-wide — `SI-7` (9 citers), `R-19` and `R-3` (7), `R-77` and `R-79` (5). At 4 it
>   would arm 13; at 3, 41; at 2, 144.
>
>   *An earlier draft of this bullet said 8, and named `R-50`/`R-95`/`R-49` among them.
>   That query counted every distinct citing slug and did not exclude same-ledger
>   citations, which the shipped code does — so it measured a different predicate from the
>   one it was quoted to justify. Caught by probing the live tap and seeing exposure **7**
>   for `R-3` where the query said 8. The code was right; the measurement was wrong, and
>   running a query is not the same as running the right one.*
> - **Exposure counts distinct citing LEDGERS, not edges**, and same-ledger citations are
>   excluded — a hand-maintained `## Index` row is the ledger talking to itself. Both
>   properties are pinned by tests written after mutation showed them unpinned.
> - **"Serves the Statement in full" was NOT built.** It fights the measured anchor
>   reserve (`char_cap / 2`), which exists because a long anchor otherwise starves every
>   neighbour. Suspending that reserve for armed anchors is a budget change that deserves
>   its own measurement, not a side effect of the tap.
>
> **Deliberately still absent:** `entry_attestation`, `condition_event`, bitemporal
> `asserted_at`, and coalescing. Without storage there is no `obligation_state`, so a
> Statement past the floor emits its obligation on **every** read until discharged — which
> is the designed behaviour ("the banner gets louder on its own"), but it is un-coalesced,
> and the amplification argument above still applies once several readers are involved.
>
> **Proof-carrying is enforced at READ time**, which is what makes the no-new-table version
> honest rather than merely cheap. Discharge is an existing `artifact_event(kind="reviewed")`
> whose payload carries `entry`, `instrument`, `observed` and `verdict`; an event missing
> any of the three is skipped, so it stays in the log as a note and moves nothing. Only
> `verdict="held"` discharges — `refuted` and `inconclusive` leave the tap armed, and the
> banner reports which, so a refutation can never read the same as never-looked-at.

**Discharge rides the existing path.** Google's g3doc freshness dates work because
discharge is "bump a date in a code review" — the *SWE at Google* account credits the
in-band `Last reviewed by …` byline for adoption and calls review-through-the-normal-CR-path
*"a low-cost means to ensure that a document is looked over from time to time."* Borrow
the named-owner-in-band and the ride-the-existing-review-path properties. Do **not**
borrow the stamp: g3doc's names no instrument, which is exactly the laundering the next
section forbids, and it is age-triggered where this is exposure-triggered.
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

### Proof-carrying, and the counter resets on the appraisal — not on the assertion

**A `reviewed` event whose payload names no instrument does not reset the counter.**
Without this the mechanism is a laundering machine: the claim acquires a verification
stamp it did not earn and reads as *more* trustworthy than before the tap — strictly
worse than no mechanism, and precisely CLAUDE.md's described failure (a cheap proxy
standing in for the target, returning a plausible value and no error). CLAUDE.md states
the general form: *"a review that reports findings without observed mutation pass/fail
counts has not verified anything."*

Required payload fields: `instrument` (the command, query, or named reasoning step, as
invoked), `observed` (its raw output, verbatim), `verdict`
(`held` | `refuted` | `inconclusive`). Any missing → the event is recorded as a `note`,
not a `reviewed`, and neither the counter nor `obligation_state` moves.

**Storing the raw invocation and its raw output is the load-bearing requirement, not
the verdict.** This is proof-carrying code's independent-checker property (Necula,
POPL '97): the artifact ships with something a *later, independent* party can re-check
cheaply, rather than with an assurance. A verdict alone is an assurance.

**The attester must not emit its own results.** IETF RATS (RFC 9334) separates an
Attester producing *Evidence* from a Verifier appraising it under an explicit
*Appraisal Policy* and emitting *Attestation Results*. Here the reading agent is the
Attester; the stored `instrument` + `observed` is the Evidence; the appraisal is what
resets the counter. The reason this separation matters is empirical, not architectural:
Huang et al., *LLMs Cannot Self-Correct Reasoning Yet* (ICLR 2024) finds that without
external feedback, self-correction often makes performance **worse**. An agent that
asserts and accepts in the same breath is the configuration that paper measures.

**MVP position, stated as a limitation rather than solved.** A second appraising agent
is out of scope for the first implementation. What ships instead is the cheapest thing
that preserves the property: Evidence is stored in re-runnable form, so any later
reader — human or agent, in any session — can re-execute the instrument and compare.
The counter resets on a *re-runnable* attestation, which is weaker than an independent
appraisal and strictly stronger than a stamp. Escalating to a second agent is a future
decision with a named trigger: if `verifies` climbs while `refuted` stays at zero, the
appraisals are not appraising.
### A refutation closes an interval — it does not reset, and it does not overwrite

`last_verdict = 'refuted'` does three things, in this order:

1. **Closes the valid-time interval**: `valid_until` = the best estimate of when the
   claim actually stopped holding (not "now" by default — the tap discovers the closure,
   it does not cause it), and `recorded_at` = now.
2. **Stops the Statement being served as fact**, routing it to the same worklist a fired
   `condition_event` lands on.
3. **Leaves the prose untouched.** The record of what was believed, and when, is the
   asset. A refutation that rewrites the claim destroys the only evidence that the
   belief was ever held — and post-mortems ask exactly that question.

This is Wikidata's rank model and Zep/Graphiti's invalidation model: a contradicted
statement is **demoted, never deleted**.

`inconclusive` also does not reset the counter — it records that someone looked and
could not tell, which is information, and leaves the tap armed.

**Verification that fails is the highest-value output this mechanism produces** and must
never be recorded identically to one that passes. It is also the health metric: if
`verifies` climbs while `refuted` stays at zero across the corpus, the appraisals are
theatre and the design has failed in the specific way it was built to avoid.
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
| 0 | **Attribution probe** — measure nearest-preceding-heading precision on this corpus | — | **DONE 2026-08-20** — naive 87.9%; use the section-bound rule |
| 1 | `**Valid:**` + `**Rests on:**`, default-is-decay, allocator stamping, `asserted_at` | — | **SHIPPED 2026-08-20**, partially — see below |
| 2 | **Four** doctor checks (not three), gated on shared exposure | 1 | **SHIPPED 2026-08-20** — one exposure term, not `max()` — see below |
| 3a | Slug bulk-mint (`doctor fix=mint_slugs`) | 0 | **SHIPPED 2026-08-20** — 4107/4107 minted |
| 3b | `origin='scan'` entry-grain materializer | 3a | **SHIPPED 2026-08-21** — 322 edges; see below |
| 3c | `rel='rests-on'` edges | 3b + declarations | **TRIGGER FIRED 2026-09-02 — re-open.** Was `not building`: re-measured 2026-08-21 at 1 resolvable declaration, revisit at ~20. Now **62**, resolver-grain and a **lower bound** — 35 entry-tokens with a unique definer, 22 rel_paths naming a catalogued artifact, 5 artifact ids. See § *Layer 3c — the trigger fired* |
| 4 | Entry-grain `context` anchor | 3 | **SHIPPED 2026-08-21** — two-pass packing; see below |
| 5a | **Close the read leaks** — buffer-slice + `grep` attribution | 0 | **RETIRED 2026-08-21** — the leak is ~4 entry-grain reads per 30h in every era, and Layer 4's `context(anchor_id=<slug>:<local>)` names the entry in the call input, so the growth path arrives pre-attributed. Reopen only on the event trigger in §6 |
| 5b | `entry_attestation`, `condition_event`, taps, coalescing, proof-carrying appraisal | 3, 4 | **the TAP shipped 2026-08-21** — read-only, no new tables, proof-carrying enforced at read. Storage (`entry_attestation`, `condition_event`, coalescing, bitemporal `asserted_at`) still designed-only |

**Layer 0 is new and it gates three others.** The nearest-preceding-heading heuristic
feeds citation edges, buffer-read attribution and grep-read attribution, and the
prior-art pass found nothing in the literature on its error rate. It is a probe, not an
implementation: run the attribution over the corpus's 3985 extracted citations, hand-check
a sample, report precision. Cheap, and it de-risks everything downstream.

Layer 5b is what turns the rest from bookkeeping into a feedback loop, and its absence
is already measured at 4086 artifacts reporting `freshness: unknown`.

### Shipped state — reconciled against `src/` on 2026-08-20

This section previously read `scheduled` for two layers that had shipped, and described
the exposure term as the opposite of what was built. A status column that says
`scheduled` about shipped code is the decay this document exists to detect, so it is
corrected here rather than left as a known-stale note.

**Layer 1 shipped in part.** Present: the three-form grammar, `parse_validity` /
`parse_rests_on`, fence-skipping line-anchored detection, `entry_sections`, and the
allocator's `**Valid:** dated <today>` stamp. **Absent:**

- **`asserted_at` did not ship** — zero occurrences in `src/`. Bitemporal storage
  (*Two clocks, never one*) remains designed-only, so a refutation cannot yet close an
  interval.
- **Default-is-decay is written but not in effect.** `resolve_validity` exists, is
  tested, and has **zero production callers** — the only mentions outside its own module
  are a `doctor.rs` doc comment explaining why `entry_dated_stale` deliberately uses
  `parse_validity` instead, since guessing an undeclared entry's age is the one thing
  that check must not do. The `Default` clock (last commit touching the heading's line
  range) is unimplemented; the undeclared population routes to
  `entry_cited_from_outside_but_undeclared` instead. **Decision 3's semantics therefore
  hold on paper and nowhere in the code** — see
  `docs/issues/archive/2026-08-20-validity-spec-terminology-contradicts-decision-3.md`.

**Layer 2 shipped four checks, and one exposure term.** `validity_unparseable` was added
because all three original checks swallow `parse_validity`'s `Err`, which left a
malformed declaration invisible to every worklist — including a calendar-invalid date,
now refused by a real calendar parse rather than a shape regex.

Exposure is **cross-file citation in-degree only**. This section formerly said Layer 2
would run "reads only, no in-degree" until Layer 3 landed; the built order is the
inverse, because `link_scan` already produced in-degree and the read counters are still
leaking (Layer 5a). `max(reads, in-degree)` is not implemented — there is one term, and
it degrades by being smaller, not by breaking.

Two properties were added beyond the design and are worth carrying forward: the reported
worklist is **scoped to the active project while the metric stays cross-repo** (an entry
load-bearing because another repo depends on it keeps its true exposure), and rows
filtered out that way are counted in `catalog_health.entry_validity_scoped_by_project`
rather than dropped silently.

**Layer 3 shipped 2026-08-20/21, and the entry graph is smaller and differently shaped
than this document assumed.** Measured on the live catalog after materializing:

- `entry_cite` went **13 rows → 335**: 322 `origin='scan'`, the 13 `origin='write'`
  untouched. 303 carry an entry-grain `<slug>:<local>` destination, 32 a bare artifact id.
- Only **44 of 1099** scanned artifacts are a source of any entry-grain edge.

**The graph is sparse on the SOURCE side, and that is structural rather than incidental.**
An edge needs the *citing* document to be a ledger with `## <ID> — <title>` headings, and
most artifacts — specs, plans, ADRs, READMEs, bug files — define no entries at all. Of
1721 resolved-and-attributable citations, **1397 sit outside any entry** and 324 inside.
So this is not "the citation graph at finer grain"; it is the much smaller object *"a
ledger entry cites something"*. That is still exactly what Layers 4 and 5 consume, but it
is roughly a sixth of what the wording below implies.

**Two defects capped it, and both are now fixed** (2026-08-21). They were independent and
compounding, and neither alone recovered the canonical example.

1. **`SelfCite` was decided at file grain** (`b750419a`). `resolve` returned `SelfCite`
   whenever the citing FILE defined the token, and that arm was matched before the one
   holding `entry_section_at` — so every intra-ledger edge was discarded before
   attribution ran. A ledger's `**Kin:**` and `**Chain.**` lines, its densest and most
   deliberate edges, contributed nothing.
   `docs/issues/archive/2026-08-21-selfcite-is-file-grain-so-intra-ledger-entry-edges-never-materialize.md`
2. **Attribution followed a token's FIRST mention** (`383b394e`). `push_citation` kept one
   citation per `(kind, raw)` per document carrying only the first occurrence's line, so a
   passing mention in a preamble or `## Index` table consumed it.
   `docs/issues/archive/2026-08-21-entry-attribution-follows-the-first-mention-only.md`

The second was deferred here as "a behaviour change to the exposure metric three shipped
`doctor` checks are gated on". That was true of the two options considered and false of the
one taken: `Citation` now carries **all** its occurrence lines while `extract` still emits
exactly one per `(kind, raw)`, so every consumer's citation count is byte-identical and
exposure cannot move by construction. Verified rather than argued — `doctor`'s
`summary.total` and every `by_check` count came back identical across the change, with
`entry_cited_from_outside_but_undeclared` at 32 both times.

**Measured live**, project-scoped, across the three states:

| | before | after `b750419a` | after `383b394e` |
|---|---:|---:|---:|
| `entry_cite` rows, `origin='scan'` | 322 | 391 | **1513** |
| of those, intra-ledger | 0 | 68 | **703** |
| distinct source ledgers | 44 | — | **85** |
| `derived` | 323 | 391 | **1345** |

So the sixth-of-the-citation-graph estimate below was measuring a defect, not a ceiling.
The 1461 figure this section previously carried is superseded: it was corpus-wide,
unfiltered by resolution, and inflated by omitting `SelfCite` from the outcomes that
prevent an edge.

**The original split reasoning, for the record.** Measured 2026-08-20, before shipping:

- **Slug bulk-mint → `origin='scan'` materializer is well-fed.** `entry_cite` holds **13
  rows, all `origin='write'`**; a corpus `link_scan` reports 4042 citations, 861
  self-cites, 443 ambiguous and 557 dangling, leaving roughly two thousand resolvable
  entry-grain rows. This is the provenance graph existing for the first time.
- **`rel='rests-on'` has no input.** Fifteen `**Rests on:**` lines exist corpus-wide and
  most are fenced examples in this spec, `docs/templates/session-log.md`, and the manual
  page; at most ~7 are real declarations, 6 of them written the day Layer 1 shipped.

  **Re-measured 2026-08-21 — the population grew and the part that matters did not.**
  Instrument: a line-anchored grep for the parser's shape that does NOT skip fenced
  blocks, so it over-counts; an upper bound, named as one. 21 lines now match; 6 are
  literal template placeholders; **2** contain anything token-shaped; **1** is genuinely
  resolvable in-corpus.

  That one is `W-6`'s route to `statement-validity-session-log:F-5` — and **its edge
  already exists as `cites`**, because a `**Rests on:**` line is body text that `extract`
  scans like any other. So 3c would materialize one row duplicating an existing one under
  a different `rel`. The other token-shaped line is the manual's `ADR-7` example, a prefix
  no ledger owns. The rest name a principle, a file outside the repo, or a `get_guide`
  topic — none of which `dst_ref` can key, and none of which is an authoring failure:
  decision 7 says such routes stay prose and still do their job.

  **Consequence for decision 8, which is the part worth carrying forward.** The Layer 5
  tap fires on `max(reads, rests-on in-degree)`. With one edge corpus-wide that term
  contributes nothing, so the tap is effectively `max(reads, 0)`. The reasoning behind
  decision 8 is sound — a claim nobody opens but forty things derive from IS load-bearing
  — but the signal has no input yet. Design Layer 5b knowing that, rather than discovering
  it after the tap is built.
  Building the materializer now ships machinery with nothing to chew on — the same
  inertness the *Risks* section raises about Layer 2, but without Layer 2's fallback,
  since an edge can only exist where an author wrote the line.

**The bulk-mint's migration hazard is historical and already guarded — do not re-add the
guard.** `artifact.slug` was dropped once by a table-copy migration
(`migrate_v6::drop_legacy_and_stamp` rebuilt `artifact` without carrying it, taking
`ux_artifact_slug` and dangling `entry_cite`'s FK with it). It self-healed on the next
open, so a twice-opening idempotency test did not catch it. Two tests close it today:
`migration_v6_single_open_preserves_v9_entry_graph_shape` pins that specific column, and
`every_schema_sql_artifact_column_survives_every_migration_path` generalises it by parsing
the canonical column list out of `SCHEMA_SQL` and checking every column on every seeded
migration path.

What the bulk-mint changes is the **cost** of a recurrence, not its likelihood: two null
slugs tolerate a dropped column silently, ~4104 populated ones with a live FK do not. See
memory `catalog-sql-hazards`.
---

**Layer 4 shipped 2026-08-21** — `ad21910f` (entry-grain anchor), `711a25cf` (two-pass
packing), `f58ab393` (mutual-first ordering). `librarian(action="context", anchor_id=…)`
now detects an entry-grain anchor and returns before the file-grain `candidates_capped`
path, serving the anchor Statement with the Statements either side of it. Neighbours are
deduplicated **by reference** rather than by edge, so an artifact that both cites and is
cited by the anchor appears once as `mutual`; ordering is mutual → cited-by → cites, then
lexicographic. The `overflow` block reports `grain`, `packing`, and `unresolved_edges`.

**The packing policy was chosen from a sweep, not copied from the sibling.** The
file-grain packer's constant does not transfer: it is a *line* budget over whole
documents where this is a *byte* budget over sections, and bytes-per-line spans roughly
5× across this corpus. Two passes — serve every neighbour whole when the anchor section
plus all neighbour sections fit the caller's budget, else excerpt each neighbour to
`NEIGHBOUR_EXCERPT_BYTES`. The sweep behind that constant, the fully-served-anchor rates
it moves, the neighbourhood-size distribution, and the one optimization point
deliberately not taken are recorded in `docs/trackers/context-performance.md` (`CTX-1`),
alongside the refuted hypothesis that neighbour classes deserve differential treatment
(`CTX-2`).

**Read those numbers with the tracker's own caveat.** They describe the entry graph as of
the last `link_scan` — a fact about an instant, not a property of the corpus. Two carry
into Layer 5 regardless: the deferral recorded in `CTX-1` **fired on the feature's first
live call** (`R-108`), and the anchor whose behaviour is most tempting to tune from sits
in the far tail of the neighbourhood distribution rather than near its centre.

**Layer 3c was closed by measurement rather than deferred** — see the re-measurement
above. That leaves **5a as the next scheduled build**, and it gates 5b.
## Risks

Ordered by how much each would change the design if it fired.

- **Exposure becomes a thing to game.** Doctorow's *Metacrap* (2001): *metrics influence
  results.* An agent that learns "opening this section costs me a proof" reads around
  it. `max()` with in-degree is a partial defence — in-degree is not under the reader's
  control — but there is no designed mitigation. Watch it from week one.
- **The appraisal could be theatre.** If `verifies` climbs while `refuted` stays at
  zero, the mechanism is laundering rather than verifying. This is the single health
  metric for the whole design and it should be on the first dashboard, not discovered
  later.
- **Attribution precision: measured, and the naive rule is not good enough.** Layer 0
  (2026-08-20) puts nearest-preceding-heading at **87.9%** on this corpus, with the
  12.1% concentrated in four ledgers and produced by one mechanism — the last entry in a
  file absorbing every citation in the trailing non-entry sections. The section-bound
  rule fixes it and is already what Layer 1's parser specifies. **Residual risk:** 87.9%
  is agreement between two algorithms, not ground truth; only 1 of the 24 hand-checked
  rows fell in the disagreement set. Run a larger ground-truth sample before Layer 5
  resets a counter on this basis. The prior-art pass found nothing in the literature on
  this heuristic, so there is no external number to fall back on.
- **Per-entry blame cost is unmeasured.** Decision 3's default depends on it; three
  options are named in Layer 1 and the choice is deferred to measurement.
- **Bulk slug minting touches `merge_worktree` and the worktree overlay.** 2 of 4087
  artifacts have a slug today. Its own change, its own review, sequenced before the
  materializer.
- **`conditional` adjudication is agent-judged** and always will be. Every surface must
  say "worklist", never "verdict".
- **The corpus may not be big enough to matter.** 12 ledgers, ~4000 entries, one
  maintainer. If exposure never reaches the threshold on more than a handful of
  Statements, Layers 2 and 5 are inert and the honest outcome is to ship Layer 1 alone.
  The first month's exposure distribution decides this, and it is cheap to find out.
## Prior art

Two passes: the project's own `memory("research/agent-memory-frameworks")` (2026-05-25)
and a commissioned external pass (2026-08-20). Citations below were independently
re-confirmed by that pass; items it could not verify are marked.

### Internal — the invariant, and a recommendation that went unharvested

**codescout is a passive embedder.** `Embedder` / `RemoteEmbedder::openai` are
embeddings-only; there is no generative or chat client in the tree. The 2026-05-25 pass
rejected an otherwise-elegant distillation design purely on that ground. Layer 5 honours
it by construction: codescout counts, serves, and records — **the host agent makes every
judgement**. That is the pass's own recommended shape (*host-driven scaffolding*).

That pass also mapped codescout against a memory-substrate taxonomy (cited there as
arXiv 2603.07670; **not re-verified**) and concluded all three substrates are present
and *"the ONLY real gap is Axis 3 — control policy."* **Layers 2 and 5 are that control
policy** — the first in the system.

And its Approach C, piece 3, verbatim: *"Add `valid_until` / `superseded_by` to
`ArtifactRow` → Zep-style temporal + Supermemory-style forgetting."* **87 days later
nothing shipped**, and the recommendation itself went unharvested — an instance of §1's
failure one level up. Two deliberate divergences: it was artifact-grain where §1's
failures are all entries, and it *hid* decayed records where this spec flags them
(hiding makes a decayed record indistinguishable from an absent one).

### External — what maps onto which decision

| Prior art | Maps to |
|---|---|
| **Bitemporal modelling / SQL:2011 temporal tables** (Snodgrass) — valid time vs. transaction time as independent axes | Layer 1 *Bitemporal storage* |
| **Zep / Graphiti** — temporal KG for agent memory; edges carry `(t_valid, t_invalid)` + ingestion time; contradiction *invalidates*, never deletes. arXiv:2501.13956 | Layer 1, Layer 5 refutation |
| **Doyle 1979 JTMS; de Kleer 1986 ATMS** — beliefs labelled by the assumptions they hold under | `condition_event` interning |
| **AGM belief revision** (Alchourrón/Gärdenfors/Makinson 1985) | refutation semantics |
| **RFC 5861 `stale-while-revalidate`** (now in RFC 9111) — serve stale in full, revalidate *because of an incoming request*, coalesce to avoid amplification | Layer 5 tap |
| **Gray & Cheriton 1989, Leases** — validity is bounded and must be renewed | decision 3 |
| **Adaptive TTL** — Alici et al. 2012 (ECIR); Basu et al. 2017 d-TTL/f-TTL (SIGMETRICS) — per-object TTL from access statistics | exposure-driven revalidation |
| **Google g3doc freshness dates** (*SWE at Google*, ch. 10) — in-band "last reviewed by", discharge through the normal review path | Layer 5 discharge |
| **Micropublications** (Clark, Ciccarese, Goble 2014) — claim → evidence → **method**, transitively closed | `**Rests on:**` + `instrument`/`observed` |
| **Nanopublications / W3C PROV / PAV** — assertion, provenance and publication-info as separate graphs | Layer 1 field separation |
| **Proof-carrying code** (Necula, POPL '97) — ship a re-checkable proof, not an assurance; the checker is small and independent | why `observed` is stored raw |
| **IETF RATS, RFC 9334** — Attester emits Evidence; a Verifier appraises under an explicit policy and emits Results | appraisal ≠ assertion |
| **Wikidata ranks + temporal qualifiers** (not directly fetched) | demote, never delete |
| **Wikipedia verifiability / `{{citation needed}}`** | decision 3, and the backlog risk |
| **NELL** (Mitchell et al., CACM 2018) — 120M confidence-weighted, provenance-carrying beliefs under never-ending curation | existence proof at scale |
| **PPS / monetary-unit audit sampling** (PCAOB AS 2315) — selection probability scales with exposure | decision 8 |
| **JIT comment-code inconsistency detection** (Panthaplackel et al., AAAI 2021) | a future automated `dated` checker |
| **FSRS / SM-2** | decision 8, **with the sign inverted** |
| **`eslint-plugin-unicorn/expiring-todo-comments`, dbt `deprecation_date` / source `warn_after`** | the machine-runnable subset of `dated` |

### What the literature does not have

Four gaps, each a place where this design must **measure rather than cite**:

1. **No formal treatment of read/exposure count as a staleness trigger for knowledge
   claims** anywhere in KG-maintenance, documentation, or SE literature. Adaptive-TTL
   caching and PPS audit sampling are analogies, not on-point results.
2. **No named failure mode for "a cheap verification stamp launders unearned trust"** in
   knowledge bases. The nearest verified result is Huang et al. on self-correction.
3. **No published post-mortem quantifying the death of an annotation field.** §4's
   1-use-in-4087 appears to be novel data.
4. **Nothing on line-number citation attribution.** See *Risks*.

*Instrument caveat.* The `researcher` MCP returned adult-content and parcel-tracking
sites as "sources" on one query, and produced a report on "attestation theatre" whose
citations were a vendor marketing page and a notarisation service — discarded. Every
citation above was confirmed through a separate search or fetch, and the two that were
not are marked inline. Recorded here because a tool that returns plausible garbage
without erroring is the exact failure class this spec exists to detect.
## References

- `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md` — Stage 2:
  slugs, `<slug>:<local>` ids, `entry_cite`, write-time `cites`
- `docs/trackers/capability-proposals.md` — CAP-5 (shipped), CAP-7 (shipped), CAP-8
  (the gram, proposed)
- `docs/issues/archive/2026-08-20-doctor-comment-misnames-entry-cite-writer.md` — the comment
  correction found while writing this
- `src/librarian/freshness.rs` — the artifact-grain ancestor of Layer 5
- `src/librarian/tools/link_scan/extract.rs`, `.../resolve.rs` — the backfill substrate
- `src/librarian/catalog/augmentation.rs:826` — `PendingSection`, the prose write path
- `get_guide("tracker-conventions")` § *Required fields*, § *Detecting these fields*

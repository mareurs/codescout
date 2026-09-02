# Statement Validity (`**Valid:**`, `**Rests on:**`)

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

Records go false and nothing notices. A tracker entry that measured something in
May is still sitting there in August, indistinguishable from one measured
yesterday, and the only thing that ever catches it is someone happening to
re-derive the number. This adds a decay class to an entry, a route back to its
proof, and four read-only `doctor` checks that rank what has gone stale by how
much rests on it.

## An entry is not a Statement

An **entry** is the markdown section — a `## <ID> — <title>` heading and its
body. A **Statement** is the *claim* that entry asserts: something that can be
true or false, which carries a validity class and owes a proof.

Not every entry is a Statement. A backlog item asserts nothing. But **declaring
nothing is not an exemption** — absence is read as decay, not as "no claim
here", because the alternative is unqueryable. (The filter AST has ops `eq, ne,
in, nin, gt, lt, gte, lte, contains, prefix` and no null/exists op, and `ne`
compiles to SQL `!=`, so `field != NULL` is never true. A non-null default
sidesteps that entirely.)

What actually decides whether an entry gets flagged is **exposure** — how many
*other* files cite it. An uncited backlog item is left alone because nothing
rests on it, not because it declared its way out.

## The three classes

A line inside the entry's section, sibling to `**Status:**`:

```text
**Valid:** invariant
**Valid:** dated 2026-08-20
**Valid:** conditional — until the jsonpath plan edit lands
```

| Class | Means |
|---|---|
| `invariant` | A law. No expiry. What gets promoted. |
| `dated <date>` | True of an instant. Every measured count. |
| `conditional — <event>` | True until a named event fires. |

Three, and the test for whether a fourth is earned is whether it maps to a
distinct sweep action.

Refused, rather than silently accepted:

- **A bare `conditional`** with no event. A condition nobody named can only ever
  produce "go re-read this".
- **An unknown class.** `conditionally speaking` does not parse as the class it
  happens to start with — the check is on a word boundary, not a prefix.
- **A calendar-invalid date.** `dated 2026-02-30` is refused even though it has
  correct `YYYY-MM-DD` shape. Both a shape regex *and* a real calendar parse run,
  because `chrono`'s `%Y-%m-%d` on its own accepts `26-08-20`, `2026-8-20` and
  `2026-08-2`, and a shape regex on its own accepts February 30th.

The first declaration in a section wins if there is more than one. Declarations
inside fenced code blocks are skipped, so a worked example teaching the syntax is
never mistaken for a declaration of it.

### `**Rests on:**` — the route back to the proof

```text
**Rests on:** ADR-7's rule that a scan reports a worklist, never a verdict
```

Code rots and `path:line` rots with it; an ADR, a decision, or a principle does
not. One durable sentence. If it names something the resolver can reach, a later
layer materializes a `rests-on` edge; if not it stays prose and still does its
job. It is parsed today and consumed by nothing yet.

Why not require an ADR reference: there are 84 catalogued ADRs against roughly
4000 entries, so most Statements have nothing to point at, and a required field
with no valid target pressures authors into writing thin ADRs.

## The four checks

`librarian(action="doctor")` reports these. All four are **read-only** — there is
no `fix=`, and each emits a worklist, never a verdict.

| Check | Fires when |
|---|---|
| `entry_conditional_past_due` | a `conditional` Statement's named event has fired |
| `entry_dated_stale` | a declared `dated` Statement is past the horizon |
| `entry_cited_from_outside_but_undeclared` | a load-bearing entry declares no class at all |
| `validity_unparseable` | a `**Valid:**` line is present and malformed |

The first three are gated on citation exposure; `validity_unparseable` is not,
because a malformed declaration means the author *tried* to declare and their
Statement is invisible to every other check regardless of who cites it.

`entry_cited_from_outside_but_undeclared` reports "load-bearing and undeclared"
and deliberately never says "promoted". A promotion, an eval-fixture list and a
kin reference are syntactically identical; using a mention count as a promotion
predicate mislabelled three of five entries when it was tried. That judgement
stays human.

## Exposure, and why the ranking is load-bearing

Exposure is the number of *other* files that cite an entry's token — a file-level
count, not an occurrence count, so one chatty file cannot inflate a token's
apparent reach. Same-file citations are excluded: 28.5% of ledger citations sit
above the first definition, in hand-maintained index rows, and counting those
would let an entry's own index row inflate its own exposure.

Ranking by exposure is not a nicety. A decayed fact nothing cites costs nothing;
one cited from a promoted skill costs a lot. An unranked list of every dated entry
past a horizon is thousands of rows and will be ignored — the same outcome as not
shipping the check, at higher cost. As of June 2025 more than 604,000 English
Wikipedia pages carried at least one `{{citation needed}}`; a checker that emits
4000 rows has the same effect as one that emits none.

### Exposure is cross-repo; the worklist is not

The catalog spans every repo on the machine, and these two facts are treated
differently on purpose:

- **The metric stays global.** A codescout entry cited from another repo is
  genuinely depended upon — that is what a cross-repo citation *is*. Scoping the
  metric would push such an entry below the threshold and stop reporting it
  entirely, which is the worst direction of error for a feature built to surface
  silent decay.
- **The reported rows are scoped to the active project.** A developer standing in
  one repo can only act on that repo's entries.

So every reported row carries its true global exposure, and nobody is handed
another project's work. Rows filtered out this way are counted in
`catalog_health.entry_validity_scoped_by_project`, broken down per project root,
and named in the report's hint — a filtered row never becomes a violation, so
`summary.total` cannot count it, and an unannounced filter would be a report that
quietly drops findings.

## Server-side stamping

`doc(action="append_entry")` stamps `**Valid:** dated <today>` into the
section it writes, unless the caller passes a class explicitly. The stamp happens
in the same file write and transaction as the entry-id high-water mark, because a
caller writing the section afterwards would do a second read-modify-write outside
that transaction — and a peer session allocating on the same file in between gets
its committed mark clobbered, walking the counter backwards.

This is not the server inventing a claim on your behalf. Absence already *means*
`dated <the entry's last commit>`, which for a new entry is today; the stamp
materializes that default rather than adding to it. New entries are born with a
declared class the same way they are born with a resolver-conformant heading — by
construction rather than by convention.

Hand-written entries are not covered. Nothing rewrites a file you edited yourself.

## What is not built yet

- **The default clock.** An undeclared entry is *defined* as
  `dated <the last commit touching its heading's line range>`, but nothing computes
  that — it needs per-entry `git blame` over ~4000 entries at unmeasured cost.
  `entry_dated_stale` therefore covers *declared* `dated` entries only, and the
  undeclared population routes to `entry_cited_from_outside_but_undeclared`.
- **`rests-on` edges.** `**Rests on:**` is parsed and stored; no consumer reads it.
- **Attestation.** Serving a Statement with a deferred proof obligation is a later
  layer.

## Related

- [Entry Citations](entry-citations.md) — the `PREFIX-N` id namespace these
  Statements live in
- [link_scan](link-scan.md) — where definitions and citations are derived
- [Tool Usage Doctor](tool-usage-doctor.md) — the wider `doctor` surface

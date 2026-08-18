---
id: d34dfcd2cc718bd8
kind: bug
status: open
title: 'BUG: an index-table row satisfies the snapshot-drift check but defines no citable token, so an entry can be written "successfully" and be permanently unreachable by citation'
owners:
- marius
tags:
- librarian
- link-scan
- entry-identity
- augmentation
- dangling-citations
topic: tracker-entry-identity
closed: ''
opened: 2026-08-18
related:
- docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
- docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md
- docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md
severity: medium
---

# BUG: an index-table row satisfies the snapshot-drift check but defines no citable token

## Summary

codescout holds **two** server-side notions of "this entry is present in the artifact's body",
and they disagree. The write path's drift advisory (`append_entry`'s `snapshot_missing`,
`update_entry`'s `snapshot_stale`) is satisfied by an **index-table row**; the citation
resolver (`link_scan`) requires a **heading** of the form `## <ID> — <title>`. So an entry
written as a row only is reported by the tool that created it as fully present, while every
citation of it — from any file, forever — silently resolves to nothing. The one advisory
designed to tell the author "the body still owes this entry something" goes quiet at exactly
the point where citations break.

Two populations, and the second is the one that matters. The **hybrid** failure is an author
slip: `docs/trackers/prompt-hamsa-audit-log.md` has `## A-1`…`## A-14` and then ten entries
(A-15…A-24) that only ever got index rows — 25 dangling cross-file citations. The **by-design**
failure needs no slip at all: a ledger that follows `get_guide("tracker-conventions")`
§ *One entry format, never two* down its second branch — index **rendered from params** — emits no
headings, so **not one of its entries is ever citable.** `docs/trackers/open-issue-work-queue.md`
is that shape, correctly built, and **zero** `BL-N` tokens are defined anywhere in the repo
against 117 cross-file citations. The guide's two sanctioned formats are not equivalent, and
nothing tells anyone.
## Symptom (Effect)

`artifact(action="update_entry", …)` on the hamsa audit log's `A-25` returned success with a
drift advisory that named the row as *present but stale* — never as *undefined*:

```
"snapshot_stale": "This tracker renders a snapshot in its body, and its `A-25` row still
shows the PREVIOUS field values — params changed, the file did not."
```

At that moment `A-25` had an Index row and no defining heading, so `link_scan` counted every
citation of it as dangling. Adding one heading changed this, with nothing else touched:

```
before:  edges_missing 9 → the 4 A-25 citers resolved to NOTHING;  dangling 625
after:   edges_missing 4 → all 4 became real edges;                dangling 618
```

The four citers that had been pointing at nothing:

```
docs/trackers/2026-08-15-tool-usage-investigation.md   → docs/trackers/prompt-hamsa-audit-log.md
docs/trackers/open-issue-work-queue.md                 → docs/trackers/prompt-hamsa-audit-log.md
docs/trackers/tool-usage-patterns.md                   → docs/trackers/prompt-hamsa-audit-log.md
docs/issues/archive/2026-08-15-il1-always-loaded-…md    → docs/trackers/prompt-hamsa-audit-log.md
```

## Reproduction

At `1a5d7cd1` on `experiments`:

1. Take any augmented tracker with a hand-maintained index table and a params
   `entry_collection` — `docs/trackers/prompt-hamsa-audit-log.md` (`59ebeebb6ed05c89`) is one.
2. `artifact(action="append_entry", id=…, entry_collection="audits", id_prefix="A", entry={…})`.
3. Add **only** the index-table row: `| A-26 | … |`.
4. `artifact(action="update_entry", …, entry_id="A-26", fields={…})` → the response reports
   `snapshot_stale` (row present, values behind) or nothing at all. It never says the entry
   defines no token.
5. Cite `A-26` from another artifact, then `librarian(action="link_scan")` → the citation is
   reported under `dangling`, and no edge is created.

## Environment

codescout v0.15.0, `experiments` @ `1a5d7cd1`, Linux, stdio MCP. Not
platform-sensitive — both predicates are pure string/regex functions.

## Root cause

Two predicates, written for different jobs, both used as "is the entry in the body":

- **Citability** — `def_re()`, `src/librarian/tools/link_scan/extract.rs:94-97`:
  `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`, and it is applied **only while in a heading**
  (`in_heading && heading_first_inline`, `extract.rs:157-163`). A table row is not a heading,
  so it defines nothing. This is deliberate and documented in the fn's own doc comment.

- **Drift detection** — `body_claimed_indices()`,
  `src/librarian/catalog/augmentation.rs:1133-1143`:
  ``(?m)^(?:#{1,6}[ \t]+|\|[ \t]*)[`*\[]*<PREFIX>-(\d+)\b`` — a heading **or** the leading cell
  of a table row. It feeds `snapshot_missing` (`augmentation.rs:601`) and
  `snapshot_stale_note` (`augmentation.rs:412`, `:434`), and the row-or-heading breadth is
  pinned by `body_claimed_indices_reads_headings_and_index_rows` (`augmentation.rs:1910-1920`).

Measured 2026-08-18, not inferred: `grep '^#{1,4} A-(1[5-9]|2[0-9]) '` over `**/*.md` returns
**exactly one** hit — the `A-25` heading written today. `A-15` through `A-24` have Index rows
and params rows and no defining heading anywhere in the repo.

The breadth is **correct for its original purpose and wrong for this one.** For *id
allocation* a row is sufficient evidence that the number is taken, and over-allocating is
safe — that is what the doc comment argues, and it is right. The defect is that the same
predicate then answers a different question, "does the body carry this entry", where a row is
**not** sufficient, because the citation resolver will not accept it.

Sibling, and why this is not a duplicate:
`docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`
(`0694a4a9946e10fe`, `mitigated`) added `snapshot_missing` so a params-only write would stop
being silent. This bug is that the predicate that fix chose is too permissive to catch the
one drift shape that breaks citations — the fix reports the *values* being stale while the
*identity* is absent.

## Evidence

### The population, in one tracker

`docs/trackers/prompt-hamsa-audit-log.md` has body sections `## A-1` … `## A-14`, then
nothing until the `## A-25` heading added today. **Ten entries** (A-15…A-24) are row-only.

`grep -c` over `**/*.md` for `\bA-(1[5-9]|2[0-4])\b` — 36 citations across 10 files:

| File | Citations | Live surface? |
|---|---|---|
| `docs/trackers/prompt-hamsa-audit-log.md` | 11 | self (Index rows) |
| `docs/issues/archive/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md` | 7 | archived |
| `docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` | 6 | **yes — open bug** |
| `docs/trackers/tracker-hygiene-log.md` | 3 | **yes** |
| `docs/superpowers/specs/2026-07-10-project-activation-bootstrap-guide-design.md` | 2 | **yes** |
| `docs/trackers/bistriceanu/agent-behavior-analysis.md` | 2 | **yes** |
| `docs/trackers/reconnaissance-patterns.md` | 2 | **yes** |
| 3 further archived bug files | 3 | archived |

**25 cross-file citations resolve to nothing; 15 of them sit on live surfaces.** No prefix
collision is masking this: `A-N` is a single-ledger namespace here, so these are *dangling*,
not *ambiguous* — they need a definition, not a qualifier.

### The controlled single-heading measurement

The cleanest evidence is a before/after on one heading, everything else held constant. Two
`link_scan(write=true)` runs, minutes apart, with one `## A-25 — …` section added between
them and two stale hex ids removed:

| | before | after |
|---|---|---|
| `edges_desired` | 770 | 774 |
| `edges_missing` | 9 | 4 |
| `dangling` | 625 | 618 |
| `self_cites` | 640 | 641 |

Four of the seven recovered danglings are the A-25 citers; the `self_cites` +1 is the audit
log's own Index row, which became a resolvable self-reference the moment the heading existed.

### The bigger population: a params-rendered index defines NOTHING, and that is the blessed design

Measured after filing, and it enlarges the bug rather than confirming it. `get_guide("tracker-conventions")`
§ *One entry format, never two* offers two sanctioned shapes: **the headings are the index**, or
**the index is rendered from `params`**. Only the first produces citable entries.

`docs/trackers/open-issue-work-queue.md` (`9a892c2a5976e296`) is the second shape, done correctly —
rows in `params`, a rendered snapshot in the body, no hand-maintained duplicate. It has **zero**
defining headings:

```
grep '^#{1,6}[ \t]*[`*\[]*BL-\d+' **/*.md   →  0 matches   (repo-wide, 2026-08-18)
grep '\bBL-\d+\b'                          →  229 matches in 38 files
                                                112 in the queue itself
                                                117 cross-file
```

So **no `BL-N` token is defined anywhere in this repo**, and all 117 cross-file citations of the
project's most-cited ledger resolve to nothing — a large share of the 618 project-wide danglings.
The queue is not misused; it is used exactly as documented. Following the guide's second option
produces a ledger whose entries can never be cited, and no surface says so.

That is the finding this bug should be read for. The A-15…A-24 gap above is the *hybrid* failure
(body sections exist, ten entries missed them). This is the *by-design* failure, it is 4.7× larger,
and no amount of author diligence prevents it.

### The advisory's breadth, demonstrated live while filing this

`append_entry` for this bug's own queue row returned:

```
"snapshot_missing": ["BL-39"],
"snapshot_hint": "This tracker keeps a rendered snapshot in its body, and 1 row(s) are not in it…"
```

Correct, and it went quiet the moment the table row was added — while `BL-39` remained, and
remains, undefined and uncitable. The advisory tracked the thing that was already fine and stayed
silent about the thing that was broken. This is the defect in one exchange.
### Why an author lands here without noticing

The tracker's documented append flow is three steps — allocate, row, body — and only the
first is enforced. `append_entry` on a params ledger writes the row and returns; the response
carries `snapshot_missing` *for ids the body does not claim at all*, which an Index-row author
satisfies immediately. From the author's side every tool call returned success and the file
visibly gained the entry. Nothing in the loop distinguishes "visible in the table" from
"citable". Compare `docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md`
(`63d36f5da3b200a7`, open) — the same three-jobs-per-entry cost, seen from the effort side
rather than the correctness side.

## Hypotheses tried

1. **Hypothesis:** the citations dangle because `A-N` is ambiguous across several ledgers, as
   `F-N`/`W-N` are.
   **Test:** `grep '^#{1,4} A-(1[5-9]|2[0-9]) ' **/*.md`; inspected `link_scan`'s `ambiguous`
   vs `dangling` buckets for these tokens.
   **Verdict:** rejected — one hit repo-wide, and the tokens report under `dangling`. A
   qualifier would not help; only a definition will.

2. **Hypothesis:** `link_scan` is at fault for refusing table rows.
   **Test:** read `def_re` and its call site, `extract.rs:91-97` and `:155-163`.
   **Verdict:** rejected. Refusing rows is deliberate and load-bearing: rows are duplicated,
   reordered and rewritten wholesale, so treating them as definitions would make the link
   graph a function of table formatting. The resolver is right; the *advisory* is wrong.

3. **Hypothesis:** widen `body_claimed_indices` to headings only, so the advisory fires.
   **Test:** read its doc comment and the pinned tests at `augmentation.rs:1910-1935`.
   **Verdict:** rejected — that would break id allocation. A row *is* valid evidence the
   number is taken, and narrowing the allocator's view would reissue ids over live rows,
   which is precisely `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`.
   The two questions need two predicates, not one retuned one.

## Fix

Not implemented. **Additive — do not retune `body_claimed_indices`;** hypothesis 3 records
why narrowing it re-opens a closed bug.

**Step 0 — settle the design fork first, because steps 1-3 mean different things on either side
of it.** A params-rendered ledger has no headings by construction, so "warn per undefined entry"
would fire on every single append to the busiest tracker in the repo, forever. That is not an
advisory, it is noise, and shipping it would train the next author to ignore the field. Pick one:

- **(a) Rendered ledgers must render a definition.** Require a `render_template` to emit
  `## <ID> — <title>` per entry (or a heading plus its row), and treat a template that cannot as a
  `tracker_design` validation error. Citations then work identically under both sanctioned formats,
  which is what the guide already implies. Cost: every existing rendered ledger's body grows and has
  to be re-rendered once.
- **(b) The resolver learns the rendered-row shape, scoped to ledgers that declare it.** Let a
  leading-cell row define a token *when the artifact declares `entry_prefix` and an
  `entry_collection`* — i.e. exactly where rows are machine-generated and cannot be reordered by
  hand. Cost: the resolver gains a second definition rule, and hypothesis 2's argument (rows are
  rewritten wholesale, so they make the link graph a function of table formatting) has to be
  re-checked against the generated case.

(a) is the smaller change to the resolver and the larger change to files; (b) is the reverse. **This
is a decision, not a detail — do not let an implementer pick it silently mid-patch.** My read is
(a): it keeps exactly one definition rule in the codebase, and the failure mode of a wrong template
is visible in the diff, where a second resolver rule's failure mode is invisible.

Then, whichever branch:

1. **One source of truth for "defines a citable token." — DONE, `de4df2cd`.**
   `body_defined_indices(body, id_prefix)` now sits beside `body_claimed_indices` in
   `src/librarian/catalog/augmentation.rs`. **Implemented differently from the plan above,
   and better:** rather than *lifting* `def_re` into a shared helper, it **calls
   `link_scan::extract` outright** and filters `definitions` by prefix. Same intent — one
   rule — achieved with zero new predicate rather than one new shared one, which is
   stricter about the thing this bug is about. It also inherits the cmark-accurate cases a
   line regex gets wrong: fenced blocks, code-first headings, setext headings, frontmatter.
   No new layering — `augmentation.rs:2` already imports from `librarian::tools`.

   Marked `#[cfg_attr(not(test), expect(dead_code))]` until its consumer lands. `expect`
   rather than `allow` so the marker fails the build and deletes itself the moment a
   production caller appears, instead of rotting as a permanent exemption; scoped to
   `not(test)` because "unused" is configuration-dependent here — the tests do call it, so a
   bare `expect` is itself an `unfulfilled_lint_expectations` error under `-D warnings`
   with `--all-targets`.

2. **A distinct advisory, because the remedy is distinct.** Add `undefined_in_body` to
   `AppendOutcome` and `UpdateEntryOutcome`, populated from the new predicate: the entry has a
   row (so `snapshot_missing` stays quiet, correctly) but no definition. Word it as the action
   owed — *"`A-26` has an index row but no `## A-26 — <title>` heading, so every citation of
   it will dangle"* — not as a status. Keep it advisory and best-effort, matching
   `snapshot_stale_note`'s existing contract (`augmentation.rs:431-434`): a missing advisory
   must never fail a committed write. Under (a) this fires only on hand-maintained ledgers;
   under (b) it fires only where the declaration is absent.

3. **Surface it where the count is already computed.** `librarian(action="doctor")` reports
   catalog drift and already touches this area (`src/librarian/tools/doctor.rs`); a
   `row_without_definition` check there gives a repo-wide sweep, which is what finds the
   pre-existing entries rather than only the next one written. This is also the only step that
   surfaces the 117 `BL-N` citations — a per-write advisory never will, because those entries were
   written months ago.

4. **Backfill.** Ten `## A-N — <title>` sections in `docs/trackers/prompt-hamsa-audit-log.md`,
   promoting each Index row's content per the compaction ladder in
   `get_guide("tracker-conventions")` § *Compaction and archival*; plus whatever branch (a) or (b)
   implies for the rendered ledgers. Expected effect, extrapolating from the A-25 measurement: on
   the order of 25 dangling citations recovered for A-N and up to 117 for BL-N. **Do this after
   (1)-(3) land**, so the new check confirms the backfill rather than the backfill hiding the
   absence of a check.

5. **Fix the guide in the same change.** `get_guide("tracker-conventions")` presents the two entry
   formats as equivalent choices. Until step 0 lands they are not, and the guide is the reason an
   author picks the one that silently breaks citations. Whichever branch wins, the guide has to say
   what a rendered ledger must emit to be citable.

Deliberately out of scope: the wider "one entry format, never two" question for the hamsa tracker
(its Index table is hand-maintained alongside body sections). That is a design change for the
tracker, tracked as BL-30 / `63d36f5da3b200a7`, and it does not need to be settled before the
predicate is fixed.
## Tests added

Three, all in `src/librarian/catalog/augmentation.rs` tests, landed with `de4df2cd`.

**The RED was against the real defect, not against absence.** All three were first run with
a deliberately-wrong stub whose body was `body_claimed_indices(body, id_prefix)` — i.e.
literally this bug — and all three failed while all four existing `body_claimed_indices`
tests stayed green:

```
an_index_row_without_a_heading_is_claimed_but_not_defined   left: {3,4,5,7}   right: {3}
defined_indices_delegate_to_link_scans_own_definition_rule  left: {1,2,3,4,5} right: {1}
body_defined_indices_is_empty_when_the_body_defines_nothing assertion failed: is_empty()
```

That run **is** the mutation check the test comments promise — it was performed before the
real implementation existed, so the tests are known to detect the defect rather than merely
to describe the fix.

- `an_index_row_without_a_heading_is_claimed_but_not_defined` — asserts **both** predicates
  on the *exact fixture* `body_claimed_indices_reads_headings_and_index_rows` already pins,
  so the disagreement is visible on identical input. Four ids claimed, one defined: `F-7`
  and `F-4` are table rows, and `F-5` is `###### **F-5** z`, a heading with no ` — title`,
  which the resolver reads as a section *about* F-5.
- `defined_indices_delegate_to_link_scans_own_definition_rule` — dashless heading,
  code-first heading, heading inside a fence, table row. Every case is already pinned by
  `link_scan`'s own tests; this one exists so a later "optimisation" that swaps the
  delegation for a local regex has to reproduce all of them. Re-approximating the rule in a
  second place is how the two predicates drifted apart to begin with.
- `body_defined_indices_is_empty_when_the_body_defines_nothing` — encodes the 117-`BL-N`
  measurement as a regression guard: a params-rendered index defines **zero** tokens, which
  is a legitimate whole-ledger shape, so the advisory in step 2 must not read it as
  per-entry breakage.

**Still unwritten**, because they belong to steps 2-3 and their expected behaviour depends
on the step-0 fork:

- `append_entry_reports_undefined_in_body_for_a_row_only_entry` and its negative twin
  `…_stays_silent_for_a_prose_only_tracker`.
- `shared_definition_rule_matches_link_scan` — now partly redundant: with the
  implementation delegating rather than duplicating, agreement is structural rather than
  asserted. `defined_indices_delegate_to_link_scans_own_definition_rule` is what keeps it
  that way.
## Workarounds

Write the `## <ID> — <title>` heading for every entry, and do not trust the write path's
silence as confirmation. To check a ledger by hand:

```
grep -c '^#\{1,4\} A-[0-9]' docs/trackers/prompt-hamsa-audit-log.md   # definitions
artifact(action="get", id="59ebeebb6ed05c89", entry_filter={})        # params rows
```

A gap between those two counts is this bug. Repo-wide, `librarian(action="link_scan")`'s
`dangling` bucket already lists every affected token — it is reported, just not gated.

## Resume

**Step 1 is done (`de4df2cd`) — the predicate exists, is tested, and is mutation-verified.**
What is left is blocked on a decision, not on code.

Decide **step 0**: (a) rendered ledgers must emit a definition, or (b) the resolver accepts a
generated row as one. Everything remaining reads differently depending on the answer — under
(b) a row-only entry is citable, so the step-2 advisory would be *wrong* to fire. Do not let
an implementer pick this silently mid-patch; whichever is convenient at that line is not the
same as whichever is right. Recommendation and the cost of each side are in § Fix. My read is
(a): one definition rule in the codebase, and a wrong template fails visibly in a diff where
a second resolver rule's failure mode is invisible.

Once that is settled, steps 2-3 in order — the `undefined_in_body` advisory, then the
`doctor` sweep. **Step 3 is the only one that surfaces the entries already broken**; a
per-write advisory never will, because those 25 `A-N` and 117 `BL-N` citations were written
weeks ago.

Do **not** start with the backfill (step 4). It is the visible half and the tempting one, and
doing it first removes the evidence that the check is missing while leaving the next row-only
entry just as silent.

One loose thread worth knowing before step 2: the RED run showed `body_claimed_indices` also
counts a heading inside a fenced block and a code-first `` `A-3` `` heading (`{1,2,3,4,5}` on
the delegation fixture). Harmless for id allocation — over-claiming is safe, by that
function's own documented argument — but it means the *claimed* set is not merely "defined
plus rows", and any step-2 wording contrasting the two should not imply that it is.
## References

- `src/librarian/tools/link_scan/extract.rs:91-97`, `:155-163` — the definition rule.
- `src/librarian/catalog/augmentation.rs:1133-1143` — `body_claimed_indices`; `:601`
  `snapshot_missing`; `:412`, `:431-434` `snapshot_stale_note`; `:1910-1935` the pinned tests.
- `get_guide("tracker-conventions")` § *Entry headings — the definition rule* and
  § *Compaction and archival* — the documented standard this violates.
- `docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`
  (`0694a4a9946e10fe`) — the sibling whose fix chose the too-permissive predicate.
- `docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md`
  (`63d36f5da3b200a7`) — the same three-jobs-per-entry cost from the effort side.
- `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`
  (`7840cd749194cdf5`) — why the allocator's row-tolerant view must not be narrowed.
- Found while archiving `docs/issues/archive/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md`
  (`b4d48dbfecc205c9`): re-pointing its citations produced four that resolved to nothing.

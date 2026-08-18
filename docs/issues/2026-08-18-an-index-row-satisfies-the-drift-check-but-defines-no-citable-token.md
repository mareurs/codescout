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

1. **One source of truth for "defines a citable token."** Lift `def_re`'s heading rule out of
   `link_scan` into a shared helper and add `body_defined_indices(body, id_prefix)` beside
   `body_claimed_indices` in `src/librarian/catalog/augmentation.rs`. Both the resolver and
   the write path then read the *same* rule, so the two cannot drift again — the drift is
   this bug. This step is identical under (a) and (b) and can land first, alone.

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

None yet — nothing is implemented. What the fix must pin:

- `an_index_row_without_a_heading_is_claimed_but_not_defined` — the two predicates give
  different answers for `| A-26 | … |`, which is the whole bug. Mutation check: make
  `body_defined_indices` accept rows and this test must go red while
  `body_claimed_indices_reads_headings_and_index_rows` stays green.
- `append_entry_reports_undefined_in_body_for_a_row_only_entry`, and its negative twin
  `…_stays_silent_for_a_prose_only_tracker` — an artifact whose body claims **no** ids is a
  legitimate params-canonical design (5 of 28 augmented trackers here, per
  `body_claimed_indices`' doc comment) and must not be told it is broken on every write.
- `shared_definition_rule_matches_link_scan` — assert the shared helper and `def_re` agree on
  a table of headings (`## A-9 Addendum` → no, backtick-first → no, `## A-9 — t` → yes). This
  is the regression guard against the two rules drifting apart again.

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

Decide **step 0** before writing any code — (a) rendered ledgers must emit a definition, or (b) the
resolver accepts a generated row as one. Everything else reads differently depending on the answer,
and an implementer who picks it silently mid-patch will pick whichever is convenient at that line.
Recommendation and the cost of each side are in § Fix.

Step 1 can land first and alone regardless: add `body_defined_indices` next to
`body_claimed_indices` in `src/librarian/catalog/augmentation.rs`, sharing `link_scan`'s heading
rule (`src/librarian/tools/link_scan/extract.rs:94-97`), with
`an_index_row_without_a_heading_is_claimed_but_not_defined` written first and watched fail. That
single predicate is what every later step consumes, and it is verifiable on its own.

Do **not** start with the backfill (step 4). It is the visible half and the tempting one, and doing
it first removes the evidence that the check is missing while leaving the next row-only entry just
as silent.
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

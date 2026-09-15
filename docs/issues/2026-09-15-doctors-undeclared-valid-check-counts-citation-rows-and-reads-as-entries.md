---
id: '2155678e29cbd91a'
kind: bug
status: open
title: 'BUG: doctor''s undeclared-Valid check counts citation rows, and its worklist reads as entries — 18 rows, 9 entries'
tags:
- cluster/unclassified
---

## Summary

`librarian(action="doctor")`'s `entry_cited_from_outside_but_undeclared` emits **one violation row
per citation edge, not per entry**, and `summary.by_check` counts rows. So the number a triage reads
as its worklist size is inflated by the citation count of its most-cited members.

Measured 2026-09-15 at `HEAD`, on this repo: `by_check` reports **18**. The distinct entries are
**9** — `CAP-7`, `A-14`, `A-15`, `A-38`, `R-53`, `R-98`, `SKF-22`, `T-001`, `T-013`. `A-38` accounts
for **ten** of the eighteen rows, all byte-identical: same `check`, same `artifact_id`
(`59ebeebb6ed05c89`), same `path`, same `detail` string. Nothing in the row distinguishes one
instance from another, so a reader deduplicating by eye has no key to deduplicate on.

This is § *Testing Discipline*'s **"a count of a defect population must arrive with its unit or not
at all"** — but committed by the instrument rather than by a session, which is what makes it worth a
file. The remedy text on each row (*"this is a worklist, not a verdict"*) correctly tells the reader
it is a worklist and then hands them a length in the wrong unit.

## Symptom (Effect)

A triage plans nine judgements and budgets for eighteen — or, worse, reports "18 entries need a
`**Valid:**` class" in a status line nobody re-derives. The error is not random: it scales with how
heavily the worst member is cited, so the most-cited entry — exactly the one the check exists to
prioritise, since it is priced by exposure — is the one that distorts the count most.

Absence of a tell is the operative part. The rows carry no ordinal, no citing-file field, and no
edge id, so ten copies of one finding are indistinguishable from ten findings until the entry names
are read out and compared.

## Reproduction

```
librarian(action="doctor")
read_file("@tool_<id>", json_path="$.summary")          # by_check.entry_cited_from_outside_but_undeclared == 18
grep '"detail": ".*is cited [0-9]+× from other files' @tool_<id>   # 18 rows, 9 distinct entry ids
```

## Environment

- codescout `experiments` at `ca7edebc`, this checkout, project scope.
- The scan is project-scoped; `catalog_health.entry_validity_scoped_by_project` reports a further
  117 entry-validity rows across 8 other roots, so the repo-wide row/entry ratio is unmeasured here.

## Root cause

**Not established — do not read the above as a diagnosis of the emit path.** What is measured is the
output: ten identical rows for one entry, against four text occurrences of `A-38` outside its own
ledger at `HEAD`. Per-citation emission is the obvious hypothesis and it does not survive
arithmetic, because ten rows do not match either four occurrences or the row's own `cited 5×`.

## Evidence

**The duplication, read from the buffer rather than counted from a summary:** rows at offsets 847,
853, 859, 865, 871, 877, 883, 889, 895, 901 are byte-identical.

**A second discrepancy, stated as an open question and NOT as a defect.** Three numbers that should
agree do not:

| number | source | unit |
|---|---|---|
| 4 | `git grep -o 'A-38'` over the three citing files, at `HEAD` | text occurrences |
| 5 | the violation row's own `detail` (*"cited 5× from other files"*) | catalog `cites` edges |
| 10 | the row count | unknown |

The first two measure different things and are not obliged to agree — a text grep is not an edge
count, and § *Testing Discipline* warns specifically that a headline from the tool beside an
enumeration from the eye reconciles nowhere. Recorded so the next reader does not re-derive it and
conclude the tool is broken on the strength of the 4-vs-5 alone.

**Two of the four text occurrences are not content citations, which bears on whether `A-38` belongs
on this list at all** (the check is priced at `exposure ≥ 5`):

- `docs/plans/2026-09-09-tool-collapse-post-merge-queue.md:101` reads ``` `A-1`…`A-38`; **next id is
  A-39.** ``` — a **range boundary**, naming the id space rather than citing the entry. This is
  `IC-6`'s mention-vs-citation trap: the grammar has no way to write "the token A-38" without
  citing it.
- `docs/trackers/prompt-surface-compaction-session-log.md:2056` reads
  ``` `docs/trackers/prompt-hamsa-audit-log.md:A-38` ``` — **file-path-qualified**, which
  `link_scan` parses as a cross-repo token with `raw: "md:A-38"`, i.e. repo `md`. `link_scan` has a
  dedicated `cross_repo_file_qualified` bucket holding **16** of these, so the class is known; what
  is unknown is whether `doctor`'s exposure count folds it in, double-counts it, or both.

## Hypotheses tried

None beyond the greps above. Filed on notice rather than investigated, per `CLAUDE.md` § *Bug
Tracking* — the count is wrong today whatever the mechanism turns out to be.

## Fix

Not designed. The reader-facing half is cheap and independent of the cause: **publish the distinct
entry count beside the row count**, or give each row a key that makes the duplication visible
(citing file, edge id). Either makes the worklist length honest without settling the emit path.

Do **not** "fix" this by deduplicating rows in the report before the cause is known — if the ten
rows turn out to carry a real distinction the payload does not currently serialise, collapsing them
destroys the evidence rather than the defect.

## Resume

Start with the emit site for `entry_cited_from_outside_but_undeclared` in
`src/librarian/tools/doctor.rs` and answer one question first: **what is the loop iterating over?**
Ten rows against `cited 5×` in their own text means the row count and the detail's count come from
different collections, and naming those two collections is the whole diagnosis.

Then, separately: the nine entries genuinely do lack a `**Valid:**` class and that work stands
regardless — except for `A-38`, whose exposure should be re-derived once the count is trusted,
since two of its four textual mentions are a range boundary and a file-qualified form.

## References

- `src/librarian/tools/doctor.rs` — the check and its emit path
- `docs/trackers/prompt-hamsa-audit-log.md` (`59ebeebb6ed05c89`) — holds `A-38`
- `CLAUDE.md` § *Testing Discipline* — "a count of a defect population must arrive with its unit or
  not at all", and "count the LIST, never the corpus"
- `CLAUDE.md` § *Parsers Over a Namespace* — the mention-vs-citation half

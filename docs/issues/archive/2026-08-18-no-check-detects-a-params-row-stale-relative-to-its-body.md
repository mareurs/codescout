---
id: '640c4fc65a64461c'
kind: bug
status: fixed
title: 'BUG: no check detects a params row whose content has gone stale relative to its body counterpart'
tags:
- librarian
- params-drift
- snapshot-stale
- windows-platform-support
topic: tracker-entry-identity
closed: 2026-08-30
no_fix_commit: true
unverified: 'The check is a HEURISTIC and its limits are measured, not estimated: sensitivity 91.4% over 536 simulated drifts, so ~1 real disagreement in 12 is silent; it covers 4 of 9 params-backed ledgers here, the other 5 declaring no `status` enum and therefore having no closed vocabulary to compare against; and a status region whose prose merely mentions another enum word is reported despite being correct. All three are stated on the scan and in its violation message. A clean run is evidence, not proof.'
---

## Summary

BL-42's data repair fixed six `WIN-N` params rows that existed in the body and not in
`params` at all. Repairing that surfaced a second, distinct defect on the same tracker:
**seven rows that existed in both, with different content** — `params` held an earlier,
sometimes-wrong snapshot while the body had moved on. No check reports this. `doctor`'s
`params_behind_body` (BL-40) only detects a row's *absence*; it is silent about a row
whose fields have simply gone stale.

## Symptom (Effect)

Confirmed live in `docs/trackers/windows-platform-support.md` (id `52451519052d207c`),
found by diffing the rebuilt-from-params table against the markdown mirror table
row-for-row, not by inspection:

| id | fields stale in `params` | severity |
|---|---|---|
| `WIN-28` | `status` (`open` vs body's `fixed`), `summary` (an earlier, superseded root-cause hypothesis), `ref` | **high** — a query for open issues returns a closed one with the wrong story |
| `WIN-29` | `area`, `status` (`open` vs `fixed`), `summary` (entirely different text — an earlier "undiagnosed" note vs the later "closed as duplicate of WIN-28" resolution), `ref` | **high**, same mechanism |
| `WIN-1`, `WIN-4`, `WIN-5`, `WIN-20`, `WIN-27` | `ref` only — pre-archive path (`docs/issues/X.md` vs the post-archive `docs/issues/archive/X.md`) | low — cosmetic, but the same root cause |

The high-severity two are not cosmetic: this tracker's own augmentation prompt names
`entry_filter={"status":{"eq":"open"}}` as the canonical way to find live work. Before this
fix, that query returned `WIN-28` and `WIN-29` as open Windows-platform issues — both
long since fixed — with a summary describing a root cause the later investigation had
already superseded.

## Reproduction

```
artifact(action="get", id="52451519052d207c", entry_filter={"status": {"eq": "open"}})
# before the fix: WIN-28, WIN-29 appear here, wrongly

# The general method — diff the body's own mirror table against params, field by field:
artifact(action="get", id="52451519052d207c", heading="## Issue index")   # body table
artifact(action="get", id="52451519052d207c")                            # params.issues
# then compare row-for-row on (area, status, summary, ref, since)
```

## Root cause

Two closed-loop maintenance paths exist and neither one is enforced:

- Editing the **body** (`artifact(update, patch={body_edits:...})`) does not touch
  `params`, and nothing warns that the mirror table just diverged from the structured
  side.
- Editing **params** (`update_entry`) *does* return a same-response warning —
  `"snapshot_stale"`, pointing back at the body — but only for the row just written, and
  only in the direction params→body. There is no equivalent scheduled or manual scan in
  the other direction: a body edit that never touches params leaves no trace at all.

`doctor`'s `params_behind_body` (BL-40) closes the *existence* half of this — a row
missing from params entirely. It does not, and was never meant to, close the *content*
half: a row present in both with different field values passes it silently, because the
check only computes set difference on ids, never on field equality.

## Fix

**Repaired the seven known-stale rows** (this session, `update_entry` — safe because
every affected row already existed; no wholesale rewrite needed). Verified by re-diffing
body vs. params on all 35 `WIN-N` rows after the fix: zero field mismatches remain.

**Not fixed: the detection gap itself.** No `doctor` check compares an existing params
row's fields against its body counterpart. A candidate shape: for every artifact with an
`entry_collection` and a body table that mirrors it (detectable by the same
`render_template`/table-heading convention `tool-usage-patterns`-style trackers already
use), parse the table's own rows and diff them field-by-field against `params`, reporting
`(entry_id, field)` pairs that disagree. This is a genuinely new check, not an extension
of `params_behind_body` — that one only ever needed a set of ids; this one needs the
parsed table content, which the codebase does not currently reuse (the mirror tables are
hand-formatted per-tracker, not rendered from a shared template that "reindex_from_body"
logic could run on).

Left as a design question rather than shipped here: whether the fix should be a
**doctor check** (report-only, matching `params_behind_body`'s shape) or a **write-time
guard** (warn on any body edit to a heading matching a known mirror-table pattern,
symmetric to the `snapshot_stale` warning `update_entry` already gives the other way).
The former is cheaper and matches precedent; the latter catches the drift at the moment
it is created rather than on the next manual scan.

## Status

`mitigated`, not `fixed`: the seven known-bad rows are repaired and verified, but the
mechanism that let them go stale (editing one side and not the other, with no check on
either side of *this* direction) is still fully present. The same class of drift can
recur on this tracker or any other mirrored one the next time someone edits a body table
row without also calling `update_entry`.

## Second instance — 2026-08-30, `open-issue-work-queue.md`, 4 rows

**Valid:** dated 2026-08-30

**Rests on:** this file's own § Summary and § Resume; queue row `BL-44`.

§ Resume asked for "the repro's row-for-row diff across the corpus … no sweep of other
mirrored trackers has been done". One more tracker has now been swept, incidentally, by a
session resuming from compaction that did not know this file existed. It drifted too.

`docs/trackers/open-issue-work-queue.md` held **four** rows present on both sides with
disagreeing content — `BL-49`, `BL-56`, `BL-60`, `BL-64` — and in every one, `params`
held the older and wronger side:

| row | params said | committed body said |
|---|---|---|
| BL-49 | `open` — "the hint actively misleads" | `partial` — hint fixed, cross-repo half open |
| BL-56 | `open` — "worth a root cause" | `zombie 2026-08-30`, hypotheses acquitted |
| BL-60 | `open` — "Not yet scouted by this session" | `done-archived`, SHA + patch-id |
| BL-64 | credits "hypothesis 4" | (body silent; the bug file settles it as 9) |

BL-60 is the sharp one: it read `open` and "not yet scouted" for a bug that had been
fixed, tested, archived, SHA-and-patch-id'd, **and** re-verified against the shipped
release binary. Its `bug` field also still pointed at `025ff58280c36d07`, the pre-archive
id, which now returns `unknown artifact id` — so the row was stale in its citation as well
as its content.

**The drop-time prediction fired verbatim.** BL-44 was dropped on the judgement that the
detection gap was "a design decision, not a queued task". The BL-42 detail section had
already written down what that would cost: *"a canonical `entry_filter={"status":{"eq":"open"}}`
query — the exact pattern this tracker's own augmentation prompt recommends — would have
surfaced two closed issues as open, with the wrong explanation."* That is exactly what
happened, on a different tracker, twelve days later, to an agent with no knowledge of the
prediction. Two of two mirrored trackers ever swept have drifted.

**And the workflow itself causes it.** The stale rows were created by `0131e504` and
`8dd4b910` — commits that correctly updated the body table via `body_edits` and simply did
not issue a matching `update_entry`. Nothing reported it, because both surfaces that could
have are **id-set** comparisons: `update_entry`'s `snapshot_stale` computes
`claimed.difference(&in_body)`, and `doctor`'s `params_behind_body` (shipped for BL-40)
computes the reverse — but every id was present on both sides, so both are silent by
construction. The asymmetry is also one-directional in *warning*: moving params without the
body is reported, moving the body without params is not.

Rows repaired 2026-08-30 via four `update_entry` calls plus a body edit. The gap is
untouched, and `BL-44` is re-opened.

## Exposed population — measured 2026-08-30

**Valid:** dated 2026-08-30

**Rests on:** `artifact(find, kind="tracker", augmented=true)` against this project's catalog.

§ Resume asks for a corpus sweep "to size the existing damage" without saying how large the
corpus is. It is **9**, and the bound is structural rather than a sample.

The drift needs two sides to disagree, so it requires a params array mirrored by a body
table — i.e. a declared `entry_collection`. A **prose ledger is immune by construction**:
its entries are `## PREFIX-N` body sections with an `entry_high_water_<PREFIX>` in committed
frontmatter and no params at all, so there is no second copy to fall behind. Scoping
contributed by a peer session whose own ledger
(`docs/trackers/resume-embedding-transport-stages-1-3.md`, ET-N) returns
`augmentation: null`; verified here — it is absent from the augmented set.

Measured: **9 augmented trackers in this project, and all 9 declare an `entry_collection`** —
`open-issue-work-queue` (`tasks`), `windows-platform-support` (`issues`),
`tool-usage-patterns` (`observations`), `provenance-subsystem` (`items`),
`prompt-hamsa-audit-log` (`audits`), `legibility-backlog` (`candidates`),
`fable-tuning-tasks` (`tasks`), `fable-tuning-findings` (`findings`),
`docs/research/README.md` (`entries`). So "augmented" and "exposed" coincide exactly here,
and `augmented=true` is a sufficient query for the sweep.

**Two of the nine have ever been checked, and both had drifted** —
`windows-platform-support` (7 rows, 2026-08-18) and `open-issue-work-queue` (4 rows,
2026-08-30). That is 2 for 2, on trackers picked for unrelated reasons rather than because
drift was suspected. **Seven remain unchecked**, which makes the sweep a bounded task, not
an open-ended one.

Caveat on the number: it counts `kind="tracker"` in **this project's** scope. Artifacts of
another kind carrying an `entry_collection`, and any tracker in a sibling repo under an
umbrella, are outside it — the query was not run at `scope="umbrella"`.

## Fix — SHIPPED 2026-08-30

**`92228de0`** on `experiments`, patch-id **`0ba7a71b3f8462e8`**.

`scan_params_status_drift` runs beside the three id-set scans in `doctor`, consuming the
same `ParamsBackedLedger` — extended with each entry's `(id, status)` and the `status`
enum its `params_schema` declares. Gate green: fmt, clippy, **4873/0** full, **3385/0**
lean. The lean count is unchanged from before the change, which is correct rather than
suspicious: `doctor` sits behind the `librarian` default feature, so that lane compiles
none of this and is not a check on it.

### The dry run reshaped the design three times, and that is the substance

§ *Fix* originally posed this as a design decision — report-only check versus write-time
guard — and that framing turned out to be the wrong question. The real obstacle was the
one `scan_params_behind_body` names in its own doc comment when it declines this
comparison: *"a text comparison against a column whose format is each tracker's own
choice — fragile."* It was right to decline on the evidence then available. Measuring the
fragility changed the answer:

| measurement | result |
|---|---|
| naive comparison, this repo | 26 findings — **20 of them `**done, archived**` against a params value of `done-archived`** |
| after absorbing that one rendering | 6 findings on 101 rows |
| sensitivity (536 simulated drifts) | **490 flagged / 46 silent — 91.4%** |
| ledgers in scope | **4 of 9**, not the 6 first estimated |
| entries reachable | 131, across **two** locators |

The fragility is dominated by a single convention — comma versus hyphen — and absorbing
it takes the check from 77% noise to usable. That is the fact the deferral could not have
known without measuring.

**Sensitivity was measured by a positive control, not asserted.** Substituting every other
enum value for each entry's true status simulates a drift of known shape; 8.6% are silent
because the substituted word already occurs in that entry's own region. The violation
message states that rate. Without the control there would have been six findings and no
evidence the check detects anything at all.

**Coverage was wrong twice before it was right.** The first estimate, 6 of 9, came from
reading `params_schema` and assuming the rendering. Only 2 ledgers render `| ID |` table
rows; checking the other four found 2 more rendering entries as a heading plus a
`**Status:**` line. A table-row-only implementation — which an earlier draft was — skipped
**30 of 131** exposed entries while reporting nothing amiss about them.

### Two decisions a later reader will be tempted to undo

- **A boundary-anchored regex, not stripped punctuation.** Stripping is the obvious way to
  absorb `done, archived`, and it makes `done` a substring of `aban`**`done`**`d` and
  `open` of `re-`**`open`**`ed` — both of which occur in this repo's real status prose.
- **The heading locator returns only the `Status:` line, never the section.** Entry prose
  routinely narrates status history (*"was dropped as a design decision"*), and matching
  against it reports the narration as the status.

Both are pinned by tests that fail under **disjoint** mutations: a `contains` substitution
kills the separator test, and strip-punctuation normalisation kills only the boundary one.

### The mutation matrix found a defect in the tests themselves

Under the first mutation the boundary test died — on its **first** assertion, the positive
separator one, never reaching the assertions its name is about. It reported as a kill for
a guard it had not exercised, and a green-or-red summary cannot show that. The test is
split so each clause fails under its own mutation. Recorded as
`reconnaissance-patterns:R-130`: *a mutation kill is not evidence that the named guard
fired.*

## Resume

If picked up: decide doctor-check vs. write-time-guard (see § Fix), scope it to
artifacts that actually declare `entry_collection` (a bare prose tracker has no params
side to drift from), and re-run the repro's row-for-row diff across the corpus once shipped
to size the existing damage — this session only checked `windows-platform-support.md`,
found because BL-42 already had me looking at it; no sweep of other mirrored trackers has
been done.

## References

- `docs/issues/archive/2026-08-18-no-check-detects-a-body-that-has-run-ahead-of-params.md` (BL-40 — the existence half this defect's content half sits beside)
- `docs/trackers/open-issue-work-queue.md` BL-42 (the data repair whose execution surfaced this)


## Fix provenance

no commit — repaired live via update_entry against the machine-local catalog; the underlying code gap is unfixed

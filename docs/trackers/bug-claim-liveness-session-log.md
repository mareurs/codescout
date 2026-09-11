---
entry_prefix: ["F", "W"]
kind: tracker
owners: ["marius"]
status: active
tags: ["bug-tracking", "status-vocabulary", "peer-sessions", "doctor"]
title: Session Log — Bug-Claim Liveness (taken state)
topic: bug claim liveness
entry_high_water_F: 7
entry_high_water_W: 2
---

# Session Log — Bug-Claim Liveness (`taken` state)

**Work stream.** Designing and shipping a session-backed `taken` status for `docs/issues/`
bug files — a claim carrying the claimer's `sessionId`, plus a `doctor` check that resolves
it against the live session registries so a dead claim cannot masquerade as active. Design
at `docs/superpowers/plans/2026-09-02-bug-claim-liveness-design.md`. Opened 2026-09-02.

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> One call, one write: the server allocates the next id, formats the
> heading as `## F-N — <title>` (the only shape `link_scan` accepts as a
> definition), records the ledger's high-water mark, and stamps
> `**Valid:** dated <today>` unless your body declares a class. **Then**
> add the Index / Wins Index row, using the id the call returned — the
> indexes are the eval surface, the sections are the evidence.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`edit_file` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_file` for the prose sections and the index tables,
> never for allocating an entry.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-09-02 | med | doc-vs-code drift | open | the design's wiring table missed three surfaces because every doc describes `doctor` as a catalog-drift scanner |
| F-2 | 2026-09-02 | med | measurement-method drift | open | enumerating by the subject's identifiers cannot find the surfaces that omit them |
| F-3 | 2026-09-11 | med | codescout-tool | open | doctor's detail text asserts a false cause on a finding whose false-positive class is already a filed bug |
| F-4 | 2026-09-11 | med | self-friction | fixed-verified | I shipped the defect class I had filed an hour earlier, into the doc comment of its own fix's test |
| F-5 | 2026-09-11 | high | process | open | a merge kills the RELEASE.md ladder, and the ladder's own precheck returns the green answer for a rung that can never be pushed |
| F-6 | 2026-09-11 | med | self-friction | fixed-verified | the gate selects by PATH and the task framing assumed STATUS, so "classify them properly" would have put defect classes on six closed bugs |
| F-7 | 2026-09-11 | med | self-friction | open | the verify-before-asserting habit was scoped to my own artifacts, so a claim about a peer's code bypassed it entirely |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-11 | high | pre-campaign grep of `tests/`/`scripts/`/hooks for the population's name, before sweeping it | 115 of 119 findings "fixed": 112 caveats deleted or backfilled with unperformed verification, a `doc(move)` re-keying an id cited in 10 places incl. a test fixture, a frontmatter rewrite closing an open bug's evidence | validated |
| W-2 | 2026-09-11 | high | a refusal on a shared tree names the AUTHOR (by sessionId) and the socket, not just the fault | three guards refused in one session and no cause was in the refused diff; without attribution each becomes an investigation into the wrong session's work | validated |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | A mistake this session made itself — recorded for transparency. Covers a predicted friction that proved a false alarm, and, as every use in this corpus actually is, an error the author caught in their own work |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Add the matching Index
row afterwards, using the id the call returned. Do not allocate the id
yourself; see *How to use* above.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — the design's wiring table missed three surfaces because every doc describes `doctor` as a catalog-drift scanner

**Valid:** dated 2026-09-02

**Observed.** The `taken`-state design (`docs/superpowers/plans/2026-09-02-bug-claim-liveness-design.md`)
shipped a § *Wiring* table naming **8** surfaces a new bug status must reach, derived by
grepping for the triage query. Scouting before writing the implementation plan found the
table incomplete by **three**, and all three are on the `doctor` side:

1. `src/librarian/tools/doctor.rs:414` — the single registration line
   (`all_violations.extend(scan_…(ctx, &cat.conn)?)`). Without it the check compiles,
   tests pass in isolation, and `librarian(action="doctor")` never calls it.
2. The `librarian` MCP tool description, which enumerates `doctor`'s checks for agents.
3. [`docs/PROBES.md`](../PROBES.md):173, the page `CLAUDE.md` tells you to read *"before
   answering a question with a number"*.

**Root cause — the reason it was missable.** `doctor` has **23** `scan_*` functions
(`symbols(name="scan_", path="src/librarian/tools/doctor.rs", kind="function")`), but
every prose surface describes it as a *catalog-drift* scanner over roughly six checks.
**Five** surfaces read, all agreeing, all wrong in the same direction — **corrected from an
initial count of three later the same day**; see `F-2` for why the first sweep was short.
The three found first:

- `docs/PROBES.md:173` — "Catalog drift: `abs_path` form, ADS colons, `..` segments,
  missing-on-disk files, worktree-scoped rows, frontmatter-id vs catalog-id mismatch"
- the `librarian` tool description — the same six, phrased differently
- `get_guide("librarian")` § *librarian(action=…) — Reference* — "Read-only catalog drift
  scan (forward-slash form, NTFS ADS colons, `..` segments, missing-on-disk files,
  `abs_path_must_be_absolute`)"

The entire statement-validity half (`entry_dated_stale`, `entry_conditional_past_due`,
`validity_unparseable`, `cited_prefix_with_no_definer`, …) and the whole bug-record half
(`terminal_status_with_caveat`, `terminal_status_without_fix_anchor`,
`non_terminal_status_with_fix_anchor`) are undescribed on all five.

The two the first sweep missed, both found by grepping the phrase `catalog drift` instead
of a check name:

- `src/librarian/tools/librarian.rs:30` — the `Librarian/description` string, i.e. **the
  tool description an agent actually reads** to decide whether `doctor` fits its question.
  The most load-bearing of the five and the one absent from the original list.
- `src/librarian/tools/doctor.rs:1` and `src/cli/doctor.rs:1` — module rustdoc
  (`//! Doctor — catalog drift scanner.`), developer-facing. `codescout doctor --help`
  renders the second of these as a fourth phrasing naming only four checks, so it is one
  site with two renders rather than a separate surface.

**The tell that confirms it is systemic, not a one-off omission.** Grepping the *newest*
check's name, `non_terminal_status_with_fix_anchor`, returns **zero** hits in
`docs/PROBES.md`, the tool description, `CLAUDE.md` and `CHANGELOG.md` — it exists only in
`doctor.rs` plus bug/tracker prose. The older `entry_dated_stale` reaches CLAUDE.md,
`docs/TAXONOMY.md`, `CHANGELOG.md`, the manual and `tracker-conventions.md`. So the
describe-it-everywhere step happened once and then stopped, and nothing gates it.

**Impact.** Had the plan been written from the 8-surface table, the new
`scan_claim_liveness` would most likely have shipped **registered but undiscoverable** —
the check runs, and no agent reading `PROBES.md` or the tool description learns it exists.
That is the failure mode the design document's own § *Wiring* warns about, one layer
further out: not `declared-not-wired`, but *wired-and-undescribed*. Cost avoided: one
silent-capability defect. Cost incurred: ~6 tool calls of scouting.

**Class.** The doc/code gap is `issue-clusters:IC-11` (`cluster/doc-contradicted-by-code`)
— specifically its "a doc surface understates what the code does" sub-shape, where a
reader who trusts the doc forms a *smaller* and confidently wrong model. Worth noting the
observer structure: the author of a new `scan_*` is the party least able to notice, because
they know their check exists and `doctor` runs it — the missing description costs them
nothing and costs every later reader the capability.

**Severity:** med — no failed call and no wrong edit; it would have caused one incomplete
implementation plan and a shipped-but-invisible check.

**Category:** doc-vs-code drift (scouted pre-plan, not pre-dispatch).

**Status:** fixed-verified — the wiring landed on all 11 surfaces. `scan_claim_liveness` is
registered in `src/librarian/tools/doctor.rs` and its four checks report live
(`claim_held_by_dead_session`, `claim_held_by_live_session`, `claim_unresolvable_here`,
`claim_without_claimant`); the `taken` vocabulary reached `docs/issues/_TEMPLATE.md`; and
`src/librarian/session_registry.rs` exists. Shipped at `8f40eaad` (patch-id
`bf101e249a2a37bcf6ab3c4dbcff7cd851b914fe`), `e801f5e1`
(`8503f8d68cb97e6db3c6d0a1ce770c743f70b457`) and `a9e37db2`
(`4c33ec234b0f593489de0a79c61594443ba6aead`). **The underlying `doctor`-is-undescribed gap is
still NOT closed by this entry** — it stays filed separately, and `F-2` carries the
unmechanised half.

Flipped 2026-09-08 by a later session executing this stream's own § *Closing the loop*
checklist, which its author never reached. **That delay is the entry's second lesson and it
cost nothing to record:** the code shipped 2026-09-02 and this line read `open` for six days,
not because anyone disagreed but because the session holding the checklist exited and the
checklist lived only in an uncommitted file. A closing step that survives only in the worktree
of the session that wrote it is not a step.

**Rests on:** `CLAUDE.md` § *Observer Blindness* (name the party who structurally cannot
see it — here, the check's own author); `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
for why an under-described instrument index is worse than none.

## F-2 — enumerating by the subject's identifiers cannot find the surfaces that omit them

**Valid:** dated 2026-09-02

**Observed.** To enumerate every surface that describes `doctor`, I grepped for **check
names** — `entry_dated_stale`, then `non_terminal_status_with_fix_anchor` — and reported
**three** surfaces in `F-1` and in the bug file it produced. Re-running the sweep hours
later against the **phrase** instead returned **five**:

```
grep(pattern="entry_dated_stale", mode="files")            → 15 files   (name-based)
grep(pattern="catalog drift|Catalog drift", ...)           → 6 matches in 5 files
```

The two missed were `src/librarian/tools/librarian.rs:30` — the `Librarian/description`
string, i.e. **the tool description an agent reads to choose an instrument**, the most
load-bearing of the five — and the module rustdoc on the two `doctor.rs` files.

**Root cause, and it is worse than an ordinary undercount.** The selector was *"a file that
names a check"*; the population was *"a file that describes `doctor`"*. Those differ, but
the damaging part is that they are **anti-correlated**: a surface that mischaracterises
`doctor` does so precisely *by not enumerating its checks* — it says "catalog drift
scanner" and stops. So the name-based selector's blind spot **is** the target population.
The more wrong a surface is, the less likely the sweep finds it, and a sweep that returns
only the accurate surfaces reports a reassuring number.

Nothing errored. Both sweeps returned a plausible integer, and the smaller one was
consistent with everything else I believed.

**Impact.** `F-1` and
`docs/issues/archive/2026-09-02-doctor-doc-surfaces-describe-six-of-its-twenty-three-checks.md`
both shipped claiming three, and the design's § *Wiring* row #10 read "the `librarian` MCP
tool description" with **no path**, because the sweep never located it — an implementer
would have had to re-find it. All three are now corrected to five with `path:line` per
site.

**How it was caught, which is the uncomfortable part.** Not by review and not by process —
by an unrelated rebuild that forced a re-verification of the served text. Had the binary
not been rebuilt, the three-surface claim would have stood in two artifacts, with its
derivation published beside it, looking fully checked. `CLAUDE.md` § *Testing Discipline*
already names this shape: **a test cannot detect what its recording filters out**, and
widening the sample does not help, because the refuting instance leaves no artifact in the
first sweep at any corpus size.

**The rule worth carrying.** When enumerating *"every place that makes claim X"*, grep the
**vocabulary of the claim**, not the **identifiers of its subject**. Identifiers appear in
the accurate descriptions; the inaccurate ones are exactly those that omit them. Corollary
for a negative or small result: state the selector next to the count, so a reader can see
what it was structurally unable to match.

**Class.** `issue-clusters:IC-18` (`cluster/selector-narrower-than-its-population`) — and
note it was committed *while filing a bug about documentation drift*, by the party who had
just written that the doc surfaces were not independent of each other. Knowing the class
prevented nothing, which is `CLAUDE.md` § *Observer Blindness*'s standing claim about this
family.

**Severity:** med — no wrong edit and no failed call, but two artifacts shipped a wrong
count with a published derivation, and one plan cited a surface it could not address.

**Category:** measurement-method drift (selector vs. population).

**Status:** open — the rule above is not mechanised anywhere; it is currently only this
entry. A candidate mechanism is a `doctor` check asserting that every registered `scan_*`
is named in `docs/PROBES.md`, which would make the count machine-derived and retire the
sweep entirely.

**Rests on:** `CLAUDE.md` § *Testing Discipline* (recording-filter law) and
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

## W-1 — the mandatory pre-campaign grep turned a 119-finding "cleanup sweep" into 2 fixes and 3 rejects

**Valid:** invariant

**Category:** process

**Status:** validated

**Observed.** Asked to "fix all the cleanup/maintenance issues" against a 156-violation
`librarian(action="doctor")` report. Four populations read as mechanical. CLAUDE.md
§ *Observer Blindness* position 3 requires, before any campaign over a population, grepping
`tests/`, `scripts/pre-commit-*` and hooks for **that population's name** — not only the docs.
Run per population, it rejected three of four:

- **`terminal_status_with_caveat` (112 findings, 72% of the report).**
  `scan_terminal_status_with_caveat`'s own header — `src/librarian/tools/doctor.rs`, at the
  function, not in any doc — reads *"Reports only; there is no `fix=`. Discharging a caveat
  means establishing the thing it says was never established, which is work, not repair."*
  `docs/conventions/cross-machine-catalog-resume.md` classes the same bucket "pre-existing
  content debt". The ~32% coverage ratio was the tell CLAUDE.md names: **neither ~0% nor
  ~100% is a boundary someone drew before it is drift.**
- **The zombie-at-archive bug `13382b706c9c77b0`.** `doctor.rs` holds
  `an_open_status_bug_under_archive_is_silent`, a test whose doc comment names *that exact
  path* as its motivating case and states the filter exists because *"a source comment citing
  an archived bug is the correct end state"* — so it "would fire on the repo's healthiest
  records". `docs/architecture/companion-plugin.md` separately documents the file as *"now
  marked `zombie` and carrying the full correction"*.
- **`frontmatter_id_mismatch` on `docs/issues/2026-09-09-build-check-renders-three-of-n-compile-errors-with-no-count.md`.**
  `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`
  (status `open`) names that exact id pair as a known false positive: worktree-minted, not
  move-orphaned.

**Counterfactual — what shipped without it.** 115 of 119 candidate findings get "fixed":
112 caveats either deleted or backfilled with verification nobody performed; one
`doc(action="move")` re-keying `13382b706c9c77b0`, which is cited by id in 4 files and by path
in 6, one of the latter being a fixture comment inside the test that asserts the silence; and
one frontmatter rewrite that closes the standing evidence for an open bug. **Two of the three
were reachable only from `tests/` and a source-function header — surfaces the doctor report
names nowhere.** The third was reachable only by grepping the artifact's own id across
`docs/issues/`.

**Why it is a win rather than ordinary care.** The rejects were not caught by reading the
report more carefully; the report is consistent with acting wrongly on all three. The grep is
an unconditional policy tied to a trigger that happens anyway ("about to sweep a population"),
which is position 3's prescribed shape — *the check that runs when nobody is worried*.

**Promote-when:** a second sweep session runs the same pre-campaign grep and reaches a
reject on a population the report presented as actionable. At n=2 this earns a line in the
hygiene skill's Phase 3 as a standing pre-triage step, rather than living only in CLAUDE.md's
Observer Blindness section where a sweeping session may not look.

## F-3 — doctor's detail text asserts a false cause on a finding whose false-positive class is already a filed bug

**Valid:** invariant

**Category:** codescout-tool

**Severity:** med

**Status:** open

**Observed.** Triaging `doctor` findings as cleanup, three of four candidates were false
positives — and the report's `detail` text discriminated on only one of them. The three sit
at three *different* distances from the reader, which is the point:

1. **Self-describing — the report is sufficient.** `entry_without_definition` on
   `docs/trackers/provenance-subsystem.md` says it in the `detail` string: *"That is the
   supported end state of the compaction ladder … not an omission. Do NOT add a heading here
   to close this: a second definer makes the token ambiguous."* A reader acts correctly from
   the report alone. This is the shape the other two owe.
2. **The report asserts a cause that is false here.** `frontmatter_id_mismatch` on
   `docs/issues/2026-09-09-build-check-renders-three-of-n-compile-errors-with-no-count.md`
   states *"a move re-keys the row and this file kept the id it was moved away from."* No move
   happened — the id was **worktree-minted**, and
   `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`
   (status `open`) already records this exact id pair as the known false positive. The check
   ships a `fix=repair_frontmatter_id`, so the prescribed action is one call away and would
   overwrite the standing evidence for that open bug.
3. **Correctly absent, and the absence read as a gap.** The zombie-at-archive row is filtered
   out on purpose (`an_open_status_bug_under_archive_is_silent`). Having enumerated doctor's
   38 checks and found no *"non-terminal status at an archive path"* among them, I proposed a
   `doc(action="move")` — i.e. **a filter working as designed presented as missing coverage.**
   Nothing in the report can correct this, because the report's job is to not mention it.

**Cost.** Rejects 2 and 3 were reachable only by grepping `tests/` and the artifact's own id
across `docs/issues/`. No wrong write landed, because the pre-campaign grep is unconditional
(`W-1`) — which is exactly why the severity is `med` and not `high`: the defence held, and it
is a policy sitting in CLAUDE.md rather than anything the tool emits.

**The asymmetry worth fixing.** Case 1 shows the remedy is affordable: doctor *can* say "this
may be the correct end state, here is why". Where a check has a known false-positive class
already filed as a bug, the `detail` could name it — `frontmatter_id_mismatch` could carry
*"worktree-minted ids produce this finding; see <bug>"* at roughly the cost of the sentence it
already writes asserting the opposite. This is § *Testing Discipline*'s remedy-text law one
layer out: the check's **predicate** is right in all three cases, and what misleads is the
**prose** — untested by construction, and here actively wrong about causation.

**Rests on:** `src/librarian/tools/doctor.rs` (the three checks and
`an_open_status_bug_under_archive_is_silent`);
`docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`.

## F-4 — I shipped the defect class I had filed an hour earlier, into the doc comment of its own fix's test

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** fixed-verified

**Observed.** Writing the fix for the params-array rule bug (`ea302e013cf7c85a`, archived), I
put a justification into a new test's doc comment that I had not checked:

> *"That matters here: the token appears in the § Development Commands region too, and a
> file-wide `contains` would pass with the carve-out deleted."*

`grep -c 'params_behind_body' CLAUDE.md` returns **1** — the sole occurrence being the
carve-out I had just written. The claim was false, and it was the *reason* given for the
test's design.

**Why this is worth an entry rather than a quiet correction.** One hour earlier I filed F-3 in
this same ledger, whose entire subject is `doctor` emitting a `detail` string that *asserts a
false cause* — prose that is confident, load-bearing, and untested by construction. I then
committed the same defect, in the same shape, into a test comment, while writing the fix for
the bug that entry supports. **Knowing the class prevented nothing.** That is `OB-1`'s measured
claim reproduced on myself: *four instances in one evening across three sessions, every one
committed by an author actively writing about that class.*

**What caught it was not care.** It was running `grep` before letting the sentence stand — the
same verify-don't-hypothesise step that had already, four times that session, inverted a
conclusion about to be published. The remedy is not "be careful with doc comments"; it is that
a causal claim in prose gets the same one-command check as a claim in code.

**The sharper half — the corrected version is a better argument than the false one.** The
scoping to the `>` blockquote is still right, for a reason reached only by being wrong: today
the scoping is *redundant*, and the redundancy is precisely what decays. The day anything else
in `CLAUDE.md` mentions that check, a file-wide assertion silently stops discriminating and
nothing reports that it has. The false version claimed present necessity; the true version
claims future necessity, which is the stronger reason to write it that way. **A fabricated
justification can defend a correct decision**, which is what makes this class survive review —
the reviewer checks the decision, agrees, and never audits the reason.

**Cost:** none shipped — caught pre-commit. `med` rather than `low` because the artifact was a
*test's rationale*, the surface a future session reads to decide whether the test still earns
its place; a false rationale there invites a correct test to be deleted as over-engineering.

**Rests on:** `grep -c 'params_behind_body' CLAUDE.md` → 1, run 2026-09-11 at HEAD `1fe07709`;
`observer-blindness:OB-1`.

## W-2 — three gate refusals in one session, and not one was findable in the refused author's own diff

**Valid:** invariant

**Category:** process

**Status:** validated

**Observed.** Three separate guards refused work on this shared checkout in one session. All
three refusals were correct, and **none of the three causes was visible in the diff of the
session being refused**:

1. **Pre-commit `refuse a stored count, or a class gaining a member it does not name`.** A
   sweep commit added a bug carrying `cluster/doc-contradicted-by-code`, and `IC-11`'s
   `**Members:**` line did not name it. The coupling lives in a *different file* the commit did
   not touch. The refusal pointed at the exact path and warned that grepping the roster returns
   0 — *"that zero means WRONG FILE, not no such class"*.
2. **`./scripts/fmt-mine.sh` refusing to format `src/tools/guide_rearm.rs`.** A peer's
   uncommitted file, attributed by sessionId, with the socket to reach them printed. Not mine,
   and deliberately no `--force`.
3. **`every_open_bug_file_declares_one_known_defect_class` reddening both test lanes.** A
   peer's *committed* bug file (`1fe07709`) carrying no `cluster/` tag — so HEAD was red before
   the session began, and none of its four changed files feeds that test.

**Counterfactual.** Without (1) the class ledger ships a member it does not name, the exact
state that makes *"which architectural problem do these bugs share?"* stop being a query.
Without (2) a peer's in-flight Rust gets rewritten — itself a filed defect here. Without (3)'s
attribution a red suite reads as evidence against one's own change, and the next hour goes into
debugging a working diff — which is
`docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`.

**The generalisable claim, and the reason it is one entry rather than three notes.** On a
checkout this shared, **the modal cause of a red is not in the diff of the session that meets
it.** Every one of these guards therefore spends its budget on *attribution* rather than
detection — naming the file, the sessionId, and the socket — and that is what converts each
refusal from an obstacle into a one-message action. A guard that had merely said "formatting
needed" or "test failed" would have produced three investigations into the wrong session's
work.

**Which identifier the guard prints is part of the claim, not a detail — and the margin is
narrower than it looks.** Measured the same day: sessionId
`f3c594ce-c424-40d3-a603-9693cfef3f63` (pid 703051, profile `.claude-kat`, socket constant
throughout) carried **three** registry names between 11:09 and ~12:45 — `codescout-53`, then
`fix-subagent-guide-starvation`, then `append-entry-unpushed-guard-fix`, observed via the
socket walk, a `fmt-mine.sh` refusal banner, and that session's own inbound `from-name=`
respectively. Each name is *descriptive of the session's current task*, so every one reads like
a durable identity rather than a label, and the third would look to any reader like a different
session entirely from the first. `CLAUDE.md` § *Observer Blindness* already holds the law —
*attribute by sessionId, never by a self-reported name* — citing a two-name instance; this is a
three-name instance inside one working day. So a refusal that names the author **by name** is
the same defect it was built to prevent, one indirection later: it decays silently and points
the reader at a session that no longer answers to it. Guard (2) prints the sessionId and the
socket, which is why its attribution survived all three renames.

**Promote-when:** a fourth refusal in a later session whose cause is again outside the refused
diff. At n=4 the claim is worth stating in `CLAUDE.md` § *Observer Blindness* as a design rule
for new guards: **a refusal on a shared tree must name the author — by sessionId — not just the
fault.**

**Rests on:** the three refusal texts as emitted 2026-09-11 between 09:30 and 12:30, HEADs
`15cbe5e6` → `1fe07709`; `scripts/file-provenance.py` output for (2) and (3); the three
name observations above, each read from the surface named beside it.

## F-5 — a merge kills the RELEASE.md ladder, and the ladder's own precheck returns the green answer for a rung that can never be pushed

**Valid:** invariant

**Category:** process

**Severity:** high

**Status:** open

**Observed.** `docs/RELEASE.md` § *Publishing a stack several sessions wrote* prescribes the
ladder: each author pushes their own commit by refspec, bottom-up, so *"no operator is ever
asked to authorise someone else's work"*. Its stated precheck is
`git rev-list --count origin/<branch>..<your-sha>` must be `1`, *"checked BEFORE the push"*.

After reconciling a 5-vs-9 divergence by merge, measured 2026-09-11 at HEAD `e4262c97`:

```
rev-list --count origin/experiments..15cbe5e6          = 1     <- precheck PASSES
merge-base --is-ancestor origin/experiments 15cbe5e6   = false <- cannot fast-forward

lowest fast-forwardable ref is the MERGE COMMIT 8c795a92, carrying 10 commits
across four sessions.
```

**Two separate defects, and the second is the one that bites.**

1. **The precheck is necessary and not sufficient**, and fails in the direction that reads as
   clearance. It counts *how many of mine are unpublished* and is silent on *whether the
   remote is still an ancestor*. A session that reads the ladder literally, gets `1`, and
   concludes it may push has verified a proposition that does not entail the one it needs.
   Remedy is one line beside the existing check: `git merge-base --is-ancestor
   origin/<branch> <your-sha>`.
2. **A merge collapses the ladder entirely, and merging is what you must do to push at all.**
   The ladder presumes a LINEAR stack: rung N becomes pushable once rung N-1 lands. A merge
   commit makes every commit beneath it unreachable as a fast-forward target, so the smallest
   publishable unit becomes the merge itself — all sessions' work, at once. Reconciling the
   divergence, which was the prerequisite for anyone publishing anything, is precisely the act
   that destroyed the mechanism designed so nobody publishes anyone else's work.

**Why this is not merely a documentation gap.** The ladder exists to keep an operator from
being asked to authorise another session's commits — RELEASE.md is explicit that the question
should never reach them. After a merge that question is unavoidable and returns in a *worse*
form than before: pre-merge it was 7 commits across 3 sessions, post-merge it is 10 across 4
and cannot be decomposed. **The remedy that unblocks the branch and the mechanism that makes
the branch publishable safely are in direct opposition**, and nothing in RELEASE.md says so.

**What a fix would have to decide** (not proposed here, because it is a real design question):
whether the ladder is abandoned once a branch diverges — in which case RELEASE.md should say
that a divergence escalates to a single operator authorisation by construction — or whether
reconciliation should be a REBASE after all, which preserves linearity at the cost of
re-keying peers' commits, the thing § *Concurrent-Work Rules* forbids for its own good
reasons. Both horns are real; this entry claims only that the corpus currently documents
neither.

**How it actually resolved, recorded because the resolution is the evidence for horn one.** The
operator was asked, and authorised publishing the other sessions' commits — the exact question
RELEASE.md says should never reach them. The push then carried 24 commits, of which 10 were
mine, with the guard's ack naming 4 foreign sessionIds and stating in its own output that the
ack *"does not speak for theirs, and it leaves each of them exactly as UNCLEARED as they were"*.
So the mechanism degraded to: one operator authorises, and the guard records that the
authorisation was not the authors'. That is a defensible terminal state and it is not the one
the ladder documents.

**Rests on:** the four `merge-base --is-ancestor` probes above, run 2026-09-11 at HEAD
`e4262c97` against `origin/experiments = f50be810`; `docs/RELEASE.md` § *Publishing a stack
several sessions wrote — the ladder*; the push of `f50be810..f062a901` and its ack output. The
insufficiency half (1) was raised to the session implementing the pre-push divergent-push guard
before this instance existed, as a predicted gap; this is its first measured occurrence.

## F-6 — the gate selects by PATH and the task framing assumed STATUS, so "classify them properly" would have put defect classes on six closed bugs

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** fixed-verified

**Observed.** A merge landed 8 bug files under `docs/issues/` with no `cluster/` tag, reddening
`every_open_bug_file_declares_one_known_defect_class`. Three sessions independently described
the population as *"8 untagged open bugs"*, and the operator, given that framing, chose to
classify them properly. Reading the test rather than its failure message:

```rust
// tracked_open_bug_files()
p.strip_prefix("docs/issues/").is_some_and(|rest| {
    !rest.contains('/') && rest.ends_with(".md") && rest != "_TEMPLATE.md"
})
```

It selects by **path** — anything directly under `docs/issues/`, never reading `status:`. Six
of the eight were `status: fixed`, closed 2026-08-19, and simply never archived. Only two were
open.

**The cost of acting on the framing.** Tagging all eight would have assigned defect classes to
six closed bugs, inflating the counts that class promotion reads — the precise outcome the
test's own failure message warns against, reached by following an instruction that said
*properly*. The correct action for those six was ARCHIVING, which removes them from the gate's
population entirely and needs no class at all.

**The generalisable shape.** A gate's failure message names the members it rejected and not
the predicate that selected them. *"open bug files with a bad defect-class declaration"* is
prose; `tracked_open_bug_files()` is the definition, and the word `open` in the message means
something different from the word `open` in the frontmatter of the files it lists. Everyone
downstream — three sessions and one operator — inherited the message's vocabulary and none
checked the selector.

**What actually caught it** was not suspicion of the framing: it was that the ids would not
resolve for a catalog write, which forced reading the frontmatter, which showed `status:
fixed`. An accident of the write path, one step before six wrong tags.

**Cheap general remedy, offered not claimed:** when a gate names a population in prose, read
its selector before acting on the population — the same move § *Observer Blindness* already
prescribes for a coverage ratio, applied to a membership predicate instead of a count.

**Rests on:** `tests/issue_clusters.rs` `tracked_open_bug_files()`; the eight files'
frontmatter as merged; `e4262c97`.

## F-7 — the verify-before-asserting habit was scoped to my own artifacts, so a claim about a peer's code bypassed it entirely

**Valid:** invariant

**Category:** self-friction

**Severity:** med

**Status:** open — no mechanism; the remedy below is a policy and I do not have a gate for it.

**Observed.** Three instances of one class in a single session — *asserting a property of a
mechanism from its description rather than from reading it* — and the defence that caught the
second did not fire on the third.

| # | the claim | caught by |
|---|---|---|
| A | `doctor`'s `params_behind_body` `detail` asserts *"a move re-keys the row"* for an id that was worktree-minted | me, reading the filed bug — became `F-3` |
| B | my own new test's doc comment: *"the token appears in the § Development Commands region too"* | me, `grep -c` before commit — returned **1**, the carve-out itself (`F-4`) |
| C | told a peer their pre-push guard would see merges *"go from rare to universal"* and warned about its false-positive rate | **the peer**, by reading their own implementation |

**The asymmetry is the finding.** B and C are the same defect. B triggered a check because the
artifact was mine and opening it was reflex. C did not, and the reason is not carelessness: I
had the peer's one-line description of their check and treated it as sufficient, because the
artifact felt like theirs to inspect.

**It was fully readable.** Their implementation — `src/librarian/tools/append_entry.rs`, the
pre-push guard script and its test — was **dirty in the shared working tree at that moment**.
I had already enumerated those exact paths, by name, in a provenance run, in order to avoid
committing them. I could have opened any of them in one call. What stopped me was not access.

**The conflation, stated plainly.** This checkout's discipline is *do not WRITE a peer's
uncommitted file* — `fmt-mine.sh` refuses it, `git commit` by pathspec exists for it, and
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` measures the
cost of getting it wrong. Somewhere that became *do not TOUCH a peer's file*, and reading is
not touching. The write prohibition is load-bearing and correct; extending it to reads removes
the only thing that would have stopped me making a false public claim about their work.

**Why the cost is not hypothetical.** The claim was addressed TO the author, who could check
it — the best possible case. Sent to any third party, or recorded in a ledger, *"the pre-push
guard's false-positive rate goes up after a merge"* is a plausible, specific, wrong statement
about a subsystem under active development, and the party best placed to refute it would never
have seen it. Severity `med` rather than `high` only because the addressing happened to route
it to the one reader who could falsify it.

**Remedy, and it is narrow on purpose:** before asserting a property of code a peer holds,
read the code — the same standard applied to one's own. The shared-checkout rules restrict
writes and say nothing about reads, so no rule had to change; what needed changing is that I
had generalised one into the other. **The tell:** a sentence of the form *"if your X does Y
then Z"* where X is someone else's and you have not opened it. State the conditional
explicitly, or open it.

**Not generalisable to "ask fewer questions of peers."** The prediction was worth making and
the peer said so — it prompted a stress-test they had not run. What was wrong was the
grammar: I asserted a property where I held a hypothesis. *"Does your check key on merge
presence or on a verified duplicate? If the former, …"* costs one sentence and is true.

**Rests on:** the three instances above, 2026-09-11; the peer's reply confirming their refuse
fires on `git grep -c '^## PREFIX-N' >= 2` in the merge's own tree rather than on merge
presence; `observer-blindness:OB-1` for the *knowing the class prevents nothing* precedent,
of which this is a same-session n=3.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->

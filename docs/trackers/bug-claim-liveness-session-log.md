---
entry_prefix: ["F", "W"]
kind: tracker
owners: ["marius"]
status: active
tags: ["bug-tracking", "status-vocabulary", "peer-sessions", "doctor"]
title: Session Log — Bug-Claim Liveness (taken state)
topic: bug claim liveness
entry_high_water_F: 3
entry_high_water_W: 1
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
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-11 | high | pre-campaign grep of `tests/`/`scripts/`/hooks for the population's name, before sweeping it | 115 of 119 findings "fixed": 112 caveats deleted or backfilled with unperformed verification, a `doc(move)` re-keying an id cited in 10 places incl. a test fixture, a frontmatter rewrite closing an open bug's evidence | validated |

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
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
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

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->

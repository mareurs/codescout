---
kind: tracker
status: active
title: Session Log — Catalog Audit Trail
owners: [marius]
tags:
  - session-log
  - audit-trail
  - librarian
entry_prefix:
  - F
  - W
entry_high_water_F: 5
entry_high_water_W: 3
---

# Session Log — Catalog Audit Trail (T-1 → T-13 → T-7)

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
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
> **`edit_markdown` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_markdown` for the prose sections and the index tables,
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
| F-1 | 2026-09-01 | high | design-vs-substrate | fixed-verified | Spec's Phase 2 volume analysis named the 0.4% term and missed the 98.5% one |
| F-2 | 2026-09-01 | med | plan-internal-consistency | fixed-verified | Spec prescribed a stamp line inside a file it also declared merge=union |
| F-3 | 2026-09-01 | low | tool-ergonomics | open | edit_code replace with a multi-symbol body leaves the old siblings behind |
| F-4 | 2026-09-01 | high | plan-vs-substrate | fixed-verified | Four interface references in my own 40-minute-old plan were wrong |
| F-5 | 2026-09-01 | high | gate-integrity | mitigated | Baseline exit code came from tail, not cargo — green and uninformative |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-01 | high | Histogram the substrate before reading a design whose central claim is a volume | Spec-as-written ships a green, working export writing ~380k rows/day of empty diffs into git; no gate in the chain reads data rather than code | validated |
| W-2 | 2026-09-01 | med | Probe an embedded language's semantics before generating code in it | Guessing SQLite's JSON subtype the other way pins escaped payloads behind passing tests; without the BLOB probe the writer-abort guard has no red to be born from | validated |
| W-3 | 2026-09-01 | high | Pre-flight scan as a row-per-pair table, run against your own plan | 3 of 5 tasks carried a plan-inherited defect; one branch ends not in a retry but in copying a forbidden UB env-mutation pattern that lands intermittently | validated |

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

## F-1 — Spec's Phase 2 volume analysis named the 0.4% term and missed the 98.5% one

**Observed:** 2026-09-01, scoping T-7 (committed audit shards). Before reading the spec's
Phase 2 bullets I histogrammed the live `catalog_audit` table, per CLAUDE.md's *"run the
reproduction before reading the fix plan — the plan is a hypothesis about the reproduction."*

**When:** Immediately before writing the T-7 implementation plan. No code written yet.

**Expected (spec):** `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md`
§ Phase 2: *"exports filter pure reindex churn by default (update rows whose changed-set ⊆
`{file_mtime, file_sha256, updated_at, missing_since}`)"* — i.e. reindex churn is the volume
term that makes a committed log affordable.

**Got (measured reality):** over a 1.74-hour window, 27,914 rows:

| class | rows | share | payload chars | share |
|---|---:|---:|---:|---:|
| `commits` update, payload literally `{}` | 27,505 | **98.5%** | 55,010 | 6% |
| `artifact_augmentation` update | 23 | 0.08% | 786,771 | **88%** |
| `artifact` update, churn keys only | 107 | 0.4% | 25,359 | 3% |
| genuine signal | 279 | 1.0% | ~22k | 3% |

Reindex churn — the *only* term the spec named — is **0.4%**. The dominant term by row count
was a defect: the UPDATE trigger carried no `WHEN` clause, so an `UPDATE` writing values a
row already held still fired and `update_diff_expr` folded to `'{}'`. The dominant term by
bytes was a second defect (whole-blob params rewrite × old-and-new capture), filed the
previous night as an *inference* with a `## Resume` reading *"Measure first (a number beats
an adjective)"* — never measured until this pass.

**Probable cause:** the spec's volume section was written from estimates during
brainstorming, on a trail that did not exist yet — there was nothing to measure at design
time, and nothing scheduled a re-measurement once there was. The two defects were both
*known* (one a deferred review Minor, one a filed bug), and neither had a number, so neither
could be weighed against a design decision.

**Workaround:** none needed — fixed upstream rather than filtered at export. `40ab56f6`
(patch-id `d3021d83634be0f6b8d7c69200f241f80f9e5f96`) adds the `WHEN` guard, clamps oversized
values in UPDATE diffs only, and reports bytes from `audit::health`. Spec § Phase 2 revised
at `b0bdc4b1` to record what the measurement said rather than what the estimate had.

**Severity:** high — this would not have failed. Implementing Phase 2 as specified yields a
working export, a green suite, and a committed file growing at ~380k rows/day, of which
98.5% carry no information. **No assertion in the suite could have caught it**, because the
defect is *rows that should not exist*, and the existing update assertions are monotone
under over-recording (CLAUDE.md § Testing Discipline). The first observer would have been
whoever noticed the repository getting large.

**Status:** fixed-verified — fix landed, gate green, and verified on the live catalog: a
reindex went from ~2,750 `commits`/`update` rows to **20 rows, 0 empty diffs**; a real
tracker append wrote **441 chars** where it would have written 7,364.

**Valid:** dated 2026-09-01

**Rests on:** the measurement being of the live shared catalog rather than a fixture — the
`{}` flood is produced by `reindex` rewriting the `commits` table, which no in-memory test
catalog exercises at that scale.

**Fix idea / Pointer:** `docs/issues/archive/2026-09-01-audit-records-the-statement-not-the-change-so-98-percent-is-empty.md`,
`docs/trackers/system-retrospective-improvements.md` T-13.

## F-2 — Spec prescribed a stamp line inside a file it also declared merge=union

**Observed:** 2026-09-01, writing the T-7 plan's shard-format section.

**When:** Drafting Task 2, deciding what a shard file's first line is.

**Expected (spec):** § Phase 2 — *"each export stamps `audit_exported_through_seq`
(catalog_meta **and shard**); doctor reports the unexported delta; merged reads label each
host's coverage window."* Read straight, that prescribes a watermark/coverage line inside
the file.

**Got (reasoning against the file's own merge semantics):** the same section specifies
`.gitattributes` `merge=union` for same-host branch merges. `union` keeps the lines from
both sides of a conflict region — so a stamp line accumulates one copy per export per merged
branch, and a *coverage* stamp is worse than a duplicate: two contradictory declared windows
in one file, with no rule saying which is current. The spec's two bullets are individually
sensible and jointly unimplementable.

**Probable cause:** the honesty-marker bullet and the merge-strategy bullet were written for
different concerns (IC-13 and conflict handling) and never checked against each other. This
is the plan-internal-consistency check the writing-plans skill prescribes, applied to a spec
rather than a plan.

**Workaround:** shards carry **data lines only**. Coverage is *derived* — `min`/`max` `seq`
per host over the rows actually present — and the local watermark stays in `catalog_meta`
where nothing merges it. Derived is also strictly more honest: a declared window that
disagrees with the rows is the failure mode a declaration exists to prevent, and merge=union
guarantees that disagreement eventually. Recorded as a Global Constraint in
`docs/superpowers/plans/2026-09-01-committed-audit-shards.md` and corrected in the spec at
`b0bdc4b1`.

**Severity:** med — caught at plan-writing time, so cost was one design decision rather than
rework. Had it shipped, the symptom would have appeared only after the first same-host
branch merge, in a file nobody re-reads until they are already investigating something else.

**Status:** fixed-verified — spec corrected, plan pins the constraint with a rationale.

**Valid:** dated 2026-09-01

**Rests on:** `merge=union`'s documented behaviour of keeping both sides' lines in a conflict
region; not re-verified against a live three-way merge, which is the check owed if anyone
later reintroduces a header line.

**Fix idea / Pointer:** spec § Phase 2 *Settled design*; plan § Global Constraints.

## F-3 — edit_code replace with a multi-symbol body leaves the old siblings behind

**Observed:** 2026-09-01, implementing the audit-trigger fix in
`src/librarian/catalog/audit.rs`.

**When:** Replacing `old_image_expr` with a block that introduced two new helpers
(`value_expr`, `changed_predicate`) alongside rewritten versions of `old_image_expr` **and**
`update_diff_expr` — four functions in one `body`, targeted at one `symbol`.

**Expected:** the named symbol's span is replaced by the supplied text, and the file then
contains what the text says.

**Got:** the *old* `update_diff_expr` survived. `edit_code(action="replace", symbol=X)`
replaces exactly X's span; every other definition in `body` is inserted, not reconciled
against same-named definitions elsewhere in the file. `symbols()` afterwards showed
`update_diff_expr` twice — lines 141-151 and 154-163 — and removing one then required
`at_line` to disambiguate, because the two were byte-distinguishable but name-identical.

**Probable cause:** the tool's contract is symbol-scoped by design and correct as
documented; the friction is that a multi-symbol `body` is a natural way to write a
refactor, and nothing warns that the extra symbols are being *added* rather than matched.
The failure is loud in Rust — duplicate definitions do not compile — so this is ergonomics,
not a correctness hazard, **but only in a language that forbids duplicate definitions.** In
Python, TypeScript or Go with distinct receivers, the second definition would silently win
or silently lose.

**Workaround:** `symbols(path)` after any multi-symbol replace, then
`edit_code(action="remove", symbol=…, at_line=…)` for the leftovers. Or, better: one
`edit_code` call per symbol.

**Severity:** low — cost was two extra tool calls and one `symbols` check, inside a session
that was already reading the file. Recorded because the shape generalises: *"replace one
function with several"* is a common refactor move and the leftover is invisible in the
edit's own response, which returned `status: ok` with a `replaced_lines` span that was
accurate about what it replaced and silent about what it added.

**Status:** open — no fix proposed for codescout itself; a plausible one is for `edit_code`
to report `also_defined: [names]` when `body` introduces symbols the target span did not
contain, which is the information the caller needs and already has to go find.

**Valid:** dated 2026-09-01

**Rests on:** `edit_code`'s documented symbol-scoped replace contract; the duplicate was
observed directly via `symbols()` output, not inferred.

**Fix idea / Pointer:** `src/librarian/catalog/audit.rs` as edited this session; the
`replaced_lines: "76-84"` response that reported success.

## W-1 — Histogram the substrate before reading a design whose central claim is a volume

**Observed:** 2026-09-01, at the start of T-7 scoping, before reading the spec's Phase 2
section in detail and before writing any plan text.

**Pattern:** When a design's central claim is a **volume, rate, or population** — "filter
the churn", "these rows dominate", "this is affordable" — histogram the live substrate
before reading the design's reasoning. Not to confirm the design; to find out what it is
*about*. One `GROUP BY tbl, op` and one `GROUP BY <changed-key-set>` was the whole
instrument.

**Counterfactual:** the spec named reindex churn as the term to filter. Implementing it as
written produces:

- a correct, working export that filters exactly the 0.4% the spec named;
- a green test suite, because every test asserts on rows that *are* written;
- `.codescout/audit/<host>-202609.jsonl` growing at ~380k rows/day, 98.5% of them payload
  `{}`;
- a `filtered_total` and a doctor delta that are all internally consistent and all counting
  noise.

**Nothing downstream catches this.** Not clippy, not the four-command gate, not a task
review reading the diff against the plan — the diff *matches* the plan. Not even the final
whole-branch review, which reads the change rather than the data. The first observer is
whoever notices the repository is large, weeks later, at which point the shards are in git
history and the fix is a rewrite rather than a `WHEN` clause. Concretely: 27,505 rows in
1.74 hours became **0** after one clause.

**Confirming data points:**
1. This session — spec's named term was 0.4%; the real term was a defect at 98.5%.
2. `codescout:R-117`'s population form (three instances, three systems): a fix naming a
   population that turned out empty. This is the mirror image — a fix naming a population
   that turned out to be 250× smaller than a population nobody had named — and it fails the
   same way, **green**.
3. The sibling bug filed the previous night carried `## Resume: Measure first (a number
   beats an adjective)` and had gone un-measured for a day. Its inference was directionally
   right and understated the size by 2×: it estimated ~50KB per append against ~25KB params;
   observed max was 104,613 chars in one row.

**Impact:** high — prevented shipping a working feature whose output was 98.5% noise, into
git, permanently.

**Promote-when:** a second design whose central claim is a volume/rate/population is
falsified by measuring before reading. At 2 datapoints, promote to CLAUDE.md § Bug Tracking
as a widening of *"run the reproduction before reading the fix plan"* — the existing law is
phrased for **bug fixes** ("the plan is a hypothesis about the reproduction"), and this is
the same law for **designs**, where there is no reproduction to run and the substrate must
be sampled instead. That gap is why the rule did not obviously apply here: the spec was
approved, there was no bug, and nothing prompted a measurement.

**Status:** validated — single datapoint in the design form; the bug-fix form of the law
already has four (CLAUDE.md, promoted 2026-08-20).

**Valid:** dated 2026-09-01

**Rests on:** `F-1`, same session — this win is F-1's counterfactual and is not independent
evidence of it.

## W-2 — Probe an embedded language's semantics before generating code in it

**Observed:** 2026-09-01, before writing the SQL for `value_expr` — the clamped/blob-safe
column expression the audit triggers embed.

**Pattern:** Before writing generated SQL (or any generated code in a language whose
semantics you are inferring rather than reading), run the *smallest* version of it through
the real engine and read the output. Three lines against `sqlite3 :memory:` here, testing
exactly the three assumptions the generator rested on.

**Counterfactual:** the three probes and what each would have cost:

1. `json_array(json_object('elided','blob','len',7))` → `[{"elided":"blob","len":7}]`.
   SQLite's JSON subtype is respected by nesting functions, so **no `json(...)` wrapper is
   needed**. Had I assumed the opposite and wrapped defensively, harmless. Had I assumed the
   subtype was *not* respected — the more natural guess, since `json_object` returns TEXT —
   I would have written the stand-in as a string and shipped payloads containing
   `"{\"elided\":\"blob\"...}"` escaped one level deep. Every test I then wrote would have
   been written *against that output*, pinning the wrong shape permanently. That is the
   expensive branch: not a failure, a wrong contract with passing tests.
2. `json_patch('{}', json_object('c', json_array(json_object(...), 'small')))` → the mixed
   clamped/verbatim fold works. Confirms the clamp composes with the existing diff fold
   rather than needing it restructured.
3. `json_object('c', X'DEADBEEF')` → **`Error: JSON cannot hold BLOB values`, exit 1.**

Probe 3 is the one that changed the work. It turned a filed bug's *inferred* hazard — the
file said "measured by the final reviewer's probes; not re-measured here" — into a
reproduction I could write a test around. `a_blob_value_does_not_abort_the_writer` was then
born red with that exact message raised on the **writer's** `UPDATE`, which is what
distinguishes "the audit row is missing" from "the caller's write failed". Without probe 3 I
would have written the blob arm anyway (the bug file asked for it) but had no way to
demonstrate the guard fires — and per CLAUDE.md's loudness law, a guard nobody can show
reaching is decoration.

**Confirming data points:**
1. This session — three probes, one of which converted an inference into a red test.
2. The pattern is already law for *tools* (activation bootstrap Phase 2: "a claim about how
   a TOOL behaves needs the call run once"). This extends it to **embedded languages**:
   generated SQL, regex dialects, template engines, shell quoting — substrates where the
   host language's type checker offers no opinion and the failure surfaces at runtime in
   production data rather than at compile time.

**Impact:** med — saved one wrong-contract-with-passing-tests branch and produced the red
that made a filed bug closable rather than merely patchable.

**Promote-when:** a second session where probing an embedded language's semantics before
generating it changes the generated code. At 2 datapoints, promote to the
`project-activation-bootstrap` guide's Phase 2 as a one-line extension of the
verify-don't-hypothesise rule: *"a claim about an embedded language — generated SQL, a regex
dialect, a template engine — needs the smallest version run through the real engine once."*
Note the routing test: this is craft-shaped, not project-shaped, so the skill or the
bootstrap guide, never a project memory.

**Status:** validated — single datapoint; promote-when not reached.

**Valid:** dated 2026-09-01

**Rests on:** the three probe outputs recorded verbatim in this session's transcript, run
against the same bundled SQLite the crate links (`rusqlite` `features = ["bundled"]`) — the
probe used the system `sqlite3` binary, which is a **different build**, so the subtype
result is strong evidence and not proof for the linked library. The blob error is confirmed
independently by the in-tree test, which does run against the bundled build.

## F-4 — Four interface references in my own 40-minute-old plan were wrong

**Observed:** 2026-09-01, SDD pre-flight conflict scan for
`docs/superpowers/plans/2026-09-01-committed-audit-shards.md`, before dispatching Task 1.

**When:** ~40 minutes after I finished writing that plan, in the same session, having scouted
every seam it names *while writing it*.

**Expected (plan):** four interface references, each written from a scout I had just done:
`crate::util::test_env::EnvGuard`; `ctx.project_root()` at two call sites in Task 4; `&root`
in Task 4's reindex fold-in.

**Got (scouted reality):** all four wrong.

| Plan said | Reality |
|---|---|
| `crate::util::test_env::EnvGuard` | No such path. The only `EnvGuard` is **private**, inside `src/agent/mod.rs:2124`'s test module, `#[cfg(feature = "server-stack")]`-gated, and its own doc comment reads *"Do NOT copy this pattern into a default-feature test"*, citing an archived UB-race bug |
| `ctx.project_root()` (×2) | No such method on `ToolContext`. `project_root()` exists on the **agent** (`src/agent/mod.rs:1466`) and is `async`. The shape the plan wanted is `src/librarian/tools/gather.rs:294` — a private free fn taking `&ToolContext` |
| `&root` in `reindex::call` | No `root` binding exists. It has `targets: Vec<PathBuf>` (`src/librarian/tools/reindex.rs:181`), 1..n roots depending on `scope` |

Two further gaps in the same scan: Task 2 duplicated the shard-filename convention instead of
calling Task 1's `shard_file_name`, and Task 1's collision test derived its suffix from
`nanos ⊕ pid` — identical for two calls in one process on a coarse clock, so both a flaky
test and a real collision.

**Probable cause:** the plan was written immediately after scouting, when the writer's model
is most confident and least tested — `codescout:R-49`'s stated mechanism. Specifically, I had
read `gather.rs` for the `catalog_meta` helpers and *inferred* a `project_root` method on
`ToolContext` from the free function I saw there. The inference was one token off and read as
a memory.

**Workaround:** six rulings recorded in the run ledger before Task 1 dispatched; the three
binding Task 1 travelled in its dispatch as explicit corrections that override the brief
text. Notably Ruling 3 does not restore the missing helper — it restructures the code so no
test needs the environment at all, splitting a pure `mint_host_id(&str)` out of
`resolve_host_id(conn)`.

**Severity:** high — three of five tasks carried a reference that would not compile. Not
merely three retries: the `EnvGuard` one had no correct substitute, so an implementer would
have found the private struct, seen it fit, and copied a pattern whose doc comment forbids it
and whose archived bug is a UB race in a multithreaded test binary. That failure is
intermittent, not a compile error.

**Status:** fixed-verified — rulings landed in the ledger, corrections carried into Task 1's
dispatch, before any subagent ran.

**Valid:** dated 2026-09-01

**Rests on:** the four symbol reads listed above, taken this session; and on the SDD skill's
pre-flight scan being run as a *table* (one row per task pair, one per task) rather than as a
verdict — three of the four were found by rows that would not have been written under a
"does the plan look consistent?" reading.

**Fix idea / Pointer:** run ledger at
`.superpowers/sdd/2026-09-01-committed-audit-shards/progress.md` § Pre-flight conflict scan
and § Rulings.

## F-5 — Baseline exit code came from tail, not cargo — green and uninformative

**Observed:** 2026-09-01, establishing the clean baseline in the fresh
`.worktrees/audit-shards-t7` worktree, before Task 1 dispatched.

**When:** The `using-git-worktrees` step that exists precisely to make later failures
attributable — *"a dirty baseline makes every later failure ambiguous."*

**Expected:** `cargo test --workspace --no-default-features 2>&1 | tail -5` run in the
background; the harness reported `exit code 0`, which I was about to record as a green
baseline.

**Got:** that `0` is **`tail`'s** exit code, not `cargo`'s. A shell pipeline exits with its
last stage. Re-run bare into a file: `cargo exit=0`, 26 result lines, all `ok` — so the
baseline genuinely was green **this time**, and that is the whole problem. The check and the
broken world are indistinguishable at the point of reading.

**Probable cause:** the `| tail -5` was reflex output-trimming to keep the log short. CLAUDE.md
§ Companion Plugin documents this exact failure — *"Two things `Bash` does not get: the IL-3
unbounded-pipe block (it masked a non-zero `cargo test` exit here)"* — and the guard that
would have refused it is a **codescout `run_command`** guard. I used native `Bash`, which is
permitted here while a shell-mode eval is in flight, so the pipe went through unblocked.

**Workaround:** run bare, redirect to a file, read the exit code, then query the file. Adopted
for every gate command in this run.

**Severity:** high — not for what it did (nothing; the baseline was green) but for what it
could not have detected. A red baseline read as green mis-attributes every subsequent failure
to the task that happens to be in flight, which in an SDD run means fix rounds spent on code
that was never broken. This is the *self-validating gate* shape from the skill's Phase 1: a
check that reads green in the broken world carries no information.

**Status:** mitigated — the practice is fixed for this run, and the structural guard exists
but only on the `run_command` path. Native `Bash` remains unguarded by deliberate policy while
the shell-mode eval runs, so the hazard is live for any session that reaches for `Bash`.

**Valid:** conditional — the shell-mode eval concludes and `security.shell_command_mode`
settles

**Rests on:** the IL-3 pipe limiter being a `run_command`-only guard, which CLAUDE.md states
directly; not re-verified against the hook source this session, and CLAUDE.md itself warns
that the hook source is not ground truth for runtime behaviour — the probe is.

**Fix idea / Pointer:** the honest generalisation is not "avoid pipes" but **"a pipeline's
exit code describes its last stage"** — so any gate whose result is read as a boolean must
not end in a filter. Candidate for the run-gate discipline in CLAUDE.md § Development
Commands if a second instance appears.

## W-3 — Pre-flight scan as a row-per-pair table caught 3 of 5 tasks' inherited defects

**Observed:** 2026-09-01, immediately before dispatching Task 1 of the T-7 SDD run — a plan I
had written myself 40 minutes earlier, after scouting each seam it names.

**Pattern:** **Run the pre-flight scan against your own plan as if a stranger wrote it, and
run it as a table rather than a judgement.** One row per task *pair* that shares a file or an
interface (what one produces vs. what the other consumes), one row per task for
self-consistency — and write the rows that come back clean too, because the discipline is
what forces contact with each interface rather than a global impression of soundness.

**Counterfactual:** the scan found six gaps; four were interface references that do not
exist (`catalog-audit-trail-session-log:F-4`). Without it:

- Task 1's implementer imports `crate::util::test_env::EnvGuard` → compile error → finds the
  real `EnvGuard` in `src/agent/mod.rs:2124` → **it fits** → copies it. That is the branch
  that does not end in a retry: the struct is feature-gated with a doc comment forbidding
  exactly this, and the archived bug it cites is a UB env race in a multithreaded test
  binary. Intermittent, not reproducible, lands in `master`.
- Task 4's implementer hits `ctx.project_root()` at two sites and `&root` at a third. Three
  compile errors, plus a design question no task owns — `reindex` has `targets: Vec<PathBuf>`,
  so "which root does a per-repo shard export write to?" has to be answered mid-dispatch by
  whoever is holding the smallest amount of context.
- Task 2 ships a second copy of the shard-filename convention, and Task 1's round-trip test
  guards a function the export never calls. Green, and covering nothing.

Concretely: **3 of 5 tasks carried a defect**, matching the T-1 run's 5-of-10 rate for the
same cause. Cost of the scan: ~8 minutes and four symbol reads.

**Confirming data points:**
1. This session — 6 gaps, 4 of them non-existent interfaces, in a plan whose author had
   scouted those very files while writing it.
2. The T-1 SDD run (2026-09-01, same day) — *"A plan's reference code is a sketch — 5 of 10
   tasks carried a defect inherited from the plan, none caught by its author"*, already
   promoted into CLAUDE.md § SDD Rulings.
3. `codescout:R-49` — three session-authored artifacts failing later scrutiny in one sitting,
   the promoted form of the same mechanism.

**Attribution caveat, stated because it changes what this datapoint proves:** two independent
mechanisms fired here — the SDD skill *mandates* a pre-flight scan, and `codescout:R-49` was
in context from this session's earlier recon pass. I cannot cleanly attribute the catch to
either, and counting it for both would inflate both. What this entry establishes is that the
scan **found** the drift; it does not establish that recon-without-SDD would have.

**Impact:** high — prevented one silent UB-pattern adoption and three compile-time failures
across two dispatches, and settled a design question (Ruling 2) that would otherwise have
been answered by an implementer with the least context in the run.

**Promote-when:** a third SDD run where the pre-flight scan finds a defect the plan's author
put there. At 3, the promotion is not "scan your plan" — CLAUDE.md already says that — but
the sharper form: **the scan must be written as a row-per-pair table, and the clean rows are
what make it work.** Two of the four findings here came from rows I would not have written
under a holistic read.

**Status:** validated — drift caught and ruled on before any subagent ran; the run's outcome
is not yet known, so the "prevented" claim is about the dispatches, not about the merge.

**Valid:** dated 2026-09-01

**Rests on:** `catalog-audit-trail-session-log:F-4`, same session — this win is
F-4's counterfactual and is not independent evidence of it.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->

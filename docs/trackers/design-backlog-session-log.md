---
kind: tracker
status: active
title: Session Log — Design-Decision Backlog Triage
owners:
  - marius
tags:
  - backlog
  - triage
  - design-decisions
  - capability-proposals
topic: design backlog triage
entry_prefix:
  - F
  - W
entry_high_water_F: 8
entry_high_water_W: 1
---

# Session Log — Design-Decision Backlog Triage

**Work stream.** Surveying the design decisions that have been waiting longest across four
surfaces that each answer a different question — `docs/trackers/capability-proposals.md`
(CAP-N, the pre-plan queue), `docs/plans/` (designs written and stalled), `docs/issues/`
(defects whose fix needs a design call), and `docs/ROADMAP.md` § *Future Improvements*
(unowned sketches) — and deciding which deserve a ruling. Opened 2026-09-01.

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
| F-1 | 2026-09-01 | med | methodological | fixed-verified | a plan's `status: draft` means two opposite things, and triage read the wrong one |
| F-2 | 2026-09-01 | low | measurement | fixed-verified | a grep-derived tool count was scoped to the regex's shape, not to the registry |
| F-3 | 2026-09-01 | high | stale-substrate | fixed-verified | the pipeline tracker's Resume routes an implementer to the strategy its own review rejected |
| F-4 | 2026-09-01 | med | methodological | fixed-verified | a design cost was illustrated with a use case the source never claimed, and no caller can reach |
| F-5 | 2026-09-01 | high | measurement | fixed-verified | two systems agreed and both were the wrong sample — a correct ruling was one message from retraction |
| F-6 | 2026-09-02 | low | measurement | wontfix-false-alarm | the ledger-count hook's "skip" was correct — the name is broader than its `files:` scope (**denominator, not a catch**) |
| F-7 | 2026-09-02 | high | architectural | fixed-verified | "impossible by construction" was a claim about Rust, not about the capability — C's last cost was reachable in the shell |
| F-8 | 2026-09-02 | med | architectural | open | #9 was carried as a prerequisite, and Strategy C dissolves it — third unexamined Strategy-A claim |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-01 | high | report that the shared index holds a path outside their stated list, and refuse to say whose | peer reports it would have committed by its stale list, risking a staged `adapter.rs` whose `selector_key` deletion is atomic with an unstaged `types.rs` default inversion — a red `experiments` for six sessions (mechanism verified; the would-have is their testimony) | validated |

---


## Open tasks

Deliberately **not** a ledger — no prefix, no ids, nothing to cite. These are this work
stream's remaining actions, worked in order. Opened 2026-09-02.

**The list is shorter than the session's running summary implied, and that is the first
finding.** Two of the four items carried forward were not tasks: one dissolved on inspection
and one is a watch condition. Both are recorded below rather than deleted, because "we checked
and there was nothing" is the observation this ledger's own § *Testing Discipline* says goes
unrecorded by default.

- [x] **T1 — Ledger-count hook "skip": DISSOLVED, not a defect.** Done 2026-09-02 →
  `design-backlog-session-log:F-6`. `.pre-commit-config.yaml:141` scopes the hook to
  `issue-clusters.md` and bug files; every commit this session touched other tracker files, so
  `Skipped` was correct on all six. Recorded as a **denominator, not a catch** — the doubt was
  instrumented and the mechanism was sound.

- [x] **T2 — `F-3`'s class filed as `capability-proposals:CAP-12`.** Done 2026-09-02. The
  substrate check changed the proposal's direction: the corpus is **507** files with a `Resume`
  section but **497** are bug files instantiating `docs/issues/_TEMPLATE.md:158`, so the real
  candidate set is **10** and a naive check would be 98% template noise. **0 of 507** declare a
  date, and every date-aware `doctor` check reads a *declared* field — so the check cannot be
  built on today's substrate at all. Entry's first decision is therefore **measure the other
  eight, do not build**, with an explicit note that "2 of 10" is not a rate: both were found
  while being worked on, which is `design-backlog-session-log:F-5`'s selection error.

- [x] **T3 — PARTIALLY DONE, and the task's own premise was wrong.** Done 2026-09-02 for #2 only,
  recorded as `run-command-pipeline.md` § *Rulings* **R2** (exclusivity, adopted as written; all
  three excluded modes verified at `path:line`, completeness derived from the review's nine-mode
  inventory rather than assumed). **#4, #5 and #8 are NOT "decidable as written"** — they encode
  *sequential* stages, and R1 has tilted #7 toward a single concurrent shell pipeline. Two
  measurements, both in the tracker: (a) bare `set -o pipefail` returns **141** on
  `seq 1 100000 | grep 5 | head -3` — a fully successful run — because `head` closes the pipe and
  SIGPIPEs upstream, so the feature's primary use case reports failure; and the tracker's own
  § *Tests needed* happy path uses `wc`, which consumes all input and therefore **cannot detect
  this**; (b) "stop on first non-zero" and #5's *"truncated stages array"* describe an event that
  does not occur in a shell pipeline, where every stage starts at once. Remedy available only
  because R1 gave us bash: read `PIPESTATUS` (Ubuntu's dash answers `Bad substitution`) and treat
  SIGPIPE on a non-final stage as success. **My task list contradicted the tracker's own § *Resume*
  step 2, which already said to rule #7 first.** Corrected order: #2 → #7 (T5) → #4/#5/#8.

- [x] **T3b — #4, #5, #8 ruled together as R4.** Done 2026-09-02. Not one-line consequences after
  all: #4 needed a real policy (classify per stage from `PIPESTATUS`; SIGPIPE `141` on a
  non-final stage with a later `0` is success), #5 lost `stopped_at` and the truncated-array
  branch entirely, #8 lost the `k/N` form. **One judgement call flagged inline** — the `141` rule
  cannot discriminate "trimmer finished early" from "downstream crashed and closed the pipe" by
  exit code alone; hedged by carrying raw `pipestatus` in the envelope so a caller can re-derive.
  R4 also adds three test cases, because § *Tests needed* was monotone under the very defect the
  ruling fixes.

- [x] **T7 — #3 ruled total (R5).** Done 2026-09-02. Ruled on merit rather than impossibility:
  no caller for per-stage has been named, the same test that retired per-stage cancellation.
  Recorded explicitly as a **reversible** no — per-stage stays reachable as `timeout <n> <stage>`
  with no schema migration — because declining a cheap available capability is a different act
  from declining an impossible one. Also retired #3's own `remaining = total - elapsed` wording,
  the third and last instance of the sequential assumption in that surfaces list.

- [x] **T4 — #6 and the source-gate bug ruled together as R7.** Done 2026-09-02. One rule: *a
  gate's predicate is per-command; evaluate it per-command, refuse the whole call, and name the
  offender.* The substrate check discharged the bug file's own open caveat (*"the gate source has
  not been read yet"*) and made the fix far cheaper than it assumed: `pipeline_segments`
  (`src/util/path_security.rs:1111-1123`) already splits on `&&`/`||`/`;`/newline quote-safely,
  `strip_heredoc_bodies` is at `:911`, `detect_il3_violation` already uses both — and **all three
  gates live in that same module**, so "one splitter, not two" needs two call sites, not a parser.
  Also **narrowed the bug's own root cause**: IL-3 *does* decompose and *does* name its segment,
  so the live defect is two gates that do not decompose, one of which also does not name. Bug file
  updated with the reading and the running firing count (**six** across two sessions, two of them
  on this session's own commands).

- [x] **T8 — #1 formally ruled (R6).** `stages` XOR `command`. Recorded as a ruling because it had
  been *leaning* since Concern 3 and never decided — and `design-backlog-session-log:F-3` is this
  same tracker's record of what an unresolved lean does to a later reader. Re-founded on
  `src/tools/run_command/mod.rs:211`, since Concern 3's companion-hook argument is dead.

- [x] **T5 — #7 ruled: Strategy C.** Done 2026-09-02, recorded as `run-command-pipeline.md`
  § *Rulings* **R3**. A confirmation rather than a trade-off: both of C's stated costs had been
  falsified first — per-stage cancellation withdrawn (`design-backlog-session-log:F-4`, no caller)
  and per-stage timeout shown reachable in the shell (`design-backlog-session-log:F-7`,
  `PIPESTATUS=0 124 0`). Concern 1's positive argument carried it unopposed. **Re-opens #3**
  (timeout policy), whose "total" lean rested on per-stage being impossible — both are now
  implementable, and total stays the default only because no caller for per-stage has been named.

- [ ] **T6 — #9 (`exec_one_stage`): NOT a prerequisite. Re-scope or drop.** Scouted 2026-09-02
  before editing; the premise did not survive R3 — see `design-backlog-session-log:F-8`. Under
  Strategy C a pipeline is **one** `bash -c` child, so there are no per-stage execs to factor out;
  `exec_one_stage` names Strategy A. Concern 2's *"both paths delegate to it"* is false under C —
  they **are** it, unchanged — and its *"before `pipeline=` goes anywhere"* urgency was about
  stopping a 10th dispatch mode, which C does not add: `pipeline=` branches **before** the exec
  block (build the tee-tapped string, arrange `PIPESTATUS` emission) and **after** it (classify
  per R4, shape the envelope per #5). The 326 lines between are untouched either way. Still
  defensible as a *readability* refactor — 326 lines, nine modes — but that is a weaker argument
  and belongs in its own commit, not smuggled in as pipeline groundwork. If re-scoped, the seam is
  `spawn_and_await(command, work_dir, timeout, ctx) -> Completed(Output) | TimedOut`, deliberately
  **excluding** `handle_successful_output` and the timeout-hint text, both of which are response
  formatting that `pipeline=` needs differently.

**Watch, not a task — `W-1`'s Promote-when.** Its criterion is a *third* instance of
report-don't-attribute changing another session's action. Two exist. Manufacturing a third is
not available, and turning a waiting-criterion into a task is how a Promote-when gets harvested
early.
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

## F-1 — a plan's `status: draft` means two opposite things, and triage read the wrong one

**Valid:** dated 2026-09-01

**Category:** methodological · **Severity:** med · **Status:** fixed-verified (retracted before the user acted on it)

**Observed.** Triaging "which design decisions have been waiting longest?", I ranked
`docs/plans/` by `Status:` line plus git first→last commit date, and reported
`docs/plans/2026-05-30-per-request-workspace-pinning.md` (`status: draft`, opened 94 days
prior) as a stalled design whose bug had "re-surfaced today" as
`docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`.
I called that pairing "the notable one".

**Got.** Both halves are false, and the scout that found it took two reads.

`docs/trackers/resume-workspace-pinning-phase-4b-5.md` states the plan is *"still
`status: draft` **on purpose**"*, quoting the plan's own § *Phase 4b — DEFERRED*: *"This
plan stays active (do NOT archive) while 4b is open."* Phases 0–3 and 4a shipped and were
**verified at the bytes 2026-08-28**; the lock-ordering proof is committed at `69c91896`;
`inject_workspace_param` (`src/server.rs:626-638`) advertises the per-call `workspace`
param on every pinnable tool. "Regime-3 correctness is closed and was live-verified."

And the bug is not that plan's defect re-surfacing. It is `severity: low`, sits in cluster
`IC-17` (*a shared resource carries no owner*), and its own Evidence section records that
the per-call `workspace=` parameter — the very mechanism the pinning plan shipped —
**resolved it first try**: *"the `workspace=` parameter worked first try, so no debugging
was needed."* Its Fix section asks for better error text, and explicitly declines the
architectural change: *"Deliberately **not** proposed: per-agent activation state."*

**Root cause — the status vocabulary is overloaded across two surfaces, and only one of
them is documented.** `get_guide("tracker-conventions")` defines `draft` for **trackers**
as *"Scoped / watching, not yet active"*. Nothing defines it for **plans**, and this repo
uses it there to mean the opposite: *shipped, deliberately unarchived because a named
residual is still open*. Both surfaces are `.md` under `docs/`, both are catalogued with
the same `status` column, and `artifact(find)` returns them side by side with no marker
saying which vocabulary a given row is speaking. A triage pass that sorts on `status` is
reading two languages as one.

**Why "check more carefully" is the wrong remedy.** The failure returns a *plausible
ranking*, not an error — a stalled plan and a deliberately-parked one are byte-identical
at the `status:` line, and the disambiguator (a `resume-*` tracker that names the plan) is
in a different file that no query joins to it. The reader who is best placed to catch this
is the one who already knows the residual exists, which is exactly the reader who does not
need the triage. This is `OB-N`-shaped: name the party who structurally cannot see it (a
triage pass that has not read the plan's own § *Phase 4b*), not the party who was careless.

**What the scout cost, and what it saved.** Two reads — the bug file, then the resume
tracker. Without them the user would have been pointed at a 94-day-old "stalled" design
that is in fact closed, and at a message-quality bug dressed as an architectural decision.
Three other drafts I ranked the same way survived the check: `two-stack-retrieval-lite`
(2026-06-16) carries no deliberate-draft note, and both 2026-04-02 plans have a single
commit each and no implementation in `src/`.

**Fix idea.** Two candidates, neither built:
1. A `resume-*` tracker should carry a `supersedes`-style edge (or at minimum a
   `**Plan:**` line the catalog indexes) so `artifact(find)` on a plan surfaces the
   tracker that explains its status. Today the pointer runs one way only —
   tracker → plan — so the plan is the row you find and the tracker is the row that
   would have corrected you.
2. Give plans their own status value for this state (`parked` / `residual-open`), or
   have `doctor` flag a `status: draft` plan that a `resume-*` tracker names, so the
   overload is visible at query time rather than at read time.

**Rests on:** `get_guide("tracker-conventions")` § *Status vocabulary* (tracker surface);
`docs/trackers/resume-workspace-pinning-phase-4b-5.md` § *Shipped, so nobody re-does it*;
`docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
§§ *Evidence*, *Fix*.

## F-2 — a grep-derived tool count was scoped to the regex's shape, not to the registry

**Valid:** dated 2026-09-01

**Category:** measurement · **Severity:** low · **Status:** fixed-verified

**Observed.** Checking whether the premise of `docs/plans/archive/2026-04-02-onnx-intent-router-design.md`
(*"codescout exposes 27 MCP tools"*) had decayed, I counted the live registry with
`grep(pattern="Arc::new\\([A-Z]", glob="src/server.rs")` → **20 matches**, added the
librarian adapters, and published *"20 non-librarian tools registered at `src/server.rs:324-349`
plus 6 librarian adapters = 26 today"*.

**Got.** The total is right and both terms are wrong. Reading the registry:

- `src/server.rs:351` registers `Arc::new(crate::tools::guide::GetGuide::new())` — inside the
  same `vec![]`, which closes at `:352`. The pattern `Arc::new\([A-Z]` cannot match it: the
  constructor is written as a fully-qualified path, so the character after `(` is a lowercase
  `c`. Base is **21**, not 20, and the line range I cited (`324-349`) stops two lines short of
  the vector's end.
- `src/librarian/tools/mod.rs:396-404` — `all_tools()` returns exactly **5**:
  `Artifact`, `ArtifactEvent`, `ArtifactAugment`, `ArtifactRefreshTool`, `Librarian`.
  `adapters_for` (`src/librarian/adapter.rs:171-182`) wraps each one-for-one, so 5 in, 5 out.

21 + 5 = 26, which is what the session's own advertised tool list holds. The published
*total* was never wrong — but only because I back-derived it from the tool list I could see
and then attributed it to a split I had not read.

**Root cause.** A regex-derived population is scoped to the regex's *shape*, not to the
concept it stands for. `Arc::new\([A-Z]` encodes an assumption — "constructors are written
bare" — that holds for 20 of 21 members, so it returns a plausible count rather than an
error. Nothing downstream fires, because 20 and 21 are near enough that no reader queries
either. This is `docs/adrs/2026-08-27-negative-results-name-their-scope.md` applied to a
*positive* count: the number owed the scope it was measured over, and did not carry it.

**What makes it worse than a miscount.** The number was doing argumentative work — it was
the evidence for *"the premise has not decayed, so this design is still worth deciding."*
That conclusion survives (26 ≈ 27), which is exactly why the bad derivation would not have
been caught: a wrong method that reaches the right answer leaves no trace.

**Fix idea.** For "how many X does the system register?", read the registry symbol
(`symbols(name=..., include_body=true)`) rather than grepping for a construction pattern —
the vector literal is the population, and reading it is the same number of calls. Where a
grep is the only route, publish the pattern next to the count so a reader can see what it
excluded.

**Rests on:** `src/server.rs:322-352`; `src/librarian/tools/mod.rs:396-404`;
`src/librarian/adapter.rs:171-182`;
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

## F-3 — the pipeline tracker's Resume routes an implementer to the strategy its own review rejected

**Valid:** dated 2026-09-01

**Category:** stale-substrate · **Severity:** high · **Status:** open

**Observed.** `docs/trackers/run-command-pipeline.md` (`5d022cd3b41009f4`, `status: draft`,
opened 2026-05-18, last touched 2026-08-16) was the design I recommended as "decision-ready
today — six questions, each with a stated lean." Scouting it before putting those questions
to the user found four decayed claims and one that actively misroutes.

**Got — the misrouter first.** The file's § *Resume* ends:

> Open the next session with: read this tracker → resolve open items 1, 3, 6 → write
> `run_pipeline_inner` **per strategy A** → tests → prompt update.

Its own § *Architectural review* — later in the same file — **rejects Strategy A** (*"rejected:
two pipeline-buffering mechanisms in tree"*) and Strategy B, and § *Tracker updates* records
*"Lean C unless per-stage timeout requirement emerges."* An implementer following the Resume
line, which is the section written to be read first, builds the design the review threw out.
The review was appended *above* the Resume and the Resume was never updated — the two sections
disagree, and the stale one is the entry point.

**Got — three decayed substrate claims.** Verified at the bytes 2026-09-01:

- **Concern 3's premise is false.** Its argument was *"the companion hook reads
  `tool_input.command`, sees only stage 0 … IL3 enforcement becomes blind to pipelines"*,
  and it carried **`Confidence: high`** — the highest in the file. But
  `detect_il3_violation`'s own doc comment (`src/util/path_security.rs:1167-1212`) records:
  *"That file still exists but is no longer wired: measured 2026-08-27, no `hooks.json`
  PreToolUse matcher targets `run_command` … This function is the only live enforcement."*
  There is no hook to be blind. **Open item #10** (*"companion hook must read
  `tool_input.stages`"*) is void outright.
- **…but Concern 3's conclusion survives, relocated and stronger.**
  `detect_il3_violation(command)` is called on a **single string** at
  `src/tools/run_command/mod.rs:211`, before `resolve_refs`. A `command` + `pipeline` schema
  would blind *codescout's own* gate — which covers every MCP client, not just Claude Code.
  So the `stages` XOR `command` ruling stands on better evidence than the one written for it.
- **Concern 2's prerequisite is unmet.** It requires extracting `exec_one_stage` *before any
  10th mode*. `symbols(name="exec_one_stage")` → **0 matches**. `run_command_inner` is now
  `src/tools/run_command/inner.rs:279-605` (326 lines).
- **Every line number in the review has drifted.** `inject_tee` cited at `inner.rs:145-186`
  → actually **175-228**; its call site cited at `:288` → actually **:406**; the
  foreground-exec block cited at `inner.rs:251-394` (~140 LOC) no longer names a block.

**Root cause.** The review section is dated and the Resume section is not, so nothing marks
which one is current — and the file offers no `**Valid:**` class on either. A design document
that accumulates review passes by *appending* leaves its oldest navigational instruction at
the bottom, where the reader is told to start. This is the same shape as `F-1`: the artifact
returns a plausible instruction rather than an error, and the disambiguator (the review that
overrides it) is 60 lines away in the section the reader reaches second.

**Why the tracker's age is not itself the finding.** Three of the eight design surfaces
(#2 mutual exclusivity, #4 pipefail, #5 output shape) reference nothing that moved and are
decidable as written. The decay is concentrated in exactly the claims that cite code — which
is what `**Rests on:**` exists to prevent.

**Fix idea.** Before this design is implemented: (a) delete or rewrite the § *Resume* strategy
line — it is the only actively harmful text in the file; (b) strike open item #10 and re-found
Concern 3 on `mod.rs:211`; (c) re-anchor the four line references, or replace them with symbol
names, which do not rot.

**Resolved 2026-09-01 — all four applied to `docs/trackers/run-command-pipeline.md`** through
the catalog (`artifact(action="update", patch={body_edits})`, id `5d022cd3b41009f4`), following
the repo's dated-record convention: original analysis kept, correction appended above it.
(a) § *Resume* now carries a six-step order that rules #7 before #3 and names Strategy **C**;
(b) #10 struck as `VOID 2026-09-01`, Concern 3 re-founded on
`src/tools/run_command/mod.rs:211` in a block above the retained text; (c) Concern 1
re-anchored to `inner.rs:175-228` / `:406`, Concern 2's dead `251-394` replaced with
`run_command_inner` at `:279-605` plus an explicit note that the sub-range is *derived from
two anchors, not read* — and that the "nine dispatch modes" count was **not** re-derived.

**The class is not closed.** The instance is fixed; the mechanism that produced it — an
append-only design document whose oldest navigational section sits last, where the reader is
told to start — has no guard. Candidate: `doctor` could flag a tracker whose § *Resume* (or
last section) predates its newest dated section. Not built, not filed.

**Rests on:** `src/tools/run_command/mod.rs:206-215`; `src/util/path_security.rs:1167-1212`
(doc comment); `src/tools/run_command/inner.rs:175-228`, `:406`, `:279-605`;
`symbols(name="exec_one_stage")` → 0 matches.

## F-4 — a design cost was illustrated with a use case the source never claimed, and no caller can reach

**Valid:** dated 2026-09-01

**Category:** methodological · **Severity:** med · **Status:** fixed-verified

**Observed.** Presenting the pipeline design's open questions, I named per-stage cancellation
as Strategy C's deciding cost — *"the one irreversible choice in the set … if you'd ever want
to kill a stuck `cargo test` stage without tearing down the pipeline, C is wrong."* The user
asked, in four words, why anyone would want that. **There is no answer.** I had taken the
review's line *"per-stage cancellation impossible"* and manufactured a use case to make it
concrete, without checking whether the capability was reachable.

**Got.** Four facts, none of which needed more than one read:

- `run_command` exposes **no per-stage control surface** — its parameters are `command`, `cwd`,
  `timeout_secs`, `run_in_background`, `interactive`, `acknowledge_risk`, `workspace`.
- The MCP call is **request/response**. There is no mid-flight channel through which a
  per-stage cancel could be issued even if one existed.
- Design surface #2 makes pipeline **mutually exclusive with `run_in_background`**, closing
  the one handle-based path that might have offered mid-flight control.
- The existing cancellation intent is **explicitly the opposite**:
  `src/tools/run_command/inner.rs:436-439` kills *"the entire pipeline … not just the shell"*
  on future-drop. Per-stage cancellation is not an unimplemented feature; killing everything
  is the deliberate design.

Per-stage *timeout* is hollow for the same reason: the only stage that realistically needs one
is the producer, which a total timeout already covers. And the behaviours callers actually want
are free in any shell pipeline and survive under C — `head` closing stdin SIGPIPEs the
producer, killing a producer EOFs its consumers — with `src/platform/unix.rs:80` resetting
SIGPIPE to `SIG_DFL` in `pre_exec` precisely so the first one works.

**What the check found instead.** The review's *real* risk was one line above, flagged at
`Confidence: medium` and never checked: *"`set -o pipefail` is not POSIX sh."* Measured —
`src/platform/unix.rs:71-86` execs `Command::new("sh")`. On this host `/bin/sh` → bash, so it
passes locally and on every local run; Debian/Ubuntu CI ships dash as `/bin/sh`, where
`set -o pipefail` errors. Strategy C's headline advantage was conditional on a shell codescout
does not exec. That became the ruling (§ *Rulings* R1: pipeline calls exec `bash -c`).

**Root cause — the failure mode is a plausible concern, not a wrong fact.** Everything I said
about C was *quoted accurately from the review*. What I added was the use case, and a use case
is a claim about the world that the source did not make. `R-19` covers asserting a checkable
fact; this is the softer sibling — dressing a source's abstract limitation in a concrete
scenario the source never claimed, which makes it *more* persuasive and *less* checked. The
tell: I could not have named a caller who would exercise it, and did not try.

**Why this entry is worth more than its severity.** It is the **first correction this session
that came from a reader's doubt rather than my own scout** — F-1, F-2 and F-3 were all found
by scouting. CLAUDE.md § *Testing Discipline* names this population as structurally
unrecorded: *"a reader who doubts a figure and re-counts it produces nothing"*, so the corpus
contains only cases where doubt failed to occur. This one **found something**, so it is a
catch rather than a denominator, and belongs in the record for exactly that reason. Note also
what made it cheap: the question cost four words and did not propose an alternative — it asked
for the *justification*, which is the shape that finds an unjustified claim fastest.

**Second defect, same turn, recorded because it is the more embarrassing one.** The message
announcing this finding ended *"**Logged as `design-backlog-session-log:F-4`**"*. It was not.
No `append_entry` call was made in that turn — the sentence was written as though the act had
followed. Claiming a completed write is worse than the analysis error it was reporting: the
analysis was checkable and got checked, whereas a false completion claim is believed by
default and, had the session ended there, would have left a citation in the transcript
pointing at an entry that does not exist. There is no guard for this beyond re-reading one's
own tool calls before asserting a write landed.

**Fix idea.** For "X is impossible under this design": before publishing it as a cost, name the
caller who would exercise X and the surface they would reach it through. If neither exists, the
limitation is real and the *cost* is zero — say so, rather than illustrating it. And never
write "logged as N" in the same message as the analysis; write the entry, read the returned id,
then cite it.

**Rests on:** `src/tools/run_command/inner.rs:436-439`; `src/platform/unix.rs:67-69`, `:71-86`,
`:80`; `src/platform/windows.rs:182-193`; the `run_command` tool schema;
`docs/trackers/run-command-pipeline.md` § *Rulings* R1.

## F-5 — two systems agreed and both were the wrong sample — a correct ruling was one message from retraction

**Valid:** dated 2026-09-01

**Category:** measurement · **Severity:** high · **Status:** fixed-verified

**Observed.** R1 was ruled on my claim that *"`set -o pipefail` is not POSIX sh; Debian and
Ubuntu ship dash as `/bin/sh`, where it errors."* I had measured only that `src/platform/unix.rs:71`
execs `sh`, and that **this** host's `/bin/sh` is bash — which is tautological evidence, since
bash obviously supports pipefail. The failure half was asserted, never run: no dash, ash,
busybox, ksh or mksh exists on this machine. Asked to verify, I found `docker` present and
measured it.

**Got — and the order the evidence arrived in is the finding.** First two probes, chosen
because the images were already local:

| system | `/bin/sh` → | version | result |
|---|---|---|---|
| Debian 13 trixie (`postgres:15`) | dash | 0.5.12-**12** | **works** — `pipeline_status=1` |
| Alpine 3.23 (`postgres:16-alpine`) | busybox | 1.37.0 | **works** — `pipeline_status=1` |

Both **falsified** the claim. At that point I had a complete, coherent, two-system refutation
and was one message away from publishing *"my claim is wrong, R1's rationale is falsified"* —
a retraction of a **correct** ruling. What stopped it was noticing the sample had been chosen
by what was on the disk rather than by what the proposition was about. The proposition was
about CI. `.github/workflows/ci.yml` runs `ubuntu-latest`. A third image was local:

| system | `/bin/sh` → | version | result |
|---|---|---|---|
| **Ubuntu 24.04 (= `ubuntu-latest`)** | dash | 0.5.12-**6ubuntu5** | **`sh: 1: set: Illegal option -o pipefail`, exit 2** |

**Root cause of the near-retraction — the sample was selected by availability, and
availability is uncorrelated with the proposition.** Two systems agreeing is evidence only if
their scopes differ *along the axis in question*. Debian and Alpine agree here because neither
is Ubuntu, and Ubuntu is the entire claim. That is CLAUDE.md § *Reaching a Peer Session*'s
"check independence, not agreement" — one blind spot counted twice — reappearing in a shell
probe rather than a session enumeration, which is why it was not recognised on sight.

**And the true discriminator is finer than any of the vocabulary in play.** Not "POSIX vs
bash": POSIX Issue 8 (2024) **adds** `pipefail`, so "not POSIX" was stale knowledge, true
before 2024 and false now. Not "dash vs bash" either: Debian and Ubuntu ship the **same
upstream dash 0.5.12** and disagree, because Debian's `-12` packaging carries the pipefail
patch and Ubuntu's `-6ubuntu5` does not. A claim pitched at the level of "shell family" cannot
be right or wrong — it is not fine-grained enough to have a truth value. Both my original
claim and my near-retraction were pitched there.

**Net effect on the ruling: none, and that is the uncomfortable part.** R1 stands. Had I never
verified, the ruling would still have been correct and the recorded rationale still wrong —
the outcome is insensitive to whether the reasoning was checked, which is exactly the shape
that lets bad rationale accumulate behind good decisions. The tracker now carries the measured
table and an explicit warning not to re-derive the conclusion from the first two rows.

**Fix idea.** When probing "does X fail on system class Y", enumerate the *deciding* system
first — the one the decision is about — and treat convenient systems as context, never as the
sample. Concretely here: read `.github/workflows/ci.yml` for `runs-on` **before** choosing an
image, not after two probes disagreed with the hypothesis.

**Rests on:** measured 2026-09-01 via `docker run --rm <img> sh -c 'set -o pipefail; false |
true; echo pipeline_status=$?'` against `postgres:15`, `postgres:16-alpine`, and
`rocm/llama.cpp:…ubuntu24.04_server`; `.github/workflows/ci.yml:37,70,88,241,273,306,500,519,542`
(`runs-on: ubuntu-latest`); `src/platform/unix.rs:71-86`;
`docs/trackers/run-command-pipeline.md` § *Rulings* R1.

## W-1 — report that the shared index holds a path outside their list — and refuse to say whose

**Valid:** dated 2026-09-01

**Impact:** high · **Status:** validated

**Pattern.** When you can see that a shared git index holds a path outside another session's
stated ownership list, **report the discrepancy and refuse to attribute it.** Say "the index
holds `X`, which your list does not name" — never "`X` is yours" or "`X` is theirs". Adjacency
is not authorship (CLAUDE.md § *Observer Blindness*), and on a six-session checkout a guess
costs a recipient a turn establishing a negative.

**What happened.** `codescout-b7` (`.claude-sdd`, pid 2601241) sent an unsolicited advisory
about shared-index commit hygiene, naming its own staged paths as `docs/issues/**`,
`docs/trackers/issue-clusters.md`, `src/librarian/tools/get.rs`. Measuring
`git diff --cached --name-only` before replying showed the index held
**`src/librarian/adapter.rs`** and **no** `get.rs`. I reported both facts without saying whose
`adapter.rs` was. They looked, and it changed their commit.

**Verified mechanism** — read directly, not relayed:

- Staged `src/librarian/adapter.rs` removes `fn selector_key(&self, input: &Value) ->
  Option<String>` — `LibrarianAdapter`'s override.
- Unstaged `src/tools/core/types.rs` **inverts the trait default** in the same breath:
  `fn selector_key(…) { None }` → `{ action_selector_key(self.name(), input) }`, its new doc
  comment reading *"Inverted 2026-09-01, and the previous default is why."*
- The two are atomic in one direction only. Commit the staged deletion **without** the unstaged
  inversion and `LibrarianAdapter` falls back to a default that is still `None`.
- `every_registered_tool_supplies_a_selector_key` exists (`src/server.rs:3431`), so the result
  is a **failing test**, not silently broken routing — louder than the peer's "kills guide
  routing" framing, and worth stating precisely because loud-and-shared is its own harm.

**Counterfactual — and the seam in it, named rather than smoothed over.** The *mechanism* above
is verified. The *would-have* is *testimony*: `codescout-b7` reports that trusting its own list
would have named `get.rs` (already committed by its own subagent in `61441b3d` — verified, 4
files, includes `get.rs`) and *"might have swept `adapter.rs`."* I cannot verify another
session's counterfactual intent, and I am not going to write it as though I could. What is
established: the list was stale, the index held a path outside it, and that path was
half of an atomic pair.

**What would NOT have caught it.** The pre-commit hook `refuse a pathspec commit carrying
unstaged content` (`scripts/pre-commit-unreviewed-content.sh`, `.pre-commit-config.yaml:71-76`)
compares each *pathspec'd file* against its worktree copy. `adapter.rs` was staged with no
worktree delta, so it passes — the hook has no notion of **cross-file** atomicity with an
unstaged `types.rs`. The guard test would have fired, but only after the commit had landed on
`experiments`, where five other sessions rebase onto it. That is the shape of
`docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`, whose fifth
instance CLAUDE.md discusses at length; this would have been a sixth.

**Second finding, from the same exchange: a NAME-keyed peer record decays, a PID-keyed one does
not.** I recorded pid 3624594 as `codescout-17` at ~19:00; 45 minutes later the registry called
it `compact-root-claude-md` — same pid, renamed mid-session, and the new name explains the
CLAUDE.md rewrite that landed under me. `codescout-b7` had carried `codescout-17` *"for hours —
taken from a relay, never checked."* Generalised: **relayed identity and remembered identity
both decay; only the registry is current.** Re-read it at the moment it is load-bearing, and
key durable notes to pid.

**Third: an agreement I declined to bank.** `codescout-b7`'s six-session enumeration matched
mine exactly. I volunteered, unprompted, that this is **not** independent corroboration — both
readings come from `/run/user/1000/cc-socks/`, which is per-user, so vantage cannot differ. One
instrument run twice, not two instruments; it could not detect a defect in the socket surface
itself. The peer replied that the same confusion *"has cost this repo real money"* earlier the
same evening, when two per-**profile** tools agreeing were cited as corroboration while sharing
a blind spot. Declining to bank an agreement that favours you is the same discipline as
`design-backlog-session-log:F-5`, pointed the other way.

**Promote-when.** A third instance of report-don't-attribute changing another session's action.
Then it belongs next to CLAUDE.md § *Observer Blindness*'s existing "never route by adjacency"
— which today tells you what **not** to do and names no positive move for the case where you
can see something the owner cannot.

**Rests on:** `git diff --cached` on `src/librarian/adapter.rs` and `git diff` on
`src/tools/core/types.rs`, read 2026-09-01; `src/server.rs:3431`;
`.pre-commit-config.yaml:71-76`; commits `39f64a5b` (3 files, excludes `adapter.rs`) and
`61441b3d` (includes `get.rs`); socket enumerations at ~19:00 and ~19:45.

## F-6 — the ledger-count hook's "skip" was correct — the name is broader than its files: scope

**Valid:** dated 2026-09-02

**Category:** measurement · **Severity:** low · **Status:** wontfix-false-alarm

**This entry is a DENOMINATOR, not a catch.** It records a doubt that was instrumented and
came back clean. It is here because `CLAUDE.md` § *Testing Discipline* names this exact
population as structurally unrecorded — *"a reader who doubts a figure and re-counts it
produces nothing"* — so a ledger holding only `F-1`…`F-5` overstates the hit rate of its own
author's suspicions. Five catches and one clean check is a different claim from five catches.

**Observed.** Every commit this session printed
`refuse a commit whose ledger counts disagree with its staged corpus...(no files to check)Skipped`.
I flagged it as suspicious **three times** across five commits — most pointedly on the commit
that *created* a ledger with two index tables and six entries, where a ledger-count check
seemed exactly applicable. Each time I deferred it, latterly on the grounds that a peer held
`scripts/pre-commit-ledger-counts.py`.

**Got — correct behaviour, and one line explains it.** `.pre-commit-config.yaml:141`:

    files: ^(docs/trackers/issue-clusters\.md|docs/issues/.*\.md)$

The hook is scoped to `issue-clusters.md` and bug files. Every commit this session made touched
`design-backlog-session-log.md`, `run-command-pipeline.md` or `capability-proposals.md` — none
matches. `Skipped` is the correct output, and would have been on all six.

**Root cause of the false alarm: the name is broader than the scope.** *"refuse a commit whose
ledger counts disagree with its staged corpus"* describes a general ledger invariant. The
implementation is a **cluster-ledger** check — bug files' `cluster/<slug>` tags against
`issue-clusters.md`'s counts. Reading the name, a `Skipped` on a commit containing a ledger
looks like the check failing to fire. Reading the `files:` line, it is the check declining work
it never claimed. Nothing is broken; the label just underdetermines the scope, which is enough
to buy three mentions of an agent's attention and a standing item on a task list.

**What it cost, stated because "nothing was wrong" is not the same as "it was free."** Three
reports to the user, each carrying a small false signal of an unresolved defect; one entry on a
carried-forward open list across two sessions of summary; and a deferral justified by peer
ownership of a file that turned out to be irrelevant — I never needed to read the script at
all, only its registration. The whole question was answerable from `.pre-commit-config.yaml`
at any point, including the first time I raised it.

**Fix idea (optional, cosmetic).** Rename to something naming the scope — *"refuse a bug-file
commit whose cluster counts disagree with issue-clusters.md"*. Not filed as a bug: the
mechanism is correct, and a hook name is not a contract. Recorded here so the next reader who
sees `Skipped` on a tracker commit spends zero attention on it.

**Rests on:** `.pre-commit-config.yaml:136-142`, read 2026-09-02; the `Skipped` line in the
pre-commit output of `89b07961`, `31b34960`, `187bb192`, `8047f552` and `1df40eb1`.

## F-7 — "impossible by construction" was a claim about Rust, not about the capability — C's last cost was reachable in the shell

**Valid:** dated 2026-09-02

**Category:** architectural · **Severity:** high · **Status:** fixed-verified

**Observed.** I put Strategy C's cost to the user as *"per-stage timeout becomes impossible —
the one irreversible choice in the set"*, quoting Concern 1's *"per-stage timeout impossible
(single shell process)"*. The user asked what per-stage timeout **is**. Measuring it to answer
showed the claim is false.

**Got.** Under `bash -c`:

    echo hi | timeout 1 sleep 30 | cat
    → exit=0   PIPESTATUS=0 124 0

A **middle** stage bounded independently and killed at 1s (`124` = `timeout` timed out), both
neighbours completing normally. Per-stage timeout is `timeout <n> <stage>` composed into the
shell string — the same layer Concern 1 **already uses** for per-stage cwd
(`(cd <dir> && <stage>) | tee …`). It needs no Rust handle, no second buffering mechanism, and
no change to the single-shell-process model.

**Root cause — the claim was true of the wrong subject.** Concern 1 reasoned about *Rust*:
one `bash -c` child means one `tokio::time::timeout`, so Rust cannot bound a stage. Correct,
and it does not follow that the **capability** is unavailable — only that Rust is the wrong
layer for it. **"Impossible by construction" is a claim about one construction.** The
disproof is not a cleverer argument; it is asking *"unavailable to whom, at which layer?"*
and then running one command.

**This is the second time in two days that a cost attributed to Strategy C evaporated on
contact, and the two failed differently — which is what makes it a pattern rather than a
repeat.** `design-backlog-session-log:F-4`'s per-stage cancellation had **no caller** — the
capability was unreachable *and unwanted*, and the test that found it was "name the caller and
the surface." This one is **wanted-and-available**, just at a layer the reviewer was not
looking at, and the test that found it was "run it." A single heuristic would have caught
neither; what catches both is refusing to carry a stated cost forward until it has been
either exercised or disproved.

**Net effect: Strategy C has no established cost, and T5 largely dissolves.** Its two stated
drawbacks are now one withdrawn and one false. What remains is Concern 1's original argument,
which runs *for* C — `inject_tee` is already one-stage-deep pipeline buffering in production,
and A/B would stand a second mechanism of that shape beside it.

**What I did NOT establish, stated because the temptation was to round it up.** `timeout` is
GNU coreutils and I did not verify it on Windows Git Bash. The same package supplies
`grep`/`head`/`tail`/`sed`, which `src/platform/windows.rs:182-193` already depends on by
design, so it is *likely* present — and `design-backlog-session-log:F-5` is this session's
record of what reasoning-about-another-system costs when the measurement was available. The
check is one command on a Windows host: `<git-bash> -c 'command -v timeout'`. If absent,
per-stage timeout degrades to unavailable-**on-Windows**, not unavailable-everywhere; total
timeout is unaffected on both platforms.

**Second-order finding, folded into the tracker.** A timed-out stage exits `124` and its
downstream neighbours then see a clean EOF and exit `0` — so the pipeline reads as successful
unless `PIPESTATUS` is consulted. That is the same failure mode as the SIGPIPE-`141` finding
recorded the same day, and the two compose into one rule: **decide from `PIPESTATUS`, never
from the pipeline's aggregate status.**

**Provenance.** Found because the user asked *"per-stage timeout what is it?"* — a request for
the **definition**, which forced writing down what the thing was, which is what exposed that
its impossibility had never been tested. The second high-value finding this session from a
short user question that proposed no alternative and only asked for grounding
(`design-backlog-session-log:F-4` was the first).

**Rests on:** measured 2026-09-02 via `bash -c 'echo hi | timeout 1 sleep 30 | cat'`,
GNU coreutils `timeout` 9.11; `docs/trackers/run-command-pipeline.md` § *Architectural review*
Concern 1 (*now harder*) and § *Rulings*; `src/platform/windows.rs:182-193`.

## F-8 — #9 was carried as a prerequisite, and Strategy C dissolves it — third unexamined Strategy-A claim

**Valid:** dated 2026-09-02

**Category:** architectural · **Severity:** med · **Status:** open

**Observed.** T6 was the last task: extract `exec_one_stage` from `run_command_inner`, carried
all session as *"a prerequisite under every strategy."* Reading the function before editing it
shows the premise is Strategy-A-shaped and **R3 removed it**.

**Got.** `run_command_inner` (`src/tools/run_command/inner.rs:279-605`) ends in one
foreground-exec block: `inject_tee` builds `effective_command`; a `#[cfg(unix)]` /
`#[cfg(windows)]` pair spawns it and returns `(child_output_fut, pgid)`; a heartbeat task
starts; one `tokio::time::timeout` awaits it and dispatches to `handle_successful_output`, an
execution error, or the timeout arm with its `killpg`.

**Under Strategy C there is exactly ONE exec.** A pipeline is a single
`bash -c 'a | tee t0 | b | tee t1 | c'` child — one spawn, one process group, one timeout, one
`PIPESTATUS`. There are no per-stage execs to factor a per-stage function out of. So:

- **The name is wrong.** `exec_one_stage` describes Strategy A, where each stage was its own
  spawn. Under C the block execs a *pipeline*, not a stage.
- **The stated justification is thin.** Concern 2's argument is *"both current foreground path
  and pipeline= delegate to it."* Under C they do not delegate to it — they **are** it,
  unchanged. `pipeline=` is a **command-construction** step *before* the block (build the
  tee-tapped shell string; arrange for `PIPESTATUS` to be emitted) and a **response-formatting**
  step *after* it (classify per R4, shape the envelope per #5). The 326 lines in between are
  untouched either way.
- **It is therefore not a prerequisite.** Concern 2's *"before `pipeline=` goes anywhere,
  extract…"* was written to stop a **10th dispatch mode** landing in an already-9-mode function.
  Under C, `pipeline=` adds no dispatch mode to that block: it branches before and after it, not
  inside it.

**Root cause — same shape as `design-backlog-session-log:F-4` and `design-backlog-session-log:F-7`, third instance, and this one
is a *prerequisite* rather than a *cost*.** All three are claims made about Strategy A's
mechanics that were carried forward unexamined after C was chosen: F-4 a cost with no caller,
F-7 a cost reachable at another layer, and now a prerequisite that the chosen strategy dissolves.
The review is not careless — it was written **before** C existed as an option, and C was added
*by that same review* in Concern 1. **A review that introduces a new alternative does not
automatically re-audit its own earlier concerns against it**, and nothing downstream forces the
re-audit either, because each concern reads as self-contained.

**What survives.** The refactor is still *defensible* — 326 lines and nine dispatch modes in one
function is past this repo's own stated threshold — but that is a **readability** argument, and a
much weaker one than "a prerequisite that unblocks the feature." It should be done, or not, on
its own merits and in its own commit, not smuggled in as pipeline groundwork. And it touches the
load-bearing parts: process-group creation, the `PgidKillGuard`, SIGPIPE reset in `pre_exec`, the
Windows Job Object, and two `#[cfg]` arms that cannot both be compiled locally.

**Recommendation.** Do **not** treat #9 as blocking. Either drop it from the pipeline plan, or
re-scope it as *"extract the foreground exec so `pipeline=` can reuse it without a tenth
branch"* — which under C means extracting a `spawn_and_await(command, work_dir, timeout, ctx) ->
Completed(Output) | TimedOut`, deliberately **excluding** `handle_successful_output` and the
timeout-hint text, since both are response formatting and `pipeline=` needs different ones.

**Rests on:** `src/tools/run_command/inner.rs:279-605` read in full 2026-09-02;
`docs/trackers/run-command-pipeline.md` § *Architectural review* Concern 2 and § *Rulings* R3,
R4, R7.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->

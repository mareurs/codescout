---
id: cf9cdcc0cd91ef1e
kind: tracker
status: active
title: Statement Validity Layers 1–2 — Session Log
tags:
- session-log
- statement-validity
- doctor
- subagent-review
topic: statement validity layers 1-2 — doctor checks, subagent review discipline
entry_prefix:
- F
- W
entry_high_water_W: 7
entry_high_water_F: 6
---

> **Work stream:** Layers 1–2 of
> `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` — the
> `**Valid:**` decay class, the `**Rests on:**` proof route, and four read-only `doctor`
> checks gated on a shared citation-exposure metric. Plan
> `docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md`, 8 tasks plus three
> fix rounds, `dc158bbb..HEAD` on `experiments`.
>
> **Two-sided:** frictions (`F-N`) and wins (`W-N`). Entries with `Status: open` carry
> forward. Promotion to permanent surfaces happens when an entry's `Promote-when` fires.
> Copied from `docs/templates/session-log.md`; the Status vocabulary and category
> conventions below are pinned there so they mean the same thing across sessions and agents.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-2 | 2026-08-20 | med | subagent | open | A locally-true claim, restated one layer up, becomes false |
| F-3 | 2026-08-20 | low | codescout-tool | fixed-verified | Pre-writing index rows in a new ledger consumes the ids they name |
| F-5 | 2026-08-20 | med | architectural | open | entry_cite's PK excludes origin, so prune-and-rematerialize cannot own a duplicated edge |
| F-6 | 2026-08-21 | med | self-friction | fixed-verified | Having read a fact is not having applied it — extract's dedup |
| F-4 | 2026-08-20 | med | architectural | fixed-verified | Gated doc surfaces were kept current; the routing that serves them was never checked |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-3 | 2026-08-20 | high | Apply mutations, never reason about them | Nine regression-guard holes ship, each one refactor from vanishing, with a green suite | promoted-to-permanent-docs |
| W-4 | 2026-08-20 | high | Require every implementer to report where the brief was wrong | A brief's bad advice loosens a date parser while appearing to tighten it | validated |
| W-6 | 2026-08-21 | high | Scout the seam before consuming machinery a spec describes | Silent edge loss on prune, plus a materializer counting calls as rows | validated |
| W-7 | 2026-08-21 | high | Measure the parameter before baking it into immutable data | A false collision worry shipped, and the cap set 10 chars past the knee | validated |
| W-5 | 2026-08-20 | med | Audit the surfaces that SERVE a concept, not the docs that describe it | Four checks ship saying "add one" without saying what one is, and the likeliest guesses are the shapes the parser refuses | validated |

> Ids start at `F-2` / `W-3`, not `F-1` / `W-1`: the index rows were pre-filled before the
> sections existed and the allocator counted them as claimed. See `F-3`.
---

## Promotion status

**Audited:** 2026-08-20, against the target surfaces themselves — opened and read, not
recalled; and after the promotion, re-verified by predicate rather than by recall.

- **W-3** — **PROMOTED 2026-08-20, all three profiles.** The base mutation-apply rule was
  already in `~/.claude/CLAUDE.md` § *Subagent Dispatch* (validated 2026-08-18, 6 datapoints,
  eduplanner-ui `calendar-insight-panel-session-log:W-4`). What this entry added is the
  **copied-from-sibling mechanism** — where to aim mutations, and the instruction to observe
  a mutation survive *before* writing the test that closes it. Neither was derivable from the
  existing text.
  Written to `~/.claude/`, `~/.claude-sdd/` and `~/.claude-kat/` by editing one and copying,
  so the three cannot drift: all at md5 `ca9421bb556db7b76d61b10c376daefa`, 176 lines.
  `grep -c 'statement-validity-session-log:W-3'` returns 1 in each.
- **W-6** — *UNFIRED, carried forward.* One work stream. Promote at a second where a
  spec's account of an existing schema turns out incomplete at the constraint layer.
- **W-7** — *UNFIRED, carried forward, and pairs with W-3.* Same law at a different
  phase: apply the instrument rather than reasoning about what it would say — W-3 at test
  time, W-7 at design time. Promote the pair as one rule at a third datapoint.
- **W-4** — *UNFIRED, carried forward.* One work stream, three instances. Promote at a
  second independent work stream.
- **F-2** — fix idea is bound for the same surface as `W-4`; not yet a promotion.
- **F-3** — resolved in place; the open question is only whether
  `docs/templates/session-log.md` should carry the warning.
- **W-5** — *UNFIRED.* One datapoint. Promote at a second post-ship audit that finds a
  runtime-surface gap the doc surfaces hid; target is `CLAUDE.md` § Prompt Surface
  Consistency, which today names three surfaces and gates them only for tool-name drift.
- **F-4** — not a promotion; filed as a bug, now **fixed and archived**
  (`docs/issues/archive/2026-08-20-doctor-entry-validity-rows-never-route-to-tracker-conventions.md`).
  Fixed in `32736ca0` (patch-id `87fd01df0ffe843d505c3619926fe0285d142b08`): `names_tracker_path`
  gained a `violations[].path` branch. Its consequence was independently mitigated first —
  the four checks carry their own remediation text (`ada22c94`) — so the information now has
  two non-overlapping routes rather than a primary and a fallback.
## Category conventions

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool |
| `subagent` | Subagent or dispatch-brief produced unexpected output or diverged from instructions |
| `plan-prose` | Plan/brief drift vs reality (wrong paths, fictional code, mismatched counts) |
| `architectural` | Structural property of the system the plan / docs didn't surface |
| `self-friction` | Predicted friction that turned out to be a false alarm |
| `release-pipeline` | Deployment-time gap |

---

## F-N entry template

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

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

A win without a **Counterfactual** is marketing — name what would have happened without
the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for >=2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this instantiates>

---
```

---

## Status vocabulary

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Fix landed AND empirically confirmed. |
| `promoted-to-bug-tracker` | Moved to a formal tracker; that tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point. Do NOT close. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by >=1 counterfactual data point. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. |
| `promoted-to-permanent-docs` | Landed in CLAUDE.md, an ADR, or a skill. Names every instance for a multi-instance target. |
| `archived` | Pattern no longer load-bearing. |

---

## W-3 — Mutation-apply found nine holes that mutation-reasoning would have missed

**Observed:** 2026-08-20, fix rounds 1–3 of the statement-validity branch.

**Pattern:** Every implement and review dispatch was instructed to **apply** each candidate
mutation to real source, run the full suite, and report the *observed* pass/fail — never to
argue about whether the tests would catch it. A coverage argument was explicitly declared
not to be a mutation result.

**Counterfactual:** Nine distinct regression-guard holes would have shipped. Every one is in
code that *reads* correct, because each is byte-identical to code the suite does cover in a
sibling function. A reviewer asked "is this check consistent with its siblings?" answers yes
— correctly — and that question never reaches "and is the consistency pinned?". Each hole was
one careless refactor from silently vanishing, with 4355 green tests asserting nothing about
it.

**Confirming data points:** predicate — distinct mutations reported SURVIVED against the full
suite, counted once even when closed in a later round.

- **Fix round 1 implementer** (8 mutations): swapping `containing_root` for
  `String::starts_with` in the new scope guard survived. The pre-existing sibling-root test
  used paths that do not collide on prefix; only a purpose-built `active-project` vs
  `active-project-2` fixture caught it. The component boundary `containing_root`'s own doc
  comment calls security-relevant. → **1**
- **Fix round 1 Opus review** (11 mutations): 4 survived at 4355/0 — the scope guard
  deletable in `scan_conditional_past_due` and in `scan_dated_stale`, the
  `entry_validity_scoped_by_project` grouping key replaceable with a constant, and its fold
  changeable from `+= n` to `= n`. → **4**
- **Fix round 3** (6 mutations): 4 survived — the new `scan_validity_unparseable`'s
  archived-row skip, its scope guard, its `declared_section_text` truncation, and its
  `call()` wiring, all deletable with a green suite. → **4**
- Fix round 2 closed round 1's four and found no new ones, so those are counted once, not
  twice. **Total 9.**

**Mechanism — new, and not named by the already-promoted clause:** a new check copied from a
sibling inherits the sibling's *discipline* but not its *tests*, because the sibling's tests
are named for the sibling. The more faithful the copy, the more invisible the gap. All nine
holes are of this shape, across three separately-reviewed rounds.

**Second-order datapoint — apply before you write the test, not after.** Fix round 2's
implementer found the Opus review's own M10 description aimed one level too high: mutating
`outside_roots_group` itself is *already* caught by three pre-existing tests, because that
function also feeds the unrelated `abs_path_outside_managed_roots` aggregate. The real gap was
narrower — only the three `.entry(outside_roots_group(&path))` call sites inside the validity
scans. They found it by applying the mutation and observing it survive **before** writing the
test, as the brief required. Writing the test from the mutation's *description* would have
pinned an already-pinned behaviour, left the real hole open, and reported green.

**Impact:** high

**Promote-when:** FIRED 2026-08-20 — promoted on the strength of 9 datapoints in one work
stream rather than waiting for a second, because all nine share one mechanism and the rule
is cheap to unwind if a second stream disagrees.

**Promoted-to:** `~/.claude/CLAUDE.md` § *Subagent Dispatch — Model Floor + Review Escalation*
**Promoted-to:** `~/.claude-sdd/CLAUDE.md` § *Subagent Dispatch — Model Floor + Review Escalation*
**Promoted-to:** `~/.claude-kat/CLAUDE.md` § *Subagent Dispatch — Model Floor + Review Escalation*

**Verification:** all three profiles byte-identical at md5 `ca9421bb556db7b76d61b10c376daefa`
(176 lines), and `grep -c 'statement-validity-session-log:W-3'` returns 1 in each — the
promoted text back-cites this entry, so verification survives any rewording.

**Status:** promoted-to-permanent-docs

**Valid:** dated 2026-08-20

**Rests on:** `~/.claude/CLAUDE.md` § *Subagent Dispatch* — "Mutation-apply discipline —
reasoning about a mutation lens is not the same as applying one."

---

## W-4 — Making implementers report brief errors caught three false claims in the controller's own briefs

**Observed:** 2026-08-20, fix rounds 1–3 of the statement-validity branch.

**Pattern:** Every dispatch brief ended with a required report field — *"every place this brief
was wrong, and what you did instead"* — and named the design calls that were the controller's
judgement rather than established fact, with the falsification test for each and an explicit
instruction to stop and report rather than silently implement the other shape.

**Counterfactual:** Three false claims in briefs I wrote reached implementers. Each was caught
and corrected rather than absorbed; without the field, all three would have been implemented
faithfully, because a brief reads as authority.

**Confirming data points:**

- **The chrono claim would have loosened the parser while appearing to tighten it.** The fix
  round 3 brief said `chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")` validates shape and
  calendar in one step, so the shape regex could go. The implementer measured it: that format
  also accepts `26-08-20`, `2026-8-20` and `2026-08-2`. Following the brief would have shipped
  a *weaker* date check inside a commit whose stated purpose was strengthening it — in the
  branch built to catch claims that decay.
- **A false absence about a test helper.** The fix round 1 brief asserted "there is no
  `sample_art` / `art_upsert`". `art_upsert` is an aliased import of
  `catalog::artifact::upsert`, in scope in `src/librarian/tools/doctor.rs`'s own test module at
  `:3197`, alongside
  `TestArtifactRowBuilder` and its `with_status`. The implementer worked around the phantom
  absence by hand-building rows; the round 2 brief corrected it at the bytes.
- **A precedent citation that argued the opposite.** The fix round 1 brief cited
  `call():263`/`detect_move_candidates` as precedent for "no active project → report
  everything". The line is 282, its `None` arm returns *empty*, and the Opus review found that
  all three in-file precedents fail **closed** — every cited precedent argued against the
  instruction it was supporting. The behaviour was independently adjudicated as correct on its
  merits (fail-closed would make the scoped-out aggregate vanish too, a 100% silent loss), so
  the instruction survived its own broken justification.

**Mechanism:** each false claim was *inherited*, not invented — restated from a subagent's
report or from the controller's own prior belief, and never re-verified at the bytes. The
`art_upsert` one is the sharpest: a Task 7 report said it accurately and **locally** (true of
`src/librarian/tools/doctor.rs`), and the controller restated it one layer up as an absolute,
where it was false.
See `F-2`.

**Impact:** high

**Promote-when:** a second independent work stream where the field catches a brief error.
Candidate surface: the dispatch section of whichever skill owns brief authoring, as a required
report field rather than a suggestion.

**Status:** validated

**Valid:** dated 2026-08-20

**Rests on:** `~/.claude/CLAUDE.md` § *Conclude Last* — "a belief you already hold is exactly
the thing your re-reading cannot audit."

---

## F-2 — A locally-true claim, restated one layer up, becomes false

**Observed:** 2026-08-20, across all 8 tasks and 3 fix rounds of the statement-validity branch.

**When:** Writing dispatch briefs for implementer and reviewer subagents, each brief
summarising what prior rounds had established so the next agent would not re-derive it
(Iron Law 6).

**Expected:** A claim verified by a prior agent and reported back is safe to restate.

**Got:** Four instances where a statement true in its original scope became false one layer
up, and the branch's own subject matter is exactly this failure:

1. **`art_upsert`.** Task 7's implementer reported "`sample_art`/`art_upsert` don't exist" —
   true of `src/librarian/tools/doctor.rs`'s test module, where they had just been burned by a
   brief that hallucinated them. Restated in the fix round 1 brief as an absolute, it was false:
   `art_upsert` is imported at `src/librarian/tools/doctor.rs:3197`. See `W-4`.
2. **The impossible date, across the parser/check boundary.** Task 2's review deferred
   `dated 2026-99-99` as "accepted — shape-only, spec-compliant", correct **about the
   parser**. Task 6 then wrote a silent `continue` when the conversion fails, correct
   **about the check**. Six tasks apart, both locally right, and between them a Statement
   that reads declared and healthy while being invisible to every worklist. Task 6's review
   called that date "rejected" (true of the conversion); Task 8's implementer called it
   "parses, then silently skipped" (true of the record's fate). Neither statement is wrong;
   the gap between the layers is where the record disappears. Filed, then fixed in round 3
   (`954c6051`).
3. **The `2026-02-30` skip test.** `dated_stale_skips_a_shape_valid_but_calendar_invalid_date`
   keeps passing after round 3 — `scan_dated_stale` still returns empty — but its comment
   ("these all pass the regex and reach `iso_to_epoch_days`") went false, because the value
   is now rejected earlier. A green test whose stated reason has rotted.
4. **The guide's own repaired sentence.** `tracker-conventions.md` now says an undeclared
   entry "already means `dated <its last commit>` by default" — true of the spec, while that
   clock is **unimplemented** (`resolve_validity` has zero production callers). The next
   sentence corrects it ("what actually decides… **today** is exposure"), so a careful reader
   lands right, but the pattern recurred inside the fix for the pattern.

**Probable cause:** A report states a finding in the scope it was measured in. The scope is
carried in the surrounding context, not in the sentence — so restating the sentence elsewhere
silently drops the qualifier that made it true.

**Workaround:** Re-verify at the bytes before putting an inherited claim in a brief, and write
the scope into the sentence ("not defined in `src/librarian/tools/doctor.rs`", not "does not
exist"). The
downstream catch is `W-4`'s required report field.

**Severity:** med

**Status:** open

**Valid:** dated 2026-08-20

**Rests on:** the spec's own thesis — a claim that was true when written and false when read is
the decay this feature exists to surface;
`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`.

**Fix idea / Pointer:** `docs/issues/2026-08-20-validity-spec-terminology-contradicts-decision-3.md`
is instance 4's spec-level twin. No code fix; this is a brief-authoring discipline bound for the
same surface as `W-4`.

---

## F-3 — Pre-writing index rows in a new ledger consumes the ids they name

**Observed:** 2026-08-20, creating this tracker.

**When:** Creating a new session log from `docs/templates/session-log.md` and filling in the
Index / Wins Index tables with the entries about to be appended — `F-1`, `W-1`, `W-2` — before
calling `append_entry` to write the sections.

**Expected:** The first `append_entry(id_prefix="W", …)` on a ledger with no `W-N` **sections**
allocates `W-1`.

**Got:** It allocated `W-3`, reporting `body_max: 2`. The next allocated `W-4`, and the first
`F` allocated `F-2`. This ledger's live entries are therefore `F-2`, `F-3`, `W-3`, `W-4`, with
no `F-1`, `W-1` or `W-2` — and the index rows naming those three defined nothing, which is the
exact dangling-citation shape `get_guide("tracker-conventions")` § *One entry format, never two*
warns about, produced inside the tracker documenting careful practice.

**Probable cause:** Working as designed, and documented — the allocator computes the next id
from "the live max across both existing params entries **and ids the markdown body already
claims (headings / index rows)**". A table row is not a definition for `link_scan`, but it *is*
a claim for the allocator. The two subsystems read the same row differently, on purpose: the
allocator is deliberately conservative so a body that ran ahead of params cannot reissue an id.

**Workaround:** Create the ledger with **empty** index tables, append every entry, then write
the index rows from the ids the server actually returned. Never pre-fill a row for an entry
that does not exist yet.

**Severity:** low

**Status:** fixed-verified

**Valid:** invariant

**Rests on:** `get_guide("tracker-conventions")` § *Entry ids* — the id namespace is server-owned
state, and a row that names an id is a claim on it even though it defines nothing.

**Fix idea / Pointer:** worth one clarifying line in `docs/templates/session-log.md` — its
placeholder rows (`| F-1 | YYYY-MM-DD | … |`) are precisely the trap, since anyone copying the
template and filling them in first hits this. TBD whether to fix the template or leave the
lesson here.

---

## F-4 — Gated doc surfaces were kept current; the routing that serves them was never checked

**Observed:** 2026-08-20, post-cohort prompt-surface audit of `ced75046..f1d862ae`
(statement-validity layers 1–2).

**When:** Asking, after the cohort shipped, whether an agent would actually encounter
`**Valid:**` at the moment it needed it — as opposed to whether the concept is documented.

**Expected:** Covered. The cohort updated `src/prompts/guides/tracker-conventions.md`
(7 mentions of the cohort's identifiers) and `docs/templates/session-log.md` (4), so the
authoring path was believed served.

**Got:** Half true, and the false half is the half that matters. Coverage is genuine for
`artifact`-mediated writes: `tracker-conventions` is auto-injected whenever
`names_tracker_path` matches (`src/librarian/adapter.rs:182-205`) and is **not** in
`PULL_ONLY_GUIDE_TOPICS` (`src/prompts/mod.rs:391`) — so it is not the reader-initiated
surface it appears to be. But `librarian(action="doctor")` — the one call that *produces*
entry-validity worklists — never matches. `names_tracker_path`
(`src/librarian/adapter.rs:276-292`) reads only `abs_path`/`rel_path`, at the top level or
one level into an `items` array; the doctor payload keys its rows under `violations`
(`src/librarian/tools/doctor.rs:466`) and names the field `path`
(`src/librarian/tools/doctor.rs:135`). So the agent holding 30
`entry_cited_from_outside_but_undeclared` rows is handed `get_guide("librarian")`, which
mentions the concept **zero** times, and never sees the guide that mentions it seven.

**Probable cause:** The cohort answered "is the concept documented?" when the operative
question is "does the surface that serves it fire on the call that needs it?" Nothing
gates guide-routing against a new check family, and the routing predicate is keyed on a
response *shape* that the new check family does not have.

**Workaround:** Call `get_guide("tracker-conventions")` explicitly after any doctor run
reporting `entry_*` / `validity_unparseable` rows. Independently, the four checks' `detail`
strings now carry their own remediation text (see the W-N entry below), so a mis-routed
agent can still fix the row from the report alone — that mitigates the consequence, not
this defect.

**Severity:** med — every entry-validity triage session gets the wrong guide, silently. No
error, no missing output; just a worklist the reader has not been taught to act on.

**Status:** fixed-verified

**Valid:** dated 2026-08-20

> The `conditional` this entry originally declared — *"until `names_tracker_path` routes
> doctor responses to `tracker-conventions`"* — **fired the same day**, in `32736ca0`.
> Reclassified to `dated`, because what remains true is the observation as of that date,
> not an open condition. Left here as a worked example of the class doing its job: the
> event was named, so the entry could be resolved rather than re-derived.

**Rests on:** A teaching surface is only as good as its trigger — coverage is a property of
the routing, not of the text.

**Fix idea / Pointer:**
`docs/issues/archive/2026-08-20-doctor-entry-validity-rows-never-route-to-tracker-conventions.md`
(fixed `32736ca0`, patch-id `87fd01df0ffe843d505c3619926fe0285d142b08`)

---

## W-5 — Auditing what SERVES a concept found the fix text already written and discarded

**Observed:** 2026-08-20, post-cohort prompt-surface audit of the statement-validity
cohort (`ced75046..f1d862ae`), run as a separate pass after the cohort's own docs commit
`f1d862ae` had already fixed the mdBook gap.

**Pattern:** After a cohort ships a new concept, audit the surfaces that serve it **at
runtime** — tool descriptions, guide-routing triggers, and the tool's own output strings —
as a pass distinct from the docs that *describe* it. Rank findings by delivery, not by
prose quality: always-on outranks gated-but-automatic, which outranks reader-initiated.

**Counterfactual:** `parse_validity` builds a `RecoverableError` carrying **both** a
`message` (what is wrong) and a `hint` (how to fix it), and every one of its three error
arms populates that hint with `FORMS` — the three valid declarations
(`src/librarian/statements.rs:27`, `:127`, `:149`, `:161`). `scan_validity_unparseable`
interpolated `err.message` alone (`src/librarian/tools/doctor.rs:2416`). The remediation
text was written, correct, and one struct field away from the report — for the entire
cohort. `scan_cited_but_undeclared` had the matching gap: its detail ended `— add one`,
with no statement of what "one" is.

Without this audit both ship as-is, and the failure is worse than silence: an agent told to
"add one" guesses, and two of the three natural guesses — a bare `conditional`, or free
text — are **exactly** the shapes `parse_validity` refuses. The check would have converted
its own `entry_cited_from_outside_but_undeclared` row into a `validity_unparseable` row,
i.e. generated its own follow-on work. The concept's mdBook page
(`docs/manual/src/concepts/statement-validity.md`, 171 lines, and genuinely good) would not
have prevented any of it: it is served to no agent.

**Confirming data points:**
1. The discarded `err.hint`, above — fixed this session. Both mutations applied and
   *observed*: reverting each detail string turns its own test red naming the exact
   pre-fix output; restored, 4368 pass. (The mutation-apply discipline is [[W-3]]'s;
   this is a further datapoint for it.)
2. F-4 (this log) — the guide that teaches the concept is never routed to the call that
   produces the rows. Same shape as the finding above: text current, delivery unchecked.
3. `tools/list` measured at 56,266 / 56,266 characters — **zero** headroom — so the
   always-on surface cannot absorb the concept without funding a trim elsewhere. A
   docs-only audit surfaces neither the constraint nor the fact that `librarian`'s
   `doctor` blurb still enumerates seven old checks and none of the four new ones.

**Impact:** med — one shipped defect caught in the surface with the highest leverage per
byte, plus a standing constraint (zero `tools/list` headroom) that any future coverage fix
must respect.

**Promote-when:** A second cohort's post-ship audit finds a runtime-surface gap that the
doc surfaces hid. At 2 datapoints, promote to `CLAUDE.md` § Prompt Surface Consistency as:
*a concept is covered when the surface that SERVES it fires on the call that needs it —
check tool descriptions, guide-routing triggers, and tool output strings, not only the
guides.*

**Status:** validated

**Valid:** dated 2026-08-20

**Rests on:** The distinction between a surface that *describes* a concept and one that
*serves* it at the moment of use. `CLAUDE.md` § Prompt Surface Consistency names three
surfaces and gates them for tool-name drift; none of the three is what an agent reads
while triaging a worklist, and no gate could have caught a missing concept.

---

## F-5 — entry_cite's PK excludes origin, so the spec's prune-and-rematerialize contract cannot own a duplicated edge

**Observed:** 2026-08-20, pre-implementation scout of Layer 3b (the `origin='scan'`
materializer), before touching `link_scan::call`.

**When:** About to mirror the artifact-grain diff machinery (`diff::diff` / `diff::apply`,
`links::by_rel`) for entry grain, and to add the prune-and-re-materialize pass the spec
describes.

**Expected (spec § Layer 3 → Resolution and materialization):** *"Rows are written to the
existing `entry_cite` table with `origin='scan'`. The `origin` column exists today as a
forward-compat placeholder that MVP only ever writes as `'write'`, so this is the use it
was reserved for. Scanner-owned rows are pruned and re-materialized per scan;
`origin='write'` rows are never touched by the scan."* Reads as a contract the existing
schema and helpers already support.

**Got (scouted reality):** Two gaps, one mechanical and one a measurement trap.

1. **No prune path exists.** `src/librarian/catalog/entry_cite.rs` exposes exactly
   `insert_with`, `outgoing`, `incoming`, `incoming_like` and a private `collect`. There is
   no delete/prune of any kind, so "pruned and re-materialized per scan" is machinery to be
   built, not reused. There is also no index on `origin` (only `idx_entry_cite_dst`), which
   is fine at ~2000 rows but is a full scan.

2. **`origin` is NOT in the primary key.** Live schema:
   `PRIMARY KEY (src_slug, src_local, dst_ref, rel)`, with `origin TEXT NOT NULL DEFAULT
   'write'`. Combined with `insert_with`'s documented `INSERT OR IGNORE`, an edge that a
   user already wrote via `append_entry(cites=…)` and that the scan independently derives
   from prose **collides on the PK and is silently ignored**. The row keeps
   `origin='write'` forever.

   The *behaviour* is correct and matches the spec's intent — an explicitly written edge is
   never clobbered, and pruning `WHERE origin='scan'` correctly leaves it alone. The defect
   is in what the scan can **report**: derived-edge count and rows-written count differ,
   and `INSERT OR IGNORE` makes a no-op indistinguishable from a write. A materializer that
   reports "N entry edges materialized" from its insert-attempt count would state a number
   its instrument did not measure — the exact class this project's CLAUDE.md § Measurement
   was written for, and the third time this work stream has produced one.

**Probable cause:** The spec was written against the schema's *shape* (the `origin` column
exists) without reading the PK it sits beside, and `insert_with`'s `INSERT OR IGNORE` is
documented on the function rather than in the schema, so the interaction is invisible from
either half alone.

**Workaround:** Build the prune helper, and count rows actually written via
`Connection::changes()` after each insert rather than counting attempts — report `derived`,
`written` and `skipped_existing` as separate numbers so a write-owned duplicate is visible
instead of inflating the materialized count.

**Severity:** med — no data loss and no wrong edges; the cost is a plausible wrong number in
the tool's own report, and one that would be believed because it comes from the tool that
did the work. Caught before any code was written.

**Status:** open

**Valid:** dated 2026-08-20

**Rests on:** `get_guide("tracker-conventions")`'s rule that entry ids key `entry_cite` rows
and must never be re-keyed — the same invariant that makes `artifact.slug` immutable and
gives `src_slug` its `ON DELETE CASCADE`.

**Fix idea / Pointer:** Layer 3b, this work stream. `entry_cite.rs` needs
`prune_scan_rows(conn, src_slugs)`; `link_scan::call`'s response needs the three-way count.

## W-6 — Scouting the seam before Layer 3b turned two silent defects into code that was written correctly the first time

**Observed:** 2026-08-21, before writing the `origin='scan'` materializer.

**Pattern:** Invoke reconnaissance at the seam — before modifying a 220-line function to
consume machinery the spec *describes* — and read the actual schema, the actual helper
surface, and the actual PK, rather than the spec's account of them.

**Counterfactual:** Both gaps were invisible from the spec, and neither would have failed
a test or raised an error. They would have shipped as a working feature reporting a wrong
number.

1. The spec calls prune-and-re-materialize a property of the existing table. `entry_cite`
   had **no delete path at all** — `insert_with`, `outgoing`, `incoming`, `incoming_like`
   and a private `collect`. The obvious implementation of "prune scanner-owned rows" is
   `DELETE WHERE origin='scan'`, and mutation M-H later confirmed that deletes rows for
   artifacts outside the scan's scope which this pass cannot re-derive — silent edge loss,
   no error.
2. `origin` is **not in the PK** (`src_slug, src_local, dst_ref, rel`). With
   `INSERT OR IGNORE`, a scan-derived edge duplicating a hand-written one is dropped and
   keeps `origin='write'`. Correct precedence — but *derived* and *written* then differ,
   and counting insert calls would publish a figure the instrument never measured.

**Confirming data points:**

1. Both became code: `prune_scan_rows` is scoped to the slugs the pass extracted, and
   `insert_with` returns rows-written so the response reports `derived` / `written` /
   `skipped_existing` separately (`7468902b`).
2. Mutation-verified: M-G (drop the origin filter), M-H (drop the src_slug scope) and M-I
   (`insert_with` hardcoded to `Ok(1)`) were all CAUGHT — M-I is defect 2 written out
   literally.
3. The live run returned `skipped_existing: 0`, so `written == derived` **by coincidence**
   on this corpus. The honest counter cost nothing and is the only reason that equality is
   evidence rather than indistinguishable from the bug.

**Impact:** high — a wrong count from the tool that did the work is the most believable
kind, and defect 1 loses data silently.

**Promote-when:** the reconnaissance skill already covers this; the specific lesson worth
promoting is narrower — *a spec that describes existing machinery is a claim about the
substrate, and the PK/constraint layer is where it is least likely to have been read.*
Promote at a second work stream where a spec's account of a schema turns out incomplete.

**Status:** validated

**Valid:** dated 2026-08-21

**Rests on:** `statement-validity-session-log:F-5`, which records both gaps and the scout
that found them; and this project's CLAUDE.md § Measurement, whose rule the second gap
would have violated.

## F-6 — Having read a fact is not having applied it — the same session quoted extract's dedup and then designed against its opposite

**Observed:** 2026-08-21, adding an `attributed` counter to the entry-grain materializer.

**When:** Writing a doc comment and a test to justify reporting `attributed` beside
`derived`.

**Expected:** That one entry citing one target five times would produce five
`attributed` and one `derived`, the deduplication happening in the materializer's
`BTreeSet`.

**Got:** `citations: 1`. `extract::push_citation` is
`if seen.insert((kind, raw.clone()))` — **one citation per `(kind, raw)` per document**,
keeping the first occurrence's line. So repeating a token inside an entry cannot inflate
anything; the collapse happens upstream, and the only collapse left in the materializer is
a bare token and its stem-qualified twin. Live corpus: 324 attributed → 322 derived, a
collapse of exactly 2.

**Probable cause — and this is the part worth keeping.** I had read that exact fact
**earlier in the same session**, in `entry_indegree`'s doc comment, and had quoted it back
in prose: *"a file-level count, not an occurrence-count… so one chatty file cannot inflate
a token's apparent reach."* It was true, I understood it, and I still wrote a rationale
that contradicted it — because when I read it the topic was *exposure*, and when I
contradicted it the topic was *attribution*. The fact was filed under the wrong question.

This is **not** `F-2` (a locally-true claim restated one layer up, losing its scope). The
claim here never lost scope; it was simply not retrieved when a different question needed
it. Re-reading more carefully would not have helped — I had already read it carefully. What
caught it was running the test and seeing `citations: 1`.

**Workaround:** None available at authoring time; the correction came from execution. The
generalisable form: when a new consumer reads an existing data structure, re-read that
structure's *producer* for invariants, even when — especially when — you have already read
it this session for another reason. A shared `Vec` serving two consumers with opposite
needs is the seam.

**Severity:** med — the false rationale would have shipped in a doc comment, and the live
numbers (324/322) would have quietly contradicted it forever. It also concealed a real
defect for a while: the same dedup means a passing mention above an entry consumes the
citation, now filed and measured at 1461 shadowed across 139 ledgers
(`docs/issues/2026-08-21-entry-attribution-follows-the-first-mention-only.md`).

**Status:** fixed-verified — doc comment corrected and the real collapse pinned by
`entry_edges_reports_citation_grain_and_edge_grain_separately`; the underlying limitation
pinned separately and filed (`1b19e0db`).

**Valid:** dated 2026-08-21

**Rests on:** the principle that execution is a different evidence channel from reading —
this project's CLAUDE.md § Conclude Last, clause 5: *"a belief you already hold is exactly
the thing your re-reading cannot audit."*

**Fix idea / Pointer:** distinct enough from `F-2` to stay its own entry; if a third
instance of "read it, didn't retrieve it" appears, the pair is worth promoting together as
two failure modes of inherited facts.

## W-7 — Measuring before implementing killed a false worry and moved the chosen number

**Observed:** 2026-08-21, choosing the slug base cap before the 4105-row backfill.

**Pattern:** When a parameter is about to be baked into immutable data, simulate the real
algorithm over the real corpus at several candidate values *before* implementing — not to
confirm the choice, but to find out what the choice is about.

**Counterfactual:** Two independent errors, in opposite directions, both of which would
have shipped and neither of which a review would have caught (both are judgement calls that
read as reasonable).

1. **A worry that was false.** I told the user truncation collisions would be "absorbed by
   the existing dedup" — unmeasured — and privately expected `foo-2 … foo-47` chains. The
   measurement: **max collision depth is 10 at every cap, including no cap at all**, because
   the worst chain comes from ten artifacts whose titles all slugify to `skill`, a 5-char
   string no cap can touch. Truncation adds *zero* depth. The objection could not occur.
2. **A number that was wrong.** I had recommended ~40 chars from intuition. The table shows
   40 nearly triples the suffixed count (269 vs a 115 baseline) to buy ten characters off a
   tail that 50 already bounds at 52. The knee is at 50; I would have guessed past it.

**Confirming data points:**

1. The measurement table (none/60/50/40/30) is preserved in `SLUG_BASE_MAX`'s doc comment,
   so the next person to question the constant reads the evidence rather than re-deriving it.
2. Two further defects fell out of *writing* the code the measurement specified: the
   exact-boundary bug (a cut landing on a separator trimmed one word too many), and mutation
   M-F surviving because the stub test reached its answer through the `None` arm and never
   exercised the guard it claimed to.
3. The shipped result matched the predicted shape but not the predicted magnitude: 216 rows
   suffixed against the probe's 173, because the probe simulated dedup over the unslugged
   rows in isolation while the real mint also collides with pre-existing slugs. A floor, and
   the shipped number sits just above it — which is the correct relationship between a
   simulation and reality, and would have been a 43-row error had I published 173 as a fact.

**Impact:** high — slugs are immutable and `entry_cite.src_slug` FKs them, so the
derivation was a one-way door for 4105 rows.

**Promote-when:** paired with `W-3`'s mutation discipline, this is the same underlying law
at a different phase — *apply the instrument, do not reason about what it would say.* If a
third work stream produces a pre-implementation measurement that inverts a stated
expectation, promote the pair as one rule covering both design-time and test-time.

**Status:** validated

**Valid:** dated 2026-08-21

**Rests on:** this project's CLAUDE.md § Measurement — *"never state a count your instrument
did not measure"* — extended one step earlier, to the parameter chosen before any
instrument runs.

## Template for new entries

<!-- New F-N / W-N entries are inserted above this line. This file declares
     entry_prefix: [F, W], so it is a guarded ledger — append via
     artifact(action="append_entry", id_prefix="W", anchor_heading="## Template for new entries",
              title=..., body=...)
     which allocates the id and writes the `## <ID> — <title>` heading itself.
     Also add the matching Index / Wins Index row. -->

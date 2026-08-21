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
entry_high_water_W: 10
entry_high_water_F: 9
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
| F-7 | 2026-08-21 | high | architectural | open | A capability premise about our OWN internals reads as recall, not assertion — so nothing audits it |
| F-8 | 2026-08-21 | high | architectural | fixed-verified | A correction lands where the error was found, not where it propagates — two retired terms left the tap's firing rule standing |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-3 | 2026-08-20 | high | Apply mutations, never reason about them | Nine regression-guard holes ship, each one refactor from vanishing, with a green suite | promoted-to-permanent-docs |
| W-4 | 2026-08-20 | high | Require every implementer to report where the brief was wrong | A brief's bad advice loosens a date parser while appearing to tighten it | validated |
| W-6 | 2026-08-21 | high | Scout the seam before consuming machinery a spec describes | Silent edge loss on prune, plus a materializer counting calls as rows | validated |
| W-7 | 2026-08-21 | high | Measure the parameter before baking it into immutable data | A false collision worry shipped, and the cap set 10 chars past the knee | validated |
| W-8 | 2026-08-21 | high | Reproduce before reading the fix plan — the plan may name the wrong mechanism | The expensive fix ships, takes an exposure recalibration, and the intra-ledger graph is still empty | validated |
| W-9 | 2026-08-21 | high | Copy ALL of a sibling's mechanisms, and re-derive its constants against the new corpus | A 30-line cap transplanted onto wrapped ledger prose leaves 1074 of 1598 sections untouched, behind a green suite | validated |
| W-10 | 2026-08-21 | high | Test the caveat you wrote into your own conclusion — a limitation you can price is a task | Layer 5a sits open forever on "revisit when the era changes", watching a trigger that cannot fire | validated |
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
(`docs/issues/archive/2026-08-21-entry-attribution-follows-the-first-mention-only.md`,
fixed same-day in `383b394e`).

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

## W-8 — running the reproduction found a SECOND mechanism, and the filed fix alone would have recovered zero

**Valid:** invariant

**Observed:** Re-entering my own bug file from the previous session
(`entry-attribution-follows-the-first-mention-only`) to implement it. CLAUDE.md requires
running the reproduction *before* reading the fix plan, on the grounds that the plan is a
hypothesis about the reproduction. Promoted at four datapoints; this is the fifth, and it
failed in a way the previous four did not.

**Got:** The reproduction did not refine the fix — it found a **second, independent
mechanism** the bug file never mentions. `resolve` decides self-citation at FILE grain and
returns `Outcome::SelfCite`, which is matched before the `Edge` arm where
`entry_section_at` lives. So for any citation whose target is defined in the same file, the
citation never reaches attribution at all.

Consequence, and this is the part reading the plan could not have produced: **fixing the
filed bug alone recovers zero intra-ledger edges.** Every one of them is short-circuited
one step earlier. A ledger's `**Kin:**`, `**Chain.**` and `kin R-3/R-28` lines — its
densest, most deliberate edges, written by hand to assert a relationship between two
entries — were 100% dropped, and would have stayed 100% dropped behind a passing test and a
plausible changelog line.

**Counterfactual:** I would have implemented option 1 of the filed plan (emit every
occurrence, move the file-level guarantee out of `extract`), taken the exposure-metric
recalibration that option carries, re-measured, and found the intra-ledger graph still
empty — with the expensive change already shipped and the actual blocker still unlooked-at.

**Why the plan could not have contained it.** The bug file's *Impact* caveat enumerates the
outcomes that stop a shadowed citation becoming an edge: "`Ambiguous`, `Dangling` or
`CrossRepo`". `SelfCite` is absent, and `SelfCite` dominated the file's own top-offenders
table — every ledger listed there is shadowed largely by ids it defines itself. The
rationale was internally coherent and cited a real measurement; the missing item is
invisible from inside it, because a list you wrote reads as complete.

That is [[reconnaissance-patterns:R-95]]'s second form exactly — *"Pick one of two, both
bad" had a third option inside a type one of them already touched* — so this is a
recurrence, not a new law. Recorded here rather than as a new R-N per the skill's own audit
rule: a recurrence is a defect in the promoted text's reach, not a new entry.

**And the omission had a price in the other direction too.** It made a cheap fix look
expensive. The second mechanism was confined to `link_scan`, touched no `extract` behaviour,
and therefore could not move the exposure metric that three shipped `doctor` checks are
gated on — the very risk that justified deferring the whole thing. Shipped same session as
`b750419a`: +68 intra-ledger edges across 17 ledgers, a 21% larger entry graph, with
`self_cites` unchanged at 867 proving the file-grain verdict never moved.

**Status:** validated

**Promote-when:** a third instance where reproduction-before-plan surfaces a mechanism the
plan does not name (as opposed to correcting a detail within the named one). At that point
the CLAUDE.md rule's wording should widen from "the plan is a hypothesis about the
reproduction" to name the enumeration failure specifically — the plan may be a hypothesis
about the *wrong mechanism*, and its own completeness is the least testable thing in it.

## W-9 — a copy inherits the sibling's NUMBER but not its UNIT — three mechanisms taken one at a time

**Valid:** invariant

**Rests on:** `statement-validity-session-log:W-3` — a copy inherits the sibling's discipline
but not its tests, because those tests are named for the sibling. This entry is its
measurement twin: a copy also inherits the sibling's *constants* without their *assumptions*.

**Observed:** 2026-08-21, Layer 4's entry-grain context packer. The file-grain packer beside
it has **three** mechanisms that jointly make its budget work:

1. an anchor reserve (half the budget, so a long anchor cannot starve neighbours);
2. a **30-line preview** on every node;
3. a value-ordering (`tracker → augmented → plain`).

**Got:** I copied **one** — the anchor reserve, with its comment and its bug-file citation
intact — and shipped. Each omission surfaced separately, and each was invisible to the
question *"is this consistent with its sibling?"*, which answers **yes**:

- the missing preview meant neighbours packed at full section length; **26% of anchors were
  short-served**, and `reconnaissance-patterns:R-3` served 3 of 25;
- the missing ordering meant selection was lexicographic, so the two neighbours that survived
  came from one ledger and the breadth that made R-3 load-bearing was invisible;
- the reserve I *did* copy was itself untested here — disabling it left all 19 context tests
  green (`W-3`'s exact shape, third instance this session).

**The part worth keeping is what happened when I went to fix it.** The obvious repair is to
copy mechanism 2 — the 30-line preview. Measured first, and it is **wrong on this corpus**:
ledger entries run **40–200+ bytes per line** against artifact bodies' ordinary prose, so a
30-line cap leaves **1074 of 1598 sections completely untouched**. One neighbour,
`reconnaissance-patterns-archived-entries:R-77`, is **ten lines and 2810 bytes** — a line cap
cannot see it at all.

The sibling's `30` is correct *for lines of prose*. Transplanted to wrapped ledger entries it
measures nothing. The cap shipped in **bytes** (1000), chosen by a sweep and validated by
reading an actual excerpt. Full data: `context-performance:CTX-1`.

**Counterfactual:** copying the preview verbatim would have produced a green suite, a
plausible commit message citing the sibling, and a fix that improved fit for a third of the
corpus while leaving two-thirds untouched — and nothing would have prompted anyone to check,
because the number came from working code.

**Status:** validated

**Promote-when:** a third instance of a transplanted constant whose unit did not transfer
(this is the second — the first was the `dated` horizon copied from `FRESHNESS_HORIZON_DEFAULT`,
which `doctor.rs` documents as *"deliberately not the deleted"* one for the same reason). At
three, add a line to `W-3`'s promoted text in `~/.claude/CLAUDE.md`: when copying a sibling's
discipline, copy **all** its mechanisms, and re-derive every constant against the new corpus
rather than the old one.

## F-7 — A capability premise about our OWN internals reads as recall, not assertion — so nothing audits it

**Valid:** invariant

**Rests on:** `statement-validity-session-log:F-4` — a surface was kept rigorously current
while the mechanism that *serves* it went unchecked. Same shape one layer down; here the
rigour and the blind spot sit in the same paragraph.

**Observed:** 2026-08-21, resuming into Layer 5a. §6 of the design spec states, as the
reason 5a is cheap: *"the server created the handle and knows which artifact and which call
produced it."* Everything downstream leans on it — 5a is sequenced as the cheap unblocker,
and 5b's `max(reads, in-degree)` exposure term consumes what 5a produces.

**Got:** the server does not know. `OutputBuffer::store_tool`
(`src/tools/output_buffer.rs:303`) writes `command = <tool name>` and `source_path: None`,
so an `@tool_*` minted by `artifact(get)` records only that *some* call to `artifact` made
it. `store_file` (`src/tools/output_buffer.rs:279`) then drops provenance again on every
buffer-to-buffer hop, and does so **deliberately** — a `@`-prefixed path would be `stat`-ed
by `get_with_refresh_flag` and evict the entry on first read. Two function bodies falsified
the premise. Neither had been opened.

**Why it survived: the claim is about our own internals.** A claim about an external API
gets checked, because nobody trusts their memory of someone else's contract. *"The server
knows X"* is a claim about code the author owns, so it gets **recalled instead of looked
up** — and it arrives in the grammar of an implementation detail rather than of an
assumption. This is `reconnaissance-patterns:R-95` with the sign inverted: R-95's rationales
run *inflated* because they justify stopping, and nobody drafts an estimate that makes the
work sound easier at the moment they decide to stop. This one runs *deflated*, because it
justifies building.

**The self-reference is the sharpest part.** That same section applies CLAUDE.md's
Measurement clause 3 explicitly and correctly — to someone else's argument: *"An earlier
draft of this spec concluded from it that read-count was unusable. That conclusion applied
clause 3 … to someone else's argument and not to its own: a zero is evidence about the
search."* The discipline was in the paragraph. It was facing outward.

**Counterfactual:** cheap to catch, had anyone opened the file — an implementer would have
gone looking for a field that does not exist and found out in a minute. Expensive as
*sequencing*: 5a sits where it sits **because** it was believed free, so both 5b's schedule
and the `max()` exposure term rest on a field that was never built. The correction now in
§6 records two routes, and the one that looks likely to survive — reconstructing
attribution from `usage.db`'s `input_json` / `output_json`, measured at 267 handle-minting
`artifact` calls and 1097 handle reads on this project — is a different implementation, in
a different module, from the one the premise implied.

**Status:** open

**Promote-when:** a second instance of a **capability** premise about codescout's own
internals surviving into a sequencing decision unread. At two, the reconnaissance skill's
*"a proposed fix is a claim about CURRENT STATE"* bullet should widen to name this sibling:
a claim that our own system **already knows** something is the same class of unverified
assertion, and is less audited than the others precisely because it sounds like recall.

## W-10 — A caveat you write into your own conclusion is an untested hypothesis — and testing it retired the layer instead of deferring it

**Valid:** invariant

**Rests on:** `reconnaissance-patterns:R-95` — a deferral rationale is a claim, and the
least-audited kind. This is its self-authored form, and `statement-validity-session-log:R-108`'s
timeline twin: there the aggregate was silent about the tail of a *distribution*, here about
the tail of a *timeline*.

**Observed:** 2026-08-21, deciding Layer 5a. I measured the read leak all-time, concluded
*descope*, and then wrote my own caveat into the conclusion: *"this is a historical count over
an era in which entry-grain reading was not yet the normal access pattern — a small number here
is evidence about the era, not the mechanism."* Then I stopped, and offered the conclusion with
the hedge attached. The user did not accept the hedge: *"this measurement is also tricky since we
added a lot of new work around trackers so the tool distribution will change slowly. lets look at
the last 24-30h."*

**Got:** four queries. Re-cut into a 30-hour window, the 30 hours before it, and everything older
(≈16 days):

- **The leak number did not move.** Leaked entry-grain reads per window: **4 / 4 / 30** — about
  four per 30 hours in *every* era. So the hedge was wrong about the conclusion; the descope never
  rested on an era-contaminated total.
- **The distribution genuinely is shifting, and in the other direction.** `append_entry` as a share
  of `artifact` calls went 3.8% → 7.3% → **9.2%**, while `get(heading=…)` went 9.2% → 12.3% →
  **6.7%**. Tracker work grew on the *writing* side. The user's premise was right and its
  consequence was the opposite of the one it was raised to support.
- **And the era cut surfaced four rows the aggregate could never have shown.** Every
  `librarian(context, anchor_id=…)` call ever recorded is five rows; four are in the last 30 hours,
  all `reconnaissance-patterns:R-3`, from two sessions — Layer 4's own smoke tests, by its author.
  Four calls out of **33,691**. No all-time statistic surfaces a four-row population.

**Why that fourth bullet is the whole entry.** Those four rows say organic entry-grain reading has
not started — but also that the path it will arrive on names the entry **in the call input**
(`<slug>:<local>`), needing no buffer provenance, no nearest-preceding-heading attribution, no
`usage.db` join. 5a exists on the premise that entry-grain reads arrive through leaky paths.
Layer 4 built a non-leaky one and made it the ergonomic one. The layer is **retired**, not
deferred — a strictly stronger verdict than the one the objection was challenging.

**Why the hedge survived: it is self-certifying.** Writing *"this may be measuring the era"* feels
like rigour — it is the sentence a careful person writes — so it discharges the discomfort that
would otherwise prompt the check. An inherited deferral at least looks like someone else's
unexamined claim. A caveat in your own conclusion looks like evidence you examined it. The tell is
grammatical: a caveat that names a **cheap, specific** alternative cut (`by era`, `by session`,
`by path`) and does not run it is a to-do wearing the costume of a limitation.

**Counterfactual:** the hedge would have shipped as a live deferral with a false rationale —
"revisit when the era changes" — and the era-change trigger would never have fired, because the
thing that changed was `append_entry`, which the trigger does not watch. 5a would have sat open
indefinitely on a premise that Layer 4 had already invalidated the day before.

**Status:** validated

**Promote-when:** a second instance of a self-authored caveat that was cheap to test and changed
the verdict when tested. At two, add to CLAUDE.md's Measurement rule a fifth clause: *before
publishing a number with a caveat, ask whether the caveat names a cut you could run in the next
five minutes — if it does, run it; a limitation you can price is a task, not a limitation.*

## F-8 — A correction lands where the error was found, not where it propagates — two retired terms left the tap's firing rule standing

**Valid:** invariant

**Rests on:** `statement-validity-session-log:F-4` — gated doc surfaces were kept current while
the routing that serves them went unchecked. Same mechanism inside one document: the correction
was written, correctly, at the site of the error, and never walked to the site that consumes it.

**Observed:** 2026-08-21, scouting Layer 5b before building it. Its tap fires on
`exposure >= 5`, where `exposure = max(reads, rests-on in-degree)`.

**Got:** both terms are empty, and each was emptied by a decision recorded elsewhere in the
same document.

- **`reads`** has no counter — no table, no column, nothing increments. Layer 5a, which was to
  supply it, was retired earlier the same day at ~4 recoverable reads per 30 hours.
- **`rests-on in-degree` is zero rows, not merely unimplemented.** `SELECT rel, COUNT(*) FROM
  entry_cite GROUP BY rel` returns `cites` and nothing else, for both origins; `artifact_link`
  carries eight rel values and `rests-on` is not among them. Layer 3c, its source, was
  cancelled by measurement — one resolvable declaration corpus-wide.

**The document already knew, in the wrong place.** The shipped-state note under Layer 2 says
plainly: *"`max(reads, in-degree)` is not implemented — there is one term, and it degrades by
being smaller, not by breaking."* That note is correct, and it is ~200 lines away, under a
different heading, describing **shipped** code. The tap section is what an implementer opens to
build 5b. Neither retirement — 3c's, from the day before, nor 5a's, from four hours earlier —
walked forward to the consumer.

**Why this shape is worse than a plain stale claim.** A correction *feels* discharged the moment
it is written: the author has confronted the error, recorded it honestly, and moved on. That
felt-completeness is what stops the second step. And the surviving text is not obviously stale —
`max(reads, rests-on in-degree)` reads as a considered design, so it recruits no suspicion. Two
independent cancellations, ten days of design work between them, and the formula they both empty
still sits in the section that would have been implemented next.

**The scout also found the fix.** The term with data is `cites` in-degree, and the real decision
is which **grain**: `entry_indegree` (shipped, gates three `doctor` checks, recomputed from files,
never reads `entry_cite`) versus `entry_cite` in-degree (1539 rows, 654 entry destinations).
They rank differently and are not interchangeable. Measured among cited destinations: the `F`/`W`
family holds 96 tokens and 442 edges, of which **at least 33 tokens carrying at least 339 edges
(77%) have more than one definer** — the condition `entry_indegree` drops on — against 7 tokens
and 63 of 1056 edges (6%) for every other prefix. Choosing the shipped term would silently exempt
most of the session-log corpus from ever arming the tap. A floor, not a count: definers were
counted only among *cited* slugs.

**Then measured directly rather than left as an inference.** `librarian(action="doctor")`,
same day: all **32** rows of `entry_cited_from_outside_but_undeclared` are present in the
response — a census, not a truncated floor — and **zero** name an `F`/`W` token; every one
names another prefix (`TU-7`, `B-1`, `CAP-5`, `H-2`, `H-5`). So 252 `F`/`W` Statements
carrying 442 entry-grain edges are **already** invisible to all three exposure-gated checks.
The exemption is shipped behaviour that 5b would inherit, not a risk 5b would introduce —
which is exactly the distinction the write-up would have blurred had I stopped at the
definer-multiplicity proxy. Naming what the predicate counts said `>1 definer`; the question
asked `does it reach the worklist`.

**And a third stale deferral, found in passing.** `entry_indegree`'s doc comment names its own
fix — count a stem-qualified citation against its specific definer instead of folding it into the
bare token — and declines it because *"it needs the `Corpus`/`by_stem` machinery `link_scan`
builds, which this function does not have."* Layer 3b built exactly that machinery, and
`entry_cite` holds its resolved output. `reconnaissance-patterns:R-95` again: a deferral
rationale is a claim about current state, and this one expired the day 3b shipped.

**Counterfactual:** 5b implemented against the written formula produces a tap that never fires,
on a corpus where nothing would look wrong — no error, no empty table, no failing test. The
`max()` returns 0 for every Statement, `obligation_state` stays `none` forever, and the health
metric the design names for exactly this failure (*"if `verifies` climbs while `refuted` stays at
zero, the appraisals are theatre"*) reads clean, because neither counter ever moves.

**Fixed 2026-08-21, and fixing it produced two unit errors of my own.** `entry_indegree`
is now keyed `(defining file, token)`; `F-9` appears on the worklist where no `F`/`W`
Statement could before. But the write-up above said **96** Statements where a Statement is
a *(ledger, token)* pair and the count is **252** — 96 is the number of distinct token
*strings*. And it implied 442 edges of suppressed exposure, where the gate reads distinct
citing **files**: across all 252 destinations `entry_cite`'s maximum distinct-citer count
is 4, none reach the threshold of 5, and the worklist moved by exactly **1**. Both are
`statement-validity-session-log:W-9` — a number measured in one unit and consumed in
another — committed inside the entry that cites W-9 as kin. The lesson survives the
correction: the exemption was real and is gone; its *magnitude* was never what I said.

**Status:** fixed-verified

**Promote-when:** a second instance of a correction that was written at the error site and not
propagated to a consumer **inside the same artifact**. At two, add to the reconnaissance skill's
Phase 3: when you correct a claim, grep the artifact for every other statement that rests on the
same premise before you close — the document is the blast radius, not the paragraph.

## F-9 — server_instructions has no room for the measured imperative, and the documented escape hatch does not apply

**Valid:** dated 2026-08-21

**Observed:** The attestation-register evals (prompt-engineering, rounds 3-5, n=10 per
arm) specify a two-part fix: the verification FACT in the payload, and an unconditional
IMPERATIVE in a trusted push channel. The payload half shipped in `7fd7a7cc` (patch-id
`50b5d487`). The imperative half does not fit, and the gap is not close.

**Got:** `build_server_instructions(None)` renders at **1687 chars** against a
`STATIC_SLICE_CHAR_BUDGET` of 1900, which reads like 213 chars of headroom. It is not.
The binding constraint is `CLIENT_INSTRUCTIONS_CHAR_LIMIT` (2048) minus
`CHANNEL_SAFETY_MARGIN` (48), and whatever the static slice does not use is the budget
`fit_dynamic_block` has for the whole `## Project Status` block. Adding **101 bytes**
collapsed it: three tests lost the semantic-index line, the onboarding hint and the
worktree banner, because the `Anchor`-priority segments alone then overflowed and the
fallback line-cut kept only the first.

Measured, not reasoned — each figure is a test run:

| static slice | dynamic budget | result |
|---|---|---|
| 1687 chars (today) | 313 | all 67 prompt tests green |
| +101 bytes (one sentence, one heading) | ~212 | 3 substantive failures |
| +155 bytes (two short sentences) | ~158 | 7 substantive failures |
| +200 bytes (the measured b2-shaped text) | ~113 | 8 substantive failures |

**Why the documented escape hatch does not apply.** `STATIC_SLICE_CHAR_BUDGET`'s own doc
comment says: *"If you need to add content, author a `get_guide(topic)` entry and
reference it from the slice — do not raise this number."* That is right for reference
material and wrong for this. `get_guide` is a PULL channel, and the finding being applied
is specifically that the imperative has to be PUSHED into a channel the model trusts —
round five measured the server-instructions position at 0/10 quarantined and 6/10 cited
as authority, which is the property a guide reference does not have. Moving it to a guide
would satisfy the budget rule by deleting the mechanism.

**Also observed, and the reason to distrust hand edits here.** Two `edit_markdown` calls
on `source.md` landed content in the WRONG SURFACE: `insert_after heading="## Deeper
guidance"` placed the section past `<!-- @end -->` and inside `onboarding_prompt`, where
it would have shipped to nobody as server instructions while polluting the onboarding
prompt. A later edit silently deleted one blank line from `onboarding_prompt` — caught
only because `prompt_surfaces_onboarding_snapshot` is byte-exact (19122 -> 19121). The
section-replace guard that refuses to drop surface markers is what caught the first;
nothing but the snapshot caught the second.

**What is NOT owed.** The presence win — 0-4/10 to 10/10 — comes entirely from the
payload half and is shipped. The imperative buys PROMINENCE (median first-flag offset
0.775 -> 0.614 in the server channel, exact permutation p=0.0045 vs the no-imperative
arm). That is a real effect with an intermediate point estimate and n=10, and trading the
Project Status block for it is a bad trade: that block is what tells a session which
project is active, which memories exist, and whether the index is built.

**Options, none taken:**

1. Ship nothing further. The measured big win is already in.
2. Cut existing slice content to make room. Every line is load-bearing by its own
   history — the `get_guide` pointer list is what
   `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md`
   was about, and the Iron Laws are the gate text.
3. Put the imperative in `build_system_prompt_draft()` instead — a third surface, written
   at onboarding into `.codescout/`, surfaced back as `## Custom Instructions`. It is
   user-authored after onboarding, so it is plausibly trusted, but that is an ASSUMPTION:
   the evals measured CLAUDE.md and MCP server instructions, not this.
4. Re-measure `CLIENT_INSTRUCTIONS_CHAR_LIMIT`. It was measured once, on one client build,
   on 2026-08-16. If the real cliff has moved the arithmetic changes — but per R-89 that
   is a probe to run, not a hope to budget against.
5. **Move `## Project Status` off the `instructions` channel entirely.** This is not a new
   idea and this entry should not have presented four options as if it were the first look
   at the problem — bug `e3437bd1ec116dec` (*mitigated*, not fixed) has proposed exactly
   this since 2026-08-17, under the heading *"Still proposed, still the maintainer's
   call"*, with the reasoning that *"putting unbounded content in a fixed channel remains
   the design error."* It is the only option that dissolves the constraint rather than
   rationing it, and it would free the whole 313-char budget.

**A SCOUTING MISS, recorded because the process failure is the reusable part.** That bug
file is this constraint's canonical account: it carries the 2048 measurement's provenance
(BL-9), the `Substitutable`/`UserAuthored`/`Anchor` tier design, the standing reproduction,
and option 5. It was found only AFTER the work, by reading `git log` for an unrelated
reason. The activation bootstrap guide prescribes
`artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})`
before starting — but this bug is `mitigated`, a TERMINAL-looking status that the
prescribed query filters out by construction, while the constraint it documents is fully
live. A mitigated bug is precisely one whose root cause is still there.

So the query to run before touching a surface with a known hard limit is the one that does
not assume the limit was ever fixed:

```
artifact(action="find", kind="bug", query="<the surface>", include_archived=true)
```

**Options 3 and 4 are closed by measurement, 2026-08-21.**

**Option 4 — the limit is real and exactly where it was recorded.** Probed through a live
`claude -p` session using round five's stub MCP server, which can serve arbitrary
`instructions` down the same path codescout's travel. The instrument is a ruler of
contiguous `[[NNNN]]` markers, each exactly 8 characters, so every position lies inside a
marker and the model reports a token it can READ rather than counting anything. Three
runs: `[[2032]]`, `[[2040]]`, `[[2040]]`. `[[2040]]` occupies chars 2040-2047, so the last
complete marker ends at exactly **2048**. `CLIENT_INSTRUCTIONS_CHAR_LIMIT` needs no change.

**And the UNIT is confirmed too, which is the half that matters most here.** Two variants
of the same ruler: pure ASCII (chars == bytes) and a `wide` variant carrying an em-dash
every 10 chars (1.16x bytes per char). Both cut at the same CHARACTER offset. Had the
limit been in bytes, `wide` would have cut ~235 chars earlier. This constant was wrong
about precisely this unit once before — 2200 compared against `String::len()`, green
throughout while the surface shipped truncated — so a probe that could not separate chars
from bytes would have re-confirmed the old error just as confidently.

**Option 3 was never an independent channel.** `build_system_prompt_draft()`'s output is
surfaced as `## Custom Instructions` by `build_project_status_segments`
(`src/prompts/mod.rs:225-229`) at `StatusPriority::UserAuthored` — a SEGMENT of the same
2048-char channel, and a droppable one, since only `Anchor` is never dropped. An imperative
placed there would sit in the same budget it is trying to escape and be among the first
things trimmed. Strictly worse than the static slice.

**A probe artefact, recorded because it nearly became a published claim.** The first ruler
used 50-char strides with runs of dots between markers, and the model's reported tail
length (53 chars) exceeded what the ruler holds between two markers (42). I dismissed the
tail as unreliable — a model cannot copy a long run of identical characters — which was
the right instinct for the wrong reason: 53 is exactly 40 dots (2008->2048) plus a 13-char
`… [truncated]` string, present in the raw output of 4 of 4 runs at exactly the cut point.
I first reported that marker as client-appended without having established it, then ran the
control that settles it — serve instructions FAR under the cap and see whether it still
appears:

| instructions served | last marker reported | `… [truncated]` present |
|---|---|---|
| 400 chars (under cap) | `[[0392]]`, the true end | **no**, 0 of 2 runs |
| 3000+ chars (over cap) | `[[2040]]`, the cut | **yes**, 4 of 4 runs |

Presence tracks over-cap content exactly, so the marker rides in the delivered
instructions rather than being a model artefact. The one dissenting run — asked for "the
final 40 characters", it returned 40 clean ruler chars — is consistent: it reported the
final 40 of the RULER, which is what the question named. The 2048 figure does not rest on
any of this; the contiguous ruler was built to avoid the question, and it stands alone.

**This falsifies a premise BL-9's design rests on, and it is worth someone re-reading.**
Both `fit_dynamic_block`'s doc comment and bug `e3437bd1ec116dec` justify producer-side
trimming with *"the client cuts from the tail at a fixed char count, mid-token, and says
nothing — so anything not fitted here vanishes silently"*. On this client build it does
not say nothing. Producer-side trimming is still the better mechanism — it chooses WHICH
segment to lose and names it, where the client just cuts the tail — so nothing here argues
for removing it. But one of its two stated justifications no longer holds, and a design
comment that reads as false is how the next person mis-costs this trade.

**Status:** open

**Promote-when:** a decision is taken between options 1-4, or a fourth surface with push
semantics and free space appears.

## Template for new entries

<!-- New F-N / W-N entries are inserted above this line. This file declares
     entry_prefix: [F, W], so it is a guarded ledger — append via
     artifact(action="append_entry", id_prefix="W", anchor_heading="## Template for new entries",
              title=..., body=...)
     which allocates the id and writes the `## <ID> — <title>` heading itself.
     Also add the matching Index / Wins Index row. -->

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
entry_high_water_W: 4
entry_high_water_F: 3
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
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-3 | 2026-08-20 | high | Apply mutations, never reason about them | Nine regression-guard holes ship, each one refactor from vanishing, with a green suite | validated |
| W-4 | 2026-08-20 | high | Require every implementer to report where the brief was wrong | A brief's bad advice loosens a date parser while appearing to tighten it | validated |

> Ids start at `F-2` / `W-3`, not `F-1` / `W-1`: the index rows were pre-filled before the
> sections existed and the allocator counted them as claimed. See `F-3`.
---

## Promotion status

**Audited:** 2026-08-20, against the target surfaces themselves — opened and read, not
recalled.

- **W-3** — *already promoted, no action.* `~/.claude/CLAUDE.md` § *Subagent Dispatch*
  carries the **Mutation-apply discipline** clause (validated 2026-08-18, 6 datapoints,
  eduplanner-ui `calendar-insight-panel-session-log:W-4`, a different ledger's entry — qualified
  by file stem because a bare `W-4` here would resolve to this ledger's own). This entry adds
  9 datapoints from a second repo and a **new
  mechanism** the existing clause does not name — copied-from-sibling code. That mechanism
  is a promotion candidate; the base rule is not.
  **Per-profile check still owed:** this machine runs three profiles (`~/.claude`,
  `~/.claude-sdd`, `~/.claude-kat`), each with its own `CLAUDE.md`, and the clause was
  verified only in `~/.claude/`. Three files that should be byte-identical have an md5 —
  compare them before calling this promoted.
- **W-4** — *UNFIRED, carried forward.* One work stream, three instances. Promote at a
  second independent work stream.
- **F-2** — fix idea is bound for the same surface as `W-4`; not yet a promotion.
- **F-3** — resolved in place; the open question is only whether
  `docs/templates/session-log.md` should carry the warning.
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

**Promote-when:** the base rule is already promoted (see *Promotion status*). The
**copied-from-sibling mechanism** promotes at one more independent work stream, or
immediately if a reviewer cites it as the reason they went looking.

**Status:** validated

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
  `catalog::artifact::upsert`, in scope in `doctor.rs`'s own test module at `:3197`, alongside
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
`doctor.rs`), and the controller restated it one layer up as an absolute, where it was false.
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
   true of `doctor.rs`'s test module, where they had just been burned by a brief that
   hallucinated them. Restated in the fix round 1 brief as an absolute, it was false:
   `art_upsert` is imported at `doctor.rs:3197`. See `W-4`.
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
the scope into the sentence ("not defined in `doctor.rs`", not "does not exist"). The
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

## Template for new entries

<!-- New F-N / W-N entries are inserted above this line. This file declares
     entry_prefix: [F, W], so it is a guarded ledger — append via
     artifact(action="append_entry", id_prefix="W", anchor_heading="## Template for new entries",
              title=..., body=...)
     which allocates the id and writes the `## <ID> — <title>` heading itself.
     Also add the matching Index / Wins Index row. -->

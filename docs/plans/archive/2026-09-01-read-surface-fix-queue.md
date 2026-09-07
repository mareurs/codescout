---
id: d59b84d36225805a
kind: plan
status: done
title: Read-surface and shared-checkout fix queue
owners:
- marius
tags:
- queue
- read-surface
- shared-checkout
topic: fix queue
---

# Read-surface and shared-checkout fix queue

> **CLOSED 2026-09-07 — four of five items terminal, and the fifth is not plan work.**
>
> Items 1, 2 and 3 shipped. Item 5 closed 2026-09-07 on a re-derivation that put its population
> at **1, not 9**, with the correct action on that 1 being **none**.
>
> What remains is item 4, *"triage the open bugs"* — and that is the **verify-open cadence**
> CLAUDE.md already mandates as standing practice, not a task a plan can finish. It is reachable
> by the canonical query (§ 4 carries it) and needs no open plan to hold it. Item 4 already
> discharged its one bounded slice — the `high` subset, triaged 2026-09-06, which produced
> `docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md`. Keeping this plan open for
> the unbounded remainder would make the plan queue read as two code-bearing plans while holding
> one.
>
> **Shipped as:** `38ba4f49` + `7dbc7b43` (item 1 — the `create` and `update` seams, fixed
> separately, which is *mutate once per guarded SITE* paying for itself within one day),
> `a35a9c35` (item 2, which had a second site of its own for the same reason), `3bf2f5f5`
> (item 3). Item 5 shipped no code, by decision.
>
> **Read § Status's own corrections before quoting any number from this file.** Item 4's
> population moved 27 → 85 in five days while reading like a total, and the field named `count`
> is not the count — sum it with `hints.more_in_scope`, or page until that field is absent.

**Opened:** 2026-09-01 · **Owner:** session `codescout-b7` · **Branch:** `experiments`

Four items, ordered. The operator asked for all four fixed; this file is the sequence and the
running state, so a later session (or a compaction) can resume without re-deciding the order.

**Order rationale, stated once.** Items 1 and 2 are code fixes with obvious tests and small blast
radius — they go first because they are *completable*, and because item 1 is actively corrupting
files right now. Item 3 needs a design call I deliberately left open. Item 4 is a survey whose
output is mostly other sessions' work, so it is last and its deliverable is a report, not commits.

## Status

| # | item | kind | state |
|---|---|---|---|
| 1 | double-frontmatter corruption in `artifact(create)` **and `update`** | code fix | **done** — 2 sites guarded, 4 tests, 3 mutations measured |
| 2 | heading-miss discards the `Available headings` hint | code fix | **done** — 2 sites guarded, 3 tests, 3 mutations measured |
| 3 | hook refusal-text "what comes next" tail | design + shell | **done** — one emitted copy, 3 hooks, end-to-end probed |
| 4 | triage the open bugs (verify-open cadence) | survey | **partial** — the `high` subset was triaged 2026-09-06 (13 rows read, 4 archived, yielding `docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md`); the remainder queued |
| 5 | repair the 9 already-corrupted files | design + data | **closed 2026-09-07 — the population is 1, not 9, and the correct action on that 1 is NONE.** § 5 below carries the derivation and the three detector traps it cost |

**Item 5 had no row in this table until 2026-09-06.** It has existed as a body section since
the queue was opened, so this table listed four items while the queue held five — and a reader
taking the table as the contents would never have learned otherwise, because nothing
cross-checks a hand-maintained index against the sections beneath it.

**Item 4's population is no longer 27, and the number moved faster than the item did.** § 4
below says *"27 are open or investigating"* — true when written on 2026-09-01. Re-derived
2026-09-06 with the canonical triage query:

```
doc(action="find", kind="bug",
    filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

**85 rows — 79 `open`, 1 `investigating`, 4 `zombie`, 1 `taken`**, so the figure comparable to
the original *open-or-investigating* unit is **80**. It roughly tripled in five days while
reading like a total the whole time. Cite the query and the instant, never the bare number.

**Re-derived 2026-09-07T07:26:27Z at `e766233f`: still 85.** Net-unchanged over the day and
not static underneath — three framework bugs closed by `074b749e` and several filed by peers
the same morning. A stable total across two reads is not evidence the population is quiet.

**AND THE FIELD NAMED `count` IS NOT THE COUNT — read `hints.more_in_scope` or you are off by
the cap.** That call returned `count: 50` with `hints.more_in_scope: 35`. The summary line
renders as `50 matched:`, which reads as *50 matched the filter* and means *50 were returned*.
The cap IS signalled, correctly, in a different field — the omnibus fix
`docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md` (`e74382e8c3483370`,
`fixed`) put it there — so this is loose phrasing over a correct payload rather than a missing
signal, and it is not re-filed. It is recorded here because this is the paragraph a future triager reads
before running that exact query, and “cite the instant” does not protect you from citing the
wrong number at the right instant: **sum the two fields, or page until `more_in_scope` is
absent.**
## 1 — double-frontmatter corruption (`a1dd1e9b0ef2f999`, archived)

`artifact(action="create")` with a body copied from `docs/issues/_TEMPLATE.md` writes **two**
frontmatter blocks: the catalog's, then the template's. The inert second block holds the only copy
of `opened` / `closed` / `severity` / `owner` / `related` — fields the catalog block does not
carry — so every one of those values is invisible to every query.

**Why it is first.** The trigger is the project's *own documented workflow*: `_TEMPLATE.md` begins
with `---` and `get_guide("tracker-conventions")` says copy it. Silent at the moment of the write —
`create` returns `{id, abs_path, wrote_to}` with no warning and the file looks correct until you
count `---` lines. Hit live on 2026-09-01 by an author who had read the bug file the same session.

**Candidate remedy, NOT yet verified against the code:** `create` already parses the body enough
to know it starts with `---`; refuse with a `RecoverableError` naming the duplicated keys and
carrying the merge. Scout before building — this is a claim about current state.


### Outcome (2026-09-01)

**Reproduced first, and the reproduction changed the record.** A probe artifact created with a
template-shaped body confirmed the doubled block (4 `---` lines) and surfaced **two harms the bug
file did not have**:

- The probe passed `status: scratch` in the body block and no `status=` parameter.
  `artifact(action="get")` reported **`status: open`** — the catalog's default for `kind: bug`.
  So a key present in both blocks with *conflicting* values resolves silently to the catalog's,
  and the caller's value is on disk, readable by nothing.
- `preview.summary` came back as `"--- status: scratch opened: 2026-09-01 severity: low owner:
  probe ---"`. The inert block is served as the artifact's **summary**, so the first thing any
  agent reads about it is a mangled YAML fragment.

**Blast radius measured rather than estimated: 9 committed files already carry a doubled block** —
8 archived bug files and `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`. The
detector was positive-controlled in both directions: it fires on a known-bad file and stays silent
on a known-good merged one.

**Fix: `reject_body_leading_frontmatter` in `src/librarian/tools/create.rs`,** called beside its
sibling `reject_reserved_extra_keys`. Refuse-not-repair, which is that sibling's stated reasoning
(*"the caller has said the same thing twice through two channels, and the right correction … is a
question about what they meant"*) and is stronger here because merging would have to pick a winner
for a conflicting `status`.

The predicate is **`frontmatter::parse`'s own** `starts_with("---\n")`, deliberately reused rather
than re-derived: a guard recognising a different set of bodies than the reader does would add a
third grammar to a namespace that already has two (`IC-6`). The refusal names an **escape** — lead
with a blank line for a literal `---` rule — because refusing a token owes one.

**Two mutations, one per direction, measured not asserted:**

| mutation | result |
|---|---|
| delete the guard call in `call()` | 25 passed / 1 failed — kills the refusal test **only** |
| widen the predicate to `body.contains("---")` | 25 passed / 1 failed — kills the acceptance twin **only**, and the refusal test stays **green** |

That second row is the argument for the twin existing. The refusal test is an *existence* assertion
and is monotone under widening: a guard refusing every body whatsoever would satisfy it completely.
Only the acceptance test mutates the other way. Probe file restored byte-exactly after each run
(`diff -q` → identical), verified rather than assumed.

**Not done here, and deliberately: repairing the 9 existing files.** That is a data migration over
other sessions' archived artifacts, it needs its own decision about whether to merge or to leave
history alone, and doing it silently inside a guard commit would be the same conflation the guard
refuses. Recorded as item 5 below.

### Follow-up the same session — the fix was premature, and the bug file said so

`38ba4f49` guarded `create` alone. The bug file's own *Tests added* section had already named
**two** seams — *"a `create` and an `update` each refusing a frontmatter-leading body"* — and a
probe confirmed `update` reproduced it identically: 4 `---` lines, `status: scratch` inert below a
catalog block reading `draft`.

**Caught by re-reading the artifact rather than by any check.** Closing after one site would have
left a passing suite, a green gate, a commit message full of measurements, and half the defect —
with nothing anywhere to prompt a re-open. `R-49`'s law (*re-entering your own bug file counts as a
seam; authorship is no exemption*) applied to a file written by a peer three days earlier.

**The guard is now shared, not duplicated**, for the reason `action_selector_key`'s doc gives about
its own callers: two copies of one predicate drift into recognising different sets of bodies, and a
guard that fires on one write surface but not its sibling is *harder* to notice than one firing on
neither.

**Third mutation, at the second site:** deleting `update`'s call kills its refusal test only — 983
passed / 1 failed — with every `create` test green. That is the per-site law measured rather than
cited: `create`'s kill genuinely said nothing about `update`.

**And one exemption pinned deliberately:** `body_edits` is *not* guarded, because it splices at a
heading inside an existing document where a leading `---` is a horizontal rule, not a block at
position 0. `body_edits_may_splice_content_that_begins_with_dashes` exists so that choice is
distinguishable from a forgotten site — the *annotate-an-inert-fixture* direction of CLAUDE.md's
fixture law.
## 2 — heading miss discards the hint (`a8e40acb96ca5739`, archived)

`heading_miss_meta`'s absent arm drops `err.hint()` while its ambiguous sibling forwards it, so a
heading typo returns a bare `not found` when the resolver has *already built* the "Available
headings" list. `cluster/declared-not-wired` (IC-3). Continues this session's read-surface work
stream directly.


### Outcome (2026-09-01)

**A second site again, and again the bug file did not name it.** `heading_miss_meta`'s absent arm
was the filed defect; the plural `headings=[…]` branch builds its own `missing` list, never calls
that helper, and was dropping the hint too. Fixing the helper alone would have shipped half the
defect — the identical shape as item 1's `create`/`update` pair, hours apart, and the second time
today that *mutate once per guarded SITE* was the difference between a fix and a partial one.

The plural branch emits `headings_hint` **once**, not per member: the hint is derived from the
document rather than the query, so every missing member would carry a byte-identical string — and N
copies of one fact is precisely the envelope bloat this work stream exists to remove.

**The bug file's own `## Tests added` pre-wrote the trap, and it was right:** *"asserting only
`heading_hint` exists is monotone under widening — `unwrap_or_default()` returns `""`, which is
present and useless."* Both miss-tests therefore assert the hint **names the fixture's real
headings**, not that the key is present.

| mutation | result |
|---|---|
| singular arm drops the hint | 56 / 1 — kills the singular test **only** |
| plural branch drops the hint | 56 / 1 — kills the plural test **only** |
| emit the hint on SUCCESS too | 56 / 1 — kills `a_resolved_heading_carries_no_hint` **only** |

The third row is the twin. The first two are presence assertions and are monotone under widening, so
emitting the key unconditionally satisfies both completely; only the third mutates that way.

**Gate:** fmt clean, clippy 0 warnings, lean lane exit 0, default lane 4836 passed / 1 failed — the
known load-sensitive `run_exits_after_idle_timeout_with_no_connections`, filed at `f5957aaa` and
verified isolated twice today against a 10s budget.
## 3 — hook refusal-text tail

Implement `docs/plans/archive/2026-09-01-shared-checkout-commit-sequence-guide.md` (archived on
completion; superseded by the convention page). Each pre-commit hook
teaches its own rule at its own collision and stops, so the sequence is learned one collision at a
time — measured cost on 2026-09-01: nine cross-session messages and roughly two hours across two
sessions.

**Open design call, and it is the reason this is not first:** which of the four hooks owns the
shared text. Four copies drift and `link_scan` cannot see shell strings, so the likely shape is one
helper emitting a common tail with each hook prepending its own rule — but that is a decision, not
a transcription.


### Outcome (2026-09-02) — the design call, made

**One emitted copy, `scripts/commit-sequence-tail.txt`, read by all three refusing hooks**
(`pre-commit-foreign-index.sh`, `pre-commit-unreviewed-content.sh`, `pre-commit-ledger-counts.py`)
immediately before each refuses. The prose — six steps, each with the measurement it was paid for,
plus the refuted guide-injection route — lives once at
[`docs/conventions/shared-checkout-commit-sequence.md`](../conventions/shared-checkout-commit-sequence.md).

**That split is a summary and its source, not two copies** — the same shape as `CLAUDE.md`'s gate
sentence against `gate-ordering.md`. It answers the open question this queue recorded (*four copies
drift*) by having one, and the second concern (*`link_scan` cannot see shell strings*) by putting
the citable prose in markdown where the scanner does reach it.

**Probed end to end, not just unit-poked.** A throwaway repo, a seeded `session-stage-log`, and the
real `pre-commit-foreign-index.sh`: **exit 1**, and the refusal carries **both** its own rule and
the shared sequence — 57 lines against ~32 before. Python's emit path and bash's `dirname`
resolution were checked separately, including from a different cwd, since those are the parts that
can vary; the `cat` of a verified-readable file cannot.

**The probe's first run was a false pass, and it is the useful part.** I seeded the stage log with
`git rev-parse :peer.txt` — a *full* blob sha — while the hook derives its key from `git diff
--cached --raw`, which is **abbreviated**. The lookup missed, the hook exited **0**, and nothing
said why. A fixture that does not match the parser under test produces a green that means nothing,
and this one would have been reported as *"the hook does not fire"* rather than *"my fixture is
wrong"*. Re-derived the key the hook's own way and it refused immediately.

**Honest bound, carried in the doc rather than only here:** a refusal still fires *after* the work
it invalidates. This shortens recovery from N collisions to one; it does not move the teaching
before the work. Only a per-session worktree does that, which
`scripts/pre-commit-unreviewed-content.sh`'s own header already says.
## 4 — verify-open triage

CLAUDE.md's verify-open cadence: reconcile every `Status: open` entry older than 14 days against
current code and the bug archive. 27 are open or investigating; the oldest is 2026-07-18. Fixes
routinely ship under messages naming no tracker entry, so entries go zombie-open by default — a
2026-05-25 pass flipped 3 of 4.

**Deliverable is a report plus status flips, not fixes.** Most of these files belong to other
sessions, and per this repo's own rule an entry is not mine to re-describe.

## 5 — repair the 9 already-corrupted files

Surfaced by item 1's blast-radius measurement, not previously known. Eight archived bug files plus
`docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` carry a doubled block today; the
guard shipped in item 1 stops new ones but repairs none.

**The open question is not mechanical.** Merging each inert block into its catalog block is easy;
deciding whether to do it to *archived* artifacts belonging to other sessions is not, and this
repo's own rule is that an entry is not mine to re-describe. A conflicting key (the item-1 probe's
`status`) makes it worse: for those, merging picks a winner and picking is exactly what the guard
refuses to do on the caller's behalf.

Likely shape: a `librarian(action="doctor")` check reporting the population read-only, with an
opt-in `fix=` that merges only the **non-conflicting** keys and reports the rest for a human. That
matches how `doctor`'s other repairs are gated.

### Resolved 2026-09-07 — the population is 1, and the adjudication says leave it

**Run the reproduction before reading the fix plan.** Doing so here replaced both halves of
this item: the count and the remedy.

**The population is 1, not 9.** Re-derived at `e766233f` over `docs/**/*.md`, 966 files with
leading frontmatter. The named casualty
`docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` is **clean** — single block,
repaired at some point since 2026-09-01 and not recorded here. The one genuine survivor is
`docs/archive/old-trackers/bug-tracker.md`.

**And the correct action on it is nothing.** It is the hard case this section predicted —
conflicting on `status` (`archived` vs `active`), `id` (`null` vs `'0ed68e66d69ceec0'`),
`title`, `owners` and `tags` — but two facts settle it without picking a winner:

- **The catalog already reads the right block.** `doc(action="find")` returns it with
  `hidden_archived: 1`, i.e. `status: archived` from block 1. Nothing is operationally wrong;
  the inert block costs a reader a moment, not the tool an answer.
- **The file forbids the edit in its own words.** Its RETIRED banner reads *"Body preserved
  verbatim for `git blame` continuity. Do not append."* Block 2 IS that preserved body — the
  frontmatter the tracker carried while it was live. Merging it would alter a body kept
  deliberately verbatim, to fix a display nit, on a retired surface.

So the `doctor` check with an opt-in `fix=` proposed above is **not built, deliberately**: it
would be gated machinery for a population of one whose correct handling is to leave it alone.
If the population ever grows, this section has the shape ready.

### The detector cost three passes, and every trap is a class this repo already documents

Worth more than the result. Each pass returned a plausible number and none errored:

| pass | rule | answer | why it was wrong |
|---|---|---|---|
| 1 | a second `---` block after the first | **1** | `---` is also a markdown horizontal rule. The single "hit" was two `<hr>`s bracketing a closed-as-rediscovery note. |
| 2 | …whose block contains ≥2 frontmatter keys | **8** | 7 were *documentation examples* — a bug-tracker template, specs designing a frontmatter shape, and a bug file **about** duplicate frontmatter, which necessarily quotes one. |
| 3 | …and is not inside a code fence | **1** | correct |

Pass 1 is CLAUDE.md § *Parsers Over a Namespace* verbatim — *"`---` read as frontmatter
wherever it appears"* — reproduced by a detector written minutes after reading that section.
Pass 2 is the same class's other half: **no escape for MENTION**, and the file it false-flagged
hardest was the one whose subject is duplicate frontmatter. The fence is the escape, and
checking it is what pass 3 added.

**The control is what makes the final `1` a measurement.** A constructed positive and negative
were run through each pass; pass 3 reports 7 fenced examples alongside the 1 unfenced hit, so
the detector demonstrably still says *yes* to something. A bare `1` from a detector that had
silently stopped matching is indistinguishable from this one.
## Not in this queue, and why

**Promoting `experiments` to `master`.** Measured 2026-09-01: `experiments` is **2423 commits
ahead**, `master` **0** ahead, last moved 2026-07-05 — so it is a clean fast-forward. It is a
release action governed by `docs/RELEASE.md`, it is the operator's call, and the tree currently
carries a peer's uncommitted failing test. Recorded here so the number is not lost, not queued.

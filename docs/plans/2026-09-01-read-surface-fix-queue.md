---
id: '99eb60901aeec260'
kind: plan
status: active
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
| 2 | heading-miss discards the `Available headings` hint | code fix | queued |
| 3 | hook refusal-text "what comes next" tail | design + shell | queued — blocked on a design call |
| 4 | triage the 27 open bugs (verify-open cadence) | survey | queued |

## 1 — double-frontmatter corruption (`c202d8febd80ca8a`)

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
## 2 — heading miss discards the hint (`9b6ba9568f3c28b3`)

`heading_miss_meta`'s absent arm drops `err.hint()` while its ambiguous sibling forwards it, so a
heading typo returns a bare `not found` when the resolver has *already built* the "Available
headings" list. `cluster/declared-not-wired` (IC-3). Continues this session's read-surface work
stream directly.

## 3 — hook refusal-text tail

Implement `docs/plans/2026-09-01-shared-checkout-commit-sequence-guide.md`. Each pre-commit hook
teaches its own rule at its own collision and stops, so the sequence is learned one collision at a
time — measured cost on 2026-09-01: nine cross-session messages and roughly two hours across two
sessions.

**Open design call, and it is the reason this is not first:** which of the four hooks owns the
shared text. Four copies drift and `link_scan` cannot see shell strings, so the likely shape is one
helper emitting a common tail with each hook prepending its own rule — but that is a decision, not
a transcription.

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

## Not in this queue, and why

**Promoting `experiments` to `master`.** Measured 2026-09-01: `experiments` is **2423 commits
ahead**, `master` **0** ahead, last moved 2026-07-05 — so it is a clean fast-forward. It is a
release action governed by `docs/RELEASE.md`, it is the operator's call, and the tree currently
carries a peer's uncommitted failing test. Recorded here so the number is not lost, not queued.

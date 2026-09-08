---
id: ccb981128140f1c3
kind: bug
status: fixed
title: 'BUG: the archive-move confirmation signal is a rename LETTER, so a correct staged D+A matches neither documented outcome'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- git
- archive-flow
- doc-vs-reality
closed: 2026-09-08
opened: 2026-09-08
owner: marius
severity: low
---

## Summary

Both the `move` response's `stage_hint` and `get_guide("tracker-conventions")` tell an
archiver to *"confirm that `git status --short` shows a single `R` rename line"*, and name
exactly one failure shape beside it: *"a ` D` plus a `??` is half-staged."*

There is a **third** outcome, and it is correct: staged `D` + staged `A`, with no `R`. Git's
rename detection is **similarity-based**, so an archive move that also rewrites the body —
which is the normal case, since archiving means writing the outcome, the fix SHA and the
patch-id into the file — falls below the default 50% threshold and is reported as an
unrelated delete plus add.

The reader is then holding an outcome that matches neither documented state, with no way to
tell it from the broken one, because **the discriminator they were given is the letter and
the letter is what stops discriminating.**

## Symptom (Effect)

A correct, complete archive reads as unconfirmed. The two readings a follower of the
instruction can reach are *"`R`, good"* and *"no `R`, something is wrong"* — and the genuinely
half-staged case also has no `R`. Both non-`R` states look alike under the stated check.

The cost is not a corrupted archive; it is a reader who either re-stages something already
correct, or — worse — learns that "no `R`" is survivable and stops treating it as a signal at
all, which is the state in which the real half-staged case slips through.

## Reproduction — measured 2026-09-08

Archiving `2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md` at
`f7d61237`, having first written the outcome into the body (140 insertions, 94 deletions):

```
git status --short -- <old> <new>
D  docs/issues/2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md
A  docs/issues/archive/2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md
```

Both staged (column 1), no `R`. The commit landed both halves — `delete mode` + `create mode`,
2 files changed. The archive is correct.

Git's own view, with the threshold lowered:

```
git show --raw f7d61237                      → D + A          (default, 50%)
git show --raw --find-renames=25% f7d61237   → R044 <old> <new>
```

**Similarity 44%.** Six points under the default. Nothing is wrong with git; the check was
written against the case where a move does not touch the body.

## Root cause

The confirmation signal was chosen as a *letter* (`R`) when the property that actually
matters is *staged-ness*. `R` is one rendering of "both halves are staged", produced only
when git also happens to infer a rename — an inference that depends on how much the body
changed, which the archiver controls and the check does not mention.

## Where it is stated

- **Code:** `src/librarian/tools/mv.rs:251-256` — the `stage_hint` string, emitted on every
  move.
- **Guide:** `get_guide("tracker-conventions")` § *Bug files*, the "stage BOTH halves"
  paragraph.
- **Guide (found during the fix, NOT when this file was written):**
  `get_guide("librarian")` § *Archiving / Moving Trackers*,
  `src/prompts/guides/librarian.md`. See § *Fix* — this list was one short, and a fix
  verified against it would have looked complete.

All were written together and say the same thing, so a reader who cross-checks one against
another finds agreement. Three surfaces, one claim, no independent check.

## Why the existing test cannot catch it

`mv.rs`'s `move_names_the_staging_action_not_only_the_two_paths` asserts
`hint.contains("git add --")` — the **imperative**, deliberately, because its own comment
records that data presence did not produce the action across six consecutive opportunities.

That assertion is **monotone under this defect**: the hint could describe the confirmation
step in any way at all, or wrongly, and still contain `git add --`. The test guards the half
that was failing then and says nothing about the half failing now. This is CLAUDE.md's
predicate-vs-remedy split one level in: the test pins *what to do* and never *how you know it
worked*.

## Fix

State the discriminator as the two PROPERTIES that matter — staged-ness and content —
rather than as a letter, on both surfaces.

**What shipped is broader than what this section originally planned, and the reason is
worth recording.** The plan below fixed staged-ness only. Running the reproduction first
surfaced a sibling defect in the *same sentence*, failing the opposite way, already filed
as `docs/trackers/bug-fix-session-log.md` § F-111: a destination holding a **stale** copy
of its source is similar enough to pair, so a broken move renders `R` identically to a
good one — and one session cited that `R` as proof the destination carried fresh bytes,
twice in a day. F-111 had already named the natural home for its own fix: *"the
`stage_hint` itself, which could name a content check beside the `R` check it already
names."*

So one sentence misled two sessions in **opposite directions**:

| | what the archiver sees | what they conclude | the truth |
|---|---|---|---|
| this bug | correct archive, body rewritten → **no `R`** | "something is wrong" | the archive is fine |
| F-111 | broken archive, stale destination → **`R` appears** | "verified" | the destination is stale |

`R` is monotone in both directions and blind in each: it vanishes when the archive is most
correct and appears when it is most broken. **A fix for staged-ness alone would have left
the false-positive half live** — which is why the shipped wording names the stale case too,
and why `--find-renames=25%` was the wrong shape of answer from the start: a lowered
threshold makes the letter appear *more* often, which is precisely the failure F-111 paid
for.

Shipped on both surfaces — `src/librarian/tools/mv.rs`'s `stage_hint` and
`src/prompts/guides/tracker-conventions.md` § *Bug files*:

> Confirm both halves are **staged, i.e. lettered in column 1** of `git status --short`:
> either one `R` line, or a `D` plus an `A`. A leading space (` D`) or a `??` is
> half-staged. Do **not** confirm on the `R` alone — it is a *similarity* verdict, not a
> staging or a content one. For content, check the destination for something you wrote
> just before the move.

**Test:** `librarian::tools::mv::tests::the_archive_confirmation_names_staged_ness_and_content_on_every_surface`.
It asserts three tokens — `column 1` (staged-ness), `similarit` (the letter disclaimed),
`stale` (the content case) — on **every** surface that states the claim, which is the check
none of them had: they were written together and say the same thing, so a reader
cross-checking one against another finds agreement and learns nothing.

**There were THREE surfaces, not the two this file named.** The third —
`get_guide("librarian")` § *Archiving / Moving Trackers*, `src/prompts/guides/librarian.md`
— still read *"expect one `R` line, never ` D` + `??`"*, and it surfaced only because the
tool auto-injected that section during **this fix's own archive move**. The defect was
surviving in the guide that explains the operation being fixed, and enumerating the call
sites from this file's *Where it is stated* list rather than from the corpus would have
shipped it. That is § *Testing Discipline*'s **mutate once per guarded SITE** law arriving
as a live cost rather than as a principle: a two-site fix verified at two sites reads
exactly like a complete one.

Two things about that test are load-bearing rather than tidy:

- **The guide assertions are scoped to the staging paragraph.** `stale` occurs **10** times
  elsewhere in `tracker-conventions.md` and **5** more in `librarian.md`, so an unscoped
  `contains("stale")` stays green with the entire paragraph deleted. Each slice is
  asserted non-empty for the same reason — if an anchor string moves, an empty slice makes
  every assertion below vacuous instead of red.
- **Mutation once per guarded SITE**, since one kill says nothing about the other.
  Reverting **any** of the three surfaces to *"shows a single `R` rename line"* reds it,
  each naming its own surface in the failure message, and no reversion is caught by
  another's assertions. All runs restored byte-identical afterwards (shared checkout).

- **The slice guard is a CEILING, not a floor — and the first version was a floor, which
  did active harm.** A missing *opening* anchor panics. A missing *closing* anchor is the
  one that hurts: the slice runs to end-of-body and absorbs unrelated prose, so the
  content assertions pass on the wrong text. Measured: delete `librarian.md`'s
  "Two consequences" line and the slice goes 462 → **3153** bytes, picking up 2 further
  `stale` hits. A lower bound is monotone under exactly that failure and cannot see it.
  Worse, the floor fired *before* the content assertions and **masked** them — site 3's
  first mutation reddened on "not a real slice" rather than on the missing discriminator,
  which proves the test is sensitive to the file without proving the assertions
  discriminate. That is § *Testing Discipline*'s monotone law holding about this test's own
  guard, found only because the site-3 mutation was actually run and its **message read**
  rather than its exit code. A fourth mutation case now pins the ceiling directly.

The pre-existing `move_names_the_staging_action_not_only_the_two_paths` asserted
`hint.contains('R')`, which was **doubly wrong**: it pinned the letter this bug removes,
and a bare `contains('R')` matches any capital R in any word, so it never discriminated the
rename line at all. Replaced with an assertion that the hint still names `git status
--short`, with the semantics delegated to the new test.

Considered and rejected: passing `--find-renames=25%` in the instruction. It makes the `R`
appear, but it teaches a threshold with no principled value, the next archiver whose body
rewrite is larger falls under 25% too, and — per F-111 above — it strengthens exactly the
signal that produced the false positive.

Fix SHA: `e1d0b333` — *fix(librarian): the archive confirmation names staged-ness and
content, not the `R` letter*
Patch-id: `20a35030151f181b37ccdc56f837c28adcaf9aff`

Single parent, so the patch-id is real. The SHA is positional and dies when `experiments`
is rebased; the patch-id is a content hash of the diff and survives rebase and
cherry-pick. Recorded as a pair at fix time — nothing owed later.

Gate green @ 2026-09-08 10:46:15–10:48:34Z: fmt 0, clippy 0, LEAN 0, DEFAULT 0. Read out
of the **DEFAULT** lane, since LEAN is vacuous for librarian code: LEAN `librarian::`=**0**
against DEFAULT `librarian::`=**1760**, with the `prompts::` control at **101 in both** —
re-derived together, so the zero is a measurement rather than a broken grep.

### The third surface had a byte budget, and the budget test refused the lazy fix

Adding the fix to `src/prompts/guides/librarian.md` reddened
`server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling`
at **12462 B against a 12244 B ceiling with margin already at 0** — that section is
auto-injected, so every byte added to it is paid by every session that triggers `doc.move`
or `doc.delete`.

The test's own message is what made the remedy unambiguous, and it is worth quoting because
it is a guard that names its remedy rather than only its predicate:

> Raising CEILING is a spec amendment, not a fix — it is not the remedy for this failure.

So the bytes were paid, not budgeted: the new wording was compressed to its operative
clause (the three discriminators and the derivation pointer), and the rest came from the
section's own prose — *"At the artifact layer"*, *"all of that in one call"*, *"the
artifact's augmentation"* and similar padding. **Net effect on the emitted surface: none.**
The full reasoning lives in `tracker-conventions` § *Bug files*, which the librarian section
already cross-references with `Derivation:` — so the terse form is not a loss, it is the
right split between a primary and a secondary surface.

Worth noting against § *Testing Discipline*'s remedy-text law: this guard named a **specific
named remedy, a plan file holding the standing fix, and two alternative diagnoses** for the
case where the obvious one is wrong. That is the shape the law asks for — the reader is
sent somewhere they can act, and the message anticipates its own misdiagnosis.
## Severity

Low. No archive was corrupted; this one committed correctly. It is filed because the
instruction's *failure* mode is silent agreement — the reader sees an outcome the check does
not describe, and the most available resolution is to stop trusting the check.

## Resume

Unclaimed. Noticed while archiving a bug in the ordinary way, and the ordinary way is what
triggers it: writing the fix SHA and outcome into a bug file before moving it is exactly what
pushes similarity under the threshold.

## References

- The half-staged case the current wording exists to prevent, and which is still real:
  `docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md`
- The move that produced the measurement: `f7d61237`.

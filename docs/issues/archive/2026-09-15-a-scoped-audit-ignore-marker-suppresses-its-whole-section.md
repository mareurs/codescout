---
id: 2190ec12967c9fcf
kind: bug
status: fixed
title: 'BUG: a scoped audit-doc-refs:ignore-refs marker suppresses its entire section, so 73 refs in PROBES.md are unguarded and the scan still exits 0'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-16
---

## Summary

A scoped `<!-- audit-doc-refs:ignore-refs \`a\` \`b\` -->` marker in `docs/PROBES.md` is suppressing
**every** reference in its section, not the tokens it names. Measured 2026-09-15 at `HEAD`:
`librarian(action="audit_doc_refs", paths=["docs/PROBES.md"])` reports **36 refs found** and **not
one** of them lies between the marker (`:163`) and the next heading (`:202`) — a span that holds
**73** path-shaped backticked tokens.

The marker's author anticipated this exact outcome and wrote six lines of prose to avoid it:

> Scoped by token, not by section, because this section carries **27 real refs** and a bare
> `audit-doc-refs:ignore` would silence every one of them — in the document whose whole job is
> telling a reader which instrument to trust.

`parser.rs` agrees, at the type level: *"Kept separate from [`blocks`] so the `Only` case still walks
its tokens: collapsing the two would make a scoped marker behave like a bare one."* That is the
observed behaviour.

## Symptom (Effect)

`audit_doc_refs` returns `exit_code: 0` on `docs/PROBES.md` having examined none of the section's
refs. A stale path there is unguarded and **cannot red** — including in the `Audit Doc Refs` CI job,
which reads the same scan.

**Two counts, two units, deliberately not reconciled.** 73 is my count of path-shaped backticked
tokens in `:163–:201` by grep, 2026-09-15. 27 is `parser.rs`'s own figure for "real refs" in that
section, written 2026-09-02. Different methods, different dates, and the section has grown; the
magnitude is *dozens*, and either number quoted alone would be a false precision.

## Reproduction

```
librarian(action="audit_doc_refs", paths=["docs/PROBES.md"], emit_tracker=false)
```

Read `findings[].md_line`: every value is `< 163` or `> 201`. Control that this is suppression and
not an empty span: `sed -n '163,201p' docs/PROBES.md | grep -o '`[A-Za-z0-9_./-]*\.\(rs\|sh\|py\|md\)`' | wc -l`
→ **73**.

## Environment

codescout `experiments` at `8289a448`. Marker at `docs/PROBES.md`:163, section `## Standalone
scripts` (`:162`) to `## Built-in \`librarian\` scans` (`:202`).

## Root cause

**ESTABLISHED 2026-09-15 by experiment. THE MARKER'S OWN EXPLANATION INVOKED THE FORM IT WAS
EXPLAINING.** The comment is re-parsed line by line — `parse_ignore_marker` runs on **every** HTML
event and line 33 reassigns `suppression` each time. Line 167 read:

> `` `audit-doc-refs:ignore` `` would silence every one of them

That line contains `audit-doc-refs:ignore` and **not** `audit-doc-refs:ignore-refs`, so
`parse_ignore_marker` returns `Some(Suppression::All)` — a *new, bare* marker, four lines into the
scoped one — and `All` then holds to the next heading. The author wrote *"a bare
`audit-doc-refs:ignore` would silence every one of them"* and, in writing it, silenced every one of
them.

**The experiment, and it is also the repair.** Rewording line 167 so the literal bare token no longer
appears, changing nothing else:

| | refs found |
|---|---|
| before | **36** |
| after | **207** |

171 refs restored to the gate, and 29 broken ones became visible having been hidden since
2026-09-02 — **28** after `72ac7150`, where sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb`
repointed a citation label on line 201, a line four of their own commits had edited *inside* the
suppressed span. Reproduced independently in this tree: 29 → 28 broken, 162 → 163 resolved. None is
`high` and the scan's `exit_code` is `0`.

**CORRECTED 2026-09-15 — that is NOT CI passing this change, and an earlier revision of this section
said it was.** `Audit Doc Refs` succeeded on run `34977620213`, but that run's head is `8ee8e4e2`,
which **predates** this repair: it scanned the *suppressed* file at 36 refs. So CI's green is
established for the **before** state — and is precisely the green-for-the-wrong-reason this record
is about — while the 207-ref state is verified by **one instrument run three times**, which is not
the same as three confirmations. **Corrected again, later the same day:** an earlier revision of this
paragraph called the peer's re-run *"independent"*. It was independent of the **operator**, not of
the **method** — this session locally, sessionId `9403d62d-…` on their side, and the run that
produced the 28 all executed `audit_doc_refs` against the same tree. `CLAUDE.md` § *Reaching a Peer
Session*: **check independence, not agreement** — two instruments sharing a scope agree *because* of
the shared blind spot, and at the point of use that is indistinguishable from corroboration. If the
parser holds a second defect in this same region, all three runs report the same clean 28 and none
of the three can see it. The accurate statement is **one method, three runs, zero CI**.
**The 171-ref jump is untested by CI until a run first carries `f3f79c1a`.** Raised by sessionId
`9403d62d-116b-46ea-ac9b-004acff2b1cb`. The distinction is timing rather than substance, and it is
exactly the difference between a check that ran and a check that *would* have — which is this bug's
own subject, committed in the write-up of it.

**This is `IC-6`, and it is the half named in `CLAUDE.md` § *Parsers Over a Namespace*: a grammar
over a namespace with no way to MENTION its own token.** The section's own example is *"an entry id
cannot be mentioned without citing it"*; here a suppression marker cannot be mentioned without
invoking it, and the only place anyone would ever mention it is the explanation of why they chose
the other form.

**Both earlier hypotheses stay recorded, because each cost a measurement and each is a plausible
re-derivation.**

1. **Line length — FALSIFIED.** Lines 175/180/185/186/201 (4441–7704 chars) are unreported and line
   208 (2453) is reported, which looked like a cap. It is not: 208 sits *past the section boundary*.
   Every unreported line is inside `:163–:201` and every reported one is outside. The correlation
   with length is an artefact of the longest cells happening to live in that section.
2. **Prefix collision — FALSIFIED.** `is_ignore_marker` is a `contains` of `"audit-doc-refs:ignore"`,
   which `"audit-doc-refs:ignore-refs"` also contains — so the scoped form looked like it might be
   swallowed by the bare one. It is not: `parse_ignore_marker` tests
   `html.contains("audit-doc-refs:ignore-refs")` **before** falling back to `Suppression::All`. The
   grammar disambiguates correctly **between two markers**; what it cannot do is tell a marker from
   a quotation of one.

**Re-deriving these numbers: `findings` is CAPPED at 50 of 207, so it is not the broken set.**
Reading it as one is `IC-13`. It was usable here only because it is ordered most-severe-first and
`resolved` entries do not begin until position 46, which puts all 28 broken and 16 unknown inside
the window — **a property of this particular response, not a guarantee.** Read
`n_refs_broken` / `n_refs_resolved`, never the length of `findings`. (Noted by sessionId
`9403d62d-116b-46ea-ac9b-004acff2b1cb`, who also caught a hand-rolled check agreeing with the
instrument on the VERDICT while disagreeing on the SUBJECT — they resolved the link *target* from
the repo root and flagged a working link; the tool flags the *label*, a different token, and is
right. Two checks agreeing on a verdict for different reasons is the shape where a hand-rolled one
reads as corroboration.)

## Evidence

**An over-capture that is real but does NOT explain this.** `backtick_re` collects every backticked
token in the marker body, and the body carries prose. Declared targets are 4 — `src/serve`,
`src/lsp/m`, `args`, `audit-doc-refs:ignore` — where the author intended 2; the last two are quoted
*in the explanation*. `parser.rs`'s own doc comment says backticks were chosen as the delimiter
*"because the marker body also carries prose, and a whitespace split would read the explanation as
targets"* — and backticked prose is read as targets anyway, which is that reasoning holding against
the parser that states it. But `Only([4 targets])` still blocks 4 refs, not 73, so this is a
separate small defect and not the cause.

**The blast radius includes this bug's own neighbours.** Twenty minutes before this file was
written, `audit_doc_refs` was used to verify an edit to `docs/PROBES.md`:180 — inside the suppressed
span. It returned `exit_code: 0` and that zero established nothing. The edit was verified by `ls` on
the link target instead, and only because the silence looked wrong.

## Hypotheses tried

`IC-6` (`addressing-without-an-escape-hatch`) was the first guess, on the prefix collision, and is
**rejected** — see Root cause (2). The grammar owes a disambiguator and has one.

## Fix

**FIXED at the grammar, not only at the call site.** The instance repair came first and is kept
below because it is what produced the measurement; the defect itself is closed by `510b2c09`.

**The repair — positional, not `contains`.** `marker_token` replaces `is_ignore_marker`'s
`html.contains("audit-doc-refs:ignore")`: the token must sit at the comment's **start**, after
`<!--`. That makes a mention **unrepresentable** as a declaration rather than policing it —
`CLAUDE.md` § *Observer Blindness* position 3 — so a marker's own documentation can now name the
form it did not use. `-refs` is tested first because it is the longer prefix, and
`parse_ignore_marker` branches on which form is *declared* rather than which is mentioned anywhere
in the body. Continuation lines of a multi-line comment carry no `<!--`, return `None`, and leave an
active suppression untouched — the call site's guard fires only on `Some`.

**`is_ignore_marker` is gone**, caught dead by clippy's `-D dead-code` on the `--all-targets` form
(a bare `cargo clippy` lints neither the lib-test target nor this). Its doc comment carried the
section-scope contract, which had no other home, so that paragraph moved to `marker_token` rather
than dying with the function.

**The regression test, and the red that proves it discriminates.**
`a_scoped_marker_quoting_the_bare_form_does_not_widen_to_the_bare_form` asserts over a **whole
parse**, not over `Suppression`'s methods — `blocks` and `blocks_everything` were correct throughout,
so a unit test on either passes against the defect. The fixture mirrors `docs/PROBES.md`: a
multi-line comment whose prose names the coarse form. Its **surviving** refs are the load-bearing
half; assert only that the named target is gone and it passes against `Suppression::All`, which is
the whole defect.

`scripts/mutation-probe.sh`, reverting the positional check to `contains` in an isolated worktree:
**KILLED (rc=101, 1 test ran)**, failing with `got []` — every ref suppressed. No peer saw that red.
**The mutation also settled a question reading could not:** `got []` proves the continuation line
reached `parse_ignore_marker` on its own, so the comment *is* re-parsed per event. Had it arrived as
one event, the old `contains` would have returned `Only` and the test would have been
non-discriminating — a guard that guards nothing, indistinguishable from this one at the point of
commit.

Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, with the four suppression tests read out of the default
lane **by name** rather than off a total.

**Still owed, and deliberately not bundled:** narrow the target capture to the run of backticked
tokens *before* the first prose word. The over-capture in § *Evidence* is real — 4 targets declared
where 2 were intended — and independent of this fix, which is why it is not closed here.

---

**The instance repair, kept because it is the measurement.** `docs/PROBES.md`:167 was reworded so
the marker's explanation no longer contains the bare token, plus a line at the site saying why it
must not be written literally there: **36 → 207 refs**. That was a workaround at one call site and
said so; `510b2c09` is what stops the next author re-creating it.

**Do not** "fix" this by deleting the marker from `docs/PROBES.md` — that unguards two genuine false
positives the author correctly annotated.

## Fix provenance

- **SHA:** `510b2c09` (`experiments`)
- **patch-id:** `21d20fcc9e10cc4dd35f28f1e15d9770c6ed9909`

The instance repair that produced the 36 → 207 measurement is `f3f79c1a`, which is not cited as the
fix: it reworded one comment in `docs/PROBES.md` and protected nothing else. `510b2c09` is the
grammar change plus its regression test and the mutation that killed it.
## Resume

Start at the caller of `parse_ignore_marker`, not at `parse_ignore_marker` itself — it is correct in
isolation. The question is how a `Suppression::Only` in force over a section ends up blocking every
ref in it: whether the value is recomputed per event, whether a later comment in the span re-parses
to `All`, or whether `blocks_everything()` is consulted where `blocks(raw)` was meant.

## References

- `src/librarian/tools/audit_doc_refs/parser.rs` — `parse_ignore_marker`, `Suppression`,
  `is_ignore_marker`, `backtick_re`
- `docs/PROBES.md`:163 — the marker, and its author's reasoning for the scoped form
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the principle: a zero must name the
  scope it examined
- `docs/issues/2026-09-15-probes-recommends-strings-to-settle-a-binarys-contents-and-a-miss-proves-nothing.md`
  — the sibling filed today, same shape one layer out: an instrument whose miss is read as absence

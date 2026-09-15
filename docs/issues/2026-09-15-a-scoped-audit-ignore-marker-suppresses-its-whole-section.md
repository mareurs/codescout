---
id: bc79a20e28c9ad1b
kind: bug
status: open
title: 'BUG: a scoped audit-doc-refs:ignore-refs marker suppresses its entire section, so 73 refs in PROBES.md are unguarded and the scan still exits 0'
tags:
- cluster/selector-narrower-than-its-population
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

**THE INSTANCE IS REPAIRED; THE DEFECT IS NOT. This record stays `open` for that reason.**

**Shipped** — `docs/PROBES.md`:167 reworded so the marker's explanation no longer contains the bare
token, plus a line at the site saying *why* it must not be written literally there. 36 → 207 refs.
That is a workaround at one call site: it repairs this file and protects nothing else. **The next
author who explains their choice of the scoped form re-creates it**, and the explanation is exactly
where the bare token naturally appears.

**Not designed** — the grammar needs an escape for *mention*, which is `IC-6`'s standing debt. The
house pattern is to narrow where safe and **name the residual at the refusal site**; the residual
here is that no escape exists at all, so the marker's documentation cannot describe its own
alternative form without triggering it.

**Do not** "fix" this by deleting the marker from `docs/PROBES.md` — that unguards two genuine false
positives the author correctly annotated, and hides the defect rather than closing it.

Two things worth doing, both still owed:

- **A regression test at the BEHAVIOUR, not the parse.** `Suppression::blocks` and
  `blocks_everything` are unit-correct by inspection and would both pass — the failure is that a
  later event overwrites a correct `Only` with `All`. Assert on a whole-file scan: a fixture whose
  scoped marker **quotes the bare form in its own comment body** must still report the section's
  unnamed refs. That is the regression that actually happened, and no unit test on the two methods
  can reach it.
- **Narrow the target capture** to the run of backticked tokens *before* the first prose word, so a
  marker's explanation cannot contribute targets — the separate over-capture in § *Evidence* (4
  declared where 2 were intended).

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

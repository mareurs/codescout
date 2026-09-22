---
id: c04e0d83106045d7
kind: bug
status: open
title: 'BUG: a served advisory hardcodes "Four such captures are recorded" against a corpus now at fourteen'
tags:
- cluster/doc-contradicted-by-code
- companion-plugin
- hooks
- shared-checkout
- stale-count
---

# BUG: a served advisory hardcodes "Four such captures are recorded" against a corpus now at fourteen

## Summary

`codescout-companion/hooks/pre-edit-dirty-check.mjs` fires whenever a session edits a file with
uncommitted changes it did not mediate. Its advisory text tells the reader how common
working-tree capture is, by citing a count:

> Four such captures are recorded in codescout's
> `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

That file's highest instance is now **Instance 14** (closed 2026-09-14, `5a2474a5`). The number
is stale by ten, it is served rather than merely written down, and it understates the hazard in
the one direction that changes a reader's decision — a reader weighing *"is this rare enough to
skip the check?"* is handed a number roughly a third of the truth.

## Symptom (Effect)

The advisory arrives well-formed and confident. Nothing marks the count as a snapshot, names the
date it was taken, or says how to re-derive it. The number reads exactly like the rest of the
message, which is current and correct.

## Reproduction

1. In a shared checkout, leave any tracked file with uncommitted changes the hook did not
   mediate — a `doc(...)` write does this by construction, as the advisory itself explains.
2. Call `edit_file` / `Edit` on that path.
3. Read the `[cs-hint]` advisory. It states "Four such captures are recorded in
   codescout's `docs/issues/2026-08-31-...`".
4. `doc(action="get", id="e421be689a23ae2a")` — the headings run to `## Instance 14`.

Observed 2026-09-14 in this session, on `docs/trackers/bug-fix-session-log.md`.

## Environment

`claude-plugins/codescout-companion`, `hooks/pre-edit-dirty-check.mjs`. Advisory text at
`:119`; a byte-equivalent copy in the file header at `:8-9`. Read against the plugin repo's
working tree, 2026-09-14.

## Root cause

A population count written into prose, in a repo with no gate that reads it.

**The count was wrong on the day it was written, not merely decayed into wrongness.** Verified
2026-09-14 by `git log -S` in `claude-plugins`: `Four such` and `instance 5, e0525462` were
**both introduced by `813a28d` (2026-09-09), the same commit**. There is no once-true value to
restore. The corpus has since reached fourteen, so the sentence is now wrong by ten as well as
wrong by construction. codescout does
ship a stored-count gate — `scripts/pre-commit-ledger-counts.py`, which is what prints
*"refuse a stored count, or a class gaining a member it does not name"* on every commit here —
but its population is `LEDGER = docs/trackers/issue-clusters.md` plus
`LEDGER_DIR = docs/trackers/issue-clusters`. It guards cluster-membership counts in this repo
and reaches nothing in `claude-plugins`, correctly and by design. So the gate's green is silent
about this sentence, and always was.

*Verified at the bytes 2026-09-14: `:89` and `:99` of that script for the population, `:154-159`
for how it enumerates it.*

## Evidence

The same comment block dates its own decay, which is why this is worth a record rather than a
one-word patch. `pre-edit-dirty-check.mjs:8-12`:

```js
// index. On a shared checkout your edit and whatever a concurrent session
// already wrote to the same file land together, under your message. Four such
// captures are recorded in codescout's
// docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md,
// and its remedy table shows explicit pathspecs do NOT defeat the co-edited-file
// layer (instance 5, e0525462): both diffs merge in the working tree before
// `add` ever runs.
```

"Four such captures" and "instance 5" sit **four lines apart, in one commit** — `813a28d`,
2026-09-09, confirmed by `git log -S` on each string independently. This is not prose that was
extended while its number went unrevisited; the number and its refutation were authored
together. No instrument compared them, and none exists that would.

## Fix

**Delete the number rather than update it**, and here the usual argument is not even the
strongest one available. The ordinary case against updating is decay: a count restored today is
wrong again on the corpus's next member. This one is worse — there is **no correct historical
value to restore**, since the count contradicted its own adjacent citation in the commit that
introduced both. Updating it to fourteen would not repair a decayed fact; it would invent a
correct one for the first time and start the same clock.

The sentence carries its full force without a count — *"Such captures are recorded in
codescout's `docs/issues/2026-08-31-...`"* — and the citation is what a reader needs. **Both
sites need it**: the served text at `:119` and the header twin at `:8-12`.

This is `CLAUDE.md` § *Observer Blindness* position 3 applied literally: for a published claim,
ship its derivation rather than its value. Here the derivation is the citation itself, already
present.

Not implemented — the file is in `claude-plugins`, a different repo, and this session has made
no edit there.

## Update 2026-09-22 — two further sites, in codescout itself, now fixed

**This record named two sites, both in `claude-plugins`. There are four.** The same sentence is
served by codescout's own `scripts/pre-commit-unreviewed-content.sh`, in the same two-site shape
this file documents for the hook — served text plus a header twin:

- `:123` — `echo "same file since you last looked. Four such captures are recorded in"`
- `:12` — `# That is not hypothetical. It has happened four times in this repo, documented in`

Both are now fixed with **this record's own prescribed remedy** — the value deleted, the citation
kept (`It has happened repeatedly in this repo`; `Such captures are recorded in`). `bash -n` clean;
no test pinned either string, checked before editing.

**Two counts in that file are legitimate and were deliberately left alone.** `FOUR SEPARATE calls`
at `:126` enumerates the steps that follow it, and `instance 4` at `:14` cites one durable record
rather than a population. A substitution keyed on the word would have corrupted both — the hazard
`CLAUDE.md` § *Parsers Over a Namespace* describes, met in the act of fixing a different one.

**The count has decayed further since this was filed.** Re-derived 2026-09-22 against
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`: **17 `## Instance`
headings, the highest numbered `Instance 18`** (2026-09-20). The title's "fourteen" is itself now
stale. Those two figures differ and are reported separately rather than reconciled into one
headline, per § *Testing Discipline* — count the LIST, never the corpus.

**Status stays `open`.** The two sites at `claude-plugins:codescout-companion/hooks/pre-edit-dirty-check.mjs:119`
and `:8-12` are untouched — a different repo, which this session did not edit, and which has its own
live session. What this closes is the in-repo half. What it adds is the finding that the class had
**twice the sites the record knew about**, which is § *Testing Discipline*'s "mutate once per
guarded SITE, not once per feature" paying out in the direction that costs: a remedy applied at the
two known sites would have left this advisory serving the stale number indefinitely, with the bug
file reporting the class closed.

## Tests added

None. A regression test would have to assert the absence of a digit in an advisory string,
which is monotone under exactly the rewording that would reintroduce one. The durable guard, if
anyone wants it, is to extend a stored-count scan to the plugin's served strings — a population
question, not an assertion question, and larger than this record.

## Resume

Decide whether the remedy is the one-word deletion above, or whether `claude-plugins` should
gain its own stored-count gate over hook advisory text. The first is minutes and closes this
instance; the second closes the class in that repo and has no owner today.

Do **not** "fix" this by re-deriving the count. If a number is kept, it owes a date and a
re-derivation command, per `CLAUDE.md` § *Testing Discipline* — a count must arrive with its
unit and its instant or not at all.

## Cluster fit

`IC-11` on its operative test — *a document states something the corpus contradicts, and nothing
checks prose against corpus systematically*. **This record's first draft justified the tag on the
class's *"true when written"* clause instead, and that justification is WITHDRAWN:** `813a28d`
introduced the count and its contradicting citation together, so this defect does have an
authoring error to find — which `IC-11`'s claim explicitly says its members do not.

Kept here rather than retagged, on the remedy: this is the class with its genesis **immediate
rather than acquired**, and the repair is identical — delete the value, keep the citation. What
it costs the class is the comfortable assumption that its members start correct and drift.
Flagged rather than silently placed. An adjudicator retagging this to `IC-20`
(`floor-published-under-the-name-of-a-total`, whose remedy *— rename it as a floor, or refuse to
print it —* matches exactly, and whose *unknowable* clause does not) gets no argument from me.

**Provenance of the correction, because it bears on how much the tag should be trusted:** the
*"true when written"* claim was this author's, asserted without checking, and used to carry the
classification. It was disputed by a peer (sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`),
and the `git log -S` that settled it was run in response to that dispute rather than to any
doubt the author held. The tag survives its own justification being false, which is exactly the
situation a later reader should want flagged.
## References

- `claude-plugins:codescout-companion/hooks/pre-edit-dirty-check.mjs:119` — the served text
- `claude-plugins:codescout-companion/hooks/pre-edit-dirty-check.mjs:8-12` — the header twin, and the self-dating contradiction
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (`e421be689a23ae2a`) — the cited corpus, Instance 14 closed 2026-09-14
- `scripts/pre-commit-ledger-counts.py:89,99,154-159` — the stored-count gate, and the population it does not reach
- `docs/issues/archive/2026-09-13-pre-edit-dirty-check-prints-a-path-resolved-against-the-wrong-root.md` — prior defect in this same hook, filed here, establishing that plugin-hook bugs land in codescout's `docs/issues/`

---
id: '04e069162e1fb15f'
kind: bug
status: open
title: 'BUG: the stage log records who STAGED, not who authored, so `git add -A` makes you the recorded owner of every peer''s file and foreign-index has nothing to refuse'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- shared-checkout
- git-hooks
- multi-session
topic: the layer-2 owner field and the relation it actually records
---

## Summary

`.git/session-stage-log` is `IC-17`'s layer-2 mechanism — the owner field the ADR names as the
model to copy. It records **who staged a (blob, path) pair**, and
`scripts/pre-commit-foreign-index.sh` refuses a bare commit carrying pairs whose recorded session
is not yours.

**So a session that runs `git add -A` becomes the recorded owner of every path it swept**,
including untracked files another session is mid-write on. The guard then correctly finds nothing
foreign and passes. The capture proceeds under the sweeper's own `Session-Id` trailer, and every
instrument reports it as theirs — because by the mechanism's own definition, it is.

This is not the guard failing. It is the owner field answering *"who staged this?"* when the
decision needs *"whose content is this?"*.

## Symptom (Effect)

Nearly fired 2026-09-07, on this checkout, and was avoided by removing the files rather than by
any guard:

Four sessions were each writing a handover file into an untracked `handover/` directory. The
directory was not in `.gitignore`, so every file read as `??`. Any session running `git add -A`
followed by a **pathspec** commit of its own paths would have staged all four under its own sid —
and a later bare commit by that same session would have committed them, refused by nothing.

Spotted by `8dba66b0`; resolved by `cda3afe5` moving the directory out of the repo entirely
(`/home/marius/codescout-handover-2026-09-07/`, verified `diff -r` identical, nothing ever
staged). **The remedy was removing the resource, not fixing the guard** — which is the right call
under time pressure and leaves the mechanism unchanged.

## Root cause

Verified at the bytes 2026-09-07:

- `scripts/post-index-change-stage-log.sh` attributes a pair to the session whose hook fires,
  from `CLAUDE_CODE_SESSION_ID`. That is *the actor*, by design — the header documents a
  superseded rule where a batch was "claimed by whoever ran `git status` next", and the fix was
  to bind attribution to the staging session. Correct for its purpose.
- It rebuilds from `git diff --cached --raw`, which enumerates **newly-added untracked files**
  identically to modifications. Nothing distinguishes "I wrote this" from "I swept this".
- `scripts/pre-commit-foreign-index.sh` partitions into `mine[]` / `theirs[]` and refuses on
  `theirs`. A path you staged is in `mine`.

The three compose exactly: **stage it and it is yours, by the only definition the system has.**

### Why this is `IC-17` and not a guard bug

`docs/issues/2026-09-06-a-withheld-commit-is-indistinguishable-from-an-unpushed-one.md` already
extended this class once, in the same direction: git records an **author** and the decision needs
an **authorisation**, so *"add an owner field"* is not a sufficient statement of the remedy — the
field has to be the right relation.

This is the second instance of that extension, and the sharper one, because here the field was
**added deliberately by this project** for exactly this hazard and still records the wrong
relation. `IC-17`'s claim says a shared resource records *what* changed and never *who*. The
refinement both members force: **recording a `who` is not sufficient if it is the wrong `who`.**

## Evidence

The near-miss above is instance 1 and was not reached. What makes the mechanism claim solid is
that it is read from the scripts rather than from the incident — the three bullets in § *Root
cause* are each a line of shipped code, and no execution is needed to see that `mine` cannot
contain a foreign path.

**Not measured:** how often `git add -A` is actually used here. `CLAUDE.md` and the commit
sequence both prescribe explicit pathspecs, so the population may be small — but the handover
episode is a case where four sessions created untracked files simultaneously, which is exactly
when a sweep is tempting.

## Instance 2 — 2026-09-16, and it falsifies the remedy this file's own title implies

The title names `git add -A`, so a reader takes the **narrow** form as the safe one. It is not.

`e5691fad-9f78-4cd1-ad14-edfdd1fee41f` ran `git add docs/trackers/bug-fix-session-log.md` — one
explicit path, no `-A`, no directory pathspec, the narrowest form
`docs/conventions/shared-checkout-commit-sequence.md` prescribes — and staged `F-168` and
`F-170`, neither theirs, alongside their own `F-169` / `F-171`. Both guards passed, verified at
the predicates rather than from the incident: `scripts/pre-commit-unreviewed-content.sh:29`
compares staged against working tree and a full stage makes them identical;
`scripts/pre-commit-foreign-index.sh:152` reads `$git_dir/session-stage-log` and `:207` puts the
path in `mine[]`, because the stage log records the **stager** and that was them.

**Why this is worse than the `-A` case rather than a milder version of it.** Under `-A` the sweep
crosses files the sweeper has no business in, and the remedy *"name your paths"* reaches it. Here
the file **is** theirs to append to. Both ownership signals the system has — *did I write in this
file* and *did I stage this blob* — correctly answer **yes**, and the entries are still not
theirs. The per-file signal is **true and useless**, so there is no narrower `git add` available
and no rule of that shape that helps.

**The unit of ownership is the ENTRY; every instrument in the chain is path-grained.** That is
this record's claim holding one level below where it was written. `c054113b` resolved the
instance the only way available — four sessions' entries in one commit with authorship stated in
the message — after `29420e72` observed that no ordering lets each session commit its own, since
all four sections were already in the file: *"you first" and "me first" both capture, and everyone
waiting is a deadlock.*

Reported by `e5691fad` as a **near miss** — they unstaged by pathspec and nothing was lost, which
is the only reason this is evidence rather than an incident. Recorded here rather than filed as a
new bug: the mechanism is this file's, and a second record would have split one class across two
paths (`bug-fix-session-log:F-171`, `issue-clusters:IC-17`).

## Fix

Not fixed. Three directions, cheapest first, none costed:

1. **`.gitignore` the shared-artifact directories** — removes this instance and no others. What
   was effectively done today, by moving the directory out of the tree.
2. **Refuse `git add -A` / `-u` at the hook**, or warn when a single index write claims paths
   the session has never written. The recorder cannot currently tell, which is the point — it
   would need a write-side signal, and `file-provenance.py` is the obvious source and is blind
   to `run_command` (`docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md`).
3. **Record the relation, not the actor** — a pair would carry *first observed writer* rather
   than *last stager*. This is the honest fix and the expensive one; it needs a write-side
   channel the working tree does not have, which is `IC-17`'s `NONE` row.

**Do not "fix" this by making `foreign-index` stricter.** It is behaving correctly on the data it
has; a stricter predicate over a wrong relation produces false refusals without closing this.

## Tests added

None. The mechanism claim is a read of three shipped scripts, and the incident was avoided rather
than reproduced. A regression test would need a two-session fixture staging each other's untracked
files, which `tests/hooks-discrimination.sh` has the machinery for — that is the place, and it is
not done.

## Workarounds

Explicit pathspecs, which this repo already prescribes. `git add -A` is the hazard verb; the
sequence in `docs/conventions/shared-checkout-commit-sequence.md` § 4 already says
`git add <paths>` and this file is a reason rather than a new rule.

## References

- `scripts/post-index-change-stage-log.sh`, `scripts/pre-commit-foreign-index.sh` — the two
  halves.
- `docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md` § *Layer 2* — names
  `.git/session-stage-log` as the model to copy. Worth reading beside this: the model is sound
  and its relation is the thing to get right at the next site.
- `docs/issues/2026-09-06-a-withheld-commit-is-indistinguishable-from-an-unpushed-one.md` — the
  first instance of the same extension.
- Near-miss spotted by sessionId `8dba66b0-af4b-4cda-a333-54a0605b318e`, resolved by
  `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`. Filed by
  `4eac25ba-b181-4dac-a5a1-ec88502a5bc5`, who extended this same mechanism twice today and did
  not notice this about it.

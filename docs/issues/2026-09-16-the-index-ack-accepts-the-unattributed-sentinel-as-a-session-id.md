---
id: ea962a09e72e6863
kind: bug
status: fixed
title: CODESCOUT_INDEX_ACK accepts `-`, the unattributed sentinel, as if it were a session id
tags:
- cluster/addressing-without-an-escape-hatch
- git-workflow
- multi-session
- shared-checkout
topic: shared-checkout commit coordination
closed: 2026-09-16
opened: 2026-09-16
owner: marius
related: []
severity: med
---

# `CODESCOUT_INDEX_ACK="-"` clears a refusal the ack was never meant to reach

## Summary

`scripts/pre-commit-foreign-index.sh` offers `CODESCOUT_INDEX_ACK` as an escape: name every
other author of a joint archive and the commit proceeds, with their sids printed as
`Co-Authored-Session-Id` trailers. The gate says in its own words, at `:278-279`, that this
is **deliberately** not a wildcard — *"Deliberately NO `all` form — a joint archive has
exactly one other party, so a wildcard would buy nothing and would import the blast radius
that `all` was filed for."*

It had one anyway, spelled differently. `scripts/post-index-change-stage-log.sh` writes the
literal `-` into the owner column for a pair it could not attribute (`:395`, `:401`; four
routes reach it), and its own header calls such a row *"frequently a PEER's"*. The ack's
membership test was plain string matching — `case ",$index_ack," in *",$owner,"*` — so `-`
satisfied it. **One character, naming nobody, covering an unbounded number of
unattributable pairs.**

## Symptom (Effect)

A rename whose source half is unattributed refuses with no ack and **passes** with `-`:

```
--- owners ---
aaaaaaaa-0000-0000-0000-aaaaaaaaaaaa    arch/r1.txt
-                                        r1.txt
--- no ack ---    EXIT=1
--- ack='-' ---   EXIT=0
```

`Co-Authored-Session-Id: -` is then printed as the record, which records nothing.

## Reproduction

Throwaway repo with the real recorder installed as `post-index-change`:

```bash
echo r1 > r1.txt; git add r1.txt; git commit -qm base
mkdir -p arch
env -u CLAUDE_CODE_SESSION_ID git rm -q --cached r1.txt   # source -> owner `-`
mv r1.txt arch/r1.txt
CLAUDE_CODE_SESSION_ID="$A" git add arch/r1.txt           # destination -> owner A
CODESCOUT_INDEX_ACK="-" <guard>                           # EXIT=0
```

The `env -u` is the whole fixture: staging with no session id is one of the four routes to
a `-` row, and it is the cheapest to reach deliberately.

## Root cause

**A sentinel and a real value share one namespace with nothing to separate them.** The owner
column holds either a session id or `-` meaning *no session id could be determined*. The ack
reads that column as a list of parties. Nothing in the ack's grammar distinguishes *"this
token names a session"* from *"this token is the recorder saying it does not know"*, so a
value that exists to represent **absence of a party** is accepted as **a party**.

That is `IC-6`'s disambiguator half rather than its escape half: the defect is not an input
that cannot be expressed, it is two meanings that cannot be told apart once expressed.

The narrowing that kept it small was accidental. The ack arm is gated on `joint`, so a `-`
row only became ackable when it was half of a rename whose partner belonged to the
committer. Nothing about the ack's own logic contained it.

## Environment

`experiments`, 2026-09-16. Shared checkout, six live sessions. `pre-commit.com` is retired
(`scripts/install-hooks.sh:145-191`); `scripts/pre-commit-run.sh` dispatches the guards
directly, so a guard edited in the worktree is live for every session immediately.

## Fix

**FIXED at `96d839c3`, patch-id `caa8669f048c56e0dd09ced10823b9f2a2131cc6`.**

The ack loop rejects `-` before the membership test and sets a flag the refusal reads. The
refusal gained a block naming the **UNATTRIBUTED** half, stating that `-` is not a session
but every session the recorder could not attribute, and naming the one route that works:
re-staging it makes it yours by the recorder's rule — which is attribution by stager, not
approval, so record them anyway.

**A tightening, which is why it landed alone and first.** It removes a way to pass and adds
none, so it needed no precondition and did not touch the axis this guard's history says to
be slowest about. It is also the direction a proposed ack *widening* would have blown open:
a wider admitting predicate reaches far more rows, and every `-` among them would have been
ackable by one character.

## Tests added

Four cases in `tests/hooks-discrimination.sh` § 5. The two that carry the fix —
`ack of '-' does NOT clear an unattributed half` and `refusal explains the UNATTRIBUTED
half` — were **observed RED against the unmodified guard before the fix existed**. The
fixture asserts its own premise first (`owner_of r1.txt` is `-`), per § 7's rule that a case
which does not establish the path is foreign can pass for entirely the wrong reason. The
no-ack control passes on both sides of the change, so this is not "refuses more" generally.
Suite: 123 passed, 0 failed.

The explanation is pinned as an ALL-CAPS role token rather than a sentence, per
`tests/pre-push-foreign-session-guard.sh:156-157` — rewording the prose does not red,
deleting the explanation does.

## Workarounds

None needed post-fix. Before it: do not pass `-`, and there was nothing to warn you.

## Resume

**Archive-eligible and deliberately not archived yet.** `doc(action="move")` re-keys the
artifact and strands inbound citations until they are repointed in the same commit; the move
is its own act with its own checklist.

Two neighbours deliberately **not** folded in, because each is a different claim:

- The same `case` pattern leaves `$owner` unquoted, so glob metacharacters in a session id
  are live — a peer running with `CLAUDE_CODE_SESSION_ID='*'` would be satisfied by any
  non-empty ack. Self-inflicted and unlikely, but it is a second unintended wildcard at the
  same site. Not fixed here; a token-equality loop replaces both.
- The ack checks that every **owner** is named and never that every **name** is an owner, so
  a typo'd sid is silently inert. The sibling push guard filed exactly that class and fixed
  it; these two are advertised as mirrors and are not.

Found by a design review of a proposed ack widening. The hole predates that proposal and is
independent of it; the reviewer's contribution was reading the sentinel as a namespace
member, which the author of the ack arm had not.

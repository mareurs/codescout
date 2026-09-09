---
id: f359dc9c75a217ca
kind: bug
status: open
title: 'BUG: a sha-refspec push bypasses the foreign-session guard entirely — and the guard''s own refusal text recommends that exact form'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- git
- guards
---

## Summary

`scripts/pre-push-foreign-session-guard.sh:124` filters the pre-push hook's stdin rows to
branch pushes:

```sh
case "$local_ref" in refs/heads/*) ;; *) continue ;; esac
```

A push by **refspec from a raw sha** — `git push origin <sha>:experiments` — gives git no local
ref, so it passes the **bare sha** as `local_ref`. The `case` falls through to `continue`, the
loop body never runs, `commit_rows` stays empty, and the guard **exits 0 having scanned nothing**.

**The guard's own refusal text prescribes that exact form.** Its remedy block reads *"Use a
refspec at EVERY rung — pushing the branch name publishes the whole stack including commits above
you: `git push origin <your-sha>:experiments`"*. The advice is correct about what it addresses —
a branch push sends a prefix, a sha push sends a set — and following it disables the check that
produced it.


## Symptom (Effect)

A push carrying any number of foreign sessions' commits succeeds silently. No refusal, no banner,
no `CODESCOUT_PUSH_ACK` demanded. Observed live 2026-09-09: 29 commits from **seven** sessions
published with the guard emitting nothing about any of them.

Worse than silence: the ack is *accepted and reported as vacuous*. Naming all six foreign sids in
`CODESCOUT_PUSH_ACK` produced six copies of

```
note: CODESCOUT_PUSH_ACK named <sid>, which authored no commit in
this push, so the ack had no effect on it.
```

for six sids that authored **25 of the 29 commits**. A reader who acks correctly is told their ack
was unnecessary, which reads as reassurance and is the opposite of the truth.


## Reproduction

Two runs of the hook, identical except for the first stdin field. Real shas; the range is the one
actually pushed at 2026-09-09.

```
# A — refname form (what `git push origin experiments` sends)
printf 'refs/heads/experiments 20bb2a47... refs/heads/experiments 0b07f9c8...\n' \
  | bash scripts/pre-push-foreign-session-guard.sh origin <url>
  -> exit 1, 8295 bytes of refusal

# B — sha refspec form (what `git push origin <sha>:experiments` sends)
printf '20bb2a47... 20bb2a47... refs/heads/experiments 0b07f9c8...\n' \
  | bash scripts/pre-push-foreign-session-guard.sh origin <url>
  -> exit 0, 0 bytes
```

**A is the control and is why B is a measurement rather than a broken invocation:** same range,
same repo, same script, non-empty foreign set proven by A's 8 KB refusal. The only difference is
the `local_ref` token.

Confirming the range really was foreign-bearing:
`git log 0b07f9c8..20bb2a47 --format='%(trailers:key=Session-Id,valueonly)' | sort -u` → seven
sids, six of them not the pushing session's.


## Environment

`experiments` at `20bb2a47`, 2026-09-09. Hook installed at `.git/hooks/pre-push`.
Also present in `tests/pre-push-foreign-session-guard.sh`'s installer fixture, which copies the
real script byte-for-byte.


## Root cause

The `refs/heads/*` filter is correct in intent — it exists to skip tag pushes and branch deletions
— and wrong in extent. git's pre-push stdin contract puts a **ref name or a sha** in field 1;
the guard treats "not a branch ref" as "not a branch push", when for a sha refspec the *remote*
ref (field 3) is the branch and field 1 simply has no name to give.

Field 3 already holds the answer and is read into `_remote_ref`, whose leading underscore records
that it was deliberately discarded.


## Evidence

- Live: 29 commits, 7 sessions, guard silent, six vacuous-ack notes for sids authoring 25 of them.
- Controlled: A/B above, exit 1 / 8295 bytes vs exit 0 / 0 bytes.
- `scripts/pre-push-foreign-session-guard.sh:121-131` — the loop, the filter, and the two range
  branches that never execute for form B.


## Hypotheses tried

- **Guard not installed?** No — `.git/hooks/pre-push` is present and executable, and form A refuses
  through that same script.
- **Ack list wrong?** No — the six sids were copied verbatim from the guard's own refusal text
  emitted minutes earlier, and `git log` over the pushed range returns exactly those six plus the
  pusher's.
- **Range empty because already pushed?** Ruled out by using the true pre-push base
  (`0b07f9c8`) in both runs; A over that range refuses.


## Fix

Not applied. Decide the branch from **field 3** when field 1 is not a ref name, rather than
skipping the row:

```sh
case "$local_ref" in
    refs/heads/*) ;;
    *) case "$_remote_ref" in refs/heads/*) ;; *) continue ;; esac ;;
esac
```

`_remote_ref` must lose its underscore. Deletions stay handled by the existing `$ZERO` check on
`local_sha`, which is unaffected.

**Do not fix by removing the refspec advice from the remedy text.** The advice is correct and
addresses a real, separately-measured hazard (an authorisation names a set; a branch push sends a
prefix). The defect is that the guard cannot see the form it recommends.


## Tests added

None — nothing is fixed. The regression test is the A/B pair above and must be **two-sided**: a
sha-form row must be refused, *and* a refname-form row must still be refused. A one-sided
"sha form now refuses" assertion is monotone under the guard refusing everything, including tag
pushes and deletions it is supposed to skip — so pin a tag-push row and a deletion row as still
skipped in the same block.

`tests/pre-push-foreign-session-guard.sh` already has the fixture and helpers; this is a new
section there, not a new file.

**This is the class's own lesson about itself:** that suite has 90 assertions and every one is
about the guard's predicate — *who is refused*. None is about the remedy text, so no mutation
reaches the sentence that recommends the bypass. `CLAUDE.md` § *Testing Discipline* names this
exact gap and the cheap partial answer: assert the remedy's **shape**, e.g. that any command form
the refusal text recommends is itself covered by a refusing test.


## Workarounds

Push by branch name (`git push origin experiments`) and satisfy the guard with
`CODESCOUT_PUSH_ACK`. This reintroduces the prefix hazard the refspec advice exists to avoid, so it
is a trade rather than a fix: you get the foreign-session check and lose the set/prefix guarantee.


## Resume

Apply the field-3 fallback, then write the four-row test block (sha refuses, refname refuses, tag
skipped, deletion skipped) and demand an observed red by reverting the fallback.

Then re-read the remedy text: with the fix in, `git push origin <sha>:experiments` will refuse like
any other push, so the rung advice becomes usable as written for the first time.


## References

- `scripts/pre-push-foreign-session-guard.sh:121-131` — the loop and the filter
- `tests/pre-push-foreign-session-guard.sh` — 90 assertions, all predicate, none remedy
- `docs/RELEASE.md` § Concurrent-Work Rules — where the refspec advice is derived
- `docs/trackers/observer-blindness.md` OB-20 — the guard's own class reference
- `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` — the hazard
  the refspec advice addresses, and the reason it must not simply be deleted

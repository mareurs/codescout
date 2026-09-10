---
id: 94b97a30a89d3271
kind: bug
status: fixed
title: 'BUG: a sha-refspec push bypasses the foreign-session guard entirely — and the guard''s own refusal text recommends that exact form'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- git
- guards
fix_patch_id: 03c1fcd5aee6ca1fb98399a0386437e33c586b06
fix_sha: d68473222e84830618e347e290ab0d656f49fa0f
fixed: 2026-09-09
---

> **DUPLICATE — do not count this file.** The same defect was already filed by sessionId
> `59112612-5fc8-4b31-8c8c-e19220d99eac` as
> `docs/issues/archive/2026-09-09-pre-push-guard-filters-on-the-local-ref-shape-so-a-refspec-push-bypasses-it.md`
> (`7e0968e2ddfcbc07`), which has precedence. Both are archived against the same fix,
> `d6847322` / patch-id `03c1fcd5aee6ca1fb98399a0386437e33c586b06`.
>
> **Kept rather than deleted, because the two are not redundant in content:** the earlier filing
> reasons from the guard's stdin contract; this one carries the runtime A/B (exit 1 / 8289 bytes
> against exit 0 / 0 bytes), the six-vacuous-ack measurement, and the mutation matrix behind the
> test. Deleting it loses evidence; counting it twice corrupts every population over
> `docs/issues/`.
>
> **Its `cluster/guard-narrower-than-its-name` tag is WITHDRAWN from `IC-14`'s member list.** One
> defect is one member, the earlier filing's classification has precedence, and a duplicate must
> not move a promotion threshold. Note recorded on `IC-14`'s `**Members:**` line.
>
> **How it happened, recorded because the mechanism is reusable and the lapse was this author's.**
> The earlier filing is `7e0968e2ddfcbc07`, and that row **was in
> the 96-artifact open-bug list this file's author read at 14:1x the same day** — title truncated
> mid-word at *"so git push ori…"*. It was not missed for want of a query. A duplicate check was
> run before filing a different bug two hours earlier and **skipped here**, because this defect
> arrived by direct measurement rather than by reading, and a finding that arrives as a surprise
> does not present as a rediscovery. **Novelty of the discovery ROUTE is not novelty of the
> DEFECT** — and the two are indistinguishable from the inside, which is why the check has to be
> unconditional rather than run when something feels familiar.

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

**Register 1 FIXED 2026-09-09.** `scripts/pre-push-foreign-session-guard.sh` now decides the
branch from **field 3** when field 1 has no ref name to give:

```sh
case "$local_ref" in
    refs/heads/*) ;;
    *) case "$remote_ref" in refs/heads/*) ;; *) continue ;; esac ;;
esac
```

`_remote_ref` lost its underscore — it had exactly one occurrence in the file, verified before
renaming, so the underscore's claim that it was unused was true. Deletions stay handled by the
existing `$ZERO` check above; tag pushes still fall out because neither field is `refs/heads/*`.

**Register 3 is NOT closed by this; register 2 now IS, in its own file** —
`docs/issues/archive/2026-09-09-the-pre-push-remedy-names-a-refspec-a-zero-commit-pusher-cannot-form.md`
(`cluster/hint-composed-without-the-request`, `IC-22`). Split deliberately: this file archives as
fixed, and leaving register 2 inside it would have archived a live defect — the split is what let
it be picked up and closed on its own evidence a day later. Register 3 needs nothing — it
is the observation that the refspec sentence is *correct* for a pusher who owns commits, which is
why neither repair may delete it, and register 2's fix explicitly kept it on the `mine_n >= 1`
path for that reason.

**What the fix does NOT buy, stated because the archived status will imply otherwise:** the guard
now *sees* the refspec form; it does not make the refspec form safe to reach for. A refspec sends
everything **reachable**, so where an uncleared commit is an ancestor there is still no refspec
that excludes it — recorded as a falsification of § *Workarounds* in
`docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md`.
## Tests added

Added to `tests/pre-push-foreign-session-guard.sh` — section *"which FIELD names the branch
depends on the push form"*, six assertions. Suite **96 passed, 0 failed** (was 90).

**Observed reds, by mutating the production path in two directions** — each row's discriminating
direction has its own witness, which a single-sided pair cannot have:

| mutation | result |
|---|---|
| revert to the one-field filter | **94/2** — both *sha refspec form* rows red; rows 1 and 3 green |
| widen the fallback to accept any field-3 value | **94/2** — *tag push* red (plus an existing `tag push: allowed`); row 2 green |

**And one row is INERT, annotated as such on the fixture rather than left to be credited.** The
branch-deletion row discriminates **none** of three mutations — including removal of the `$ZERO`
check it appears to test, which leaves the suite at **96/0**. The reason is worth carrying: with
that check gone, a deletion row falls through and the range becomes `<base>..0000000`, naming no
valid object, so `git log` yields nothing, `commit_rows` stays empty and the guard exits 0 anyway.
The row reaches the right answer by a route unrelated to what it appears to assert. It is kept as
a documentation pin of the intended contract and explicitly **not** as a regression guard — false
coverage is the failure mode that stops the next person looking.

**Row 5 is deliberately absent** and annotated absent in the fixture: it belongs to register 2 and
is owned by that register's own bug file.
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

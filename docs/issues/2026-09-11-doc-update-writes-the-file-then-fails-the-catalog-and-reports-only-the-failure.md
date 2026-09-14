---
id: '9c58cc41923b5c23'
kind: bug
status: investigating
title: doc(action=update) writes the file, fails the catalog write, and reports only the failure — so the retry duplicates the section
owners:
- marius
tags:
- cluster/unclassified
topic: catalog/file write atomicity
unverified: 'Partial: bf068e61 fixed doc(action=update) only. The identical file-write-then-commit window in append_entry (src/librarian/catalog/augmentation.rs) is confirmed and NOT fixed -- a tx.commit() failure after a successful std::fs::write leaves the section on disk with its id never persisted, and a retry allocates a fresh id and writes a second section. Left unfixed because a peer session was actively editing that function (adding a `section` parameter), so it was flagged to them rather than edited concurrently. Ordering is also unchanged by design: catalog-first was considered and rejected.'
---

## Summary

`doc(action="update")` writes the markdown file and the catalog row as **two separate stores**,
and when the catalog write loses a lock race it returns the bare string `database is locked` —
after the file write has already landed on disk. The error names only the store that failed, so
it reads as "the call did nothing". The natural response is to retry, and the retry appends the
section a second time.

## Symptom (Effect)

Observed 2026-09-11 on `docs/issues/archive/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`
(`e64f73913b100c9f`), a shared checkout with at least two peer sessions active:

```
doc(action="update", id="e64f73913b100c9f",
    patch={body_edits:[{action:"insert_after", heading:"### Fourteenth observation…", …}],
           extra:{last_observed:"2026-09-11"}})
-> database is locked            # the whole response; no field, no hint, no partial-write flag

# retry, identical arguments
-> {"id":"e64f73913b100c9f","updated":true,"wrote_to":"…"}

# result on disk
729: ### Fifteenth observation, 2026-09-11 — …
761: ### Fifteenth observation, 2026-09-11 — …     <- byte-identical duplicate
```

`diff` of the two ranges is empty. The frontmatter `extra` patch appears to have landed only on
the retry, so the two stores disagreed in opposite directions within one call.

## Reproduction

Not reduced to a deterministic script — the trigger is a lock race against a concurrent catalog
writer. Shape: hold a write transaction on `~/.local/share/librarian/catalog.db` past the busy
timeout, issue a `doc(action="update")` carrying `body_edits`, observe the error, then check the
target file's bytes before retrying.

## Root cause

Not confirmed in code; stated as the shape the evidence supports and **not** as a mechanism read
out of the source. What the evidence does establish is ordering: the file write is committed
before the catalog write is attempted, and the error path returns without reporting or undoing
the completed half.

## Evidence

- The duplicate sections above, byte-identical, at `:729` and `:761`.
- `last_observed: 2026-09-11` present exactly once — so the frontmatter/catalog half did **not**
  double-apply while the body half did.
- The repair was `doc(action="update", patch={body_edits:[{action:"remove", heading:…,
  occurrence:2}]})`, which worked — `occurrence` is the disambiguator that makes the duplicate
  addressable at all.

## Why it is worth a file rather than a note

**The remedy the error text invites is the one that causes the damage.** A bare
`database is locked` is SQLite's ordinary busy-timeout response and is correctly read as
transient — retry is the textbook answer, and here it is wrong. Nothing in the response
distinguishes "nothing happened, retry" from "half happened, inspect first".

**And the blast radius is larger on a ledger than it was here.** This landed on a prose bug
file, where a duplicated `###` heading is untidy. The same partial write against a tracker whose
entries are `## PREFIX-N` headings mints a **second definer** of an entry id — `link_scan` binds
a token to that heading shape and to nothing else, so an ambiguous token resolves to nothing.
That is `IC-6` (*addressing without an escape hatch*), and it is the state the pre-push guard's
`git grep -c '^## PREFIX-N' >= 2` refusal exists to catch — i.e. the corpus already treats this
outcome as serious enough to block a push over.

**`append_entry` is not obviously exempt.** It performs a file write and a high-water-mark
record; whether those share one transaction with the catalog row was not verified here, and the
answer decides whether a locked retry can mint a duplicate entry id. Worth checking before
assuming this is confined to `update`.

## Hypotheses tried

None — filed on first observation, per capture-on-notice. No fix attempted.

## Fix

`doc(action="update")`'s `call()` writes the file (`std::fs::write`) then upserts the
catalog row (`artifact::upsert_and_mint_slug`) as two separate fallible steps, and the
second one's `?` propagated the bare `rusqlite`/`anyhow` error straight to the caller.

Fixed the reporting half, not the ordering half: a new `file_written_but_catalog_failed`
helper (`src/librarian/tools/update.rs`) wraps ANY failure of the catalog upsert in a
`LibrarianRecoverableError` that states BOTH facts — the file was already written, and
the catalog record failed — and explicitly tells the caller not to retry the same patch
blind. A `rusqlite::ErrorCode::DatabaseBusy` cause gets wording naming it as the common
transient case; every other cause gets the same "do not retry blind" framing without the
transient language, since borrowing that framing for a non-lock cause (a constraint
violation, a corrupt row) would be a false reassurance.

**Ordering itself is unchanged.** The file write still happens before the catalog write,
so the race this bug is about can still occur — what changed is that the caller now gets
an accurate diagnosis instead of a message that reads as "nothing happened". Reordering
(catalog first) was considered and rejected: catalog writes are the side more prone to
lock contention, so putting them first does not remove the race, and a catalog-succeeds/
file-fails ordering leaves `file_sha256` claiming bytes that are not actually on disk —
plausibly worse than the current duplicate-content failure mode, and file writes fail far
more rarely than a busy-timeout does.

**`append_entry` is not exempt, and is now confirmed rather than merely suspected.**
(`src/librarian/catalog/augmentation.rs`) opens a single `IMMEDIATE` transaction, does
`std::fs::write` for the section, then `tx.commit()`. A file-write failure rolls the
transaction back cleanly (matches its own "no id was allocated" error text) — but a
`tx.commit()` failure AFTER the file write succeeds leaves the section on disk with its id
never persisted, and a retry allocates a fresh id and writes a second section. Not fixed
here: a peer session is actively working in that function (adding a `section` parameter),
so this is flagged to them directly rather than edited concurrently. Left open as scope for
a follow-up.

**Fix SHA:** `bf068e61`, patch-id `ffc2ac691b32b33b35e6ff8bfd79aa34f9ed97a8`.

## Workarounds

**Do not retry a `database is locked` from `doc(action="update")` blind.** Read the target file
first and check whether the edit landed; if it did, the remaining work is the catalog half only.
`occurrence: N` on a `body_edits` `remove` is the repair for a duplicate that already exists.

## Resume

**Partially fixed 2026-09-13.** The reporting/remedy-text half of `doc(action="update")` is
fixed and tested (see § Fix). The ordering/race itself is NOT fixed — the file write still
precedes the catalog write, so this can still happen; what changed is that it now surfaces
accurately instead of as a bare `database is locked`.

**The general form** (named by the session that confirmed a second instance, `8bd791df`):
a filesystem write and a SQLite commit cannot be atomic with respect to each other, so every
write-then-commit pair has a window where the file is ahead of the catalog. The only question
a given site owes is what a RETRY does to a file that is already ahead of its catalog row —
that is decidable per site, unlike "does this have the same shape", which is not. Two answers
so far: `append_entry` DUPLICATES (a fresh id, a second section) because the row that would
have recorded "already applied" was never persisted; `resync_snapshot_row` (`update_entry`,
added 2026-09-13) CONVERGES because no id is allocated, so a retry re-renders the same row
idempotently — milder, but the same window, and worth naming rather than letting the milder
consequence stand in for "fine".

**Owed, in priority order:**
1. The same fix's WIRING is untested (`NOT REACHED BY ANY UNIT TEST` annotation at the call
   site in `update.rs`) — `file_written_but_catalog_failed` is unit-tested directly against a
   hand-built error, but no test constructs real cross-connection lock contention to exercise
   the join between `call()` and the helper.
2. `append_entry`'s DUPLICATING instance of the general form (§ Fix) — confirmed by reading
   the code, not fixed, flagged to the session working in that function.
3. Whether `write_field_to_frontmatter`'s callers (e.g. `event_create`) have the same ordering,
   and which answer (duplicates / converges) applies — not checked this pass.
4. A `git grep` for every other `std::fs::write` followed by a `tx.commit()` or catalog upsert
   in `src/librarian/`, asking the DUPLICATES-or-CONVERGES question at each site, rather than
   waiting for the next instance to be found by accident.

**A misroute happened while chasing (2), worth recording since it is this repo's own
Observer Blindness class in a live instance.** The append_entry finding was about
`8bd791df`'s code and was first sent to `f3c594ce` — correct several turns earlier when
they said they were adding a `section` parameter to that function, wrong by the time the
message went out, because they had since moved to a different file entirely. The routing
used a REMEMBERED conversation instead of a live check at send-time — attribution by
adjacency, exactly what `CLAUDE.md` § *Reaching a Peer Session* names. `file-provenance.py
--all <path>` names the current writer of a path in one call and is the cheap fix; use it
at SEND time, not from memory of who said what earlier in the session.

## References

- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- `docs/issues/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` — `database is locked`
  as a *test* symptom under contention; a different subject, listed so the two are not merged.

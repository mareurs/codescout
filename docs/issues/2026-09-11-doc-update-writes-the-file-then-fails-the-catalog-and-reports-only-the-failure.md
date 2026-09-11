---
id: '9c58cc41923b5c23'
kind: bug
status: open
title: doc(action=update) writes the file, fails the catalog write, and reports only the failure — so the retry duplicates the section
owners:
- marius
tags:
- cluster/unclassified
topic: catalog/file write atomicity
---

## Summary

`doc(action="update")` writes the markdown file and the catalog row as **two separate stores**,
and when the catalog write loses a lock race it returns the bare string `database is locked` —
after the file write has already landed on disk. The error names only the store that failed, so
it reads as "the call did nothing". The natural response is to retry, and the retry appends the
section a second time.

## Symptom (Effect)

Observed 2026-09-11 on `docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`
(`ee9d8d80ad5ecdc8`), a shared checkout with at least two peer sessions active:

```
doc(action="update", id="ee9d8d80ad5ecdc8",
    patch={body_edits:[{action:"insert_after", heading:"### Fourteenth observation…", …}],
           extra:{last_observed:"2026-09-11"}})
-> database is locked            # the whole response; no field, no hint, no partial-write flag

# retry, identical arguments
-> {"id":"ee9d8d80ad5ecdc8","updated":true,"wrote_to":"…"}

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

## Workarounds

**Do not retry a `database is locked` from `doc(action="update")` blind.** Read the target file
first and check whether the edit landed; if it did, the remaining work is the catalog half only.
`occurrence: N` on a `body_edits` `remove` is the repair for a duplicate that already exists.

## References

- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- `docs/issues/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` — `database is locked`
  as a *test* symptom under contention; a different subject, listed so the two are not merged.

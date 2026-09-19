---
kind: bug
status: fixed
tags:
- cluster/authorship-unrecoverable-after-the-fact
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

# `audit_log`'s `actor` records CONTACT and reads as AUTHORSHIP

`librarian(action="audit_log", row_id=...)` answers *"whose connection wrote this
catalog row?"*. A reader on a shared checkout asks it *"who authored this
artifact?"* and gets a confident, well-formatted, wrong name.

The mechanism is that **`reindex` writes an audit row for every artifact it
re-embeds**, stamped with the reindexing session's id. So the most recent
reindexer becomes the `actor` of record on artifacts it has never written a byte
of — and `reindex` is a routine, encouraged call.

## Observed (2026-09-13, ~14:5x)

A peer (`9403d62d`) asked whether the untracked architecture-boundary-probe work
was mine. It is not — I have never touched it. Yet:

```
librarian(action="audit_log", row_id="51baab12f451feb6")   # its session log
librarian(action="audit_log", row_id="c61d542269b5c6de")   # its measurement tracker
```

both return rows with `actor: codescout:8bd791df-5ff4-40fe-af30-69cc3fefc2f7`
— me. Had the peer run that check instead of asking, the instrument would have
contradicted my answer and neither of us could have said which was wrong.

The real writer is `actor: codescout:anonymous`, via `doc.create` / `doc.update`:

| at (+03:00) | actor | verb | payload |
|---|---|---|---|
| 2026-09-11 13:07:48 | anonymous | `librarian.reindex` | insert |
| 2026-09-13 09:13:55 | anonymous | `doc.update` | `file_sha256`, `updated_at`, `file_mtime` |
| 2026-09-13 09:29:57 | anonymous | `doc.create` | `slug` |
| 2026-09-13 09:35:39 | anonymous | `doc.update` | `file_sha256`, … |
| 2026-09-13 09:19:04 | **8bd791df (me)** | `librarian.reindex` | **`embedded_sha256` only** |
| 2026-09-13 12:27:43 | **8bd791df (me)** | `librarian.reindex` | **`embedded_sha256` only** |

## The obvious discriminator does not work

A reader who notices the problem reaches for `verb` — and `verb` is
**per-connection, not per-row**. `Catalog::set_audit_verb`
(`src/librarian/catalog/mod.rs:578`) is explicit:

> *Best-effort verb tag for subsequent audit rows on this connection. The verb
> persists until the next stamp — it means "last dispatched verb", not "verb of
> this exact statement"; audit_log documents this.*

`audit_log` does document it, in its `note` field
(`src/librarian/tools/audit_log.rs:257`) — and that note is about `verb`'s own
accuracy. **Nothing anywhere says `actor` is contact rather than authorship**, which
is the reading that actually costs something. I asserted `verb` as the
discriminator to the peer before reading `set_audit_verb`, and had to correct it.

**What IS reliable is the payload shape**, and it is documented nowhere:

- `embedded_sha256` alone changed → a re-embedding pass. Not a content write.
- `file_sha256` / `slug` / `source` changed → a real content write.

## Why this class

`IC-10`, whose roster entry already reads *"`Session-Id` commit trailer, hook
installed and live; **uncommitted half still none**"*. This is precisely the
uncommitted half: `audit_log` is the one instrument that looks like it answers
authorship for an artifact that has never been committed, and it does not. It
fails in the class's signature way — a plausible name, not an error.

It also compounds with `scripts/file-provenance.py`, whose `UNKNOWN` is loudly
labelled *"a statement about coverage, NOT about ownership"*. A reader who
correctly refuses to read `UNKNOWN` as unowned then reaches for `audit_log` as the
second opinion — and gets a **name**, which reads as the stronger answer. Two
instruments, and the more confident one is the wrong one.

## What is NOT established

- **Whether any session has actually acted on this.** Today produced a near-miss
  (the peer asked rather than checked), not a measured wrong action. I have not
  searched the corpus for a past misattribution traceable to this.
- **How wide the blast is — PARTIALLY MEASURED 2026-09-13 ~15:03, and the unit
  matters more than the number.** `audit_log(actor="codescout:8bd791df-…")`
  returns `filtered_total: 12480`. That is **not** 12,480 codescout artifacts:
  the tool's own `shards.note` says `filtered_total` sums a machine-wide local
  count (*"filter_where carries no repo predicate"*) with repo-scoped shard rows
  — **two halves counted over different populations**. So it bounds the hazard
  across every repo sharing this catalog and is not a per-project figure; a
  per-project count needs a different query than this one. What IS clean: the
  reindex that landed this very bug file reported `embedded: 1330`, so one
  routine call stamped my sid on ~1330 rows in a single pass. The defect
  demonstrated itself through the act of recording it.
- **Whether `codescout:anonymous` is itself recoverable.** It means a codescout
  writer with no `CLAUDE_CODE_SESSION_ID` (CLI invocation, or a session started
  without it). Whether anything else in the trail narrows it, I did not check.

## Fix directions, neither started

**FIXED 2026-09-19** — `e208bc258a79ceed561a7ba852ba691328448dfc`, patch-id `0bf2dc88fe776ad5dea07e90129b6e5767d06854`. Direction 1 (the note) taken over direction 2 (a derived `content_write` field) on this file's own stated criterion: a consumer search found ZERO programmatic readers of `actor`/`note` anywhere — only docs, the tool, and tests — so with nothing acting on it the note is honest and cheaper. It now names the payload-shape discriminator: `embedded_sha256`-only is reindex CONTACT; `file_sha256`/`slug`/`source` is a real content write. Existing rows untouched — they are an immutable record.

1. **Cheapest, and matches the precedent already set today.** `file-provenance.py`
   had the same shape and was fixed by *surfacing the caveat in the output*
   (`568cd7d2`, bug archived `a328c8bb`). Add to `audit_log`'s `note`: that a
   `librarian.reindex` row with an `embedded_sha256`-only payload is contact, not
   authorship, and that `actor` answers "whose connection", not "who wrote".
2. **Stronger, more invasive.** Give the response a derived per-row field — e.g.
   `content_write: true|false` from the payload keys — so the discriminator is a
   field rather than a thing the reader must reconstruct. This is the version that
   survives a reader who never opens the `note`.

Direction 1 is what a session could ship today; direction 2 is what makes the
instrument answer the question it is actually asked.

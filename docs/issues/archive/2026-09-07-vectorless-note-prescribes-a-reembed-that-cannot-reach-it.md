---
id: 4cc472b73906ecf8
kind: bug
status: fixed
title: 'BUG: `reindex`''s `vectorless_note` prescribes `reembed=true`, which cannot escape the state it describes'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
topic: absorbing vectorless state remedy
closed: 2026-09-07
opened: 2026-09-07
owner: marius
related: []
severity: medium
---

# BUG: `reindex`'s `vectorless_note` prescribes `reembed=true`, which cannot escape the state it describes

## Summary

`librarian(action="reindex")` detects artifacts in the absorbing stamped-seen-but-unembedded
state and tells you to escape it with `librarian(action="reindex", reembed=true)`. That command
re-embeds **chunks**, and these artifacts have **no chunk rows at all** — so it walks the entire
corpus, reports success, and changes nothing. The working escape is `codescout backfill-chunks`,
which shipped 14 minutes after the note and was never named in it.

## Symptom (Effect)

Run 1, ordinary reindex — detection is correct:

```json
{"added": 47, "updated": 62, "removed": 17, "unchanged": 1401, "embedded": 28850,
 "vectorless": 10,
 "vectorless_note": "10 artifact(s) under the indexed root(s) have NO searchable representation,
  and no ORDINARY reindex will give them one: their content is stamped as seen while unembedded,
  which is an ABSORBING state. Escape it with librarian(action=\"reindex\", reembed=true)."}
```

Run 2 — following that instruction exactly, to completion (~8 minutes):

```json
{"added": 0, "updated": 4, "removed": 0, "unchanged": 1508, "embedded": 28881,
 "vectorless": 10,
 "embed_error_count": 0, "embed_errors": [],
 "vectorless_note": "... Escape it with librarian(action=\"reindex\", reembed=true)."}
```

**`vectorless` is unchanged at 10.** The whole corpus was re-embedded (`embedded: 28881`), no
errors were raised, and the note repeats the same instruction. There is no signal distinguishing
this from a run that had nothing to do.

Then `codescout backfill-chunks`:

```json
{"artifacts": 2807, "embedded": 61613, "skipped_empty": 0, "missing_file": 1}
```

after which the same query the reindex uses returns `0` for this root.

## Reproduction

```
git rev-parse HEAD          # 4b30601c at filing
```

1. `librarian(action="reindex")` on a project holding at least one artifact with no
   `artifact_chunk` rows. Note `vectorless: N` and the note.
2. `librarian(action="reindex", reembed=true)`. Wait for it.
3. Note `vectorless: N`, unchanged.

Observed live 2026-09-07, both runs end to end.

## Environment

Linux (`ripper`), `experiments` @ `4b30601c`, `cargo rb` binary
(`server-stack,local-embed`), remote embedder (`tei-embed`, healthy).

## Root cause

`vectorless` counts artifacts with **no chunk rows**:

```sql
SELECT COUNT(*) FROM artifact a
WHERE a.abs_path LIKE ?1
  AND NOT EXISTS (SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id)
```

`reembed=true` requeues *existing* chunks for embedding. Its object is the chunk row; the
defect is the **absence** of chunk rows. The two never meet, so the flag is a no-op over this
population by construction rather than by accident — and it is a no-op that costs a full-corpus
embed pass.

`backfill_chunk_vectors` (`src/librarian/indexer.rs:1079`) says so in its own doc comment:

> **Why it bypasses the indexer rather than calling it.** The artifacts this exists for are
> exactly the ones the indexer *declines to process* … A backfill routed through the normal walk
> would inherit that gate and report success having done nothing. This one consults
> `content_unchanged` NOWHERE.

measured 2026-09-07: the two reindex runs quoted above, plus the SQL run directly against
`~/.local/share/librarian/catalog.db` before and after the backfill (10 → 0 for this root).

## Evidence

### The note predates its own correction by fourteen minutes

```
98eb5adc  2026-09-02 22:23:49 +0300  feat(librarian): count the artifacts no reindex can ever embed
488192e8  2026-09-02 22:37:48 +0300  feat(librarian): a resumable chunk backfill that escapes the
                                     indexer's absorbing state
```

`git merge-base --is-ancestor 98eb5adc 488192e8` → true. The note named the only lever that
existed when it was written. The real escape shipped a quarter of an hour later, in a commit whose
subject line *is* the note's own phrase — "escapes the indexer's absorbing state" — and the prose
was never updated. Same author, same evening, same work stream.

### Nothing tests the remedy

The detection has a test (`vectorless == Some(1)`, inverted 2026-09-04 as fix (b) of the
stamp-before-embed bug). The *string telling you what to do about it* has none, and could not
easily have one that would fail here — an assertion that the note mentions `reembed` passes.

## Hypotheses tried

1. **Hypothesis:** the reembed run failed silently and the count is stale.
   **Test:** read `embed_error_count`, `embed_errors`, `embed_note` from run 2.
   **Verdict:** rejected — `0`, `[]`, `"28881 embedded"`. The pass ran and succeeded; it simply
   had no effect on this population.
2. **Hypothesis:** the count is wrong / the query is broken.
   **Test:** ran the production SQL directly, scoped and unscoped, plus totals as a control.
   **Verdict:** rejected — scoped `0`, catalog-wide `1`, `artifact` 4710, `artifact_chunk` 99226.
   A broken query would return 0 for both scopes; it does not.

## Fix

Change the note to name `codescout backfill-chunks`, and say why it is a CLI rather than an MCP
action (it holds the catalog lock for the whole run — `src/cli/backfill_chunks.rs:12-17`).

Text along the lines of: *"…which is an ABSORBING state: these artifacts have no chunk rows, so
`reembed=true` has nothing to re-embed and will not reach them. Escape it from a shell with
`codescout backfill-chunks`."*

Note the sibling defect this exposes: `backfill-chunks` is catalog-wide, so the note should say
so — see `docs/issues/archive/2026-09-07-backfill-chunks-walks-the-whole-catalog-not-the-project.md`.

- **SHA (experiments):** `45eac50e`
- **patch-id:** `0f70f33bbc19f0a95ecb08a1966592e63a9b05d0`

**Applied.** The note now names `codescout backfill-chunks`, says explicitly that `reembed=true`
does *not* reach this population and why (it requeues existing chunk rows; these artifacts have
none), and states the CLI's scope — `--project` by default, `--all` for the catalog.

**The test was part of the defect and is fixed with it.** It asserted
`note.contains("reembed=true")` under the comment *"the note must name the ESCAPE, not only the
condition"*. The instinct was right and the value was wrong, so it passed throughout while the
note sent every reader into an 8-minute no-op. It now asserts in **both** directions — that the
note names `backfill-chunks`, and that it does *not* prescribe `reembed` as the escape — because
either assertion alone is satisfiable by the wrong text. Pinning the whole sentence would red on
every rewording and is deliberately avoided; per `CLAUDE.md` § *Testing Discipline* this buys
arrival, not correctness, and arrival is what was missing.

Mutation run: restoring the old remedy text reds the test, and the failure message prints the
whole note so the next reader sees which sentence is wrong rather than a boolean.

## Tests added

None yet. A shape assertion is cheap and would red on exactly the regression that happened:
assert the note names `backfill-chunks` and does **not** name `reembed` as the escape. Per
`CLAUDE.md` § *Testing Discipline*, that buys arrival and not correctness — but arrival is what
was missing, and the pinned-prose objection does not apply to a two-token check.

## Workarounds

Ignore the note. Run `codescout backfill-chunks` from a shell. Verify with the production SQL
rather than another 8-minute reindex:

```sql
SELECT COUNT(*) FROM artifact a WHERE a.abs_path LIKE '<root>%'
  AND NOT EXISTS (SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id);
```

## Resume

Edit the `vectorless_note` string in `src/librarian/tools/reindex.rs` (added by `98eb5adc`) to
name `codescout backfill-chunks`. Then add the shape assertion described in § *Tests added* and
confirm it reds against the current string before it greens against the new one.

## References

- `src/librarian/indexer.rs:1079` `backfill_chunk_vectors` — the doc comment that contradicts the note
- `src/cli/backfill_chunks.rs` — why it is a CLI
- `docs/issues/archive/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` — the state itself, archived as fixed; the fix stops new entries and cannot heal existing rows
- `CLAUDE.md` § *Testing Discipline* — "a suite tests a guard's PREDICATE and never its REMEDY TEXT"

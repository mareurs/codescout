---
id: 6ee86ac5b140576f
kind: bug
status: fixed
title: doc(update) stamps the content hash without rebuilding chunks, so every later reindex correctly skips the file
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-06
opened: 2026-09-04
owner: marius
related:
- '6ae552cfc223cd6d'
severity: high
unverified: 'No regression test guards THIS file''s entrance into the trap state. fdad1a99''s guard (index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it) enters via a non-embedding RUN; doc(update) enters via a STAMP. Both produce file_sha256==disk && embedded_sha256!=disk so the same escape releases both, but that is an argument and only one entrance is observed. The 2026-09-06 reproduction covers the other, and a reproduction is not a guard: making update.rs also stamp embedded_sha256 would restore this bug with the suite green.'
---

# BUG: `doc(action="update")` stamps the content hash without rebuilding chunks, so every later reindex correctly skips the file forever

## Summary

> **FIXED at `fdad1a99` (patch-id `d7c4618ce3ffb77079df92b500d62f9649bfb979`, `experiments`).**
> Read this paragraph with care: its *mechanism* still verifies today and its *prognosis* does
> not. `doc(update)` really does stamp the hash and leave `artifact_chunk` stale — but an
> ordinary `reindex` now repairs it, because the embed decision moved off `file_sha256`.
> The sentence below beginning "The desync is permanent" is the **false** half, refuted by the
> reproduction in § *Fix*. Kept verbatim rather than edited, because a report whose every
> individual claim checks out and whose conclusion is wrong is the thing worth being able to
> recognise later.

`doc(action="update")` writes a new body to disk **and** stamps the matching `file_sha256` into
the artifact row, without touching `artifact_chunk`. The chunk rows — line ranges,
`entry_token`, `entry_part`, and the vectors keyed to them — keep describing the *previous*
body. Every subsequent `librarian(action="reindex")` then compares hashes, correctly concludes
the content is unchanged, and skips the file. The desync is **permanent and self-sealing**: the
only escape is `reembed=true`, which nothing schedules and no signal requests.

Affected: any artifact edited through `doc(update)` — which is the prescribed edit path for
every guarded tracker, and therefore for most of `docs/trackers/`.
## Symptom (Effect)

No error, no warning, no counter. The observable is a silent disagreement between two catalog states. Measured 2026-09-04 03:47 on `docs/trackers/retrieval-benchmark.md` (id `cc4843e5c1a020bd`), 4 minutes after a `doc(action="update")` that inserted ~64 lines:

```
file on disk     : 1559 lines,  sha256 e05f1d6e936a574b4f7e0cd3bcced7cd26475f617327e88a19d996ba550df0ee
catalog row      :              sha256 e05f1d6e936a574b4f7e0cd3bcced7cd26475f617327e88a19d996ba550df0ee
artifact_chunk   : chunk_count=82   max_end_line=1491
```

The hashes are byte-identical — the row asserts it has seen exactly this content — while its chunks stop **68 lines short of the file**. The inserted text is invisible to chunk-grain retrieval, and there is no field anywhere that says so.

Downstream, this presents three different ways, none of which names the cause:

1. **Wrong line ranges.** A published `matched.start_line` precedes the heading whose token the chunk carries, by a per-file constant. That is the open bug `docs/issues/archive/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md` (`c77fb370f61fc309`), whose remaining unexplained residue this accounts for.
2. **Wrong entry attribution.** A chunk keeps the `entry_token` of whatever entry occupied that position in the old body, so any consumer re-deriving the entry from `(path, line)` resolves to the wrong one.
3. **Stale vectors.** New text is never embedded, so an edited artifact is unfindable by a query quoting its own new content.

## Reproduction

Minimal, deterministic, ~2 minutes. Ran on `experiments` at `ceab2662`, release binary built 03:07:40.

```
1. Pick any catalogued markdown artifact with multiple chunks.
2. doc(action="update", id=<id>, patch={body_edits: [{heading: "## Something",
        action: "insert_after", at: "after-heading-line", content: "<~60 new lines>"}]})
3. Compare, and note they now disagree:
     wc -l <file>
     sqlite3 catalog.db "select max(end_line) from artifact_chunk where artifact_id='<id>'"
4. librarian(action="reindex")            # NOT reembed
5. Re-run step 3. Nothing has moved.
```

Observed at step 4: `{"added": 1, "updated": 1, "unchanged": 1474, ...}` — the edited artifact is in the `unchanged` bucket. Observed at step 5: `chunk_count=82  max_end_line=1491`, byte-identical to step 3, against a 1,559-line file.

**`force=true` does not repair it either**, and this is the part that misleads. It bypasses the unchanged-*row* skip so metadata is re-derived, but it never reaches `replace_chunks` — see Root cause. Only `reembed=true` escapes.

## Environment

Linux, `experiments` @ `ceab2662`, release build 2026-09-04 03:07:40, MCP over stdio, catalog `~/.local/share/librarian/catalog.db` schema v12, sqlite-vec backend, embeddings on (`127.0.0.1:48081`).

## Root cause

Three facts compose into a one-way door. Every one read at the bytes on 2026-09-04, and the composition then measured end to end (Reproduction above), not inferred.

1. **`replace_chunks` has exactly ONE production caller.** `references(symbol="replace_chunks", path="src/librarian/catalog/chunk.rs")` returns 25 sites across 5 files; the only non-test one is `src/librarian/indexer.rs:184`, inside `embed_queue_items`. Rebuilding chunks is therefore reachable *only* through the embed-queue path.

2. **That path is gated on content change.** `src/librarian/indexer.rs:395` — `if !force_rewalk && content_unchanged && meta_unchanged { … continue }` — with the embed branch inside it at `:409` guarded by `if want_embeddings && force_embed`. So a content-unchanged artifact reaches `embed_queue_items` **only** when `force_embed` is set, which is what `reembed=true` sets. `force_rewalk` falls *through* the early return but does not call the embed branch, which is why `force=true` rebuilds nothing.

3. **`doc(update)` makes the content look unchanged.** `src/librarian/tools/update.rs:633` writes the new body; `:661` stamps `file_sha256: sha_of_bytes(new_content.as_bytes())`; `:664` upserts the row. The file contains **zero** references to `indexer`, `embed` or `chunk`.

So the writer updates the *sentinel* for a state it does not update. The reindex's freshness gate asks "have the chunks gone stale?", cannot observe that, and substitutes the proxy "has the file hash changed?" — a proxy this writer defeats by construction, silently, returning a plausible `unchanged` rather than an error.

**The asymmetry is the diagnostic, and it inverts the intuition.** `append_entry` writes through `catalog/augmentation.rs` and does **not** stamp `file_sha256` (its only occurrence there, `:1686`, is a test fixture). Its edits therefore look changed to the next reindex and **self-heal**. The conscientious path — stamping the hash you just wrote, which is obviously-correct bookkeeping in isolation — is the one that breaks; the path that forgets recovers.

## Evidence

### The natural experiment (2026-09-04 03:40)

`librarian(reindex, reembed=true, scope="project")` reindexed codescout and nothing else, leaving the catalog holding a treated group and an untreated control. Probe: [`scripts/probe-chunk-coord-drift.py`](../../scripts/probe-chunk-coord-drift.py) and [`scripts/probe-chunk-drift-by-root.py`](../../scripts/probe-chunk-drift-by-root.py), promoted out of scratch space and committed so this evidence stays reproducible; counting rule = *published `start_line` < the line of the heading that DEFINES that chunk's own token*:

```
codescout    drift    0 of 2940 resolvable (0.00%)   across 0 files
OTHER-REPOS  drift  143 of  632 resolvable (22.63%)  across 10 files

docs/trackers/bug-fix-session-log.md      resolvable=150  drift=0
docs/trackers/open-issue-work-queue.md    resolvable= 98  drift=0
```

The two files `c77fb370f61fc309` named at −2 and −1 are reported **positively** — 150 and 98 resolvable chunks, zero drift each — rather than by absence from a truncated list. A single `reembed=true` took the treated root to exactly zero.

**Three defensible numbers, none interchangeable:** 143/632 = 22.63% *in non-reindexed repos*; 0/2940 *in codescout*; 143/3572 = 4.00% *corpus-wide*. Quote the population or quote nothing.

### The live reproduction (2026-09-04 03:47)

Quoted verbatim under *Symptom* above. Note the recursion: the 68 invisible lines are the benchmark section documenting this very defect, so the record of the bug was itself unindexed by the bug.


### The defect re-accumulates, measured five hours later (2026-09-04 08:48)

The treated root did not stay repaired. Same probe, same rule, no intervening reindex:

```
codescout    drift   16 of 2940 resolvable (0.54%)   across 1 file
  docs/trackers/observer-blindness.md    CONSTANT {-1: 16}
OTHER-REPOS  drift  143 of  632 resolvable (22.63%)  across 10 files   [unchanged]
```

So `reembed=true` is a **repair with a half-life**, not a fix: five hours of ordinary tracker editing through `doc(action="update")` put 16 chunks of one file back into the defect, at a per-file constant of `-1`, while the untreated roots sat exactly still. That the untreated figure is byte-identical across five hours is the control — it rules out probe drift and corpus churn, leaving the treated root's regression attributable to writes.

**Do not cite a stored figure from this file; re-run the probe.** A number from this defect is valid only at its instant, and the direction of decay is always upward.

**The file is `docs/trackers/observer-blindness.md`** — the tracker cataloguing defect classes the right party structurally cannot see, made unobservable to its own readers by the mechanism it catalogues. Every `matched.start_line` it publishes now resolves one line early, so a consumer re-deriving the entry from `(path, line)` lands on the preceding `OB-N`. Nothing reports this; the file reads correctly on disk and wrongly through retrieval. Cause not attributed — several sessions wrote to it in that window, including this one at `edc0087f` — and attributing it is not needed for the finding, which is the **rate**, not the author.
## Hypotheses tried

1. **Hypothesis:** the frontmatter height is mis-measured, so `line_offset` is short by a constant.
   **Test:** read `frontmatter::body_line_offset` (`src/librarian/frontmatter.rs:118-124`) and its four tests.
   **Verdict:** **rejected.** It computes `doc[..doc.len()-body.len()].lines().count()` after an `ends_with` guard, so the offset is the prefix's own line count by construction and cannot be short by 1–2. A non-suffix pair returns `0`, never a wrong non-zero.

2. **Hypothesis:** the chunk rows are stale relative to the file.
   **Test:** the natural experiment plus the live reproduction, both above.
   **Verdict:** **confirmed.** 0.00% drift in the reindexed root; the un-reindexed roots unchanged at 22.63%.

3. **Hypothesis (recorded in `c77fb370f61fc309`, and the reason hypothesis 2 was struck off for four days):** staleness is excluded, because "a forced re-walk of all 1,471 artifacts left it in place, and got worse".
   **Test:** re-read what `force=true` actually does — `src/librarian/indexer.rs:395`, `:409`.
   **Verdict:** **the refutation was invalid, not merely wrong.** `force_rewalk` never reaches `replace_chunks`, so that re-walk rebuilt **zero** chunks. It was an instrument that could not express the hypothesis it was aimed at, and it returned a plausible "still there" rather than an error. *Before citing a re-run as a refutation, name the write the re-run was supposed to perform, and check that it performed it.*

4. **Hypothesis:** the per-file delta *shape* discriminates staleness (CONSTANT) from arithmetic (MIXED).
   **Test:** read the shapes in the untreated group, which is known-pure staleness.
   **Verdict:** **rejected — my own heuristic, corrected same session.** Insertions at several points in one file shift entries below each one by different amounts, so staleness produces `MIXED` too. The treated/untreated split discriminates; the delta shape does not.

## Fix

**Fixed — but not where this file was looking, and not by a change to `doc(update)` at all.**
`fdad1a99` ("gate the embed on a stamp the embedder writes, not on `file_sha256`"), landed
2026-09-04, the same day this was filed. Patch-id recorded below.

Everything this file says about the *update* path is still true and was re-verified 2026-09-06:
`src/librarian/tools/update.rs:661` stamps `file_sha256` from the new body, and a grep of that
file for `chunk|reembed|embed_queue` returns **0 matches**. `doc(update)` still writes a body
and leaves `artifact_chunk` describing the previous one.

What changed is the claim that the desync is **permanent and self-sealing**. It is not, because
the reindex decision no longer reads `file_sha256`:

```rust
// src/librarian/indexer.rs:440
let needs_embed = if want_embeddings {
    artifact::embedded_sha256(cat, &id)?.as_deref() != Some(sha.as_str())
} else { false };
```

`doc(update)` stamps `file_sha256` and **never touches `embedded_sha256`**, so after an update
`content_unchanged` is true *and* `needs_embed` is true. The unchanged-row early return
(`indexer.rs:446`) therefore takes its escape at `:465` and calls `embed_queue_items`, whose doc
comment states the part that closes this bug:

> Writes the artifact's `artifact_chunk` rows as a side effect, because the chunk ids the queue
> is keyed on are assigned there — the queue and the rows cannot be built independently without
> the two disagreeing.

So an **ordinary** reindex rebuilds the rows. `reembed=true` was never required.

### Reproduced end-to-end, 2026-09-06

Run against the live catalog rather than reasoned from the code, because this file's central
claim was about a *sequence*, and only a sequence can refute it. Artifact
`863fb5cf6bf011ef`:

| stage | `file_sha256` | `embedded_sha256` | chunks | `MAX(end_line)` | content bytes | file lines |
|---|---|---|---|---|---|---|
| before                    | `4fd916de6333` | `39031c35d395` | 17 | 145 | 5627 | 145 |
| after `doc(update)`       | `33a94169ff6b` | `39031c35d395` | 17 | **145** | **5627** | **149** |
| after ordinary `reindex`  | `33a94169ff6b` | `33a94169ff6b` | 17 | **149** | **5927** | 149 |

Row 2 is this bug, reproduced exactly: the hash matches disk while the chunks describe a
145-line body that is now 149 lines. Row 3 is the refutation of "permanent": no `reembed`, no
force lever, just `librarian(action="reindex")`.

### The residual, and why it is not worth a fix

`needs_embed` is `false` when `want_embeddings` is false (`reindex.rs:356`,
`ctx.embedding.is_some()`), so on a deployment with **no embedder** the escape is not taken and
the rows stay stale. That is a real gap and it is benign in both directions: on such a
deployment `semantic_find` has no vectors to return, so the stale `start_line` / `entry_token`
rows have no consumer; and the moment an embedder is configured, an ordinary run re-queues the
content, which is exactly what
`index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it`
(`src/librarian/indexer.rs:2556`) pins. Filed as no further work rather than left implied.

**SHA:** `fdad1a99` on `experiments`. **patch-id:** see § *Tests added*.
## Tests added

**Fix identifiers:** `fdad1a99` on `experiments`, patch-id
`d7c4618ce3ffb77079df92b500d62f9649bfb979`.

The regression guard that closes this file is
`index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it`
(`src/librarian/indexer.rs:2556`), and its own header is worth reading before adding anything
beside it. It pins the **sequence**, not either run's outcome: run A indexes with no embedder
and commits `file_sha256`; run B has an embedder and both force levers `false` — exactly how
`index_repo` calls it — and must still queue. Its comment records why the pre-existing tests
were no evidence here: they exercised `force_embed` / `force_rewalk`, *"the escape hatch — and
they are monotone under the defect, because the trap state is precisely the state in which
`force_embed` still works."*

**No test was added for the `doc(update)` → `reindex` sequence specifically**, and that is the
honest gap in this archive. `fdad1a99` was written against the indexer, so its guard enters the
trap state through a non-embedding *run*; this file's route in is a `doc(update)` **stamp**.
Both produce `file_sha256 == disk && embedded_sha256 != disk`, so the same escape releases
both — but that is an argument, and the guard only observes one of the two entrances. The
2026-09-06 reproduction in § *Fix* is what covers the other one, and a reproduction is not a
regression test: nothing reds if someone later makes `update` stamp `embedded_sha256` too,
which would restore this bug exactly.

**If you are here to harden it, that is the test to write** — assert that after a
`doc(action="update")` an ordinary `reindex` moves the artifact's `MAX(end_line)`, and mutate
`update.rs` to also stamp `embedded_sha256` to confirm it reds. Cheap, and it guards the
entrance this file actually documents.
## Workarounds

`librarian(action="reindex", reembed=true)` repairs the whole corpus and is the only thing that does. It is expensive (~7 min here, full write-lock hold) so it is a repair, not a habit. `force=true` looks like it should work and does not.

Nothing repairs a single artifact today.

## Resume

N/A — fixed and archived. One optional piece of work is named at the end of § *Tests added*
(a regression test for this file's own entrance into the trap state); it is a hardening, not
an outstanding defect.

**How this file came to be wrong, since it is the third instance in one sweep.** It was filed
2026-09-04 and `fdad1a99` landed 2026-09-04 — the fix and the report crossed. The report's
mechanism was *right* and its prognosis was *wrong*, which is the harder shape to catch: every
line about `update.rs` still verifies today, so re-reading the file confirms it. Only running
the sequence refutes it, and this repo already has the rule that would have caught it —
*"Run the reproduction before reading the fix plan — the plan is a hypothesis about the
reproduction."*

That rule earned its place again here: reading the code alone was **not enough**, because the
escape at `indexer.rs:465` is inside the `content_unchanged` early return, which is the last
place you look when the claim is *"reindex correctly skips the file"*. The reproduction found
it in three SQL queries.
## References

- `docs/issues/archive/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md` (`c77fb370f61fc309`) — the symptom this explains; its hypothesis-3 refutation is invalidated here.
- `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` (`a766aad35b0b7610`) — the sibling absorbing state, same `reembed=true` escape, different write.
- `docs/issues/archive/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md` (`6ae552cfc223cd6d`) — why the prescribed workaround is expensive.
- `docs/trackers/retrieval-benchmark.md` § *2026-09-04 (dawn)* — the natural experiment and its numbers.
- `src/librarian/indexer.rs:184`, `:395`, `:409`; `src/librarian/tools/update.rs:633`, `:661`, `:664`; `src/librarian/frontmatter.rs:118`.

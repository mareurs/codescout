---
id: '921a192357e54bad'
kind: bug
status: open
title: doc(update) stamps the content hash without rebuilding chunks, so every later reindex correctly skips the file
tags:
- cluster/gate-keyed-on-unobservable-event
closed: ''
opened: 2026-09-04
owner: marius
related:
- '7695ad877b44e96a'
- a766aad35b0b7610
- '823d9ccaa13e2def'
severity: high
---

# BUG: `doc(action="update")` stamps the content hash without rebuilding chunks, so every later reindex correctly skips the file forever

## Summary

`doc(action="update")` writes a new body to disk **and** stamps the matching `file_sha256` into the artifact row, without touching `artifact_chunk`. The chunk rows — line ranges, `entry_token`, `entry_part`, and the vectors keyed to them — keep describing the *previous* body. Every subsequent `librarian(action="reindex")` then compares hashes, correctly concludes the content is unchanged, and skips the file. The desync is **permanent and self-sealing**: the only escape is `reembed=true`, which nothing schedules and no signal requests.

Affected: any artifact edited through `doc(update)` — which is the prescribed edit path for every guarded tracker, and therefore for most of `docs/trackers/`.

## Symptom (Effect)

No error, no warning, no counter. The observable is a silent disagreement between two catalog states. Measured 2026-09-04 03:47 on `docs/trackers/retrieval-benchmark.md` (id `cc4843e5c1a020bd`), 4 minutes after a `doc(action="update")` that inserted ~64 lines:

```
file on disk     : 1559 lines,  sha256 e05f1d6e936a574b4f7e0cd3bcced7cd26475f617327e88a19d996ba550df0ee
catalog row      :              sha256 e05f1d6e936a574b4f7e0cd3bcced7cd26475f617327e88a19d996ba550df0ee
artifact_chunk   : chunk_count=82   max_end_line=1491
```

The hashes are byte-identical — the row asserts it has seen exactly this content — while its chunks stop **68 lines short of the file**. The inserted text is invisible to chunk-grain retrieval, and there is no field anywhere that says so.

Downstream, this presents three different ways, none of which names the cause:

1. **Wrong line ranges.** A published `matched.start_line` precedes the heading whose token the chunk carries, by a per-file constant. That is the open bug `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md` (`7695ad877b44e96a`), whose remaining unexplained residue this accounts for.
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

`librarian(reindex, reembed=true, scope="project")` reindexed codescout and nothing else, leaving the catalog holding a treated group and an untreated control. Probe: `chunk-coord-drift.py` / `drift-by-root.py`, counting rule = *published `start_line` < the line of the heading that DEFINES that chunk's own token*:

```
codescout    drift    0 of 2940 resolvable (0.00%)   across 0 files
OTHER-REPOS  drift  143 of  632 resolvable (22.63%)  across 10 files

docs/trackers/bug-fix-session-log.md      resolvable=150  drift=0
docs/trackers/open-issue-work-queue.md    resolvable= 98  drift=0
```

The two files `7695ad877b44e96a` named at −2 and −1 are reported **positively** — 150 and 98 resolvable chunks, zero drift each — rather than by absence from a truncated list. A single `reembed=true` took the treated root to exactly zero.

**Three defensible numbers, none interchangeable:** 143/632 = 22.63% *in non-reindexed repos*; 0/2940 *in codescout*; 143/3572 = 4.00% *corpus-wide*. Quote the population or quote nothing.

### The live reproduction (2026-09-04 03:47)

Quoted verbatim under *Symptom* above. Note the recursion: the 68 invisible lines are the benchmark section documenting this very defect, so the record of the bug was itself unindexed by the bug.

## Hypotheses tried

1. **Hypothesis:** the frontmatter height is mis-measured, so `line_offset` is short by a constant.
   **Test:** read `frontmatter::body_line_offset` (`src/librarian/frontmatter.rs:118-124`) and its four tests.
   **Verdict:** **rejected.** It computes `doc[..doc.len()-body.len()].lines().count()` after an `ends_with` guard, so the offset is the prefix's own line count by construction and cannot be short by 1–2. A non-suffix pair returns `0`, never a wrong non-zero.

2. **Hypothesis:** the chunk rows are stale relative to the file.
   **Test:** the natural experiment plus the live reproduction, both above.
   **Verdict:** **confirmed.** 0.00% drift in the reindexed root; the un-reindexed roots unchanged at 22.63%.

3. **Hypothesis (recorded in `7695ad877b44e96a`, and the reason hypothesis 2 was struck off for four days):** staleness is excluded, because "a forced re-walk of all 1,471 artifacts left it in place, and got worse".
   **Test:** re-read what `force=true` actually does — `src/librarian/indexer.rs:395`, `:409`.
   **Verdict:** **the refutation was invalid, not merely wrong.** `force_rewalk` never reaches `replace_chunks`, so that re-walk rebuilt **zero** chunks. It was an instrument that could not express the hypothesis it was aimed at, and it returned a plausible "still there" rather than an error. *Before citing a re-run as a refutation, name the write the re-run was supposed to perform, and check that it performed it.*

4. **Hypothesis:** the per-file delta *shape* discriminates staleness (CONSTANT) from arithmetic (MIXED).
   **Test:** read the shapes in the untreated group, which is known-pure staleness.
   **Verdict:** **rejected — my own heuristic, corrected same session.** Insertions at several points in one file shift entries below each one by different amounts, so staleness produces `MIXED` too. The treated/untreated split discriminates; the delta shape does not.

## Fix

Not implemented. Three options, in ascending cost, all in mechanism terms:

- **A — make the writer honest.** Have `doc(update)` call `embed_queue_items` (or a narrower `rechunk_artifact`) after `std::fs::write` at `src/librarian/tools/update.rs:633`. Correct at the source, but drags an embed round-trip into what is currently a fast local write, and `update` has no embedder handle today.
- **B — do not stamp what you did not do.** Drop the `file_sha256` stamp at `:661` and let the next reindex notice, matching `append_entry`'s accidental-but-correct behaviour. One line, and it converts a permanent desync into ordinary index lag. Costs one re-chunk per edited artifact per reindex, which is what the design already pays everywhere else.
- **C — stop keying the gate on a defeatable proxy.** Give `artifact_chunk` its own content hash and compare *that* in the `:395` early return, so the gate observes the state it guards instead of a correlate. Strictly the most correct, and the only one that also catches a future third writer.

**B is the recommended first move** — smallest diff, no new state, and it removes the one-way door rather than adding a second mechanism to compensate for it. C is worth filing as the follow-up.

**Do not "fix" this by scheduling periodic `reembed=true`.** That is a full re-embed of the corpus (28,140 chunks, ~7 minutes, holding the project write lock throughout — see `823d9ccaa13e2def`) standing in for a one-line write. It also fails silently the moment someone stops running it.

## Tests added

**None yet** — the fix is not written, and this is a gap rather than an omission. The regression test the fix owes is behavioural and cheap: seed an artifact, `doc(update)` its body with an insertion, run `index_repo_sync` with `force_embed=false`, and assert the artifact's chunk rows reflect the **new** body. It must be observed RED first: today it returns chunks from the old body with no error, which is exactly the shape a passing test would also produce if the assertion were written against the row's hash rather than against the chunks.

Note what a test asserting `content_unchanged == false` would prove: nothing. That is a claim about the sentinel, and the sentinel is the thing that lies. Assert on `max(end_line)` or on chunk content — the state the user actually reads.

## Workarounds

`librarian(action="reindex", reembed=true)` repairs the whole corpus and is the only thing that does. It is expensive (~7 min here, full write-lock hold) so it is a repair, not a habit. `force=true` looks like it should work and does not.

Nothing repairs a single artifact today.

## Resume

Implement option **B**: delete the `file_sha256` stamp at `src/librarian/tools/update.rs:661` and let the row carry the *pre-edit* hash, so the next `reindex` sees a content change and re-chunks. Then write the RED-first regression test described under *Tests added* — assert on `max(end_line)` of `artifact_chunk`, never on `content_unchanged`. Check `update.rs`'s second write site at `:823` (`trim_history`'s path) for the same defect before claiming the fix is complete; it writes the file and was not read in this pass.

Then re-run `drift-by-root.py` against a non-codescout root to confirm the corpus-wide figure moves for a reason other than a manual `reembed`.

## References

- `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md` (`7695ad877b44e96a`) — the symptom this explains; its hypothesis-3 refutation is invalidated here.
- `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` (`a766aad35b0b7610`) — the sibling absorbing state, same `reembed=true` escape, different write.
- `docs/issues/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md` (`823d9ccaa13e2def`) — why the prescribed workaround is expensive.
- `docs/trackers/retrieval-benchmark.md` § *2026-09-04 (dawn)* — the natural experiment and its numbers.
- `src/librarian/indexer.rs:184`, `:395`, `:409`; `src/librarian/tools/update.rs:633`, `:661`, `:664`; `src/librarian/frontmatter.rs:118`.


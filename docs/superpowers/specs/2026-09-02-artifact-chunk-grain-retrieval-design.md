---
kind: spec
status: active
title: Chunk-grain retrieval for librarian artifacts
owners: []
tags:
  - librarian
  - embeddings
  - semantic-search
  - retrieval-grain
---

# Design — chunk-grain retrieval for librarian artifacts

Addresses `docs/issues/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md`
(artifact `7a37f1179d2f0e21`, severity high).

## Decisions taken

| # | Decision | Chosen |
|---|---|---|
| D1 | What a semantic hit returns | **The entry** — chunk-grain hits with file, line range and matched text |
| D2 | Approach | **Port** the code index's chunk design into the catalog, not a fresh build |
| D3 | `chunk_id` derivation | **Opaque surrogate**, not `<artifact_id>#<ix>` |
| D4 | Chunk budget | **Unchanged at 512 tokens / 2,048 chars** — an earlier revision proposed raising it and that was retracted |
| D5 | Grain conflict between consumers | One parameter, `max_per_artifact` — `context` passes 1, `find` passes ~3 |
| D6 | Migration shape | Build `artifact_vec_v2` alongside, swap transactionally — no dark window |
| D7 | Ranking measurement | **In scope** — an artifact-TC benchmark suite ships with the work |
| D8 | The 31% coverage hole | **Prerequisite**, diagnosed before or alongside migration |

## Problem

### What is broken

`src/librarian/indexer.rs:67` builds the embed queue from
`chunk_markdown(body, 512).into_iter().next()` — the first chunk and nothing
else. `chunk_markdown` forces a section break at every heading, so chunk 0 of a
tracker is its H1 plus the preamble. The 189-entry bug-fix session log is
represented by one 768-dim vector encoding its title and scope note.

The binding constraint is the schema: `artifact_vec` is declared
`vec0(id TEXT PRIMARY KEY, embedding FLOAT[768])` (`schema.sql:49-52`), which
permits one vector per artifact **if `id` denotes an artifact**.

### What is NOT broken — a correction to the bug file's original framing

codescout runs **two** vector indexes over the same markdown:

| | `artifact_vec` | `code_chunk` / `code_vec` |
|---|---|---|
| database | `~/.local/share/librarian/catalog.db` | `.codescout/embeddings/<project>.db` |
| grain | 1 vector per artifact | line-anchored chunks |
| markdown held | 1,363 files, **≤1 vector each by construction** | 1,363 files, **33,032 chunks** |
| `bug-fix-session-log.md` | **1** vector | **809** chunks, lines 1–9648 |
| scope | 10 repos, 4,525 artifacts | one project, 1,872 files |
| served to | `artifact(find, semantic=)`, `librarian(context, topic=)` | `semantic_search` |

`semantic_search` is **not affected**. Probed 2026-09-02 by running both paths
against a phrase at line 7814 of `docs/trackers/bug-fix-session-log.md`:
`semantic_search(mode="full")` returned it ranked 2nd and 3rd;
`artifact(find, semantic=)` did not return the file at all.

The affected consumers are exactly the two readers of `artifact_vec`.
`src/librarian/catalog/find.rs:299` (`semantic_find`) is its only production
caller, reached from `artifact(find, semantic=)` and from
`src/librarian/tools/context.rs:679`.

This is why D2 is *port* rather than *build*: chunk-grain markdown retrieval
already exists in this binary, with its ranking and dedup semantics worked out.

### Retraction — the chunk budget is not a defect

An intermediate revision of the bug file called the `512` literal "4× too small"
because `chunk_size_for_model("CodeRankEmbed")` returns 2048. **That reasoning
was wrong.** The model's figure is a *ceiling*; this project deliberately chunks
well below it:

- `AST_CHUNK_TARGET = 3000` (`src/embed/ast_chunker.rs:953`) caps every
  code-index chunk — *"smaller chunks produce sharper embeddings for retrieval
  regardless of file type"*.
- `STACK_CHUNK_TARGET = 1200` is *"benchmark-backed, and under every local
  model's real ceiling"*.
- A since-deleted `DEFAULT_CAP = 4096` existed because large-context models
  *"would otherwise default to ~20k chars per chunk, which both slows indexing
  and dilutes ranking signal"*.

At 512 tokens the librarian chunks at 2,048 chars — inside that deliberate
1,200–3,000 window.

The error's precedent is in the same archived record that settled the decision:
`docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`
carries a § *Correction to this file's own analysis* retracting a 92% figure
built by *"reading `chunk_size_for_model`'s raw output and attributing it to"* a
different function. The same mistake recurred thirteen days later, one layer
along. `chunk_size_for_model` returns a number that looks and is named like a
budget; the only thing making it a ceiling lives in a constant three files away.

## Measurements

All derived 2026-09-02. Each states its population; none is cited from elsewhere.

**Entry alignment.** Population: headings matching
`^#{2,4}\s+[A-Z]{1,3}-\d+\s+[—–-]\s` — entries the resolver's definition rule
actually defines — over `git ls-files docs/trackers docs/issues`.
**n = 1,482** (1,027 at `##`, 391 at `###`, 64 at `####`).

| chunk budget | entries that are exactly ONE chunk | chunks over those entries |
|---|---|---|
| 2,048 chars (today, retained) | 607 / 1,482 = 41.0% | 3,302 |
| 8,000 chars (ceiling, rejected) | 1,297 / 1,482 = 87.5% | 2,080 |

This is a trade-off, not a gap: larger chunks align with entry boundaries *and*
embed more bluntly. `entry_token` (D1) delivers the alignment benefit without
paying the ranking cost, which is why D4 keeps the budget.

**Corpus size.** Population: `git ls-files docs`, markdown only — 1,325 files,
22,309,731 bytes. **26,530 chunks** at 2,048 chars. Extrapolated across the
catalog's 10 repos / 4,525 artifacts: **~90,500 chunks ≈ 278 MB** of float32
vectors at 768 dims, against a `catalog.db` currently 66 MB. The extrapolation
scales by artifact count and assumes other repos resemble this one — re-derive
per repo before sizing storage.

**Concentration.** `bug-fix-session-log.md` is 498 of 26,530 chunks (1.88%);
the top 8 artifacts hold 7.1%. Low enough that swamping is a quality concern
rather than an emergency, but the large trackers are topically broad, so a wide
query draws disproportionately from them.

**Vector coverage.** Verified against `artifact_vec_rowids` (a plain table; the
`vec0` module is not loaded in the bare `sqlite3` CLI, so `artifact_vec` itself
is unqueryable there). **1,406 of 4,525 artifacts (31.1%) have a vector.**
Restricted to codescout — the repo *with* a configured embedder —
**717 of 1,357 (52.8%) have none.**

## Design

### 1. Schema and storage

`artifact_vec` needs **no schema change**. Its `id` column is already
`TEXT PRIMARY KEY`; nothing requires it to denote an artifact. Re-key it on
chunk ids and the virtual table is untouched. The bug file's "must become a
compound key" was a natural reading of the constraint but is not required.

```sql
CREATE TABLE artifact_chunk (
  chunk_id     TEXT PRIMARY KEY,
  artifact_id  TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  chunk_ix     INTEGER NOT NULL,
  start_line   INTEGER NOT NULL,
  end_line     INTEGER NOT NULL,
  entry_token  TEXT,            -- 'W-81' when the enclosing heading defines one
  content      TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  UNIQUE(artifact_id, chunk_ix)
);
CREATE INDEX idx_artifact_chunk_artifact ON artifact_chunk(artifact_id);
```

**D3 — `chunk_id` is an opaque surrogate.** `id = sha256(abs_path)`, so
archiving re-keys an artifact, and archiving is a bug file's normal end state.

| scheme | on `artifact(move)` | on re-index |
|---|---|---|
| derived (`<artifact_id>#<ix>`, or path-encoded like the code index) | every chunk id changes → O(chunks) through `gc.rs:406` `migrate_vec_id` | id-set diff is clean |
| opaque surrogate | one `UPDATE artifact_chunk SET artifact_id` | upsert on `(artifact_id, chunk_ix)` |

`migrate_vec_id` exists only because `vec0` rejects `UPDATE ... SET id`, forcing
read-delete-insert. Making every archive move a loop through it is the cost the
surrogate avoids.

**Cascade.** The `AFTER DELETE ON artifact` trigger (`schema.sql:54-58`) becomes
a fan-out and must read `artifact_chunk` **before** the FK cascade empties it —
so `BEFORE DELETE`, or explicit code in `gc.rs`, which already handles
`artifact_vec` by hand for this same "no FK, trigger-only" reason.

### 2. Indexer and chunking

**Swap the chunker, and delete one.** `chunk_markdown` returns `Vec<String>` —
no line numbers, so it cannot support a chunk-grain result at all.
`split_markdown` returns `Vec<RawChunk>` carrying 1-indexed `start_line` /
`end_line`, and the code index already uses it (`ast_chunker.rs:982`).
`chunk_markdown`'s only caller in the tree is the defective line itself, so
consolidating removes a chunker rather than adding one.

**Heading depth — add a parameter, default 3.** The two disagree:
`chunk_markdown` breaks on levels 1–6, `split_markdown` on 1–3
(`chunker.rs:93`). 64 of 1,482 entries (4.3%) are defined at `####`.

Do **not** "fix" `split_markdown` to 1–6. Its chunk ids encode `start_line`, so
re-chunking would invalidate all 33,032 code-index chunks and force a full
re-embed of a different index to fix 64 headings in this one. A depth parameter
defaulting to 3 leaves code-index chunking byte-identical; the artifact path
passes 6. Blast radius: one existing call site.

**Per-chunk empty filtering.** Today's `embed_queue_item` returns `None` on an
empty first chunk, because the embedder's guard bails the **whole batch** on a
single empty input (`archive/2026-05-17-reindex-embedding-dim-mismatch.md`).
That hazard multiplies with N chunks. The filter must move inside the per-chunk
loop, or one whitespace-only section aborts an entire reindex.

**Title and heading context.** `embed_artifact` prepends `title`; every chunk
keeps that. `RawChunk.metadata` is the existing field for a searchable header
prepended before embedding, so the enclosing entry heading can ride along —
giving a mid-entry chunk the words `W-81 — Choose a gate's surface` even when
the heading is thousands of characters upstream.

**Scale.** `indexer.rs:652` batches 100 and parallelizes round-trips, sized for
4,525 embeds. This makes it ~90,500 — a 20× jump. Progress reporting and
timeout behaviour need checking, not redesigning.

### 3. Retrieval semantics

**The two consumers want different grains.** `context.rs:679-695` requests 51
hits, truncates to 50, and keeps only `h.row.id` — artifact ids — then ranks by
the link graph. Chunk-grain hits would silently degrade it: 51 chunks might be
8 distinct artifacts, so a bundle packing 50 candidates today would pack 8,
while `candidates_capped = rows.len() > 50` reported "capped". No error; it just
gets quietly worse — the same silent-partial shape as the bug being fixed.

**D5 — one parameter serves both, and solves swamping.** `semantic_find` gains
`max_per_artifact`:

- `context` passes **1** → best chunk per artifact, 50 distinct artifacts.
  Behaviour preserved, and better ranked: an artifact now qualifies on its best
  passage rather than its preamble.
- `artifact(find, semantic=)` passes **3** → chunk-grain hits, no single ledger
  monopolizing a page.

Neither consumer needs a mode flag or a second code path.

**Widening loop.** `k = (target * 5).max(100)`, capped at `K_CAP = 2000`. With a
per-artifact cap, candidates collapse before counting toward `target`, so the
multiplier must account for it. `K_CAP = 2000` against ~90,500 chunks is a much
smaller slice of the corpus than 2,000 against 4,525 artifacts was; re-tune both.

**`SemanticPage` extends.** `widenings` and `exhausted` exist so that
"backfilled past the point of relevance" is a readable state rather than an
indistinguishable one. `SemanticHit` gains the chunk fields; `exhausted` gains a
sibling reporting **cap-suppressed hits**, or the same silent-partial defect
reappears one level up.

**Payload size.** Chunk content at ~2 KB × 10 hits is a large response. Return a
bounded snippet plus the line range, per `docs/PROGRESSIVE_DISCLOSURE.md`, and
let the caller fetch the full span with `artifact(get, start_line=, end_line=)`.

### 4. Migration and backfill

**Legacy vectors are discarded, not reinterpreted.** All 1,406 are preamble-only
embeddings keyed by artifact id. Left in a table whose keys now mean chunk ids,
they are indistinguishable at query time from real chunks — shipping the bug for
31% of artifacts, invisibly mixed with the fix.

**D6 — build alongside, swap transactionally.** Wiping in place leaves
`artifact(find, semantic=)` and `librarian(context, topic=)` returning nothing
until backfill completes, and ~90,500 embeds is not a seconds-long window.
Create `artifact_vec_v2`, backfill, then drop-and-rename in one transaction.
`migrate_v6.rs` is the precedent, and its own comment records the trap:
*"DROP TABLE implicitly drops the artifact_vec_cascade_delete trigger"* — so
trigger re-creation is a required step. `schema_version` (`schema.sql:63`) gates
it.

**Re-runnable and interruptible.** The catalog is machine-local and gitignored
(`docs/conventions/cross-machine-catalog-resume.md`), so this is not a one-time
event — every checkout pays it. It cannot be a single long transaction.

**D8 — the coverage hole is a prerequisite, not a parallel question.** 52.8% of
codescout artifacts have no vector. The cause is not established. The pattern
fits an already-archived mechanism —
`archive/2026-07-25-reindex-reembed-noop-without-force.md`: reindex does not
re-embed unchanged content without `force`, so runs where the embedder was down
or unconfigured leave holes no later run fills, because content hashes never
changed. Coverage is not recency-driven: 2026-08-25 is 81% covered while both
neighbouring days are ~85% missing.

**This is a hypothesis, not a reproduction.** It does not need diagnosing to be
designed around — the backfill must force, and must not route through the
unchanged-content early return at `indexer.rs:284`. But it does need diagnosing
before ship, because a migration that reproduces the hole delivers chunk-grain
retrieval that is dark over half this repo, which is the original bug's failure
mode one layer up.

Diagnosis and fix stay in their own bug file. Only the *ordering* is fixed here:
causes are independent, execution is not.

### 5. Testing and measurement

**The core regression test must fail today**, and its obvious form is monotone
in the wrong direction. "An artifact whose distinguishing content lies after its
first heading is retrievable by that content" is an **existence** assertion —
satisfied by an implementation that returns everything. Both legs in one test:

1. the deep passage retrieves the artifact, **and**
2. the hit's `start_line` / `end_line` bracket that passage, **and**
3. a second, topically-distant passage in the same artifact ranks below it.

Line-range assertions are what make widening detectable: a chunk that swallowed
the file reports the wrong span.

**Mutate once per site.** Five sites, five separate kills:

| site | mutation |
|---|---|
| `indexer.rs` chunk loop | re-introduce `.next()` |
| `semantic_find` cap | ignore `max_per_artifact` |
| `gc.rs` cascade fan-out | delete only `chunk_ix = 0` |
| `entry_token` extraction | always write `NULL` |
| migration legacy-wipe | leave the 1,406 old rows |

Precedent: `artifact_augment`'s two shape-writing paths killed *different*
tests, neither failing under the other's mutation.

**The cap test has the opposite monotonicity.** "No more than 3 hits from one
artifact" is an **absence** assertion — a cap that drops everything passes.
Assert all three: exactly 3 present from the swamping artifact, the 4th
suppressed, and a lower-ranked chunk from a *different* artifact present. Only
the third clause separates "capped" from "broken".

**The `context` test asserts distinctness, not count.** `max_per_artifact = 1`
must yield 50 **distinct** artifact ids; a count of 50 is satisfied by 50 chunks
from one ledger, which is the regression the parameter exists to prevent.

**The empty-chunk test must run a bulk reindex**, not a single embed. The caller
that reaches the batch-abort hazard is the batch, and the observer is the
reindex report; a per-chunk test reaches neither.

**D7 — what green will not prove, and the instrument gap.**
`scripts/run-tc-benchmark.py`'s 25-TC suite scores against
`bench_<model>_code_chunks`. An `artifacts` collection exists in Qdrant but is
not in the scored suite. **The path being changed has no benchmark.** Every test
above proves the mechanism works on fixtures; none answers *did artifact
retrieval get better*, which is the purpose of the change.

So an artifact-TC suite ships with this work: 10–15 cases whose ground truth is
a known entry (`bug-fix-session-log:W-81` for *"choosing where a gate lives"*),
scored before and after. The corpus is unusually suited to this — 1,482 entries
with stable citable ids make ground truth cheap, which is not true of code
retrieval.

The benchmark tracker states the rule this follows, earned the same way:
**"check the corpus the instrument actually reads, not the one you are standing
in."** "This project has a retrieval benchmark" and "this project can measure
this retrieval change" are different claims, and the failure mode of conflating
them is a number, not an error — 25 scores, all of them about code.

## Open questions

1. **Snippet budget.** How many characters of `content` a chunk hit returns
   before the caller must fetch the span. Needs a number from
   `docs/PROGRESSIVE_DISCLOSURE.md`'s existing budgets rather than a new one.
2. **`K_CAP` and the `k` multiplier** under a per-artifact cap. Both are
   currently tuned for artifact-grain candidates; the correct values are
   empirical and should come from the artifact-TC suite once it exists.
3. **278 MB of vectors.** If unacceptable, the levers are quantization or a
   chunk-eligibility rule — **not** the chunk budget, which is spent on ranking
   quality (D4).
4. **Qdrant parity.** This design is written against the sqlite-vec backend.
   `artifact_store.rs:216` is the Qdrant write path and needs the same
   chunk-keyed treatment; whether both backends migrate together is undecided.

## Prerequisites

- **P1** — diagnose the 31% / 52.8% vector-coverage hole (D8). Own bug file;
  blocks migration ship, not migration design.

## References

- `docs/issues/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md` (`7a37f1179d2f0e21`)
- `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md` — the budget decision, and the precedent for this spec's retraction
- `docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md` — the mechanism P1 most likely instantiates
- `docs/issues/archive/2026-05-17-reindex-embedding-dim-mismatch.md` — why empty inputs abort a batch
- `docs/trackers/retrieval-benchmark.md` — the harness, its scope, and the corpus rule
- `docs/conventions/cross-machine-catalog-resume.md` — why migration is per-machine
- `src/librarian/indexer.rs:67`, `:284`, `:652`
- `src/librarian/catalog/find.rs:299`, `src/librarian/tools/context.rs:679`
- `src/librarian/catalog/schema.sql:49-58`, `src/librarian/catalog/migrate_v6.rs:202-214`
- `src/librarian/catalog/gc.rs:406`
- `crates/codescout-embed/src/chunker.rs:83`, `:93`, `:127`
- `crates/codescout-embed/src/lib.rs:104-115`
- `src/embed/ast_chunker.rs:953`, `:982`

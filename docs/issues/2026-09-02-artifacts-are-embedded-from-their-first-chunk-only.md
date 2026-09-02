---
id: '7a37f1179d2f0e21'
kind: bug
status: open
title: 'BUG: an artifact is embedded from its first chunk only — a 189-entry ledger is represented by its preamble'
tags:
- cluster/capped-result-presented-as-complete
- librarian
- embeddings
- semantic-search
- retrieval-grain
- silent-partial-result
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
---

# BUG: an artifact is embedded from its FIRST CHUNK only — a 189-entry ledger is represented by its preamble

## Summary

`embed_queue_item` takes `chunk_markdown(body, 512).into_iter().next()` — the
first chunk and nothing else. For a tracker, chunk 0 is the H1 plus the preamble
before the first heading. So semantic search over this project's durable memory
is searching **titles and preambles**, never entries. The 189-entry bug-fix
session log is represented by **1,404 of 766,860 bytes — 0.18%**. Corpus-wide,
**1 of 1,601 entries** falls inside its artifact's only embedded chunk.

## Symptom (Effect)

No error. `artifact(action="find", semantic=…)` and `librarian(action="context",
topic=…)` return plausible, ranked results. They are ranked on content the caller
assumes was searched and was not.

> **Corrected 2026-09-02 — `semantic_search` is NOT affected. The original
> wording named it first, and that error was load-bearing.**
>
> codescout runs **two** vector indexes over the same markdown, in two
> databases. `semantic_search` rides `code_chunk`/`code_vec` in the
> project-local `.codescout/embeddings/<project>.db`, which is fully chunked:
> **33,032 markdown chunks over 1,363 files**, including **809 chunks covering
> lines 1–9648** of the same 766 KB tracker that `artifact_vec` represents with
> one preamble vector.
>
> Probed rather than reasoned: both paths were run against a phrase at line
> 7814 of `docs/trackers/bug-fix-session-log.md`.
> `semantic_search(mode="full")` returned it **ranked 2nd and 3rd**;
> `artifact(find, semantic=…)` did not return the file at all.
>
> Affected consumers are exactly the two that read `artifact_vec`:
> `src/librarian/catalog/find.rs:299` (`semantic_find`) is its sole production
> caller, reached from `artifact(find, semantic=…)` and from
> `src/librarian/tools/context.rs:679`.
>
> This matters past accuracy. It means chunk-grain markdown retrieval **already
> exists in this binary**, so the fix below is a **port with a working reference
> implementation**, not an invention — and building it blind would have been the
> two-implementations defect this corpus already names.

**measured 2026-09-02** — the five artifacts with the most entries:

| entries | vector? | embedded B | body B | covered |
|---:|---|---:|---:|---:|
| 189 | yes | 1,404 | 766,860 | **0.18%** |
| 125 | yes | 426 | 672,097 | **0.06%** |
| 74 | yes | 377 | 247,231 | **0.15%** |
| 71 | yes | 251 | 153,846 | **0.16%** |
| 51 | **NO** | 701 | 237,080 | — |

The 189-entry ledger's single 768-dim vector encodes
`# Session Log — Bug-Fix Work Stream / > **Scope:** …` and not one of its 189
entries.

And coverage is partial on top of that: **1,400 of 4,495 artifacts have a vector
at all — 31.1%.** 3,095 have none, including a 51-entry ledger.

## Root cause

```rust
// src/librarian/indexer.rs:66-75
chunk_markdown(body, 512).into_iter().next()
```

`.next()` discards every chunk after the first. `src/librarian/embedding.rs:14-17`
then embeds `title + "\n\n" + first_chunk`. Both backends consume the same item —
sqlite-vec via `write_embeddings` (`indexer.rs:479`) and Qdrant via
`artifact_store.rs:216`.

Chunk 0's extent is set by `crates/codescout-embed/src/chunker.rs:127-201`:
*"Headings always force a new section even if small."* So for any document whose
first heading follows a preamble, chunk 0 **is** the preamble — capped at
512 × 4 = 2,048 chars.

**This is not a missing chunker.** The chunker already produces every chunk;
`indexer.rs:69` throws them away. The binding constraint is the schema:

```sql
-- src/librarian/catalog/schema.sql:49-52
CREATE VIRTUAL TABLE artifact_vec USING vec0(
  id        TEXT PRIMARY KEY,
  embedding FLOAT[768]
);
```

`id TEXT PRIMARY KEY` permits exactly one vector per artifact. Storing more
requires a compound key.

*Lines cited are a subagent's reads; the coverage figures are re-derivable from
`artifact_vec_rowids` (ordinary SQLite — note the `vec0` module is NOT loaded in
the plain `sqlite3` CLI, so `artifact_vec` itself is unqueryable there).*

## The chunk budget — RETRACTED as a defect; it is a benchmarked trade-off

> **Retracted 2026-09-02, the same day it was added.** An earlier revision of
> this section called the `512` literal at `indexer.rs:67` a defect — *"4× too
> small"* — because `chunk_size_for_model("CodeRankEmbed")` returns 2048.
> **That reasoning was wrong. The model's figure is a CEILING, and this project
> deliberately chunks well below it.**
>
> The prior decision is explicit and benchmark-backed —
> `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`:
> `STACK_CHUNK_TARGET = 1200` is *"benchmark-backed, and under every local
> model's real ceiling"*; `AST_CHUNK_TARGET = 3000`
> (`src/embed/ast_chunker.rs:953`) caps **every** code-index chunk under the
> comment *"smaller chunks produce sharper embeddings for retrieval regardless
> of file type"*; and a since-deleted `DEFAULT_CAP = 4096` existed precisely
> because large-context models *"would otherwise default to ~20k chars per
> chunk, which both slows indexing and **dilutes ranking signal**"*.
>
> At 512 tokens the librarian chunks at **2,048 chars — inside the project's
> deliberate 1,200–3,000 window.** It is not misconfigured.
>
> **The error has a name, and its precedent is in this record's own ancestor.**
> That archived file carries a § *Correction to this file's own analysis*
> retracting a 92% figure built by *"reading `chunk_size_for_model`'s raw output
> and attributing it to"* a different function. This retraction is the same
> mistake one layer along — raw ceiling read as target. Two authors, one
> function, thirteen days apart. `chunk_size_for_model` returns a number that
> looks like a budget and is named like a budget; the only thing making it a
> ceiling lives in a constant three files away.

### What the measurement actually shows

The entry-alignment figures stand as data. Only their interpretation was wrong.

Population: headings matching `^#{2,4}\s+[A-Z]{1,3}-\d+\s+[—–-]\s` — entries the
resolver's definition rule actually defines — over
`git ls-files docs/trackers docs/issues`. **n = 1,482 defined entries** (1,027 at
`##`, 391 at `###`, 64 at `####`). Not the same population as the 1,601 counted
in § *Summary*, which uses a different selector; both are stated with their
derivation rather than reconciled.

| chunk budget | entries that are exactly ONE chunk | chunks over those entries |
|---|---|---|
| 2,048 chars — today's literal | 607 / 1,482 = **41.0%** | 3,302 |
| 8,000 chars — the model's ceiling | 1,297 / 1,482 = **87.5%** | 2,080 |

Read correctly this is a **trade-off**, not a gap: larger chunks align better
with entry boundaries *and* embed more bluntly. This project has already priced
the second half empirically, on this corpus, and chosen small.

### The trade-off dissolves — `entry_token` buys the alignment for free

Entry alignment was only ever wanted so that a hit could **name an entry**.
Storing the enclosing entry's token on **every** chunk delivers exactly that
without touching the budget: a chunk from the middle of a five-chunk `W-81`
still reports `bug-fix-session-log:W-81`, carrying its own line range for the
precise match.

So the budget stays at 512 tokens, retrieval keeps its benchmarked sharpness,
and entry-grain naming comes from a `TEXT` column instead of from blunter
vectors. **No budget change is part of this fix.**

### Retained from the retracted section: resizing alone could never have worked

Independently true, and worth keeping because it explains why first-chunk-only
is not a sizing bug at all. `chunk_markdown` forces a section break at **every**
heading regardless of size (`chunker.rs:141-152`, *"headings always force a new
section even if small"*), so chunk 0 ends at the first heading whatever the cap
is. Raising the budget changes only those files whose preamble already exceeds
the cap — rare. There was never a version of this bug that a bigger number
fixed.
## Cost model — measured, no longer unmeasured

The original *"do not treat embed-every-chunk as free"* warning was right to
refuse a guess. Derived 2026-09-02 over `git ls-files docs` → 1,325 markdown
files, 22,309,731 bytes:

| chunk budget | chunks for codescout `docs/**.md` |
|---|---|
| **2,048 chars — today's literal, which this fix KEEPS** | **26,514** |
| 8,000 chars — the model ceiling, *not* chosen; see the retraction | 22,414 |

Extrapolated across the catalog's **10 repos / 4,523 artifacts** (of which 1,355
are codescout): **~90,500 chunks ≈ 278 MB of float32 vectors at 768 dims**,
against a `catalog.db` that is 66 MB today.

**The extrapolation is crude** — it scales by artifact count and assumes other
repos' documents resemble this one's. Re-derive per repo before sizing storage.
If 278 MB proves unacceptable, the lever is vector quantization or a
chunk-eligibility rule, **not** the chunk budget — that one is spent on ranking
quality and buying it back costs retrieval.
## Evidence

### The failure is silent in the direction that matters

A search that should have matched an entry returns *something* — the artifacts
whose preamble happens to be topically close. There is no empty result to
notice. This is the second Testing-Discipline law: the refuting outcome leaves
no artifact.

### It compounds with a known-empty edge

`**Rests on:**` materialises zero edges. Its parser half is fixed —
`docs/issues/archive/2026-09-02-parse-rests-on-truncates-at-line-one.md`,
`experiments` `1b071cd7`, patch-id
`9d0f25f5581c517c4b5ff663fea05d0858f855f0` — but the **edge** is still unbuilt.
So neither
the lexical route nor the semantic route currently reaches an individual
Statement. Any design that says *"then we semantically retrieve the Statements
that rest on this"* is resting on a layer that does not exist.

## Hypotheses tried

1. **Hypothesis:** the chunker is not wired up, so only one chunk exists.
   **Test:** read `crates/codescout-embed/src/chunker.rs:127-201`.
   **Verdict:** rejected — the chunker produces all chunks; the indexer keeps
   one.
2. **Hypothesis:** 31% coverage is a stale index that a reindex would fix.
   **Test:** not run.
   **Verdict:** deferred — see *Resume*. The two questions (chunk grain, and
   why 69% have no vector) are independent and the second is unmeasured.

## Fix

**Not yet applied.** Two schema/indexer changes plus one payload field. Revised
2026-09-02; an intermediate revision listed a chunk-budget change as item 1 and
it has been retracted — see § *The chunk budget*.

1. **Schema** — one vector per artifact is the binding constraint, but a
   **compound key is not required**: `vec0` accepts a TEXT primary key, so
   `artifact_vec` can simply be re-keyed on an opaque `chunk_id`, with an
   `artifact_chunk` side table carrying `artifact_id`, `chunk_ix`, `start_line`,
   `end_line`, `entry_token`, `content`, `content_hash` and
   `UNIQUE(artifact_id, chunk_ix)`. That is `code_chunk`/`code_vec`'s existing
   shape — a **port with a working reference implementation**, not an invention.

   Prefer an **opaque** `chunk_id` over a derived `<artifact_id>#<ix>`: since
   `id = sha256(abs_path)`, archiving re-keys an artifact, and archiving is a
   bug file's normal end state. A derived id makes every move an O(chunks) loop
   through `gc.rs:406`'s `migrate_vec_id` — which exists only because `vec0`
   rejects `UPDATE ... SET id`. An opaque id makes a move one
   `UPDATE artifact_chunk SET artifact_id`, touching no vectors.

   The `AFTER DELETE ON artifact` cascade at `schema.sql:54-58` must fan out
   over an artifact's chunks, and must read `artifact_chunk` **before** the FK
   cascade empties it — so `BEFORE DELETE`, or explicit code in `gc.rs`, which
   already handles `artifact_vec` by hand for this same "no FK, trigger-only"
   reason.

2. **Indexer** — `indexer.rs:69`, stop discarding; embed every chunk. Switch
   from `chunk_markdown` (returns `Vec<String>`, no line numbers) to the
   line-aware `split_markdown` → `Vec<RawChunk>`, which the code index already
   uses via `ast_chunker.rs:982`. Note the two disagree on heading depth:
   `chunk_markdown` breaks on levels **1–6**, `split_markdown` on **1–3** only,
   which affects the 64 entries defined at `####`.

   `chunk_markdown` has exactly **one caller in the tree** — the defective line
   itself — so consolidating on `split_markdown` removes a chunker rather than
   adding one.

3. **`entry_token`** — populate from the enclosing `## <ID> — <title>` heading
   (`link_scan` already has the parser) so a chunk hit names a **citable** entry
   rather than a line range. This is what makes entry-grain retrieval real, and
   it replaces the budget change an earlier revision proposed.

**The chunk budget stays at 512 tokens / 2,048 chars.** Changing it is not part
of this fix and would cost ranking quality.

Then the retrieval semantics: `semantic_find` (`find.rs:299`) returns
chunk-grain hits rather than artifact rows — decided 2026-09-02 in favour of
returning the entry, not the artifact.
## Tests added

None yet. A regression test should assert that an artifact whose distinguishing
content lies **after** its first heading is retrievable by that content — i.e.
it must fail today.

## Workarounds

Do not rely on `semantic_search` / `semantic=` to find an entry inside a large
tracker. Use `grep` for exact tokens, `link_scan`-derived citations, or
`artifact(action="get", heading=…)` when the heading is known.

## Resume

Two separable next actions. (a) Establish why **3,095 of 4,495** artifacts have
no vector — run `librarian(action="reindex", reembed=true)` on a scratch copy
and re-count `artifact_vec_rowids`; that number may be an unrelated coverage
bug and should not be folded into this one without measuring. (b) For the grain
fix, start at `src/librarian/catalog/schema.sql:49-58` — the compound key is the
gating change; `indexer.rs:69` is one line once the schema allows it.

## References

- `src/librarian/indexer.rs:66-75`, `:479`
- `src/librarian/embedding.rs:14-17`
- `src/librarian/artifact_store.rs:216`
- `crates/codescout-embed/src/chunker.rs:127-201`
- `src/librarian/catalog/schema.sql:49-58`
- Coverage query: `SELECT COUNT(*) FROM artifact a WHERE NOT EXISTS (SELECT 1 FROM artifact_vec_rowids v WHERE v.id = a.id)` against `~/.local/share/librarian/catalog.db`

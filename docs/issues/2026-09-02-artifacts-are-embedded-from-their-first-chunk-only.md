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

No error. `semantic_search` and `artifact(action="find", semantic=…)` return
plausible, ranked results. They are ranked on content the caller assumes was
searched and was not.

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

## Evidence

### The failure is silent in the direction that matters

A search that should have matched an entry returns *something* — the artifacts
whose preamble happens to be topically close. There is no empty result to
notice. This is the second Testing-Discipline law: the refuting outcome leaves
no artifact.

### It compounds with a known-empty edge

`**Rests on:**` materialises zero edges (see
`docs/issues/2026-09-02-parse-rests-on-truncates-at-line-one.md`). So neither
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

**Not yet applied**, and it is two changes, not one:

1. **Schema** — `artifact_vec`'s `id TEXT PRIMARY KEY` must become a compound
   key (`artifact_id`, `chunk_ix`) before more than one vector per artifact can
   be stored. Note the `AFTER DELETE ON artifact` cascade at
   `schema.sql:54-58` must follow.
2. **Indexer** — `indexer.rs:69`, stop discarding; embed each chunk.

Then decide the retrieval semantics — max-over-chunks or mean — and whether a
hit reports the artifact or the chunk. Reporting the chunk is what would make
entry-grain retrieval real, and is the point of doing this.

**Do not treat "embed every chunk" as free**: the 189-entry ledger alone is
766 KB, and the cost model for re-embedding the whole corpus at chunk grain is
unmeasured.

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


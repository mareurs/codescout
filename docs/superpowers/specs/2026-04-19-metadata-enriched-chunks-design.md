# Metadata-Enriched Chunks + Chunk Size Tune — Design

**Date:** 2026-04-19
**Status:** Draft
**Motivation:** CodeRankEmbed benchmark scored 23/60, lowest of 4 tested models. Real cross-project usage analysis (238 calls, 13 projects) shows users write 3–10 word multi-concept keyword queries (median 6 words, 50 chars). Current chunking cuts natural functions mid-body at ~1305 chars and chunks have no searchable metadata surface beyond their raw code body.

---

## Goals

1. Improve semantic search retrieval quality on multi-concept keyword queries (94% of real traffic)
2. Reduce chunk count and DB size by keeping natural function boundaries
3. Preserve progressive disclosure (user-visible output unchanged)

## Non-Goals

- Bridging cross-file architectural queries (Tier 4 benchmark: requires hybrid BM25 or reranker, out of scope here)
- Supporting bare identifier lookups via semantic search (1.7% of traffic; `find_symbol` is the right tool)
- Adding pagination tuning (zero pagination events across 238 real calls)

---

## Design

### 1. Chunk size tune

Set `chunk_size = 1600` in `.codescout/project.toml`.

- Zero code change
- Keeps functions up to ~40–45 lines whole (vs ~35 at 1305)
- Natural target: AST chunker already aims for `AST_CHUNK_TARGET = 3000`; `enforce_max_chunk_size(1600)` clamps large functions to 1600 while leaving typical functions untouched
- Estimated: 15–20% fewer chunks than current baseline

**Rationale:**
- Bigger (3000): embedding signal dilutes — multiple concepts per vector
- Smaller (800): loses surface area for multi-concept keyword queries

### 2. Metadata headers

Each chunk gets a compact searchable header prepended **before embedding**. Header is stored separately in the DB and is NOT shown in search results.

**Format:**

```
{relative_path} :: {container_chain} :: {symbol_kind} {name}{signature}
```

**Examples:**

```
src/embed/index.rs :: impl IndexStore :: fn build_index(force: bool) -> Result<()>
src/embed/ast_chunker.rs :: fn nodes_to_chunks
src/tools/semantic.rs :: impl Tool for SemanticSearch :: fn call
src/main.py :: class UserService :: def authenticate(token: str)
src/app.ts :: class AuthController :: method login
```

**Per-chunk-type rules:**

| Chunk type | Header |
|------------|--------|
| Top-level function | `{path} :: {kind} {name}{sig}` |
| Method inside container | `{path} :: {container} :: {kind} {name}{sig}` |
| Struct/enum/const/type | `{path} :: {kind} {name}` |
| Gap chunk (between nodes) | `{path}` only |
| Container header chunk | `{path} :: {container}` (one chunk for impl/class signature) |
| Sub-split of oversized function | same header repeated across all slices — intentional; all pieces belong to one symbol |
| Markdown / unsupported lang | `metadata = NULL` — markdown splitter emits its own heading context |

**Per-language keyword mapping:**

| Language | Function | Class/Struct | Interface | Other |
|----------|----------|--------------|-----------|-------|
| Rust     | `fn`     | `struct` / `enum` | `trait` | `impl`, `mod`, `const`, `type` |
| Python   | `def` / `async def` | `class` | — | — |
| TypeScript | `function` / `method` | `class` | `interface` | `type` |
| JavaScript | `function` / `method` | `class` | — | — |
| Java     | `method` | `class` | `interface` | `constructor`, `enum` |
| Kotlin   | `fun`    | `class` / `object` | — | `property` |
| Go       | `func` / `method` | — | — | `type`, `var`, `const` |
| Bash     | `function` | — | — | — |

Keyword derived from tree-sitter node kind (already listed in `LANGUAGE_REGISTRY`).

**Signature extraction:**
- First line of the node
- Truncate at `{` (block start), `:` (Python), `=>` (arrow functions), or 100 chars — whichever comes first
- Multi-line generics collapse to first line only

### 3. Storage & data flow

**Schema:**

```sql
ALTER TABLE chunks ADD COLUMN metadata TEXT;
```

**Migration strategy:**

1. Bump `SCHEMA_VERSION` constant in `src/embed/schema.rs` (add if not present; scan `maybe_migrate_to_vec0` for the existing pattern)
2. On index open: if stored schema version < current, drop `chunks` + `vec0` tables and force rebuild
3. This reuses the `force rebuild` path already used for model mismatches — same code, new trigger

Rationale: `ALTER TABLE ADD COLUMN` would leave existing rows with `NULL` metadata, which means embeddings are inconsistent (old rows embedded without metadata, new with). A full rebuild is the only clean path. Cost: ~1 min reindex on first run after upgrade, one-time.

**Write path:**

```
source file
  → tree-sitter AST
  → nodes_to_chunks (threads container_path through recursion)
  → EnrichedChunk { content, metadata, start_line, end_line }
  → DB insert: INSERT INTO chunks (content, metadata, ...) VALUES (...)
  → embed text = metadata.map(|m| format!("{m}\n{content}")).unwrap_or(content)
  → vec0 insert
```

**Read path:**

- Semantic search: query embedding uses model's `embed_query` (with CodeRankEmbed prefix if applicable). Query text is **not** modified with metadata-style prefixes.
- Result display: returns `content` only. User sees clean code.
- Optional UX polish (deferred): surface `metadata` in compact output mode as a result header. No retrieval impact, purely cosmetic.

**Signature change in `nodes_to_chunks`:**

```rust
// Before
fn nodes_to_chunks(
    source, nodes, chunk_size, doc_prefixes, ts_lang, spec
) -> Vec<RawChunk>

// After
fn nodes_to_chunks(
    source, nodes, chunk_size, doc_prefixes, ts_lang, spec,
    file_path: &str,
    container_path: &[&str],
) -> Vec<EnrichedChunk>
```

`container_path` accumulates during recursion: `&[]` at top level, `&["impl IndexStore"]` in inner recursion, etc.

**New type:**

```rust
pub struct EnrichedChunk {
    pub content: String,
    pub metadata: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
}
```

The markdown splitter continues to emit `RawChunk`; a thin adapter converts its output to `EnrichedChunk { metadata: None, .. }` at the call site in `split_file`.

### 4. Testing

**Unit tests (`src/embed/ast_chunker.rs` tests module):**

1. `metadata_rust_top_level_function`
2. `metadata_rust_method_in_impl`
3. `metadata_rust_trait_impl` — `impl Trait for Struct`
4. `metadata_rust_nested_mod`
5. `metadata_python_class_method`
6. `metadata_typescript_class_method`
7. `metadata_sub_split_repeats_header` — oversized function, 3 sub-chunks, all share metadata
8. `metadata_gap_chunk_file_only`
9. `metadata_unknown_language_none`
10. `metadata_markdown_skipped`

**Integration tests (`src/embed/index.rs`):**

- `build_index_stores_metadata_column` — fixture project, assert non-null metadata for source, null for markdown
- `semantic_search_embeds_with_metadata` — MockEmbedder captures sent text; verify `metadata\ncontent` was embedded

**Migration test:**

- `old_db_without_metadata_column_triggers_rebuild` — create vec0 DB with old schema, run `build_index`, assert clean rebuild

**Validation benchmark (manual, not CI):**

Re-run the 20-query benchmark (`docs/research/2026-04-03-embedding-model-benchmark.md`) with:
- `chunk_size = 1600`
- metadata headers enabled
- CodeRankEmbed

Compare against:
- CodeRankEmbed baseline: 23/60 (no metadata, chunk_size=1305)
- AllMiniLML6V2Q baseline: 34/60

**Success criteria:**
- Total score ≥ 30/60 (beats every baseline) → ship
- Total score 25–29 → worthwhile but modest; ship + iterate
- Total score < 25 → metadata format is wrong; redesign before shipping

**Expected gains (hypothesis):**
- Tier 1 (+2–4): header tokens help direct-concept matches
- Tier 2 (+3–5): multi-keyword queries match file+symbol+body simultaneously
- Tier 3 (marginal): larger chunks help; metadata doesn't bridge files
- Tier 4 (~0): genuine cross-file queries stay out of reach for pure-semantic retrieval

### 5. Error handling

- Symbol extraction failure (malformed AST) → `metadata = Some(file_path)`, no symbol part
- No container context → just `{path} :: {kind} {name}`
- No kind mapping for a node type → omit kind keyword, use name only
- Metadata never blocks indexing: header is best-effort enrichment

### 6. Rollout

- Onboarding version bump in `src/tools/workflow.rs` — forces system prompt refresh
- Schema change forces rebuild on first indexing — no feature flag needed
- Update `docs/manual/src/experimental/asymmetric-query-prefix.md` to note metadata interaction
- Add new experimental doc: `docs/manual/src/experimental/metadata-enriched-chunks.md`
- Cherry-pick to `master` only after benchmark validation confirms ≥30/60

---

## Open questions

None at design time. Implementation may surface edge cases in signature extraction (multi-line Rust generics, TypeScript decorators, Kotlin annotations) — handle pragmatically with the 100-char signature cap.

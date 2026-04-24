# Architecture — codescout-embed

## Module Structure

```
src/
  lib.rs         Public API surface + factory functions
  embedder.rs    Embedding type + Embedder trait (core abstraction)
  chunker.rs     Text splitting (code + markdown paths)
  local.rs       LocalEmbedder  [feature = "local-embed"]
  remote.rs      RemoteEmbedder [feature = "remote-embed"]
```

## Core Abstraction: `Embedder` trait

Defined in `embedder.rs`, `async_trait`-boxable:
- `fn dimensions(&self) -> usize` — vector dimensionality
- `async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>` — batch
- `async fn embed_query(&self, text: &str) -> Result<Embedding>` — default impl
  delegates to `embed`; override for models needing a query prefix (CodeRankEmbed)

`Embedding = Vec<f32>`. Both implementations are `Send + Sync`.

## LocalEmbedder (fastembed)

- Wraps `fastembed::TextEmbedding` behind `Arc<Mutex<...>>` for thread safety
- Constructor is async (`new`) but ONNX session init runs on `spawn_blocking`
  to keep the tokio executor responsive
- Model strings: fastembed `EmbeddingModel` variant names, e.g.
  `JinaEmbeddingsV2BaseCode`, `BGESmallENV15Q`, `AllMiniLML6V2Q`
- Dimensions stored eagerly at construction time via a probe embed

## RemoteEmbedder (HTTP)

- Fields: `client` (reqwest), `endpoint` (String), `model` (String),
  `api_key` (Option<String>), `cached_dims` (Arc<AtomicUsize>),
  `query_prefix` (Option<String>)
- `cached_dims` starts at 0; populated after first successful `embed()`.
  Clones share the same `Arc<AtomicUsize>`.
- Constructors: `openai(model, api_key)`, `ollama(model)`, `custom(base_url, model)`,
  `from_url(url, model, api_key)` — last one normalises URL to always end with
  `/v1/embeddings`
- Security: rejects plaintext HTTP when an API key is present, unless the host
  is loopback (`localhost`, `127.0.0.1`, `[::1]`)
- HTTP client: 300-second timeout to avoid blocking `index_project` on a hung
  Ollama during GPU discovery
- Batching: chunks of 32 texts, 3 retries with exponential backoff (500ms base,
  doubles per attempt). 5xx retried, 4xx immediately fatal.
- Empty/whitespace texts are filtered before HTTP; zero-vectors substituted
  back at original positions

## Chunker

Three public functions:
1. `split(source, chunk_size, chunk_overlap) -> Vec<RawChunk>` — line-based
   sliding window. Emits 1-indexed line numbers. Overlap computed by
   `estimate_overlap_lines` (counts chars from tail).
2. `split_markdown(source, chunk_size, chunk_overlap) -> Vec<RawChunk>` —
   splits on `#`/`##`/`###` headings first, then applies `split` to oversized
   sections with offset-adjusted line numbers.
3. `chunk_markdown(text, max_tokens) -> Vec<String>` — returns plain strings
   (no line tracking). Pass 1: heading + blank-line boundary split. Pass 2:
   subdivide sections exceeding `max_tokens * 4` chars.

`RawChunk` fields: `content: String`, `start_line: usize` (1-indexed),
`end_line: usize` (1-indexed inclusive), `metadata: Option<String>` (set by
callers for AST-derived context headers; not returned in search results).

## Factory: `create_embedder_with_config`

Resolution order (feature-gated):
1. `url` supplied → `RemoteEmbedder::from_url` (strips routing prefix from model)
2. `local:` prefix → `LocalEmbedder::new` (ONNX)
3. `ollama:` prefix → probes Ollama reachability first, then `RemoteEmbedder::ollama`
4. `openai:` prefix → `RemoteEmbedder::openai`
5. `custom:` prefix → hard error with migration hint (use `url` field instead)
6. No prefix, `local-embed` feature → tries `LocalEmbedder::new` directly
7. Unknown → actionable error listing all options

`create_embedder(model)` is the legacy single-arg interface; delegates to
`create_embedder_with_config(model, None, None)`.

## Data Flow: Code Indexing

1. Caller reads file text
2. `split(text, chunk_size_for_model(model_spec), overlap)` → `Vec<RawChunk>`
3. Each `RawChunk.content` (optionally prefixed with `metadata`) is passed to
   `embedder.embed(texts)` as a batch
4. Returned `Vec<Embedding>` (each `Vec<f32>`) stored in sqlite-vec alongside
   chunk line ranges

## Data Flow: Semantic Search

1. Query string → `embed_one(embedder, query)` → single `Embedding`
2. KNN search in sqlite-vec → chunk row + cosine similarity
3. Chunk's `(file, start_line, end_line)` used to surface source location

## Useful Semantic Search Queries

- `semantic_search("chunk overlap line tracking", project_id="codescout-embed")`
- `semantic_search("remote embedder retry backoff batch size")`
- `semantic_search("model prefix resolution factory embedder")`
- `semantic_search("ONNX fastembed spawn_blocking")`
- `semantic_search("markdown heading split chunk")`

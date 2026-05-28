# codescout-embed — Architecture

## Module Structure

```
src/
  lib.rs       — public API, re-exports, create_embedder_with_config, chunk_size_for_model
  embedder.rs  — Embedder trait + Embedding type alias
  chunker.rs   — RawChunk struct, split(), split_markdown(), chunk_markdown()
  local.rs     — LocalEmbedder (fastembed; cfg feature = "local-embed")
  remote.rs    — RemoteEmbedder (reqwest/HTTP; cfg feature = "remote-embed"), probe_ollama()
```

## Key Abstractions

### `Embedder` trait (`embedder.rs`)

```rust
pub type Embedding = Vec<f32>;
#[async_trait]
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>;
    async fn embed_query(&self, text: &str) -> Result<Embedding>; // default: delegates to embed
}
```

`dimensions()` on `RemoteEmbedder` returns 0 until after the first successful `embed()` call;
callers that need a guaranteed non-zero value must embed a sample text first.

### `RawChunk` struct (`chunker.rs`)

```rust
pub struct RawChunk {
    pub content: String,
    pub start_line: usize,   // 1-indexed
    pub end_line: usize,     // 1-indexed, inclusive
    pub metadata: Option<String>, // searchable header prepended before embedding; None for markdown
}
```

### `LocalEmbedder` (`local.rs`, `local-embed` feature)

Wraps `fastembed::TextEmbedding` behind `Arc<Mutex<...>>` (fastembed 5 changed `embed()` to
`&mut self`). ONNX session creation is on `spawn_blocking`. Dimensions derived by embedding
a "probe" string at construction time.

### `RemoteEmbedder` (`remote.rs`, `remote-embed` feature)

Fields: `client` (reqwest), `endpoint` (URL), `model`, `api_key`, `cached_dims: Arc<AtomicUsize>`,
`query_prefix: Option<String>`.

Constructors:
- `RemoteEmbedder::openai(model, api_key)` → `https://api.openai.com/v1/embeddings`
- `RemoteEmbedder::ollama(model)` → `$OLLAMA_HOST/v1/embeddings` (default localhost:11434)
- `RemoteEmbedder::from_url(url, model, api_key)` — normalises URL to end in `/v1/embeddings`
- `RemoteEmbedder::custom(base_url, model)` — internal; reads `EMBED_API_KEY` from env

## Data Flow: Creating an Embedder

1. Caller calls `create_embedder_with_config(model, url, api_key)` from `lib.rs`.
2. Resolution order:
   a. `url` supplied → `RemoteEmbedder::from_url(url, bare_model, api_key)`
   b. `model` has `local:` prefix → `LocalEmbedder::new(model_id).await` (spawn_blocking)
   c. `model` has `ollama:` prefix → `probe_ollama(host)` (2s timeout) then `RemoteEmbedder::ollama`
   d. `model` has `openai:` prefix → `RemoteEmbedder::openai(model_id, api_key)`
   e. `model` has `custom:` prefix → hard error with migration hint
   f. No prefix → try as local model name; error with options if no feature or unknown model
3. Returns `Box<dyn Embedder>`.

## Data Flow: Embedding Texts (RemoteEmbedder)

1. Filter empty/whitespace inputs — bail! if all inputs are empty (avoids dim-mismatch bug).
2. Chunk filtered texts into batches of 32.
3. For each batch: retry up to 3 times with 500 ms / 1000 ms / 2000 ms exponential backoff.
   - 4xx responses: no retry (bad request, wrong model).
   - 5xx responses: retry.
   - Network errors: retry.
4. Response body capped at 32 MiB before JSON decode (prevents hostile server memory exhaustion).
5. Results sorted by `index` field (server may reorder).
6. `cached_dims` set on first success via `AtomicUsize` (Relaxed ordering).
7. Reconstruct full-length result with zero vectors for filtered-out empty inputs.

## Data Flow: Chunking

### Code / plaintext — `split(source, chunk_size, chunk_overlap)`

Line-based sliding window. Accumulates lines until `chunk_size` chars reached, then backs
up by `estimate_overlap_lines` lines for overlap. Uses 1-indexed line numbers in `RawChunk`.

### Markdown — `split_markdown(source, chunk_size, chunk_overlap)`

Splits on `#`, `##`, `###` headings first into sections, then calls `split()` on sections
that exceed `chunk_size`.

### Markdown (token budget) — `chunk_markdown(text, max_tokens)`

Two-pass: (1) split on headings + blank lines into sections; (2) subdivide on word boundaries
(`max_chars = max_tokens × 4`). Returns `Vec<String>` (no line tracking). Used by librarian
for documentation indexing.

## Chunk Size Derivation — `chunk_size_for_model(model_spec)`

Formula: `floor(max_tokens × 0.85 × 3)` where:
- 0.85 = 15% headroom for tokenisation variance and BOS/EOS control tokens
- 3 chars/token = conservative lower bound for code (actual is 3–4)

Fallback for unknown models: 512 tokens → 1305 chars.

## Security Design

- HTTPS enforced when `api_key` is set; loopback addresses exempted (Ollama/local dev).
- `is_https_or_loopback()` helper in `remote.rs` centralises the check.
- `rustls` with ring provider, installed once via `std::sync::Once`.

## Semantic Search Queries (no index built yet — use grep)

```
grep("BATCH_SIZE|MAX_RETRIES", "crates/codescout-embed/src/remote.rs")
grep("chunk_size_for_model", ...)
grep("cached_dims", ...)
grep("spawn_blocking", ...)
grep("query_prefix|CodeRankEmbed", ...)
```

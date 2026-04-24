# codescout-embed — Architecture

## Module Structure

```
src/
  lib.rs         — public API surface: re-exports, chunk_size_for_model, create_embedder*
  embedder.rs    — Embedder trait + Embedding type alias (always compiled)
  chunker.rs     — RawChunk, split(), split_markdown(), chunk_markdown() (always compiled)
  remote.rs      — RemoteEmbedder (#[cfg(feature = "remote-embed")])
  local.rs       — LocalEmbedder (#[cfg(feature = "local-embed")])
```

## Key Abstractions

### `Embedder` trait (`embedder.rs`)
```rust
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>;
    async fn embed_query(&self, text: &str) -> Result<Embedding>; // default: delegates to embed
}
pub type Embedding = Vec<f32>;
```
Object-safe async trait. `embed_query` provides a hook for asymmetric models
(e.g. CodeRankEmbed) that need a prefix on queries but not documents.

### `RemoteEmbedder` (`remote.rs`)
- Targets any OpenAI-compatible `/v1/embeddings` endpoint (Ollama, OpenAI, LM Studio, custom)
- Constructors: `openai()`, `ollama()`, `custom()`, `from_url()`
- `dimensions()` returns 0 until first successful `embed()` (lazy via `Arc<AtomicUsize>`)
- `embed()` batches in groups of 32, retries server errors up to 3× with exponential backoff
- Filters empty/whitespace inputs before sending (servers reject them with 400); fills zeros in output
- `embed_query()` prepends `query_prefix` for asymmetric models (CodeRankEmbed)
- `probe_ollama()` is a standalone health-check with 2-second timeout

### `LocalEmbedder` (`local.rs`)
- Wraps `fastembed::TextEmbedding` behind `Arc<Mutex<>>` (fastembed v5 requires `&mut self`)
- Construction offloads to `tokio::task::spawn_blocking` (ONNX session init is CPU-heavy)
- `embed()` also uses `spawn_blocking` for every batch call
- `parse_model()` maps string names to `fastembed::EmbeddingModel` enum variants

### Chunker (`chunker.rs`)
- `RawChunk { content, start_line, end_line, metadata }` — chunk with 1-indexed line tracking
- `split()` — character-budget splitter with configurable overlap, uses `estimate_overlap_lines()`
- `split_markdown()` — splits first on `#`/`##`/`###` heading boundaries, then sub-splits large sections with `split()`
- `chunk_markdown()` — simpler heading-based splitter returning `Vec<String>` (no line tracking); uses token budget (4 chars/token approximation)

### Factory (`lib.rs`)
- `create_embedder_with_config(model, url, api_key)` — resolution order:
  1. `url` set → `RemoteEmbedder::from_url`
  2. `local:` prefix → `LocalEmbedder`
  3. `ollama:` prefix → `RemoteEmbedder::ollama` (probes daemon first; hard error if unreachable)
  4. `openai:` prefix → `RemoteEmbedder::openai`
  5. `custom:` prefix → hard error with migration hint (prefix removed)
  6. No prefix → tries `LocalEmbedder` directly
- `create_embedder(model)` — thin wrapper calling `create_embedder_with_config(model, None, None)`
- `chunk_size_for_model(model_spec)` — computes safe chunk size in chars from documented model max-token limits using formula `max_tokens × 0.85 × 3`

## Data Flow

### Remote embedding path
1. Caller: `create_embedder_with_config("ollama:nomic-embed-text", None, None)`
2. `probe_ollama` → GET http://localhost:11434 (2s timeout) — errors if unreachable
3. `RemoteEmbedder::ollama("nomic-embed-text")` constructed
4. `embed(&["text1", "text2"])`:
   - Filter empty → batches of 32 → POST `/v1/embeddings` with `{model, input}`
   - Parse `EmbedResponse.data[]`, sort by `index`, extend `embedded`
   - Cache dims in `cached_dims` on first success
   - Reconstruct full-length result with zeros for originally-empty slots

### Local embedding path
1. Caller: `create_embedder("local:AllMiniLML6V2Q")`
2. `LocalEmbedder::new("AllMiniLML6V2Q")`
   - `spawn_blocking` → `parse_model("AllMiniLML6V2Q")` → `fastembed::EmbeddingModel::AllMiniLML6V2Q`
   - `fastembed::TextEmbedding::try_new(opts)` (downloads model if missing)
   - Probe embed to discover dims
3. `embed(&["text"])`:
   - `spawn_blocking` → `Mutex::lock()` → `model.embed(owned, None)`

## Semantic Search Examples
- `semantic_search("embedding batch retry backoff", project="codescout-embed")`
- `semantic_search("chunk overlap line tracking", project="codescout-embed")`
- `semantic_search("model feature gate cfg", project="codescout-embed")`
- `semantic_search("query prefix asymmetric embedding", project="codescout-embed")`
- `semantic_search("chunk size token budget model spec", project="codescout-embed")`

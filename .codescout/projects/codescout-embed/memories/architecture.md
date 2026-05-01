# codescout-embed — Architecture

## Module Structure

```
src/
  lib.rs          — public re-exports, chunk_size_for_model, create_embedder_with_config
  embedder.rs     — Embedder trait + Embedding type alias
  chunker.rs      — RawChunk struct, split(), split_markdown(), chunk_markdown()
  local.rs        — LocalEmbedder (cfg: local-embed feature)
  remote.rs       — RemoteEmbedder + probe_ollama (cfg: remote-embed feature)
```

## Key Abstractions

### `Embedder` trait (`src/embedder.rs`)
Async trait implemented by both backends:
- `dimensions(&self) -> usize` — vector size (0 until first embed for RemoteEmbedder)
- `embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>` — batch embed
- `embed_query(&self, text: &str) -> Result<Embedding>` — single query embed with optional model prefix (default delegates to `embed`)

### `Embedding` (`src/embedder.rs`)
Type alias: `Vec<f32>`. Dimensionality is model-dependent (384–768 typical).

### `RawChunk` (`src/chunker.rs`)
Produced by the text splitter:
- `content: String` — chunk text
- `start_line: usize`, `end_line: usize` — 1-indexed, inclusive
- `metadata: Option<String>` — prepended header for embedding (not returned in results)

### `LocalEmbedder` (`src/local.rs`, `local-embed`)
- Wraps `fastembed::TextEmbedding` behind `Arc<Mutex<...>>`
- ONNX session created via `spawn_blocking` (keeps async executor unblocked)
- Dimensions discovered by embedding a probe string at construction
- Supported models: `AllMiniLML6V2Q` (default, 384d), `NomicEmbedTextV15Q` (768d),
  `JinaEmbeddingsV2BaseCode` (768d), and non-quantized variants

### `RemoteEmbedder` (`src/remote.rs`, `remote-embed`)
- HTTP POST to `/v1/embeddings` (OpenAI-compatible format)
- Constructors: `openai()`, `ollama()`, `custom()`, `from_url()`
- Dimensions cached lazily via `Arc<AtomicUsize>` (0 until first embed)
- Batches of 32 texts per HTTP request; 3 retries with exponential backoff
- Empty/whitespace texts filtered and replaced with zero vectors
- 32 MiB response cap to prevent runaway memory from hostile servers
- `query_prefix` support for asymmetric models (e.g., CodeRankEmbed)
- Security: rejects plaintext HTTP when `api_key` is set, unless loopback host

## Data Flow: Embed Documents

1. Caller splits file text with `split(source, chunk_size, overlap)` → `Vec<RawChunk>`
2. Chunk size is determined by `chunk_size_for_model(model_spec)` (formula: `max_tokens × 0.85 × 3 chars/token`)
3. Caller passes `&[&str]` (chunk content strings) to `embedder.embed(texts)`
4. `RemoteEmbedder.embed()` filters empties, batches 32 at a time, POSTs to server, retries on 5xx
5. `LocalEmbedder.embed()` owns strings, dispatches to `spawn_blocking`, calls `fastembed::TextEmbedding::embed`
6. Returns `Vec<Vec<f32>>` — one vector per input text

## Data Flow: Embed Query

1. Caller calls `embed_one(embedder, query_text)` (or `embedder.embed_query(text)`)
2. `RemoteEmbedder.embed_query()` prepends optional `query_prefix` before calling `embed`
3. `LocalEmbedder` uses default `embed_query` (no prefix; fastembed handles it internally)
4. Returns single `Vec<f32>`

## Embedder Creation: `create_embedder_with_config`
Resolution order:
1. `url` set → `RemoteEmbedder::from_url` (any OpenAI-compatible endpoint)
2. `model` starts with `local:` → `LocalEmbedder::new`
3. `model` starts with `ollama:` → checks reachability via `probe_ollama`, then `RemoteEmbedder::ollama`
4. `model` starts with `openai:` → `RemoteEmbedder::openai`
5. `model` starts with `custom:` → hard error (prefix removed, migration hint shown)
6. No prefix → tries as local model name, falls back to error with options list

## Design Patterns
- Feature-gated backends: zero cost when neither feature enabled (chunker + trait only)
- `Arc<Mutex<>>` for fastembed (fastembed 5 changed `embed` to `&mut self`)
- `Arc<AtomicUsize>` for shared cached dimensions across clones of `RemoteEmbedder`
- Chunk size derived from model spec, not user-configurable, to prevent misconfiguration
- `probe_ollama` uses 2-second timeout; embed HTTP client uses 300-second timeout

## Good semantic_search Queries
- `semantic_search("chunk overlap line tracking", project="codescout-embed")`
- `semantic_search("fastembed ONNX local model download", project="codescout-embed")`
- `semantic_search("OpenAI compatible HTTP embedding retry backoff", project="codescout-embed")`
- `semantic_search("chunk size model tokens conservative formula", project="codescout-embed")`
- `semantic_search("query prefix asymmetric CodeRankEmbed", project="codescout-embed")`

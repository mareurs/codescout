# Semantic Search Guide

Semantic search lets you find code by describing what it does rather than
knowing what it is called. This page walks you through the full setup from
choosing a backend to writing effective queries. For a reference of the
individual tools, see [Semantic Search Tools](tools/semantic-search.md).

> For an explanation of how semantic search works under the hood — chunking,
> scoring, and when to use it vs symbol tools — see
> [Semantic Search Concepts](concepts/semantic-search.md).

## Choosing an Embedding Backend

codescout requires an external OpenAI-compatible embedding endpoint. The
`[embeddings]` block in `.codescout/project.toml` selects the server and model:

```toml
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

Any server speaking the OpenAI `/v1/embeddings` API works — Ollama, llama.cpp,
vLLM, TEI, OpenAI. See [Embedding Backends](configuration/embedding-backends.md)
for setup details and model recommendations.
## Setting Up Ollama

Install Ollama, pull the default model, and verify it responds correctly before
touching `project.toml`.

```bash
# Install Ollama (Linux/macOS)
curl -fsSL https://ollama.com/install.sh | sh

# Pull the default model
ollama pull mxbai-embed-large

# Verify the embedding endpoint is responding
curl http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model": "mxbai-embed-large", "input": "test"}'
```

A successful response looks like:

```json
{
  "object": "list",
  "data": [{ "object": "embedding", "index": 0, "embedding": [0.012, -0.034, ...] }],
  "model": "mxbai-embed-large"
}
```

If `curl` returns a connection error, Ollama is not running. Start it with
`ollama serve` in a separate terminal and retry.

If you run Ollama on a non-default host or port, set the `OLLAMA_HOST`
environment variable before starting Claude Code:

```bash
export OLLAMA_HOST=http://192.168.1.10:11434
```

## Configuring codescout

The `[embeddings]` section of `.codescout/project.toml` controls which
model is used and how files are chunked. The defaults work well for most
projects:

```toml
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

`url` is required; codescout has no in-process embedding backend. Chunk size
defaults to 1600 characters and can be overridden via `[embeddings] chunk_size`.

To use OpenAI instead, set the model and export your API key:

```toml
[embeddings]
model = "openai:text-embedding-3-small"
```

```bash
export OPENAI_API_KEY=sk-...
```

## Building the Index

Once `project.toml` is configured (or using the default), build the index:

```json
{ "name": "index", "arguments": { "action": "build" } }
```

What happens internally:

1. codescout walks the project tree, skipping directories listed in
   `ignored_paths` (by default: `.git`, `node_modules`, `target`,
   `__pycache__`, `.venv`, `dist`, `build`, `.codescout`).
2. Each source file is split into chunks using an AST-aware chunker. Each
   top-level function, method, or class becomes its own chunk. Oversized
   containers (impl blocks, classes) are recursively split into one chunk per
   inner method plus a header chunk for the container signature. Chunk size is
   derived from the model's context window — no configuration needed.
3. Each chunk is sent to the configured embedding backend, which returns a
   dense vector.
4. The vectors and chunk metadata are stored in
   `.codescout/embeddings.db` (SQLite).

**How long it takes:** With Ollama on a modern laptop, expect roughly 80–120
files per minute. OpenAI's API is faster in wall-clock time because requests
are batched and network latency is low — typically 3–5x faster for large
projects. A 10,000-line project usually indexes in under two minutes with
either backend.

**Incremental updates:** Running `index(action: build)` again after editing a few
files is cheap. codescout hashes each file's content and only re-embeds
files whose hash has changed since the last run. Unchanged files are skipped
at negligible cost.

**Force reindex:** Use `force: true` when you change the `model` in
`project.toml`. Vectors from different models are not comparable, so the entire
index must be rebuilt:

```json
{ "name": "index", "arguments": { "action": "build", "force": true } }
```

You can check index health at any time:

```json
{ "name": "workspace", "arguments": { "action": "status" } }
```

The output shows `config.embeddings.model` (from `project.toml`) and
the `index.model` (what was used to build the current index). If they
differ, a force reindex is needed.

## Searching Effectively

### Natural Language Queries

Describe what the code does in plain language. You do not need to know the
function name or file location:

```json
{ "name": "semantic_search", "arguments": { "query": "how errors are handled" } }
```

```json
{ "name": "semantic_search", "arguments": { "query": "database connection setup" } }
```

```json
{ "name": "semantic_search", "arguments": { "query": "authentication token validation" } }
```

Concrete, specific queries outperform vague ones. Prefer "retry logic with
exponential backoff" over "error handling". Prefer "connection pool
initialization" over "database".

### Code Snippet Queries

Paste a function signature or a short snippet as the query to find similar
code elsewhere in the project. This is useful for spotting duplication or
locating the canonical version of a pattern:

```json
{
  "name": "semantic_search",
  "arguments": {
    "query": "fn connect(host: &str, port: u16) -> Result<Connection>"
  }
}
```

### Interpreting Scores

Each result includes a `score` between 0 and 1 (cosine similarity):

| Score range | Interpretation |
|-------------|----------------|
| > 0.85 | Strong match — the chunk directly addresses your query |
| 0.6 – 0.85 | Related — the concept is present but may not be the primary focus |
| < 0.6 | Tangential — treat as background context at best |

The top result is not always the most useful one. Scan the top five results
before drilling into any single chunk.

### Recommended Workflow

Semantic search is the entry point for concept-first exploration. After finding
relevant chunks, use the symbol tools to navigate the surrounding code:

1. `semantic_search` — find the files and line ranges where a concept lives.
2. `symbols` on those files — see the surrounding structure.
3. `symbols` with `include_body: true` — read the exact implementation.
4. `references` — trace callers if needed.

## Tuning

### Chunk Size

Chunk size defaults to **1600 characters** and is configurable via
`[embeddings] chunk_size` in `project.toml`. The AST chunker splits source
files at top-level definition boundaries (functions, methods, classes), so most
chunks are well within budget regardless of the model's context window. The
budget mainly controls when a single oversized leaf node is recursively
line-split into sub-chunks (each sub-chunk keeps a signature-prefix header so
embeddings remain self-describing).

If you change the chunk size, run `index(action: "build", force: true)` to
rebuild — old chunks cannot be mixed with new ones.
### Model Choice

The embedding model has the largest effect on search quality. General-purpose
text models (`nomic-embed-text`, `text-embedding-3-small`) work well for
documentation and comments. Code-specific models
(`jina-embeddings-v2-base-code`) tend to perform better on function signatures
and code identifiers.

After changing the model, always run `index(action: "build", force: true)`.
## Troubleshooting

**"No results" or empty results list**

The index may not be built yet. Run `workspace(action: status)` to check. If
`index.indexed` is false, run `index(action: build)`. If the index exists but results are
empty, the query may be too generic — try a more specific description.

**"Connection refused" when indexing**

An external embedding server (Ollama, llama.cpp, vLLM, TEI, etc.) is not
running or `url` is wrong. Start it, or fix the `url` in
`.codescout/project.toml`. codescout has no in-process embedding fallback.

**"Model not found" error**

The model has not been pulled. For Ollama, run `ollama pull <model-name>` (or
whatever model is configured) and retry.

**Stale results after editing many files**

Run `index(action: build)` without extra arguments. The incremental update will re-embed
only the files that changed.

**Results seem wrong after changing the model**

The index was built with a different model and the vectors are no longer
compatible. Run `index(action: build)` with `force: true`. You can confirm the
mismatch by checking `workspace(action: status)`: if the config model and the index model
differ, a force reindex is required.

**Indexing is very slow**

If using an external server (Ollama, llama.cpp, vLLM, TEI), check it is
running locally and not routing over a slow network connection. For the
fastest throughput on large projects, OpenAI's hosted API
(`text-embedding-3-small`) batches requests efficiently.

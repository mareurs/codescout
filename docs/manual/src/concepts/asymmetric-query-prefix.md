# Asymmetric Query Prefix for Embedding Models
Some embedding models are trained **asymmetrically**: documents and queries use
different input conventions. The canonical example is **CodeRankEmbed**, which
expects every query to be prefixed with:

```
Represent this query for searching relevant code:
```

Without the prefix, query vectors land in a different part of the embedding
space than the indexed document vectors, and `semantic_search` recall drops
sharply (typical effect: 30–50% worse top-5 recall).

## What this does

codescout now distinguishes **document** embedding from **query** embedding at
the `Embedder` trait level. `RemoteEmbedder` automatically prepends the
model-specific query prefix when `semantic_search` runs, while index-time
embedding (through `embed`) stays unprefixed.

The prefix is selected from the model name:

| Model name contains | Query prefix |
|---|---|
| `coderank` (any case) | `Represent this query for searching relevant code: ` |
| *any other model* | none (symmetric) |

Detection is purely name-based — no external config and no registry lookup.

## Configuration

No user-facing configuration. Set your model via `project.toml` as usual:

```toml
[embeddings]
model = "ollama:coderank-embed"
# or any remote endpoint serving a CodeRankEmbed variant
url = "http://127.0.0.1:43300/v1/embeddings"
```

codescout detects the `coderank` substring in the model name and applies the
prefix to every query automatically. Re-index after switching models, since
document vectors are model-specific.

## Trait surface

For library consumers, two points on the `Embedder` trait:

- `embed(&[&str])` — unchanged, document-side batch embedding.
- `embed_query(&str)` — new, single-query embedding with prefix applied.
  Default impl delegates to `embed` with no prefix; `RemoteEmbedder`
  overrides to apply the model-specific prefix.

The free function `embed_one(embedder, text)` now routes through
`embed_query`, so all code paths that embed a single query string benefit
from the prefix automatically.

## Adding a new asymmetric model

Extend `RemoteEmbedder::query_prefix_for` in `src/embed/remote.rs`:

```rust
fn query_prefix_for(model: &str) -> Option<String> {
    let l = model.to_lowercase();
    if l.contains("coderank") {
        Some("Represent this query for searching relevant code: ".into())
    } else if l.contains("your-model") {
        Some("Your model's query prefix: ".into())
    } else {
        None
    }
}
```

No other call site needs to change — the trait default + override pattern
keeps the per-model knowledge localized.

## Limitations

- Local (fastembed) models do not currently apply prefixes. Add an override on
  `LocalEmbedder::embed_query` if you run an asymmetric model locally.
- The match is a simple substring check; if a non-asymmetric model happens to
  contain `coderank` in its name you'll get an incorrect prefix. Rename the
  model or extend the match if this bites you.

## Why this matters

Embedding models are often evaluated on symmetric tasks (document-to-document
similarity). Asymmetric models can score higher on code search benchmarks but
silently underperform when a pipeline treats query and document embedding as
the same operation. Making the distinction explicit at the trait level means
the right thing happens automatically once the model is selected.


# Global Config

<!-- audit-doc-refs:ignore — names config files the reader creates. A clean checkout
     has no .codescout/config.toml, and that is the normal state, not drift. -->

A two-layer configuration system that merges a user-level global config with the
per-project `.codescout/project.toml`, so machine-wide defaults can be set once
without touching every project.

## File locations

<!-- audit-doc-refs:ignore — same reason: user-created config paths. -->

| Layer | Path |
|-------|------|
| Global | `$XDG_CONFIG_HOME/codescout/config.toml` (defaults to `~/.config/codescout/config.toml`) |
| Project | `.codescout/project.toml` in the project root |

The two file names differ, and this table said `config.toml` for both until
2026-09-17. Nothing reads `.codescout/config.toml` — a file placed there has no
effect and produces no warning, which is the worst shape a config error can take.

Project-level values win **per key**. A key present in the project file overrides
the global value for that key only; keys the project does not mention keep the
global value, including keys inside the same table (see *Merge semantics*).

## Supported fields

All fields from the project config are supported in the global config — both
layers deserialise the **same types**, so they cannot drift apart. (Before
2026-09-17 the global `[embeddings]` was a separate struct carrying only `model`;
a global `url` or `api_key` was discarded at parse time with no warning.)

```toml
# ~/.config/codescout/config.toml

[embeddings]
model = "local:AllMiniLML6V2Q"              # or e.g. "openai:text-embedding-3-small"
url = "https://embeddings.example.com/v1"   # any OpenAI-compatible endpoint
api_key = "sk-…"                            # sent only over https or loopback

[security]
max_index_bytes = 524288000   # 500 MB
write_lock_timeout_secs = 30
```

Two notes on the `[embeddings]` example, because the previous version of this page
got both wrong:

- `model` must be a resolvable spec — a `local:` / `local-dir:` / `ollama:` /
  `openai:` prefix, or a bare name when `url` is set. A bare HuggingFace repo id
  is **not** one, and fails with *"Unknown model"*; this page's example used to be
  exactly that.
- There is **no `chunk_size`**. The key still deserialises so old files do not
  break, and it is ignored. `CODESCOUT_CHUNK_TARGET` is the live knob.

## Load behaviour

- Missing global config file is silently ignored (not an error).
- Malformed TOML propagates as an error (fail-fast rather than silent misconfiguration).
- File-size guard rejects configs over 1 MiB (`src/config/global.rs`). This page
  said 64 KB until 2026-09-17; the code has never enforced that.
- `HOME` fallback used when `XDG_CONFIG_HOME` is not set.
- An **unknown key is dropped silently.** That is deliberate — it keeps an older
  file loading after a field is retired — but it means a typo (`modle = …`) costs
  you the setting with no diagnostic. Check that a value took effect rather than
  assuming it did.

## Merge semantics

Tables **are** deep-merged, recursively, key by key. A project config that sets
one field of `[embeddings]` overrides that field and **inherits the rest** from
the global layer:

```toml
# ~/.config/codescout/config.toml
[embeddings]
model = "local:AllMiniLML6V2Q"
url = "https://embeddings.example.com/v1"
api_key = "sk-…"

# <project>/.codescout/project.toml
[embeddings]
model = "openai:text-embedding-3-small"
```

→ the project's `model`, with the global `url` and `api_key`. Scalars are replaced
wholesale; only tables merge.

This section claimed the opposite until 2026-09-17 — that a project `[embeddings]`
replaced the global table entirely — which discouraged exactly the pattern the two
layers exist for. The behaviour is pinned by `merge_toml_base_fills_missing_key`
and, end to end, by `the_global_layer_fills_a_gap_inside_the_project_embeddings_table`.

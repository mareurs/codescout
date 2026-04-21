> ⚠ Experimental — may change without notice.

# Global Config

A two-layer configuration system that merges a user-level global config with
the per-project `.codescout/config.toml`, so workspace-wide defaults can be
set once without touching every project.

## File locations

| Layer | Path |
|-------|------|
| Global | `$XDG_CONFIG_HOME/codescout/config.toml` (defaults to `~/.config/codescout/config.toml`) |
| Project | `.codescout/config.toml` in the project root |

Project-level values always win. Any key present in the project config
overrides the global value for that project.

## Supported fields

All fields from the project config are supported in the global config.
Common use cases:

```toml
# ~/.config/codescout/config.toml

[embeddings]
model = "jinaai/jina-embeddings-v2-base-code"
chunk_size = 1024

[security]
max_index_bytes = 524288000   # 500 MB
write_lock_timeout_secs = 30
```

## Load behaviour

- Missing global config file is silently ignored (not an error).
- Malformed TOML propagates as an error (fail-fast rather than silent misconfiguration).
- File-size guard rejects configs over 64 KB.
- `HOME` fallback used when `XDG_CONFIG_HOME` is not set.

## Merge semantics

Tables are merged key-by-key; scalar values are overridden wholesale. There is
no deep-merge within nested tables — if a project config sets `[embeddings]`,
the entire `[embeddings]` table from the global config is replaced, not merged
field-by-field.

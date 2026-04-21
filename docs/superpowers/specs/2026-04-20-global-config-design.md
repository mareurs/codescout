# Global Config Design

**Date:** 2026-04-20
**Status:** draft

## Problem

codescout has no machine-level config. Every project starts from hardcoded defaults. Users who want the same embedding model, shell policy, or ignored patterns across all projects must repeat the same settings in every `.codescout/project.toml`.

## Goal

A global config file at `~/.config/codescout/config.toml` (XDG-aware) that sets machine-wide defaults. Per-project `project.toml` overrides any field it explicitly sets. Fields absent in both fall back to hardcoded defaults unchanged.

## Merge Semantics

Global = base. Project = overlay. Project wins on any key present in both.

Implemented as a `toml::Value` table merge before deserialization — no `Option<T>` sprawl, no changes to `ProjectConfig` struct. Scalar and array fields are replaced wholesale by the project value (no array unioning).

## Scope — Configurable Fields

Only a curated subset of sections makes sense globally. `[project]` (name, languages) and `[lsp]` are inherently per-project and excluded.

### `[embeddings]`
- `model`
- `drift_detection_enabled`

### `[security]`
- `shell_enabled`
- `shell_command_mode`
- `shell_output_limit_bytes`
- `shell_dangerous_patterns`
- `file_write_enabled`
- `github_enabled`
- `max_index_bytes`
- `indexing_enabled`

Excluded: `profile`, `extra_write_roots`, `write_lock_timeout_secs` — these are per-project trust/path decisions.

### `[ignored_paths]`
- `patterns`

## File Location

Resolved in order:
1. `$XDG_CONFIG_HOME/codescout/config.toml` if `$XDG_CONFIG_HOME` is set
2. `~/.config/codescout/config.toml` otherwise

File is optional — absent = no globals applied.

## Components

### New: `src/config/global.rs`

```rust
pub struct GlobalConfig {
    pub embeddings: Option<GlobalEmbeddingsSection>,
    pub security: Option<GlobalSecuritySection>,
    pub ignored_paths: Option<GlobalIgnoredPathsSection>,
}
```

Each section struct uses `Option<T>` for every field so only explicitly-set values participate in the merge.

- `GlobalConfig::load() -> Result<Option<GlobalConfig>>` — reads XDG path; returns `None` if file absent, `Err` if malformed
- `global_config_path() -> Option<PathBuf>` — resolves XDG path; returns `None` if `$HOME` unset
- `GlobalConfig::to_toml_value() -> toml::Value` — serializes only `Some` fields; absent fields produce no keys (merge-safe)

### Modified: `src/config/project.rs`

`ProjectConfig::load_or_default` grows the two-layer load:

```
1. GlobalConfig::load() -> base toml::Value (empty table if None)
2. load project.toml -> overlay toml::Value
3. merge_toml(base, overlay) -> merged toml::Value
4. toml::from_value::<ProjectConfig>(merged)
```

`merge_toml(base, overlay)` helper: for `toml::Value::Table`, recurse key-by-key; overlay wins on conflict. For all other variants, overlay replaces base.

### Modified: `src/config/mod.rs`

Add `pub mod global`.

## Error Handling

| Condition | Behavior |
|---|---|
| Global file absent | Silently ignored — `None` returned |
| Global file malformed TOML | Hard error with file path in message |
| Unrecognized keys in global file | Silently ignored (forward compat) |
| `$HOME` unset | Skip global config, debug log |
| Project file absent | Unchanged — `default_for(name)` as today |

## Testing

- **`merge_toml` unit tests**: project key wins; global fills gap; nested tables merge correctly; non-table values replaced wholesale
- **`GlobalConfig::load` unit tests**: missing file → `None`; malformed → error with path; valid TOML parses correctly
- **`load_or_default` integration test**: temp global config + temp project.toml; assert merged result has correct field precedence
- Existing `ProjectConfig` tests: no changes needed

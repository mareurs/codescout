# Design: `GatherSource::ConfigValue`

**Date:** 2026-05-02
**Phase:** Augmentation followups — Phase 3
**Tracker:** artifact `79a6276776a1b5da`

## Summary

Add a `ConfigValue` variant to `GatherSource` that reads a single typed value from a TOML, YAML, or JSON config file and annotates it with the last git commit that touched the file. Addresses the backend-kotlin flag-tracker use case: surface a feature flag's current value and when it last changed, without the LLM having to grep.

## Scope

- New `GatherSource::ConfigValue { path, key }` variant in `gather.rs`
- New `gather_config_value` function inline in `gather.rs`
- New `last_changed` helper using `git2::Repository::blame_file`
- Match arm wired into `gather_all`
- 6 unit tests in existing `#[cfg(test)]` module

No new files, no new dependencies.

## Data Shape

### Input (JSON deserialization)

```json
{ "source": "config_value", "path": "Cargo.toml", "key": "package.version" }
```

Fits the existing `#[serde(rename_all = "snake_case", tag = "source")]` pattern on `GatherSource`.

### Output (under `source_key = "config_value"`)

```json
{
  "path": "Cargo.toml",
  "key": "package.version",
  "value": "0.8.1",
  "last_changed_commit": "abc1234def5678",
  "last_changed_at": "2026-04-10T11:32:00+00:00"
}
```

- `value`: native JSON type — string, number, bool, array, or object. Not stringified.
- `last_changed_commit` / `last_changed_at`: `null` if git unavailable, file untracked, or no commits touch the file. Never an error.

## Key Walking

Split `key` on `.` to produce segments. Walk the parsed value tree:

- Segment on an object → field lookup
- Segment on an array → parse as `usize` index; `RecoverableError` if non-numeric
- Segment missing at any level → `RecoverableError` (warning, gather continues)

Examples:
```
"package.version"  →  ["package", "version"]
"servers.0.host"   →  ["servers", "0", "host"]
```

Dotted keys containing literal dots (TOML quoted keys) are not supported. Simple split only.

## Format Detection

By file extension:

| Extension | Parser |
|-----------|--------|
| `.toml` | `toml` crate |
| `.yaml`, `.yml` | `serde_yml` crate |
| `.json` | `serde_json` |
| anything else | `RecoverableError` — warning, skip |

## Git Blame Lookup

```rust
fn last_changed(project_root: &Path, rel_path: &str) -> Option<(String, String)> {
    let repo = git2::Repository::open(project_root).ok()?;
    let blame = repo.blame_file(Path::new(rel_path), None).ok()?;
    let hunk = blame.iter().max_by_key(|h| h.final_signature().when().seconds())?;
    let commit_id = hunk.final_commit_id().to_string();
    let time = hunk.final_signature().when();
    let dt = chrono::DateTime::from_timestamp(time.seconds(), 0)?;
    Some((commit_id, dt.to_rfc3339()))
}
```

Returns `None` on any failure. Caller sets both fields to `json!(null)`. Best-effort, never blocks the gather.

## Error Handling

| Situation | Handling |
|-----------|----------|
| `..` or absolute path | `anyhow::bail!` — security, hard error |
| Unknown extension | `RecoverableError` → warning, gather continues |
| Key not found at any level | `RecoverableError` → warning, gather continues |
| Array index is non-numeric | `RecoverableError` → warning, gather continues |
| File unreadable | `anyhow::bail!` — hard error |
| File parse error (malformed) | `anyhow::bail!` — hard error |
| Git unavailable / untracked | `None` → `null` fields, no error |

## Tests

All in `gather.rs` `#[cfg(test)]` module, using the existing `mk_ctx(&tmp)` helper:

1. `gather_config_value_toml_key_found` — reads `package.version` from a temp `.toml` file; asserts value matches
2. `gather_config_value_yaml_key_found` — reads a nested key from a temp `.yaml` file
3. `gather_config_value_json_key_found` — reads a key from a temp `.json` file
4. `gather_config_value_array_index` — key `"servers.0.host"` walks into an array; asserts correct value
5. `gather_config_value_key_not_found` — missing key returns `Err` (RecoverableError), not panic
6. `gather_config_value_unknown_extension` — `.conf` file returns `Err` (RecoverableError)

No git tests — `last_changed` returns `null` in tempdir context (no repo), tested implicitly by all six cases above.

## Decisions Log

| Question | Decision | Reason |
|----------|----------|--------|
| git2 vs subprocess for last commit | git2 `blame_file` | Already linked, no fork, works without `git` on PATH |
| Quoted dotted keys (TOML `"a.b"`) | Not supported, simple split | YAGNI — real configs don't use this |
| Value type in output | Native JSON type | Lossy stringify is less useful downstream |
| Where code lives | Inline in `gather.rs` | Consistent with existing gather helpers, no new file needed |

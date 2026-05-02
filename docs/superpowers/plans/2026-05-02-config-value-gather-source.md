# GatherSource::ConfigValue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `GatherSource::ConfigValue` so augmented artifacts can surface a single typed value from a TOML/YAML/JSON config file plus the last git commit that touched it.

**Architecture:** New variant in the existing `GatherSource` enum; new `gather_config_value` + `last_changed` helpers inline in `gather.rs`; wired into the `gather_all` dispatch loop. No new files, no new dependencies.

**Tech Stack:** Rust, `toml` crate, `serde_yml` crate, `serde_json`, `git2`, `chrono` — all already in `Cargo.toml`.

---

### Task 1: Add `ConfigValue` variant + deserialization test

**Files:**
- Modify: `crates/librarian-mcp/src/tools/gather.rs:39-40`

- [ ] **Step 1: Write the failing test**

Add inside the `#[cfg(test)]` module at the bottom of `gather.rs` (after `unknown_source_produces_warning`):

```rust
#[test]
fn gather_source_config_value_deserializes() {
    let src: GatherSource = serde_json::from_str(
        r#"{"source":"config_value","path":"Cargo.toml","key":"package.version"}"#,
    )
    .unwrap();
    assert!(matches!(src, GatherSource::ConfigValue { .. }));
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p librarian-mcp gather_source_config_value_deserializes 2>&1 | tail -20
```

Expected: compile error — `ConfigValue` variant does not exist.

- [ ] **Step 3: Add the variant to `GatherSource`**

In `gather.rs`, insert the new variant just before the `#[serde(other)] Unknown` line (line 39):

```rust
    ConfigValue {
        path: String,
        key: String,
    },
```

The enum block should now end with:
```rust
    ConfigValue {
        path: String,
        key: String,
    },
    #[serde(other)]
    Unknown,
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p librarian-mcp gather_source_config_value_deserializes 2>&1 | tail -10
```

Expected: `test tests::gather_source_config_value_deserializes ... ok`

---

### Task 2: Implement `gather_config_value` + `last_changed`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/gather.rs` (add functions after `gather_file`, add tests)

- [ ] **Step 1: Write all six failing tests**

Add after `gather_source_config_value_deserializes` in the `#[cfg(test)]` module:

```rust
#[test]
fn gather_config_value_toml_key_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "[package]\nversion = \"1.2.3\"\n",
    )
    .unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.toml", "package.version").unwrap();
    assert_eq!(result["value"], serde_json::json!("1.2.3"));
    assert_eq!(result["path"], "config.toml");
    assert_eq!(result["key"], "package.version");
}

#[test]
fn gather_config_value_yaml_key_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.yaml"),
        "database:\n  host: localhost\n",
    )
    .unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.yaml", "database.host").unwrap();
    assert_eq!(result["value"], serde_json::json!("localhost"));
}

#[test]
fn gather_config_value_json_key_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.json"),
        r#"{"feature":{"enabled":true}}"#,
    )
    .unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.json", "feature.enabled").unwrap();
    assert_eq!(result["value"], serde_json::json!(true));
}

#[test]
fn gather_config_value_array_index() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.json"),
        r#"{"servers":[{"host":"a"},{"host":"b"}]}"#,
    )
    .unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.json", "servers.1.host").unwrap();
    assert_eq!(result["value"], serde_json::json!("b"));
}

#[test]
fn gather_config_value_key_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("config.toml"), "[package]\nname = \"x\"\n").unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.toml", "package.missing_key");
    assert!(result.is_err());
}

#[test]
fn gather_config_value_unknown_extension() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("config.conf"), "key=value\n").unwrap();
    let ctx = mk_ctx(&tmp);
    let result = gather_config_value(&ctx, "config.conf", "key");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run all six tests to verify they fail**

```bash
cargo test -p librarian-mcp gather_config_value 2>&1 | tail -20
```

Expected: compile error — `gather_config_value` does not exist.

- [ ] **Step 3: Add `last_changed` helper after `gather_file` (around line 282)**

Insert after the closing `}` of `gather_file`:

```rust
fn last_changed(project_root: &std::path::Path, rel_path: &str) -> Option<(String, String)> {
    let repo = git2::Repository::open(project_root).ok()?;
    let blame = repo.blame_file(std::path::Path::new(rel_path), None).ok()?;
    let hunk = blame
        .iter()
        .max_by_key(|h| h.final_signature().when().seconds())?;
    let commit_id = hunk.final_commit_id().to_string();
    let seconds = hunk.final_signature().when().seconds();
    use chrono::TimeZone as _;
    let dt = chrono::Utc.timestamp_opt(seconds, 0).single()?;
    Some((commit_id, dt.to_rfc3339()))
}
```

- [ ] **Step 4: Add `gather_config_value` after `last_changed`**

```rust
fn gather_config_value(ctx: &ToolContext, path: &str, key: &str) -> anyhow::Result<serde_json::Value> {
    guard_relative_path(path)?;
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let full = base.join(path);
    let content = std::fs::read_to_string(&full)
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {e}", full.display()))?;

    let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut val: serde_json::Value = match ext {
        "toml" => {
            let parsed: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("TOML parse error in '{path}': {e}"))?;
            serde_json::to_value(parsed)?
        }
        "yaml" | "yml" => {
            let parsed: serde_yml::Value = serde_yml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("YAML parse error in '{path}': {e}"))?;
            serde_json::to_value(parsed)?
        }
        "json" => serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("JSON parse error in '{path}': {e}"))?,
        other => anyhow::bail!("unsupported config extension '.{other}' for '{path}'"),
    };

    for segment in key.split('.') {
        val = match val {
            serde_json::Value::Object(map) => map
                .get(segment)
                .ok_or_else(|| anyhow::anyhow!("key '{segment}' not found in '{path}'"))?
                .clone(),
            serde_json::Value::Array(arr) => {
                let idx: usize = segment.parse().map_err(|_| {
                    anyhow::anyhow!("array index '{segment}' is not a number in '{path}'")
                })?;
                arr.get(idx)
                    .ok_or_else(|| {
                        anyhow::anyhow!("array index {idx} out of bounds in '{path}'")
                    })?
                    .clone()
            }
            _ => anyhow::bail!(
                "cannot traverse into scalar at segment '{segment}' in '{path}'"
            ),
        };
    }

    let (commit, at) = last_changed(&base, path)
        .map(|(c, a)| (json!(c), json!(a)))
        .unwrap_or((json!(null), json!(null)));

    Ok(json!({
        "path": path,
        "key": key,
        "value": val,
        "last_changed_commit": commit,
        "last_changed_at": at,
    }))
}
```

- [ ] **Step 5: Run all six tests to verify they pass**

```bash
cargo test -p librarian-mcp gather_config_value 2>&1 | tail -15
```

Expected: all 6 tests pass. `last_changed_commit` and `last_changed_at` will be `null` in tempdir context (no git repo) — that is correct.

---

### Task 3: Wire `ConfigValue` into `gather_all`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/gather.rs:gather_all`

- [ ] **Step 1: Write the failing integration test**

Add to the `#[cfg(test)]` module:

```rust
#[test]
fn gather_all_config_value_source() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("app.toml"),
            "[server]\nport = 8080\n",
        )
        .unwrap();
        let ctx = mk_ctx(&tmp);
        let sources = vec![GatherSource::ConfigValue {
            path: "app.toml".to_string(),
            key: "server.port".to_string(),
        }];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_key, "config_value");
        assert_eq!(results[0].data["value"], serde_json::json!(8080));
    });
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p librarian-mcp gather_all_config_value_source 2>&1 | tail -15
```

Expected: compile error — `ConfigValue` not handled in `gather_all` match (non-exhaustive patterns).

- [ ] **Step 3: Add the match arm in `gather_all`**

In `gather_all`, add the new arm just before the `GatherSource::Unknown` arm:

```rust
            GatherSource::ConfigValue { path, key } => {
                match gather_config_value(ctx, path, key) {
                    Ok(data) => results.push(GatherResult {
                        source_key: "config_value".to_string(),
                        data,
                    }),
                    Err(e) => warnings.push(format!(
                        "config_value gather failed for '{path}': {e}"
                    )),
                }
            }
```

- [ ] **Step 4: Run to verify it passes**

```bash
cargo test -p librarian-mcp gather_all_config_value_source 2>&1 | tail -10
```

Expected: `test tests::gather_all_config_value_source ... ok`

---

### Task 4: Full test suite, fmt, clippy, commit

**Files:** none new

- [ ] **Step 1: Run full librarian-mcp test suite**

```bash
cargo test -p librarian-mcp 2>&1 | tail -20
```

Expected: all tests pass, count increases by 7 from baseline (292 → 299).

- [ ] **Step 2: Run fmt**

```bash
cargo fmt -p librarian-mcp
```

Expected: no output (or reformatted files with no errors).

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -p librarian-mcp -- -D warnings 2>&1 | tail -20
```

Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/librarian-mcp/src/tools/gather.rs
git commit -m "feat(augmentation): GatherSource::ConfigValue

Reads a single typed value from TOML/YAML/JSON config files.
Annotates output with last git commit that touched the file (git2 blame).
Key path: dotted segments, numeric index for arrays.
Missing key or unknown extension → warning, gather continues.

7 new tests (deserialization, TOML/YAML/JSON extraction, array index,
error cases, gather_all integration).

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

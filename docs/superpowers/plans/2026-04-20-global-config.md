# Global Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `~/.config/codescout/config.toml` as a machine-wide config layer that sets defaults for any project, overridable per-project via `.codescout/project.toml`.

**Architecture:** New `src/config/global.rs` holds `GlobalConfig` (all-`Option<T>` fields, serializes only `Some` values). `ProjectConfig::load_or_default` does a two-layer TOML merge — global as base, project as overlay — before deserializing into the existing `ProjectConfig` struct. The `merge_toml` helper recurses into tables; scalars/arrays are replaced wholesale by the overlay.

**Tech Stack:** `toml` (already a dependency), `serde` with `skip_serializing_if`, standard `std::env` for XDG path resolution.

---

## File Map

| File | Change |
|---|---|
| `src/config/global.rs` | Create: `GlobalConfig`, section structs, `global_config_path`, `GlobalConfig::load`, `GlobalConfig::to_toml_value` |
| `src/config/project.rs` | Modify: add `merge_toml` helper, rewrite `load_or_default` |
| `src/config/mod.rs` | Modify: add `pub mod global` |

---

### Task 1: `merge_toml` helper + unit tests

**Files:**
- Modify: `src/config/project.rs` (add helper + tests in existing `tests` module)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/config/project.rs`:

```rust
#[test]
fn merge_toml_overlay_wins_scalar() {
    let base = toml::toml! { [embeddings] model = "base-model" };
    let overlay = toml::toml! { [embeddings] model = "project-model" };
    let merged = super::merge_toml(base, overlay);
    assert_eq!(merged["embeddings"]["model"].as_str(), Some("project-model"));
}

#[test]
fn merge_toml_base_fills_missing_key() {
    let base = toml::toml! { [embeddings] model = "global-model" };
    let overlay = toml::toml! { [embeddings] drift_detection_enabled = false };
    let merged = super::merge_toml(base, overlay);
    assert_eq!(merged["embeddings"]["model"].as_str(), Some("global-model"));
    assert_eq!(merged["embeddings"]["drift_detection_enabled"].as_bool(), Some(false));
}

#[test]
fn merge_toml_nested_tables_merge_recursively() {
    let base = toml::toml! { [security] shell_enabled = false \n shell_command_mode = "warn" };
    let overlay = toml::toml! { [security] shell_command_mode = "unrestricted" };
    let merged = super::merge_toml(base, overlay);
    assert_eq!(merged["security"]["shell_enabled"].as_bool(), Some(false));
    assert_eq!(merged["security"]["shell_command_mode"].as_str(), Some("unrestricted"));
}

#[test]
fn merge_toml_non_table_overlay_replaces_base() {
    let base = toml::Value::String("base".into());
    let overlay = toml::Value::String("overlay".into());
    let merged = super::merge_toml(base, overlay);
    assert_eq!(merged.as_str(), Some("overlay"));
}

#[test]
fn merge_toml_empty_overlay_returns_base() {
    let base = toml::toml! { [embeddings] model = "base-model" };
    let overlay = toml::Value::Table(toml::map::Map::new());
    let merged = super::merge_toml(base, overlay);
    assert_eq!(merged["embeddings"]["model"].as_str(), Some("base-model"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test merge_toml 2>&1
```

Expected: compile error — `merge_toml` not defined.

- [ ] **Step 3: Add `merge_toml` to `src/config/project.rs`**

Add just before `impl ProjectConfig` (around line 405):

```rust
fn merge_toml(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base_map), toml::Value::Table(overlay_map)) => {
            for (k, v) in overlay_map {
                let merged = if let Some(base_val) = base_map.remove(&k) {
                    merge_toml(base_val, v)
                } else {
                    v
                };
                base_map.insert(k, merged);
            }
            toml::Value::Table(base_map)
        }
        (_, overlay) => overlay,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test merge_toml 2>&1
```

Expected: `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(config): add merge_toml helper for two-layer config merge"
```

---

### Task 2: `src/config/global.rs` — structs, path resolution, load

**Files:**
- Create: `src/config/global.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/config/global.rs` with only the test module (will fail to compile until structs exist):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn global_config_path_uses_xdg_config_home() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg");
        let path = global_config_path().unwrap();
        assert_eq!(path, std::path::PathBuf::from("/tmp/xdg/codescout/config.toml"));
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn global_config_load_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        // Override HOME so global_config_path points into our temp dir
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn global_config_load_parses_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        std::fs::write(&config_path, "[embeddings]\nmodel = \"local:BGESmallENV15\"\n").unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap().unwrap();
        assert_eq!(result.embeddings.model, Some("local:BGESmallENV15".to_string()));
    }

    #[test]
    fn global_config_load_errors_on_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        std::fs::write(&config_path, "[embeddings\nbroken toml").unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("config.toml"), "error should mention file path: {msg}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test global_config 2>&1
```

Expected: compile error — structs not defined.

- [ ] **Step 3: Implement `src/config/global.rs`**

Replace the file contents with:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub embeddings: GlobalEmbeddingsSection,
    #[serde(default)]
    pub security: GlobalSecuritySection,
    #[serde(default)]
    pub ignored_paths: GlobalIgnoredPathsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalEmbeddingsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_detection_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSecuritySection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_output_limit_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_dangerous_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_write_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_index_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalIgnoredPathsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<String>>,
}

pub fn global_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("codescout").join("config.toml"))
}

impl GlobalConfig {
    pub fn load() -> Result<Option<Self>> {
        let path = match global_config_path() {
            Some(p) => p,
            None => {
                tracing::debug!("$HOME not set, skipping global config");
                return Ok(None);
            }
        };
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading global config {}", path.display()))?;
        let config: GlobalConfig = toml::from_str(&text)
            .with_context(|| format!("parsing global config {}", path.display()))?;
        Ok(Some(config))
    }

    pub fn to_toml_value(&self) -> toml::Value {
        toml::Value::try_from(self)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn global_config_path_uses_xdg_config_home() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg");
        let path = global_config_path().unwrap();
        assert_eq!(path, std::path::PathBuf::from("/tmp/xdg/codescout/config.toml"));
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn global_config_load_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn global_config_load_parses_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "[embeddings]\nmodel = \"local:BGESmallENV15\"\n").unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap().unwrap();
        assert_eq!(result.embeddings.model, Some("local:BGESmallENV15".to_string()));
    }

    #[test]
    fn global_config_load_errors_on_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "[embeddings\nbroken toml").unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("config.toml"), "error should mention file path: {msg}");
    }

    #[test]
    fn to_toml_value_emits_only_some_fields() {
        let config = GlobalConfig {
            embeddings: GlobalEmbeddingsSection {
                model: Some("local:BGESmallENV15".to_string()),
                drift_detection_enabled: None,
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert!(val["embeddings"]["model"].as_str().is_some());
        assert!(val.get("embeddings")
            .and_then(|e| e.get("drift_detection_enabled"))
            .is_none());
    }

    #[test]
    fn to_toml_value_security_emits_set_fields() {
        let config = GlobalConfig {
            security: GlobalSecuritySection {
                shell_enabled: Some(false),
                github_enabled: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert_eq!(val["security"]["shell_enabled"].as_bool(), Some(false));
        assert_eq!(val["security"]["github_enabled"].as_bool(), Some(true));
        assert!(val.get("security")
            .and_then(|s| s.get("shell_command_mode"))
            .is_none());
    }
}
```

- [ ] **Step 4: Add `pub mod global` to `src/config/mod.rs`**

```rust
pub mod global;
pub mod project;
pub mod workspace;
```

- [ ] **Step 5: Run tests**

```bash
cargo test global_config 2>&1
```

Expected: `4 passed`.

- [ ] **Step 6: Commit**

```bash
git add src/config/global.rs src/config/mod.rs
git commit -m "feat(config): add GlobalConfig with XDG path resolution and load"
```

---

### Task 4: Wire `load_or_default` — two-layer merge

**Files:**
- Modify: `src/config/project.rs`

- [ ] **Step 1: Write the failing integration tests**

Add to the `tests` module in `src/config/project.rs`:

```rust
#[test]
fn load_or_default_applies_global_when_project_absent() {
    let dir = tempfile::tempdir().unwrap();
    // Write global config
    let global_dir = dir.path().join(".config").join("codescout");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(
        global_dir.join("config.toml"),
        "[embeddings]\nmodel = \"local:BGESmallENV15\"\n",
    ).unwrap();
    std::env::set_var("HOME", dir.path());
    std::env::remove_var("XDG_CONFIG_HOME");

    // No project.toml in project dir
    let project_dir = dir.path().join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
    assert_eq!(cfg.embeddings.model, "local:BGESmallENV15");
    assert_eq!(cfg.project.name, "my-project");
}

#[test]
fn load_or_default_project_wins_over_global() {
    let dir = tempfile::tempdir().unwrap();
    let global_dir = dir.path().join(".config").join("codescout");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(
        global_dir.join("config.toml"),
        "[embeddings]\nmodel = \"global-model\"\n",
    ).unwrap();
    std::env::set_var("HOME", dir.path());
    std::env::remove_var("XDG_CONFIG_HOME");

    let project_dir = dir.path().join("proj");
    let codescout_dir = project_dir.join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    std::fs::write(
        codescout_dir.join("project.toml"),
        "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
    ).unwrap();

    let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
    assert_eq!(cfg.embeddings.model, "project-model");
}

#[test]
fn load_or_default_global_fills_gap_in_project() {
    let dir = tempfile::tempdir().unwrap();
    let global_dir = dir.path().join(".config").join("codescout");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(
        global_dir.join("config.toml"),
        "[security]\nshell_enabled = false\n",
    ).unwrap();
    std::env::set_var("HOME", dir.path());
    std::env::remove_var("XDG_CONFIG_HOME");

    let project_dir = dir.path().join("proj");
    let codescout_dir = project_dir.join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    std::fs::write(
        codescout_dir.join("project.toml"),
        "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
    ).unwrap();

    let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
    // Global fills in security.shell_enabled=false even though project.toml didn't set it
    assert!(!cfg.security.shell_enabled);
    // Project's embeddings.model still wins
    assert_eq!(cfg.embeddings.model, "project-model");
}

#[test]
fn load_or_default_no_global_behaves_as_before() {
    let dir = tempfile::tempdir().unwrap();
    // Point HOME somewhere with no codescout config
    std::env::set_var("HOME", dir.path());
    std::env::remove_var("XDG_CONFIG_HOME");

    let project_dir = dir.path().join("proj");
    let codescout_dir = project_dir.join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    std::fs::write(
        codescout_dir.join("project.toml"),
        "[project]\nname = \"proj\"\n",
    ).unwrap();

    let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
    // Hardcoded default model when no global config
    assert_eq!(cfg.embeddings.model, super::default_embed_model());
    assert!(cfg.security.shell_enabled);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test load_or_default_applies_global 2>&1
```

Expected: `FAILED` — `load_or_default` doesn't call `GlobalConfig::load` yet.

- [ ] **Step 3: Rewrite `load_or_default` in `src/config/project.rs`**

Use `replace_symbol` to replace the `load_or_default` method body. The new implementation:

```rust
pub fn load_or_default(root: &Path) -> Result<Self> {
    use crate::config::global::GlobalConfig;

    let global_base: toml::Value = GlobalConfig::load()?
        .map(|g| g.to_toml_value())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));

    let config_path = root.join(".codescout").join("project.toml");

    let project_overlay: toml::Value = if config_path.exists() {
        let metadata = std::fs::metadata(&config_path)?;
        if metadata.len() > 1024 * 1024 {
            anyhow::bail!(
                "project.toml exceeds 1 MiB limit ({} bytes)",
                metadata.len()
            );
        }
        let text = std::fs::read_to_string(&config_path)?;
        toml::from_str(&text)?
    } else {
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let mut project_map = toml::map::Map::new();
        project_map.insert("name".to_string(), toml::Value::String(name));
        let mut root_map = toml::map::Map::new();
        root_map.insert("project".to_string(), toml::Value::Table(project_map));
        toml::Value::Table(root_map)
    };

    let merged = merge_toml(global_base, project_overlay);
    Ok(toml::Value::try_into(merged)?)
}
```

- [ ] **Step 4: Run all new integration tests**

```bash
cargo test "load_or_default_applies_global\|load_or_default_project_wins\|load_or_default_global_fills\|load_or_default_no_global" 2>&1
```

Expected: `4 passed`.

- [ ] **Step 5: Run the full test suite to catch regressions**

```bash
cargo test 2>&1
```

Expected: all existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(config): wire two-layer global+project merge into load_or_default"
```

---

### Task 5: Polish — fmt, clippy, full verification

**Files:** None new.

- [ ] **Step 1: Format**

```bash
cargo fmt 2>&1
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy -- -D warnings 2>&1
```

Fix any warnings before proceeding.

- [ ] **Step 3: Full test run**

```bash
cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 4: Commit fmt/clippy fixes if any**

```bash
git add -p
git commit -m "style(config): cargo fmt after global config implementation"
```

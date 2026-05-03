# Activation Stale Onboarding Warning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject a `system_prompt_stale` warning into the `activate_project` response when the stored onboarding version is behind the compiled `ONBOARDING_VERSION`, so the LLM sees it at session start without needing to call `onboarding()` first.

**Architecture:** `build_activation_response` in `src/tools/config/mod.rs` already reads project config via `with_project`; we add `onboarding_version` to that read, call the existing `onboarding_version_stale` predicate, and inject a `system_prompt_stale` JSON field when true. `format_activate_project` gets a stale-warning prefix in compact output.

**Tech Stack:** Rust, serde_json, existing `onboarding_version_stale` + `ONBOARDING_VERSION` from `src/tools/onboarding.rs` (already `pub(crate)`).

---

## File Map

| File | Change |
|------|--------|
| `src/tools/config/mod.rs` | Add import; extend `build_activation_response` tuple; inject `system_prompt_stale`; update `format_activate_project` |
| `src/tools/config/tests.rs` | Add 4 new tests |

---

### Task 1: Failing tests for `build_activation_response` stale detection

**Files:**
- Modify: `src/tools/config/tests.rs`

- [ ] **Step 1: Add two failing integration tests**

Append to `src/tools/config/tests.rs`:

```rust
#[tokio::test]
async fn activation_response_includes_stale_warning_when_no_stored_version() {
    // No project.toml → onboarding_version = None → stale
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let stale = &result["system_prompt_stale"];
    assert!(stale.is_object(), "system_prompt_stale missing; got: {result}");
    assert!(
        stale["stored_version"].is_null(),
        "stored_version should be null for None"
    );
    assert_eq!(
        stale["current_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION as u64
    );
    assert!(
        stale["action"].as_str().unwrap().contains("refresh_prompt"),
        "action should mention refresh_prompt"
    );
}

#[tokio::test]
async fn activation_response_no_stale_warning_when_version_current() {
    let dir = tempdir().unwrap();
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    // Write project.toml with current onboarding version
    std::fs::write(
        cs_dir.join("project.toml"),
        format!(
            "name = \"test\"\nlanguages = []\nonboarding_version = {}\n",
            crate::tools::onboarding::ONBOARDING_VERSION
        ),
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert!(
        result["system_prompt_stale"].is_null(),
        "system_prompt_stale should be absent; got: {result}"
    );
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test activation_response_includes_stale_warning_when_no_stored_version activation_response_no_stale_warning_when_version_current 2>&1 | tail -20
```

Expected: both fail (field missing / `system_prompt_stale` not yet injected).

---

### Task 2: Implement stale detection in `build_activation_response`

**Files:**
- Modify: `src/tools/config/mod.rs`

- [ ] **Step 1: Add import at top of file**

In `src/tools/config/mod.rs`, change the existing imports block from:

```rust
use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};
use std::path::PathBuf;
```

to:

```rust
use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
use crate::tools::onboarding::{onboarding_version_stale, ONBOARDING_VERSION};
use serde_json::{json, Value};
use std::path::PathBuf;
```

- [ ] **Step 2: Extend the `with_project` tuple in `build_activation_response`**

In `build_activation_response`, replace the destructuring assignment (the block starting with `let (` through `).await?;`) with:

```rust
let (
    project_name,
    project_root_str,
    project_root_path,
    languages,
    read_only,
    memories,
    has_index,
    security,
    stored_onboarding_version,
) = ctx
    .agent
    .with_project(|p| {
        let memories = p.memory.list().unwrap_or_default();
        let has_index = crate::embed::index::project_db_path(&p.root).exists();
        let security = if !p.read_only {
            Some((p.config.security.profile, p.config.security.shell_enabled))
        } else {
            None
        };
        Ok((
            p.config.project.name.clone(),
            p.root.display().to_string(),
            p.root.clone(),
            p.config.project.languages.clone(),
            p.read_only,
            memories,
            has_index,
            security,
            p.config.project.onboarding_version,
        ))
    })
    .await?;
```

- [ ] **Step 3: Compute staleness flag after the `with_project` block**

Immediately after the destructuring (before the `let index =` line), add:

```rust
let version_stale = onboarding_version_stale(stored_onboarding_version);
```

- [ ] **Step 4: Inject `system_prompt_stale` into the result**

After the `let mut result = json!({ ... });` block (after all the `if let Some(ws)`, `if let Some((profile, shell))`, and `if !auto_registered` blocks, but before `Ok(result)`), add:

```rust
if version_stale {
    result["system_prompt_stale"] = json!({
        "stored_version": stored_onboarding_version,
        "current_version": ONBOARDING_VERSION,
        "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
    });
}
```

- [ ] **Step 5: Run the two tests from Task 1 to confirm they now pass**

```bash
cargo test activation_response_includes_stale_warning_when_no_stored_version activation_response_no_stale_warning_when_version_current 2>&1 | tail -20
```

Expected: both PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tools/config/mod.rs src/tools/config/tests.rs
git commit -m "feat(config): inject system_prompt_stale warning in activate_project when onboarding version behind"
```

---

### Task 3: Failing tests for `format_activate_project` warning prefix

**Files:**
- Modify: `src/tools/config/tests.rs`

- [ ] **Step 1: Add two failing unit tests**

Append to `src/tools/config/tests.rs`:

```rust
#[test]
fn format_activate_project_prepends_warning_when_stale() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "system_prompt_stale": {
            "stored_version": 20,
            "current_version": 22,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ SYSTEM PROMPT STALE (v20 → v22):"),
        "compact should start with stale warning but was: {compact}"
    );
    assert!(
        compact.contains("activated · my-project (rw)"),
        "compact should still contain activation summary but was: {compact}"
    );
}

#[test]
fn format_activate_project_no_warning_when_current() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        !compact.contains("STALE"),
        "no stale warning expected but was: {compact}"
    );
    assert_eq!(
        compact,
        "activated · my-project (rw) · 1 memories · index: not_indexed"
    );
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test format_activate_project_prepends_warning_when_stale format_activate_project_no_warning_when_current 2>&1 | tail -20
```

Expected: `format_activate_project_prepends_warning_when_stale` fails (no prefix yet).

---

### Task 4: Implement warning prefix in `format_activate_project`

**Files:**
- Modify: `src/tools/config/mod.rs`

- [ ] **Step 1: Update `format_activate_project`**

Replace the entire `format_activate_project` function body with:

```rust
fn format_activate_project(result: &Value) -> String {
    let name = result["project"].as_str().unwrap_or("?");
    let ro = result["read_only"].as_bool().unwrap_or(true);
    let mode = if ro { "ro" } else { "rw" };
    let mem_count = result["memories"].as_array().map(|a| a.len()).unwrap_or(0);
    let index_status = result["index"]["status"].as_str().unwrap_or("unknown");

    let mut parts = vec![format!(
        "activated · {name} ({mode}) · {mem_count} memories · index: {index_status}"
    )];

    if let Some(ws) = result["workspace"].as_array() {
        parts.push(format!("{} workspace projects", ws.len()));
    }

    if let Some(libs) = result["auto_registered_libs"].as_object() {
        let count = libs.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let without = libs
            .get("without_source")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if without > 0 {
            parts.push(format!(
                "auto-registered {} libs ({} without source)",
                count, without
            ));
        } else {
            parts.push(format!("auto-registered {} libs", count));
        }
    }

    let body = parts.join(" · ");

    if let Some(stale) = result["system_prompt_stale"].as_object() {
        let stored = stale.get("stored_version").and_then(|v| v.as_u64()).unwrap_or(0);
        let current = stale
            .get("current_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        format!(
            "⚠ SYSTEM PROMPT STALE (v{stored} → v{current}): run onboarding(action=\"refresh_prompt\") now.\n{body}"
        )
    } else {
        body
    }
}
```

- [ ] **Step 2: Run all four new tests**

```bash
cargo test format_activate_project_prepends_warning_when_stale format_activate_project_no_warning_when_current activation_response_includes_stale_warning_when_no_stored_version activation_response_no_stale_warning_when_version_current 2>&1 | tail -20
```

Expected: all 4 PASS.

- [ ] **Step 3: Run the full test suite**

```bash
cargo test 2>&1 | tail -30
```

Expected: all pass, no regressions.

- [ ] **Step 4: Clippy + fmt**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/tools/config/mod.rs src/tools/config/tests.rs
git commit -m "feat(config): prepend stale-onboarding warning in format_activate_project compact output"
```

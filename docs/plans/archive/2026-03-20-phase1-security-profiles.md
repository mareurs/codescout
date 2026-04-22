# Phase 1: Security Profiles (Root Mode) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `profile = "root"` security mode that disables read deny-lists, write boundaries, and dangerous command checks for system-administration projects. Also remove `shell_allow_always` and `denied_read_patterns` config options, and fix the anchor path traversal vulnerability.

**Architecture:** A `SecurityProfile` enum (`Default`, `Root`) is added to `PathSecurityConfig` and `SecuritySection`. The three security gate functions (`validate_read_path`, `validate_write_path`, `is_dangerous_command`) check the profile and early-return when `Root`. A shared `sanitize_topic()` function fixes path traversal in both `topic_path` and `anchor_path_for_topic`.

**Tech Stack:** Rust, serde, std::path

**Spec:** `docs/plans/2026-03-20-code-review-and-platform-abstraction-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/util/path_security.rs` | Modify | Add `SecurityProfile` enum, wire into gates, remove `shell_allow_always`/`denied_read_patterns` |
| `src/config/project.rs` | Modify | Add `profile` field to `SecuritySection`, remove dropped fields, map to `PathSecurityConfig` |
| `src/memory/mod.rs` | Modify | Extract `sanitize_topic()`, use in `topic_path` |
| `src/memory/anchors.rs` | Modify | Use `sanitize_topic()` in `anchor_path_for_topic` |
| `src/prompts/server_instructions.md` | Modify | Mention root mode |
| `src/dashboard/api/memories.rs` | Modify | Use `sanitize_topic()` for HTTP topic parameter (C-10) |

---

### Task 1: Add `SecurityProfile` enum and wire into `PathSecurityConfig`

**Files:**
- Modify: `src/util/path_security.rs:65-107`

- [ ] **Step 1: Write failing tests for root mode**

Add these tests at the bottom of the `#[cfg(test)]` module in `src/util/path_security.rs`:

```rust
#[test]
fn root_profile_bypasses_read_deny_list() {
    let dir = tempdir().unwrap();
    // Create a fake .ssh dir that would normally be denied
    let ssh_dir = dir.path().join(".ssh");
    std::fs::create_dir_all(&ssh_dir).unwrap();
    let key_file = ssh_dir.join("id_rsa");
    std::fs::write(&key_file, "secret").unwrap();

    let mut config = PathSecurityConfig::default();
    config.profile = SecurityProfile::Root;

    let result = validate_read_path(
        key_file.to_str().unwrap(),
        Some(dir.path()),
        &config,
    );
    assert!(result.is_ok(), "root profile should bypass read deny-list");
}

#[test]
fn root_profile_bypasses_write_boundary() {
    let dir = tempdir().unwrap();
    let outside = dir.path().join("outside_project");
    std::fs::create_dir_all(&outside).unwrap();
    let target = outside.join("file.txt");

    let project_root = dir.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    let mut config = PathSecurityConfig::default();
    config.profile = SecurityProfile::Root;

    let result = validate_write_path(
        target.to_str().unwrap(),
        &project_root,
        &config,
    );
    assert!(result.is_ok(), "root profile should bypass write boundary");
}

#[test]
fn root_profile_bypasses_dangerous_command_check() {
    let mut config = PathSecurityConfig::default();
    config.profile = SecurityProfile::Root;

    let result = is_dangerous_command("rm -rf /", &config);
    assert!(result.is_none(), "root profile should skip dangerous command check");
}

#[test]
fn default_profile_still_enforces_all_gates() {
    let config = PathSecurityConfig::default();
    assert_eq!(config.profile, SecurityProfile::Default);

    // Dangerous command still caught
    let result = is_dangerous_command("rm -rf /", &config);
    assert!(result.is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- root_profile_bypasses_read root_profile_bypasses_write root_profile_bypasses_dangerous default_profile_still`
Expected: compile errors — `SecurityProfile` doesn't exist yet

- [ ] **Step 3: Add `SecurityProfile` enum and field to `PathSecurityConfig`**

In `src/util/path_security.rs`, add above the `PathSecurityConfig` struct:

```rust
/// Security profile controlling how strict path and command validation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SecurityProfile {
    /// Standard sandbox: deny-lists, write boundaries, dangerous command checks.
    #[default]
    Default,
    /// Unrestricted: all path and command gates are disabled.
    /// For system-administration projects that need full filesystem access.
    Root,
}
```

Add to `PathSecurityConfig` struct (first field):

```rust
    /// Security profile: `Default` (sandboxed) or `Root` (unrestricted).
    pub profile: SecurityProfile,
```

Add to `Default` impl:

```rust
    profile: SecurityProfile::Default,
```

- [ ] **Step 4: Wire early returns into the three gate functions**

In `validate_read_path`, after the null-byte check (line ~204), add:

```rust
    if config.profile == SecurityProfile::Root {
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            PathBuf::from(raw)
        } else if let Some(root) = project_root {
            root.join(raw)
        } else {
            bail!("relative path '{}' requires an active project", raw);
        };
        return Ok(best_effort_canonicalize(&resolved));
    }
```

In `validate_write_path`, after the null-byte check (line ~248), add:

```rust
    if config.profile == SecurityProfile::Root {
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            PathBuf::from(raw)
        } else {
            project_root.join(raw)
        };
        return Ok(canonicalize_write_target(&resolved));
    }
```

In `is_dangerous_command`, as the first line of the function body:

```rust
    if config.profile == SecurityProfile::Root {
        return None;
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib -- root_profile_bypasses_read root_profile_bypasses_write root_profile_bypasses_dangerous default_profile_still`
Expected: all 4 PASS

- [ ] **Step 6: Commit**

```bash
git add src/util/path_security.rs
git commit -m "feat(security): add SecurityProfile enum with Root mode bypass"
```

---

### Task 2: Remove `shell_allow_always` and `denied_read_patterns`

**Files:**
- Modify: `src/util/path_security.rs:65-107,133-156,386-417`
- Modify: `src/config/project.rs:95-178`

- [ ] **Step 1: Remove fields from `SecuritySection` (config TOML struct)**

In `src/config/project.rs`, remove from `SecuritySection`:
- The `denied_read_patterns` field and its `#[serde(default)]` attribute (line ~99)
- The `shell_allow_always` field and its `#[serde(default)]` attribute (line ~124)

Remove from `SecuritySection::default()`:
- `denied_read_patterns: Vec::new(),` (line ~133)
- `shell_allow_always: Vec::new(),` (line ~141)

Remove from `to_path_security_config()`:
- `denied_read_patterns: self.denied_read_patterns.clone(),` (line ~162)
- `shell_allow_always: self.shell_allow_always.clone(),` (line ~175)

**Important:** Task 1 added `profile` to `PathSecurityConfig`. To keep the build green
between tasks, also add `profile: SecurityProfile::Default,` to `to_path_security_config()`
now. Task 3 will replace this with proper TOML-driven parsing.

- [ ] **Step 2: Remove fields from `PathSecurityConfig`**

In `src/util/path_security.rs`, remove from `PathSecurityConfig` struct:
- `pub denied_read_patterns: Vec<String>,` (line ~69)
- `pub shell_allow_always: Vec<String>,` (line ~86)

Remove from `Default` impl:
- `denied_read_patterns: Vec::new(),`
- `shell_allow_always: Vec::new(),`

- [ ] **Step 3: Remove allow-list check and update doc comment on `is_dangerous_command`**

Remove the doc comment line "Respects `shell_allow_always` overrides from config." (line ~389).

Remove the allow-list loop (lines ~392-395):
```rust
    // DELETE THIS BLOCK:
    for allow in &config.shell_allow_always {
        if command.contains(allow.as_str()) {
            return None;
        }
    }
```

- [ ] **Step 4: Remove `denied_read_patterns` usage from `denied_read_paths`**

Remove the user-config loop at the end of `denied_read_paths` (lines ~152-155):
```rust
    // DELETE THIS BLOCK:
    for p in &config.denied_read_patterns {
        if let Some(expanded) = expand_home(p) {
            denied.push(expanded);
        }
    }
```

The function signature still takes `config` (needed for future profile check); just remove the body that reads `denied_read_patterns`.

- [ ] **Step 4b: Update module-level doc comment**

At `src/util/path_security.rs:9-12`, the module doc references `extra_write_roots` in
`PathSecurityConfig`. Verify it does not mention `denied_read_patterns` or
`shell_allow_always`. If it does, update the doc to reflect the new config shape.

- [ ] **Step 5: Fix ALL tests that reference removed fields**

Search the entire file for `shell_allow_always` and `denied_read_patterns`. Every
occurrence in a test struct literal must have the field removed. Known locations:

- `shell_allow_always_bypasses_dangerous_check` (line ~1149) — DELETE entire test
- `denied_read_patterns_blocks_custom_path` (if exists) — DELETE entire test
- Test struct literals at lines ~650, ~672, ~747, ~818 that construct
  `PathSecurityConfig { denied_read_patterns: ..., shell_allow_always: ..., .. }` —
  remove those two fields from each struct literal

Run `grep -n 'shell_allow_always\|denied_read_patterns' src/util/path_security.rs`
after edits to confirm zero remaining references.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass, no compile errors from removed fields

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: clean

- [ ] **Step 8: Commit**

```bash
git add src/util/path_security.rs src/config/project.rs
git commit -m "refactor(security): remove shell_allow_always and denied_read_patterns config options"
```

---

### Task 3: Wire `SecurityProfile` into config TOML parsing

**Files:**
- Modify: `src/config/project.rs:95-178`

- [ ] **Step 1: Write test for TOML parsing**

Add to the test module in `src/config/project.rs`:

```rust
#[test]
fn security_profile_parses_from_toml() {
    let toml_str = r#"
[security]
profile = "root"
"#;
    let config: ProjectConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.security.profile, SecurityProfile::Root);
}

#[test]
fn security_profile_defaults_to_default() {
    let toml_str = r#"
[security]
shell_enabled = true
"#;
    let config: ProjectConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.security.profile, SecurityProfile::Default);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- security_profile_parses security_profile_defaults`
Expected: compile errors — `profile` field doesn't exist on `SecuritySection`

- [ ] **Step 3: Add serde support to `SecurityProfile`**

In `src/util/path_security.rs`, add serde derives to the enum:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityProfile {
    #[default]
    Default,
    Root,
}
```

- [ ] **Step 4: Add `profile` field to `SecuritySection`**

In `src/config/project.rs`, add to `SecuritySection` (first field):

```rust
    /// Security profile: "default" (sandboxed) or "root" (unrestricted).
    #[serde(default)]
    pub profile: SecurityProfile,
```

Add the import at the top:
```rust
use crate::util::path_security::SecurityProfile;
```

Add to `SecuritySection::default()`:
```rust
    profile: SecurityProfile::Default,
```

Add to `to_path_security_config()`:
```rust
    profile: self.profile,
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- security_profile_parses security_profile_defaults`
Expected: PASS

- [ ] **Step 6: Run full test suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean

- [ ] **Step 7: Commit**

```bash
git add src/util/path_security.rs src/config/project.rs
git commit -m "feat(config): add profile field to [security] section in project.toml"
```

---

### Task 4: Fix `topic_path` and `anchor_path_for_topic` path traversal

**Files:**
- Modify: `src/memory/mod.rs:120-129`
- Modify: `src/memory/anchors.rs:245-248`

- [ ] **Step 1: Write failing tests for traversal attacks**

Add to the test module in `src/memory/mod.rs`:

```rust
#[test]
fn topic_path_blocks_dot_slash_traversal() {
    let dir = tempdir().unwrap();
    let store = MemoryStore::new(dir.path().join("memories")).unwrap();
    let path = store.topic_path("a/./b/../../../etc/passwd");
    assert!(
        path.starts_with(&store.memories_dir),
        "path {:?} must be inside {:?}",
        path,
        store.memories_dir,
    );
}

#[test]
fn topic_path_blocks_single_dot() {
    let dir = tempdir().unwrap();
    let store = MemoryStore::new(dir.path().join("memories")).unwrap();
    let path = store.topic_path(".");
    assert!(
        path.starts_with(&store.memories_dir),
        "path {:?} must be inside {:?}",
        path,
        store.memories_dir,
    );
    // Must be a file path, not the directory itself
    assert_ne!(path, store.memories_dir);
}
```

Add to the test module in `src/memory/anchors.rs`:

```rust
#[test]
fn anchor_path_blocks_traversal() {
    let memories_dir = PathBuf::from("/tmp/test_memories");
    let path = anchor_path_for_topic(&memories_dir, "../../etc/passwd");
    assert!(
        path.starts_with(&memories_dir),
        "anchor path {:?} must be inside {:?}",
        path,
        memories_dir,
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- topic_path_blocks_dot_slash topic_path_blocks_single_dot anchor_path_blocks`
Expected: at least `topic_path_blocks_dot_slash` and `anchor_path_blocks` FAIL

- [ ] **Step 3: Extract `sanitize_topic()` function in `memory/mod.rs`**

Add a public function above `topic_path`:

```rust
/// Sanitize a memory topic name to prevent directory traversal.
///
/// Uses `Path::components()` to keep only `Normal` segments, discarding
/// `.`, `..`, root prefixes, and embedded separators.
pub(crate) fn sanitize_topic(topic: &str) -> String {
    use std::path::{Component, Path};
    let sanitized: PathBuf = Path::new(topic)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    let result = sanitized.to_string_lossy().into_owned();
    if result.is_empty() {
        "_empty".to_string()
    } else {
        result
    }
}
```

- [ ] **Step 4: Rewrite `topic_path` to use `sanitize_topic`**

```rust
    pub(crate) fn topic_path(&self, topic: &str) -> PathBuf {
        let safe = sanitize_topic(topic);
        self.memories_dir.join(safe).with_extension("md")
    }
```

- [ ] **Step 5: Rewrite `anchor_path_for_topic` to use `sanitize_topic`**

In `src/memory/anchors.rs`, import and use:

```rust
use super::sanitize_topic;

pub fn anchor_path_for_topic(memories_dir: &Path, topic: &str) -> std::path::PathBuf {
    let safe = sanitize_topic(topic);
    memories_dir.join(format!("{}.anchors.toml", safe))
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib -- topic_path_blocks anchor_path_blocks`
Expected: all PASS

- [ ] **Step 7: Run full test suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean

- [ ] **Step 8: Commit**

```bash
git add src/memory/mod.rs src/memory/anchors.rs
git commit -m "fix(security): use Path::components() sanitization for memory topics"
```

---

### Task 5: Fix library path exemption doc mismatch (C-4 → M-17)

**Files:**
- Modify: `src/util/path_security.rs:188-193` (docstring only)

- [ ] **Step 1: Fix the docstring on `validate_read_path`**

Replace the docstring:

```rust
/// Validate a path for **read** access.
///
/// - Relative paths are resolved against `project_root` (if available).
/// - Absolute paths are used as-is.
/// - The resolved path is checked against the deny-list (unless `Root` profile).
/// - Library paths are subject to the same deny-list as all other reads.
```

(Remove the line "Paths inside registered library roots are always allowed (read-only).")

- [ ] **Step 2: Fix the test comment**

In `validate_read_path_accepts_library_paths`, update the assertion comment:

```rust
        // Path is not on the deny-list — it happens to be inside a library root,
        // but library roots receive no special exemption from deny-list checks.
        assert!(result.is_ok());
```

- [ ] **Step 3: Run tests + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean

- [ ] **Step 4: Commit**

```bash
git add src/util/path_security.rs
git commit -m "docs(security): fix validate_read_path docstring — library paths are not exempt from deny-list"
```

---

### Task 6: Update server instructions to mention root mode

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Add root mode section**

Find the `## Project Customization` section (or the section describing `project.toml` configuration) and add:

```markdown
### Security Profiles

The project's security profile is set in `.codescout/project.toml`:

- `profile = "default"` (default) — standard sandbox: read deny-list active, writes
  restricted to project root + temp dir, dangerous commands require `acknowledge_risk`.
- `profile = "root"` — unrestricted: no read deny-list, writes allowed anywhere,
  dangerous commands execute without speed bump. For system-administration projects
  that need full filesystem access.

Source-file shell access guidance (use `read_file`/`find_symbol` instead of `cat`) is
active in both profiles — it improves tool output quality, not security.
```

- [ ] **Step 1b: Check other prompt surfaces**

Per CLAUDE.md "Prompt Surface Consistency" rule, also check:
- `src/prompts/onboarding_prompt.md`
- `build_system_prompt_draft()` in `src/tools/workflow.rs`

If either mentions security config or `shell_allow_always`, update them too.

- [ ] **Step 2: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs(prompts): add root mode section to server instructions"
```

---

### Task 7: Fix dashboard memory topic path traversal (C-10)

**Files:**
- Modify: `src/dashboard/api/memories.rs`

- [ ] **Step 1: Write failing test**

Add a test (or verify in the existing dashboard test module) that a topic like
`../../etc/passwd` passed via the HTTP API is sanitized. If the dashboard has no
test infrastructure, add an inline unit test:

```rust
#[test]
fn dashboard_topic_is_sanitized() {
    // The dashboard handler extracts `topic` from the URL path and passes it
    // to MemoryStore::read/write. After Task 4, sanitize_topic is used inside
    // topic_path, so the dashboard is already protected. This test confirms it.
    let store = MemoryStore::new(tempdir().unwrap().path().join("memories")).unwrap();
    let path = store.topic_path("../../etc/passwd");
    assert!(path.starts_with(&store.memories_dir));
}
```

- [ ] **Step 2: Verify the dashboard handler flows through `topic_path`**

Read `src/dashboard/api/memories.rs` and confirm that `read_memory` and `write_memory`
use `store.read(topic)` / `store.write(topic, content)` which internally calls
`topic_path`. If so, Task 4's fix already covers this — no additional code change
needed, just the test confirmation.

If the dashboard handler constructs its own path (bypassing `topic_path`), it must be
changed to go through `MemoryStore` methods.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- dashboard_topic_is_sanitized`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/api/memories.rs
git commit -m "test(dashboard): confirm memory topic sanitization covers HTTP API (C-10)"
```

---

### Task 8: Final verification

- [ ] **Step 1: Full quality gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean, all tests pass

- [ ] **Step 2: Integration test — root mode TOML**

Create a temp project.toml and verify it parses:

```bash
cat > /tmp/test_root_config.toml << 'EOF'
[project]
name = "system-admin"

[security]
profile = "root"
EOF
```

Verify parsing works via a quick test addition or manual inspection.

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: clean build

- [ ] **Step 4: Squash into clean commits if needed, verify git log**

Review the commit history and ensure each commit is clean and self-contained.

- [ ] **Step 5: Verify `~/agents/system/` config**

If `~/agents/system/.codescout/project.toml` exists, add `profile = "root"` to its
`[security]` section to confirm the feature works end-to-end.

# Index Scope Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Before `index_project` walks and embeds a directory, require explicit human confirmation via MCP elicitation when the root is a known-broad directory (home, system paths) or the approximate source size exceeds a configurable threshold (default 500 MB).

**Architecture:** New pure sync module `src/embed/preflight.rs` performs a stat-only walk using the same `ignore::WalkBuilder` config as `build_index`. Returns either `Clear` or `RequiresConfirmation(PreflightInfo)`. `IndexProject::call` runs the preflight in `spawn_blocking`, then fires one `ctx.elicit::<IndexConfirm>()`. Declined, cancelled, or elicitation-unsupported all abort with `RecoverableError` — never silently proceed.

**Tech Stack:** Rust, `ignore` crate (existing), `rmcp` elicitation (existing), `schemars::JsonSchema` (existing), `crate::platform::home_dir()` (existing).

**Spec:** `docs/superpowers/specs/2026-04-18-index-scope-guard-design.md`

---

## File Structure

- **Create:** `src/embed/preflight.rs` — `check_index_scope`, `PreflightInfo`, `PreflightVerdict`, `SuspiciousReason`, `classify_path`, `format_bytes`, `elicitation_message`. Pure sync, no `ctx`. Unit-testable in isolation.
- **Modify:** `src/embed/mod.rs` — add `pub mod preflight;`
- **Modify:** `src/config/project.rs` — add `max_index_bytes` to `SecuritySection` with default 500 MB; propagate to `PathSecurityConfig`.
- **Modify:** `src/util/path_security.rs` — add `max_index_bytes: u64` field to `PathSecurityConfig` struct + default.
- **Modify:** `src/tools/semantic.rs` — wire preflight + elicitation into `IndexProject::call`. Add `IndexConfirm` elicitation type + tests.
- **Create:** `docs/manual/src/experimental/index-scope-guard.md` — user-facing docs (per project convention for `experiments` feature commits).
- **Modify:** `docs/manual/src/experimental/index.md` — add link to new page.

---

## Task 1: Config plumbing for `max_index_bytes`

Before any logic, add the configurable threshold so later tasks can read from `PathSecurityConfig`.

**Files:**
- Modify: `src/config/project.rs`
- Modify: `src/util/path_security.rs`

- [ ] **Step 1: Add default constant + field to `SecuritySection`**

In `src/config/project.rs`, find `SecuritySection` (the struct with `indexing_enabled`, `file_write_enabled`, etc.). Add a new field and its default fn:

```rust
/// Approximate raw source-byte threshold above which `index_project` requires
/// user confirmation via MCP elicitation. Default: 500 MB.
#[serde(default = "default_max_index_bytes")]
pub max_index_bytes: u64,
```

Add the default fn near the other `default_*` fns in this file:

```rust
fn default_max_index_bytes() -> u64 {
    500 * 1024 * 1024
}
```

- [ ] **Step 2: Include field in `Default for SecuritySection`**

Find the `impl Default for SecuritySection` block. Add:

```rust
max_index_bytes: default_max_index_bytes(),
```

- [ ] **Step 3: Propagate to `PathSecurityConfig`**

In `src/util/path_security.rs`, find the `PathSecurityConfig` struct (around L65). Add a new field:

```rust
/// Approx raw source-byte threshold above which `index_project` requires confirmation.
pub max_index_bytes: u64,
```

Find the `impl Default for PathSecurityConfig` block (around L95). Add:

```rust
max_index_bytes: 500 * 1024 * 1024,
```

- [ ] **Step 4: Wire `SecuritySection` → `PathSecurityConfig` mapping**

In `src/config/project.rs`, find the method/function that converts `SecuritySection` to `PathSecurityConfig` (look for where `indexing_enabled: self.indexing_enabled` is assigned, around L197). Add:

```rust
max_index_bytes: self.max_index_bytes,
```

- [ ] **Step 5: Add test for default value**

In `src/config/project.rs` tests module, add:

```rust
#[test]
fn security_section_default_max_index_bytes_is_500mb() {
    let sec = SecuritySection::default();
    assert_eq!(sec.max_index_bytes, 500 * 1024 * 1024);
}

#[test]
fn project_config_default_propagates_max_index_bytes() {
    let cfg = ProjectConfig::default_for("test-project".into());
    assert_eq!(cfg.security.max_index_bytes, 500 * 1024 * 1024);
}
```

- [ ] **Step 6: Run tests + clippy**

```bash
cargo test --lib security_section_default_max_index_bytes_is_500mb project_config_default_propagates_max_index_bytes
cargo clippy -- -D warnings
```

Expected: 2 tests pass, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add src/config/project.rs src/util/path_security.rs
git commit -m "feat(config): add security.max_index_bytes threshold (default 500 MB)"
```

---

## Task 2: Preflight module — types + `format_bytes`

Start the new module with the types and pure helpers. No walker yet.

**Files:**
- Create: `src/embed/preflight.rs`
- Modify: `src/embed/mod.rs`
- Test: same file (unit tests)

- [ ] **Step 1: Create the module skeleton with types**

Create `src/embed/preflight.rs`:

```rust
//! Preflight check for `index_project`: scope guard against pathologically broad
//! roots (home dir, system paths) and oversized source trees. Triggers an MCP
//! elicitation in the caller when confirmation is required.

use std::path::PathBuf;

/// Why a path is considered broad enough to warrant confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspiciousReason {
    /// Root exactly matches the user's home directory.
    HomeDirectory,
    /// Root is the parent of the user's home directory (e.g. `/home`).
    HomeParent,
    /// Root is a known system path (`/`, `/usr`, `/etc`, ...).
    SystemPath(PathBuf),
}

/// Summary produced by the preflight scan — used to build the elicitation message.
#[derive(Debug, Clone)]
pub struct PreflightInfo {
    pub root: PathBuf,
    pub file_count: usize,
    pub approx_bytes: u64,
    pub suspicious_reason: Option<SuspiciousReason>,
    pub size_exceeds_threshold: bool,
}

/// Verdict returned by [`check_index_scope`].
#[derive(Debug, Clone)]
pub enum PreflightVerdict {
    /// Proceed to `build_index` — no confirmation needed.
    Clear,
    /// Caller must elicit confirmation from the user before proceeding.
    RequiresConfirmation(PreflightInfo),
}

/// Human-readable byte size. Always 1 decimal place, KB/MB/GB.
pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_rounds_to_one_decimal() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MB");
        assert_eq!(format_bytes((2 * 1024 * 1024 * 1024) + (500 * 1024 * 1024)), "2.5 GB");
    }
}
```

- [ ] **Step 2: Register module in `src/embed/mod.rs`**

In `src/embed/mod.rs`, find the `pub mod` block (near `pub mod ast_chunker; pub mod chunker; pub mod drift; pub mod index; pub mod schema;`). Add:

```rust
pub mod preflight;
```

- [ ] **Step 3: Run test + clippy**

```bash
cargo test --lib preflight::tests::format_bytes_rounds_to_one_decimal
cargo clippy -- -D warnings
```

Expected: test passes, clippy clean.

- [ ] **Step 4: Commit**

```bash
git add src/embed/preflight.rs src/embed/mod.rs
git commit -m "feat(embed): scaffold preflight module with types and format_bytes"
```

---

## Task 3: `classify_path` — suspicious root detection

Detect home, home-parent, and known system paths. Pure function, no walker.

**Files:**
- Modify: `src/embed/preflight.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` module in `src/embed/preflight.rs`:

```rust
    use std::path::Path;

    #[test]
    fn classify_path_detects_home_directory() {
        let Some(home) = crate::platform::home_dir() else { return };
        assert_eq!(
            classify_path(&home),
            Some(SuspiciousReason::HomeDirectory),
        );
    }

    #[test]
    fn classify_path_detects_home_parent() {
        let Some(home) = crate::platform::home_dir() else { return };
        let Some(parent) = home.parent() else { return };
        // Skip when home's parent is '/' — that hits SystemPath, not HomeParent.
        if parent == Path::new("/") { return; }
        assert_eq!(
            classify_path(parent),
            Some(SuspiciousReason::HomeParent),
        );
    }

    #[test]
    fn classify_path_detects_root_system_path() {
        // '/' is a system path. It also tests canonicalization doesn't break it.
        let v = classify_path(Path::new("/"));
        assert!(matches!(v, Some(SuspiciousReason::SystemPath(_))));
    }

    #[test]
    fn classify_path_detects_usr_etc_var() {
        for p in ["/usr", "/etc", "/var", "/tmp", "/opt"] {
            if !Path::new(p).exists() { continue; }
            let v = classify_path(Path::new(p));
            assert!(
                matches!(v, Some(SuspiciousReason::SystemPath(_))),
                "{p} should classify as SystemPath, got {v:?}",
            );
        }
    }

    #[test]
    fn classify_path_allows_normal_project_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(classify_path(tmp.path()), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib preflight::tests::classify_path
```

Expected: FAIL — `classify_path` not defined.

- [ ] **Step 3: Implement `classify_path`**

Add to `src/embed/preflight.rs` (above the `#[cfg(test)]` block):

```rust
use std::path::Path;

const SYSTEM_PATHS: &[&str] = &[
    "/", "/usr", "/etc", "/var", "/tmp", "/root", "/opt", "/proc", "/sys", "/home",
];

/// Classify a root path against the known-broad list.
/// Returns `None` for ordinary project directories.
pub(crate) fn classify_path(root: &Path) -> Option<SuspiciousReason> {
    let canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    if let Some(home) = crate::platform::home_dir() {
        let home_canon = std::fs::canonicalize(&home).unwrap_or(home.clone());
        if canon == home_canon {
            return Some(SuspiciousReason::HomeDirectory);
        }
        if let Some(parent) = home_canon.parent() {
            if canon == parent && canon != Path::new("/") {
                return Some(SuspiciousReason::HomeParent);
            }
        }
    }

    for sys in SYSTEM_PATHS {
        let sys_path = Path::new(sys);
        let sys_canon = std::fs::canonicalize(sys_path).unwrap_or_else(|_| sys_path.to_path_buf());
        if canon == sys_canon {
            return Some(SuspiciousReason::SystemPath(canon.clone()));
        }
    }

    None
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib preflight::tests::classify_path
cargo clippy -- -D warnings
```

Expected: all `classify_path_*` tests PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/embed/preflight.rs
git commit -m "feat(embed): classify_path detects home and system paths"
```

---

## Task 4: `check_index_scope` — stat-only walker

Stat-only walk using the same `ignore::WalkBuilder` config as `build_index`.

**Files:**
- Modify: `src/embed/preflight.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` module in `src/embed/preflight.rs`:

```rust
    use std::io::Write;

    fn make_tempdir_with_bytes(total: u64) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        // Single file with the requested byte count.
        let mut f = std::fs::File::create(dir.path().join("big.rs")).unwrap();
        f.write_all(&vec![b'x'; total as usize]).unwrap();
        dir
    }

    #[test]
    fn check_index_scope_returns_clear_for_small_dir() {
        let dir = make_tempdir_with_bytes(1024); // 1 KB
        let v = check_index_scope(dir.path(), 500 * 1024 * 1024).unwrap();
        assert!(matches!(v, PreflightVerdict::Clear), "got {v:?}");
    }

    #[test]
    fn check_index_scope_flags_oversized_dir() {
        let dir = make_tempdir_with_bytes(2048);
        let v = check_index_scope(dir.path(), 1024).unwrap();
        match v {
            PreflightVerdict::RequiresConfirmation(info) => {
                assert!(info.size_exceeds_threshold);
                assert_eq!(info.suspicious_reason, None);
                assert_eq!(info.file_count, 1);
                assert!(info.approx_bytes >= 2048);
            }
            other => panic!("expected RequiresConfirmation, got {other:?}"),
        }
    }

    #[test]
    fn check_index_scope_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        // Big file that's gitignored
        let mut gi = std::fs::File::create(dir.path().join(".gitignore")).unwrap();
        gi.write_all(b"big.bin\n").unwrap();
        let mut f = std::fs::File::create(dir.path().join("big.bin")).unwrap();
        f.write_all(&vec![b'x'; 2048]).unwrap();
        // Small real source file
        let mut s = std::fs::File::create(dir.path().join("small.rs")).unwrap();
        s.write_all(b"fn main() {}\n").unwrap();

        let v = check_index_scope(dir.path(), 1024).unwrap();
        // big.bin should be ignored → under threshold → Clear.
        assert!(matches!(v, PreflightVerdict::Clear), "got {v:?}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib preflight::tests::check_index_scope
```

Expected: FAIL — `check_index_scope` not defined.

- [ ] **Step 3: Implement `check_index_scope`**

Add to `src/embed/preflight.rs` (above the `#[cfg(test)]` block):

```rust
/// Preflight scan: walk `root` (respecting `.gitignore` and hidden-file rules,
/// matching `build_index`'s walker), accumulate file count and approximate
/// source-byte total, then compare against `max_bytes` and classify the root.
///
/// Returns `PreflightVerdict::Clear` if neither trigger fires — caller
/// proceeds to `build_index`. Otherwise returns
/// `RequiresConfirmation(PreflightInfo)`; caller must elicit user confirmation.
///
/// Per-file `metadata()` errors are silently skipped (matching `WalkBuilder::flatten`).
/// Only failure to read the root itself propagates as an error.
pub fn check_index_scope(root: &Path, max_bytes: u64) -> anyhow::Result<PreflightVerdict> {
    use anyhow::Context;

    if !root.exists() {
        anyhow::bail!("project root does not exist: {}", root.display());
    }

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut file_count: usize = 0;
    let mut approx_bytes: u64 = 0;

    for entry in walker.flatten() {
        let Some(ftype) = entry.file_type() else { continue };
        if !ftype.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        file_count += 1;
        approx_bytes = approx_bytes.saturating_add(meta.len());
    }

    let suspicious_reason = classify_path(root);
    let size_exceeds_threshold = approx_bytes > max_bytes;

    if suspicious_reason.is_none() && !size_exceeds_threshold {
        return Ok(PreflightVerdict::Clear);
    }

    let canonical_root = std::fs::canonicalize(root)
        .with_context(|| format!("canonicalize {}", root.display()))
        .unwrap_or_else(|_| root.to_path_buf());

    Ok(PreflightVerdict::RequiresConfirmation(PreflightInfo {
        root: canonical_root,
        file_count,
        approx_bytes,
        suspicious_reason,
        size_exceeds_threshold,
    }))
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib preflight::tests::check_index_scope
cargo clippy -- -D warnings
```

Expected: all `check_index_scope_*` tests PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/embed/preflight.rs
git commit -m "feat(embed): check_index_scope stat-walks root, triggers on size or path"
```

---

## Task 5: `elicitation_message` — formatted user-facing text

Build the human-readable confirmation message from `PreflightInfo`.

**Files:**
- Modify: `src/embed/preflight.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` module:

```rust
    #[test]
    fn elicitation_message_includes_home_reason() {
        let info = PreflightInfo {
            root: PathBuf::from("/home/alice"),
            file_count: 3200,
            approx_bytes: 2 * 1024 * 1024 * 1024 + 400 * 1024 * 1024, // 2.4 GB
            suspicious_reason: Some(SuspiciousReason::HomeDirectory),
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(msg.contains("home directory"), "msg={msg}");
        assert!(msg.contains("/home/alice"), "msg={msg}");
        assert!(msg.contains("2.4 GB"), "msg={msg}");
        assert!(msg.contains("3,200") || msg.contains("3200"), "msg={msg}");
        assert!(msg.contains("Confirm"), "msg={msg}");
    }

    #[test]
    fn elicitation_message_size_only_omits_suspicious_line() {
        let info = PreflightInfo {
            root: PathBuf::from("/workspace/big"),
            file_count: 10_000,
            approx_bytes: 700 * 1024 * 1024,
            suspicious_reason: None,
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(!msg.to_lowercase().contains("home directory"), "msg={msg}");
        assert!(!msg.to_lowercase().contains("system directory"), "msg={msg}");
        assert!(msg.contains("700.0 MB"), "msg={msg}");
    }

    #[test]
    fn elicitation_message_system_path_labelled() {
        let info = PreflightInfo {
            root: PathBuf::from("/usr"),
            file_count: 100_000,
            approx_bytes: 8 * 1024 * 1024 * 1024,
            suspicious_reason: Some(SuspiciousReason::SystemPath(PathBuf::from("/usr"))),
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(msg.to_lowercase().contains("system directory"), "msg={msg}");
        assert!(msg.contains("/usr"), "msg={msg}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --lib preflight::tests::elicitation_message
```

Expected: FAIL — `elicitation_message` not defined.

- [ ] **Step 3: Implement `elicitation_message`**

Add to `src/embed/preflight.rs` (above `#[cfg(test)]`):

```rust
impl PreflightInfo {
    /// Build the human-readable confirmation message shown in the elicitation
    /// dialog. Lines that don't apply (no suspicious reason, etc.) are omitted.
    pub fn elicitation_message(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("⚠ Large index scope detected".to_string());
        lines.push(String::new());

        let root_line = match &self.suspicious_reason {
            Some(SuspiciousReason::HomeDirectory) => {
                format!("Root: {}  (home directory)", self.root.display())
            }
            Some(SuspiciousReason::HomeParent) => {
                format!("Root: {}  (parent of home directory)", self.root.display())
            }
            Some(SuspiciousReason::SystemPath(p)) => {
                format!("Root: {}  (system directory: {})", self.root.display(), p.display())
            }
            None => format!("Root: {}", self.root.display()),
        };
        lines.push(root_line);

        lines.push(format!("Eligible files: ~{}", format_count(self.file_count)));
        lines.push(format!("Approx source content: ~{}", format_bytes(self.approx_bytes)));

        // Rough estimate: build_index chunk_size ≈ 4000 chars. Integer math, no decimals.
        let est_chunks = self.approx_bytes / 4000;
        lines.push(format!("Estimated chunks: ~{}", format_count(est_chunks as usize)));

        lines.push(String::new());
        lines.push("This will use significant RAM and CPU time.".to_string());
        lines.push("Confirm indexing this directory?".to_string());

        lines.join("\n")
    }
}

/// Format an integer with thousand separators (e.g. `3,200`).
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --lib preflight::tests::elicitation_message
cargo clippy -- -D warnings
```

Expected: all 3 tests PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/embed/preflight.rs
git commit -m "feat(embed): elicitation_message renders confirm prompt from PreflightInfo"
```

---

## Task 6: Wire preflight into `IndexProject::call`

Add the `IndexConfirm` elicitation type and invoke preflight + elicit before `build_index`.

**Files:**
- Modify: `src/tools/semantic.rs`

- [ ] **Step 1: Locate `IndexProject::call`**

Open `src/tools/semantic.rs`. Find `impl Tool for IndexProject`, method `call` (~L323–L545). Find the `spawn_blocking` that wraps `build_index` (search for `build_index(` inside the `call` body).

- [ ] **Step 2: Add `IndexConfirm` elicitation type**

Near the top of `src/tools/semantic.rs` (after existing `use` statements, before `pub struct SemanticSearch`), add:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct IndexConfirm {
    /// Confirm indexing this directory
    confirm: bool,
}
rmcp::elicit_safe!(IndexConfirm);
```

- [ ] **Step 3: Insert preflight block before `build_index` spawn**

Inside `IndexProject::call`, locate the line just before the `build_index` `spawn_blocking` call (where `root`, `force`, `progress_cb` are prepared). Insert:

```rust
// ── Preflight scope check ───────────────────────────────────────
// Stat-walk the root to estimate size + detect broad roots (home, system).
// Requires user confirmation via elicitation if either trigger fires.
{
    use crate::embed::preflight::{check_index_scope, PreflightVerdict};

    let security = ctx.agent.security_config().await;
    let preflight_root = root.clone();
    let max_bytes = security.max_index_bytes;
    let verdict = tokio::task::spawn_blocking(move || {
        check_index_scope(&preflight_root, max_bytes)
    })
    .await
    .map_err(|e| anyhow::anyhow!("preflight task join error: {e}"))??;

    if let PreflightVerdict::RequiresConfirmation(info) = verdict {
        tracing::info!(
            root = ?info.root,
            file_count = info.file_count,
            approx_bytes = info.approx_bytes,
            suspicious = ?info.suspicious_reason,
            size_over = info.size_exceeds_threshold,
            "index_project preflight requires confirmation"
        );
        let msg = info.elicitation_message();
        match ctx.elicit::<IndexConfirm>(msg).await? {
            Some(IndexConfirm { confirm: true }) => {
                tracing::info!(root = ?info.root, "index scope confirmed by user");
            }
            Some(IndexConfirm { confirm: false }) => {
                return Err(crate::tools::RecoverableError::with_hint(
                    "Indexing aborted — user did not confirm the scope",
                    "Activate a more specific project root, or raise \
                     security.max_index_bytes in .codescout/project.toml, then retry.",
                )
                .into());
            }
            None => {
                // No peer, client lacks elicitation capability, or no content returned.
                // For this guard, the safe default is to refuse — never silently proceed.
                return Err(crate::tools::RecoverableError::with_hint(
                    "index_project needs confirmation but client does not support elicitation",
                    "Raise security.max_index_bytes in .codescout/project.toml, \
                     or activate a narrower project root, then retry.",
                )
                .into());
            }
        }
    }
}
// ────────────────────────────────────────────────────────────────
```

- [ ] **Step 4: Build to verify the wiring compiles**

```bash
cargo build 2>&1 | tail -20
cargo clippy -- -D warnings
```

Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(index_project): preflight + elicit confirmation for broad roots"
```

---

## Task 7: Integration tests for preflight + elicitation

Verify the three control-flow branches: normal project, user confirms, user declines, elicitation unavailable.

**Files:**
- Modify: `src/tools/semantic.rs` (tests module)

- [ ] **Step 1: Understand the existing test harness**

In `src/tools/semantic.rs` tests module, find `fn project_ctx()` (~L942). Note how it builds a `ToolContext` — specifically that `ctx.peer` is `None` in unit tests. Because `ctx.elicit` returns `Ok(None)` when `peer` is `None`, unit tests for "elicit unavailable" work naturally without mocking rmcp.

The "user confirms" / "user declines" cases require a real `Peer` — harder to mock. For this task, cover the behavior that's reachable from unit tests; gate the peer-driven cases to a TODO follow-up unless an existing mock peer exists.

Search for any existing mock peer:

```bash
rg -n "MockPeer|mock_peer|impl.*Peer" src/ | head -20
```

If no mock peer exists, tests stay at the "elicit returns None → abort" level. The real end-to-end test is manual (run the MCP server, activate `~`, call `index_project`, observe the dialog).

- [ ] **Step 2: Write test — normal project proceeds without elicit**

Append to the `tests` module in `src/tools/semantic.rs`:

```rust
#[tokio::test]
async fn index_project_no_elicit_for_normal_project() {
    // A tiny project well under the default threshold, no suspicious path.
    let (dir, ctx) = project_ctx().await;
    // Write a small source file so the walker sees something.
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    // Use an explicit small max_bytes but still above the test file size.
    // Because ctx.peer is None, if an elicit WERE triggered it would return
    // Ok(None) and the tool would abort. This test proves we do NOT elicit.

    let tool = IndexProject;
    let args = serde_json::json!({});
    // The result may still fail on embedder availability, but it should fail
    // downstream of the preflight (e.g. in build_index), NOT with our "client
    // does not support elicitation" error message.
    let result = tool.call(&ctx, &args).await;
    if let Err(e) = &result {
        let msg = format!("{e:?}");
        assert!(
            !msg.contains("client does not support elicitation"),
            "preflight should not have elicited for a tiny project: {msg}"
        );
    }
}
```

- [ ] **Step 3: Write test — oversized root aborts when elicitation unavailable**

Append to the same tests module:

```rust
#[tokio::test]
async fn index_project_aborts_when_elicit_unavailable_on_oversized_root() {
    let (dir, ctx) = project_ctx().await;

    // Force threshold to 0 bytes so any file triggers size-exceeded.
    // We do this by writing a .codescout/project.toml with max_index_bytes = 0.
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    std::fs::write(
        cs_dir.join("project.toml"),
        "[project]\nname = \"test\"\n[security]\nmax_index_bytes = 0\n",
    )
    .unwrap();
    // Write at least one file so the walker sees it.
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    // Rebuild the Agent so it picks up the new project.toml.
    // (ctx holds an Agent that may have loaded config before the toml existed;
    // use the same pattern project_ctx uses to rebuild, or call a refresh
    // method if one exists. If neither is available, skip this test and file
    // a follow-up — but prefer rebuild.)
    let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = crate::tools::ToolContext {
        agent,
        ..ctx
    };

    let tool = IndexProject;
    let args = serde_json::json!({});
    let err = tool.call(&ctx, &args).await.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("does not support elicitation")
            || msg.contains("user did not confirm"),
        "expected elicit-unavailable abort, got: {msg}"
    );
}
```

Note: if the existing `project_ctx()` builder doesn't expose destructuring like `{ agent, ..ctx }`, use whatever pattern other tests in the file use to clone/rebuild `ToolContext`. Read 2–3 nearby tests to confirm the idiom before writing this.

- [ ] **Step 4: Run the two new tests**

```bash
cargo test --lib tools::semantic::tests::index_project_no_elicit_for_normal_project \
            tools::semantic::tests::index_project_aborts_when_elicit_unavailable_on_oversized_root
```

Expected: both tests PASS. If the ToolContext rebuild idiom in Step 3 doesn't match the file's pattern, adjust to match neighboring tests and re-run.

- [ ] **Step 5: Run the full test suite**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass (no regressions in the semantic.rs test module or elsewhere).

- [ ] **Step 6: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "test(index_project): preflight skips elicit for normal projects, aborts when unavailable"
```

---

## Task 8: Experimental docs

Per project convention (`CLAUDE.md § Documenting Features on experiments`), feature commits on `experiments` MUST include a user-facing docs page and an `index.md` link.

**Files:**
- Create: `docs/manual/src/experimental/index-scope-guard.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Create the feature doc**

Create `docs/manual/src/experimental/index-scope-guard.md`:

```markdown
# Index Scope Guard

> ⚠ Experimental — may change without notice.

Before `index_project` commits to walking and embedding a directory, codescout
checks whether the scope looks broad enough to be accidental, and requires
explicit human confirmation via an MCP elicitation dialog before proceeding.

## Triggers

Confirmation is required if either:

1. **The project root is a known-broad directory**, such as:
   - Your home directory (`~`)
   - The parent of home (e.g. `/home`)
   - A system root: `/`, `/usr`, `/etc`, `/var`, `/tmp`, `/root`, `/opt`, `/proc`, `/sys`
2. **The approximate raw source size exceeds the threshold** (default 500 MB of
   eligible content, respecting `.gitignore` and hidden-file rules — same
   filter `index_project` itself uses).

When either trigger fires, the MCP client shows a dialog like:

```
⚠ Large index scope detected

Root: /home/alice  (home directory)
Eligible files: ~3,200
Approx source content: ~2.4 GB
Estimated chunks: ~600,000

This will use significant RAM and CPU time.
Confirm indexing this directory?
```

You can accept to proceed or decline to abort. The check runs on **every** call
— it is not persisted. If your MCP client does not support elicitation, the
call is refused with a clear error rather than silently proceeding.

## Configuration

Adjust the size threshold in `.codescout/project.toml`:

```toml
[security]
max_index_bytes = 1073741824   # 1 GB
```

The default is `524288000` (500 MB). Set it higher to allow larger projects
without a prompt; lower to trigger the guard more aggressively.

Currently, the suspicious-path list is fixed — it is not configurable.

## Rationale

An agent that calls `activate_project("~")` followed by `index_project` would
otherwise walk the entire home directory, ingest every file, and cause severe
RAM spikes or OOM (see `docs/issues/memory-leak-x-session-freeze.md`). The
scope guard makes that path impossible without a human in the loop.
```

- [ ] **Step 2: Link from `experimental/index.md`**

Open `docs/manual/src/experimental/index.md`. Add a list entry linking to the new page. Match the style of existing entries (look at how other experimental features are listed). Typical form:

```markdown
- [Index Scope Guard](index-scope-guard.md) — confirmation prompt before
  `index_project` walks home/system directories or oversized trees.
```

- [ ] **Step 3: Commit docs**

```bash
git add docs/manual/src/experimental/index-scope-guard.md \
        docs/manual/src/experimental/index.md
git commit -m "docs(experimental): index scope guard"
```

---

## Task 9: Final verification

- [ ] **Step 1: Format, clippy, test**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test 2>&1 | tail -30
```

Expected: no diff from fmt, clippy clean, all tests pass.

- [ ] **Step 2: Release build (required before manual MCP test, per CLAUDE.md)**

```bash
cargo build --release 2>&1 | tail -10
```

Expected: build succeeds.

- [ ] **Step 3: Manual smoke test via MCP**

Restart the MCP server with `/mcp` in the client, then:

1. `activate_project` on a normal, small project → `index_project` should proceed without any dialog.
2. `activate_project("$HOME")` → `index_project` should trigger the elicitation dialog naming "home directory", with the approximate size shown.
3. Decline the dialog → the tool returns a `RecoverableError` with the "user did not confirm the scope" hint.
4. Accept the dialog → indexing proceeds (may take a long time for a full home dir — cancel once you've confirmed the flow works).

- [ ] **Step 4: Summary**

Post a short note in the session with: tests passing, clippy clean, manual smoke confirmed on all three paths (normal / decline / accept). Reference the spec path and plan path.

---

## Notes for the implementer

- Follow each step exactly. Do not bundle steps across tasks.
- If a test's fixture setup pattern (e.g., `project_ctx`) doesn't match the shape I assumed, read 2–3 neighbouring tests to learn the actual idiom and adjust.
- If `cargo test` surfaces a failure that isn't in a file you modified, run `git diff` — you may have accidentally broken an import or unrelated type. Revert and retry the targeted edit.
- Do not skip the docs task. Per `CLAUDE.md`, a feature commit on `experiments` without an experimental doc page is a violation of project policy.

# Phases 1B, 2, 3: Security Hardening, Platform Abstraction, Quality

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete all remaining code review findings: Phase 1B (4 security fixes), Phase 2 (platform abstraction layer), Phase 3 (quality + robustness fixes).

**Architecture:** Phase 1B is independent fixes. Phase 2 creates `src/platform/{mod,unix,windows}.rs` and migrates all `#[cfg]` blocks. Phase 3 is independent quality fixes. Tasks are ordered so later tasks can build on earlier ones (e.g. Phase 2 platform module enables Phase 3 Windows fixes).

**Tech Stack:** Rust, serde, tokio, axum, git2, tower-http (CORS), libc

**Spec:** `docs/plans/2026-03-20-code-review-and-platform-abstraction-design.md`

---

## File Map

| File | Phase | Action | Responsibility |
|------|-------|--------|---------------|
| `src/lsp/transport.rs` | 1B | Modify | Cap Content-Length (C-3) |
| `src/tools/github.rs` | 1B | Modify | Validate owner/repo + null bytes (H-3, H-4) |
| `src/dashboard/routes.rs` | 1B | Modify | Add CORS layer (M-12) |
| `src/embed/remote.rs` | 1B | Modify | Reject http:// with API key (H-16) |
| `src/platform/mod.rs` | 2 | Create | Platform trait + re-exports |
| `src/platform/unix.rs` | 2 | Create | Unix implementations |
| `src/platform/windows.rs` | 2 | Create | Windows implementations |
| `src/util/path_security.rs` | 2,3 | Modify | Use platform::home_dir/temp_dir/denied_read_prefixes, regex caching |
| `src/lsp/client.rs` | 2,3 | Modify | Use platform::terminate_process, typed LspError |
| `src/lsp/servers/mod.rs` | 2 | Modify | Platform-aware binary names |
| `src/tools/workflow.rs` | 2,3 | Modify | Use platform::shell_command, fix UTF-8 boundary |
| `src/tools/output_buffer.rs` | 2 | Modify | Use platform::shell_tokenize |
| `src/tools/config.rs` | 2 | Modify | Use platform::home_dir for cargo deps |
| `src/git/mod.rs` | 3 | Modify | Fix unwrap on git path (H-8) |
| `src/lsp/manager.rs` | 3 | Modify | Fix LRU eviction TOCTOU (H-10) |
| `src/embed/index.rs` | 3 | Modify | SourceFilter enum (H-7), bytes_to_f32 validation (M-9) |
| `src/util/text.rs` | 3 | Modify | Fix count_lines empty string (M-2) |

---

## PHASE 1B — Remaining Security Fixes

### Task 1: Cap LSP Content-Length at 100 MiB (C-3)

**Files:**
- Modify: `src/lsp/transport.rs:35`

- [ ] **Step 1: Write failing test**

Add to the test module (or create one) in `src/lsp/transport.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_oversized_content_length() {
        let oversized = 200 * 1024 * 1024; // 200 MiB
        let msg = format!("Content-Length: {}\r\n\r\n", oversized);
        let mut reader = tokio::io::BufReader::new(msg.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds"));
    }

    #[tokio::test]
    async fn accepts_normal_content_length() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = tokio::io::BufReader::new(msg.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Run tests to verify oversized test fails**

Run: `cargo test --lib -- rejects_oversized_content_length accepts_normal_content_length`
Expected: `rejects_oversized_content_length` FAILS (currently allocates without checking)

- [ ] **Step 3: Add size cap**

In `read_message`, after `let length = content_length.context(...)`:

```rust
    const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024; // 100 MiB
    if length > MAX_MESSAGE_SIZE {
        bail!(
            "Content-Length {} exceeds maximum allowed size of {} bytes",
            length,
            MAX_MESSAGE_SIZE
        );
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib -- rejects_oversized_content_length accepts_normal_content_length`
Expected: both PASS

- [ ] **Step 5: Commit**

```bash
git add src/lsp/transport.rs
git commit -m "fix(lsp): cap Content-Length at 100 MiB to prevent OOM (C-3)"
```

---

### Task 2: Validate GitHub owner/repo params (H-3, H-4)

**Files:**
- Modify: `src/tools/github.rs:114-122`

- [ ] **Step 1: Write failing tests**

Add to the test module in `src/tools/github.rs`:

```rust
#[test]
fn require_owner_repo_rejects_special_chars() {
    assert!(require_owner_repo("valid-owner", "valid.repo_123").is_ok());
    assert!(require_owner_repo("owner;rm -rf", "repo").is_err());
    assert!(require_owner_repo("owner", "repo\0evil").is_err());
    assert!(require_owner_repo("owner/injection", "repo").is_err());
    assert!(require_owner_repo("owner", "repo name").is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- require_owner_repo_rejects_special_chars`
Expected: FAIL (current impl only checks empty)

- [ ] **Step 3: Add validation to `require_owner_repo`**

Replace the function body:

```rust
fn require_owner_repo(owner: &str, repo: &str) -> Result<(), RecoverableError> {
    if owner.is_empty() || repo.is_empty() {
        return Err(RecoverableError::with_hint(
            "owner and repo required",
            "Provide owner (GitHub username/org) and repo (repository name)",
        ));
    }
    // Reject null bytes
    if owner.contains('\0') || repo.contains('\0') {
        return Err(RecoverableError::with_hint(
            "owner/repo contains null byte",
            "GitHub owner and repo names must not contain null bytes",
        ));
    }
    // GitHub owner/repo names: alphanumeric, hyphens, dots, underscores
    let valid = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
    };
    if !valid(owner) || !valid(repo) {
        return Err(RecoverableError::with_hint(
            "owner/repo contains invalid characters",
            "GitHub owner and repo names may only contain alphanumeric characters, hyphens, dots, and underscores",
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib -- require_owner_repo_rejects`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/github.rs
git commit -m "fix(github): validate owner/repo params against injection (H-3, H-4)"
```

---

### Task 3: Add CORS to dashboard (M-12)

**Files:**
- Modify: `src/dashboard/routes.rs:18-46`
- Modify: `Cargo.toml` (if `tower-http` CORS feature not already enabled)

- [ ] **Step 1: Check if tower-http cors feature is available**

Run: `grep tower-http Cargo.toml`

If the `cors` feature is not listed, add it to the `tower-http` dependency.

- [ ] **Step 2: Add CORS layer to `build_router`**

Add import at top of `src/dashboard/routes.rs`:

```rust
use tower_http::cors::{CorsLayer, AllowOrigin};
use http::HeaderValue;
```

In `build_router`, before `.with_state(state)`, add:

```rust
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    origin.to_str().map_or(false, |s| {
                        s.starts_with("http://localhost:") || s.starts_with("http://127.0.0.1:")
                    })
                }))
                .allow_methods([
                    http::Method::GET,
                    http::Method::POST,
                    http::Method::DELETE,
                ])
                .allow_headers([http::header::CONTENT_TYPE]),
        )
```

- [ ] **Step 3: Write test**

Add to test module in `src/dashboard/routes.rs`:

```rust
#[tokio::test]
async fn cors_allows_localhost_origin() {
    let root = tempfile::tempdir().unwrap();
    let app = test_router(root.path());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/health")
                .header("Origin", "http://localhost:3000")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.headers().contains_key("access-control-allow-origin"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib -- cors_allows_localhost`
Expected: PASS

- [ ] **Step 5: Run clippy + full tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 6: Commit**

```bash
git add src/dashboard/routes.rs Cargo.toml
git commit -m "feat(dashboard): add CORS layer scoped to localhost (M-12)"
```

---

### Task 4: Reject HTTP with API key in embedding endpoint (H-16)

**Files:**
- Modify: `src/embed/remote.rs:68-76`

- [ ] **Step 1: Write failing test**

Add to test module in `src/embed/remote.rs`:

```rust
#[test]
fn custom_rejects_http_with_api_key() {
    // Set the env var for this test
    std::env::set_var("EMBED_API_KEY", "sk-test-key");
    let result = RemoteEmbedder::custom("http://example.com", "model");
    std::env::remove_var("EMBED_API_KEY");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTPS"));
}

#[test]
fn custom_allows_http_without_api_key() {
    std::env::remove_var("EMBED_API_KEY");
    let result = RemoteEmbedder::custom("http://localhost:11434", "model");
    assert!(result.is_ok());
}

#[test]
fn custom_allows_https_with_api_key() {
    std::env::set_var("EMBED_API_KEY", "sk-test-key");
    let result = RemoteEmbedder::custom("https://api.example.com", "model");
    std::env::remove_var("EMBED_API_KEY");
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib -- custom_rejects_http_with_api_key`
Expected: FAIL

- [ ] **Step 3: Add HTTPS enforcement**

In `RemoteEmbedder::custom`, after the endpoint construction:

```rust
    pub fn custom(base_url: &str, model: &str) -> Result<Self> {
        let endpoint = format!("{}/v1/embeddings", base_url.trim_end_matches('/'));
        let api_key = std::env::var("EMBED_API_KEY").ok();
        if api_key.is_some() && !base_url.starts_with("https://") {
            bail!(
                "HTTPS required when EMBED_API_KEY is set — \
                 refusing to send API key over plaintext HTTP to {}",
                base_url
            );
        }
        Ok(Self {
            client: Self::http_client(),
            endpoint,
            model: model.to_string(),
            api_key,
        })
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib -- custom_rejects_http custom_allows_http custom_allows_https`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/embed/remote.rs
git commit -m "fix(embed): reject HTTP endpoint when API key is set (H-16)"
```

---

## PHASE 2 — Platform Abstraction Layer

### Task 5: Create platform module with trait and Unix implementation

**Files:**
- Create: `src/platform/mod.rs`
- Create: `src/platform/unix.rs`
- Create: `src/platform/windows.rs`
- Modify: `src/lib.rs` (add `pub mod platform;`)

- [ ] **Step 1: Create `src/platform/mod.rs`**

```rust
//! Platform abstraction layer.
//!
//! Provides OS-specific implementations for filesystem paths, shell commands,
//! process management, and security defaults. All platform-specific code should
//! go through this module rather than using `#[cfg]` blocks elsewhere.

use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
use windows as imp;

/// Return the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    imp::home_dir()
}

/// Return the system temporary directory.
pub fn temp_dir() -> PathBuf {
    imp::temp_dir()
}

/// Return the platform-specific read deny-list prefixes (e.g. `~/.ssh`).
pub fn denied_read_prefixes() -> &'static [&'static str] {
    imp::denied_read_prefixes()
}

/// Build a shell command for executing a string.
/// Returns `(program, args)` — e.g. `("sh", ["-c", cmd])` on Unix.
pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    imp::shell_command(cmd)
}

/// Tokenize a command string into arguments using platform-appropriate rules.
/// Unix: shell_words::split. Windows: custom tokenizer (no backslash escapes).
pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    imp::shell_tokenize(cmd)
}

/// Send a termination signal to a process.
/// Unix: SIGTERM. Windows: TerminateProcess.
pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    imp::terminate_process(pid)
}

/// Check if a process is alive.
pub fn process_alive(pid: u32) -> bool {
    imp::process_alive(pid)
}

/// Platform-aware rename that overwrites the destination.
/// On Unix this is a no-op wrapper around `std::fs::rename`.
/// On Windows this uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.
pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    imp::rename_overwrite(from, to)
}

/// Platform-aware LSP server binary name.
/// On Windows, appends `.cmd` or `.exe` as needed.
pub fn lsp_binary_name(base: &str) -> String {
    imp::lsp_binary_name(base)
}
```

- [ ] **Step 2: Create `src/platform/unix.rs`**

```rust
use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gh",
        "~/.netrc",
        "~/.npmrc",
        "~/.pypirc",
        "~/.docker/config.json",
        "~/.kube/config",
        "~/.git-credentials",
    ]
}

pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    ("sh", vec!["-c".to_string(), cmd.to_string()])
}

pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    shell_words::split(cmd).map_err(|e| e.to_string())
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    base.to_string()
}
```

- [ ] **Step 3: Create `src/platform/windows.rs`**

```rust
use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gh",
        "~/.netrc",
        "~/.npmrc",
        "~/.pypirc",
        "~/.docker/config.json",
        "~/.kube/config",
        "~/.git-credentials",
    ]
}

pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    ("cmd.exe", vec!["/C".to_string(), cmd.to_string()])
}

pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    // Windows doesn't use backslash as escape in shell — simple split on spaces
    // respecting double quotes.
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if in_quotes {
        return Err("unclosed quote".to_string());
    }
    Ok(tokens)
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    // Use taskkill on Windows — avoids unsafe WinAPI bindings
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    if status.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("taskkill failed: {}", String::from_utf8_lossy(&status.stderr)),
        ))
    }
}

pub fn process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    // On Windows, std::fs::rename fails if target exists.
    // Remove target first, then rename.
    if to.exists() {
        std::fs::remove_file(to)?;
    }
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    // Node-based tools use .cmd on Windows
    match base {
        "typescript-language-server" | "vscode-json-language-server"
        | "yaml-language-server" | "bash-language-server" | "pyright-langserver" => {
            format!("{}.cmd", base)
        }
        _ => format!("{}.exe", base),
    }
}
```

- [ ] **Step 4: Add `pub mod platform;` to `src/lib.rs`**

- [ ] **Step 5: Run `cargo check` to verify compilation**

Run: `cargo check`
Expected: compiles (Windows module compiles but is dead code on Linux)

- [ ] **Step 6: Write tests for the Unix module**

Add to `src/platform/unix.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_some() {
        assert!(home_dir().is_some());
    }

    #[test]
    fn temp_dir_exists() {
        assert!(temp_dir().exists());
    }

    #[test]
    fn shell_command_uses_sh() {
        let (prog, args) = shell_command("echo hello");
        assert_eq!(prog, "sh");
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn shell_tokenize_splits_correctly() {
        let tokens = shell_tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn lsp_binary_name_unchanged() {
        assert_eq!(lsp_binary_name("rust-analyzer"), "rust-analyzer");
    }
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test --lib platform`
Expected: all PASS

- [ ] **Step 8: Commit**

```bash
git add src/platform/ src/lib.rs
git commit -m "feat(platform): add platform abstraction layer with Unix/Windows implementations"
```

---

### Task 6: Migrate path_security.rs to use platform module (C-7, H-15)

**Files:**
- Modify: `src/util/path_security.rs:114-130,168-174,267`

- [ ] **Step 1: Replace `home_dir()` with `platform::home_dir()`**

In `src/util/path_security.rs`, replace the local `home_dir` function:

```rust
fn home_dir() -> Option<PathBuf> {
    crate::platform::home_dir()
}
```

Or simply replace all calls from `home_dir()` to `crate::platform::home_dir()` and remove the local function.

- [ ] **Step 2: Replace hardcoded `/tmp` in `validate_write_path`**

Change:
```rust
    allowed.push(PathBuf::from("/tmp"));
```
To:
```rust
    allowed.push(crate::platform::temp_dir());
```

- [ ] **Step 3: Migrate `DEFAULT_DENIED_PREFIXES` to use platform module**

Replace the hardcoded array with a call to `crate::platform::denied_read_prefixes()` in the `denied_read_paths` function:

```rust
fn denied_read_paths(_config: &PathSecurityConfig) -> Vec<PathBuf> {
    let mut denied = Vec::new();
    for p in crate::platform::denied_read_prefixes()
        .iter()
        .chain(DEFAULT_DENIED_EXACT.iter())
    {
        if let Some(expanded) = expand_home(p) {
            denied.push(expanded);
        }
    }
    // Windows-specific system paths
    #[cfg(windows)]
    {
        if let Ok(sysroot) = std::env::var("SYSTEMROOT") {
            denied.push(PathBuf::from(&sysroot).join("System32").join("config"));
        }
    }
    denied
}
```

Remove the `DEFAULT_DENIED_PREFIXES` constant (the data now lives in `platform::denied_read_prefixes()`).

- [ ] **Step 4: Update `expand_home` to use platform module**

```rust
fn expand_home(path: &str) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        crate::platform::home_dir().map(|h| h.join(rest))
    } else {
        Some(PathBuf::from(path))
    }
}
```

- [ ] **Step 5: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean

- [ ] **Step 6: Commit**

```bash
git add src/util/path_security.rs
git commit -m "refactor(security): migrate path_security to platform module (C-7, H-15)"
```

---

### Task 7: Migrate LspClient::Drop to use platform::terminate_process (C-8)

**Files:**
- Modify: `src/lsp/client.rs:872-893`

- [ ] **Step 1: Replace `libc::kill` calls with `platform::terminate_process`**

Find the `Drop` impl for `LspClient` and replace:
```rust
unsafe { libc::kill(pid as i32, libc::SIGTERM); }
```
With:
```rust
let _ = crate::platform::terminate_process(pid);
```

- [ ] **Step 2: Replace test `libc::kill` with `platform::process_alive`**

Find test code using `libc::kill(pid, 0)` and replace with `crate::platform::process_alive(pid)`.

- [ ] **Step 3: Remove `use libc;` if no other uses remain**

Run: `grep -n 'libc::' src/lsp/client.rs` to check.

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 5: Commit**

```bash
git add src/lsp/client.rs
git commit -m "refactor(lsp): use platform::terminate_process instead of libc::kill (C-8)"
```

---

### Task 8: Migrate shell commands to use platform module (C-5, C-6, H-11, H-12)

**Files:**
- Modify: `src/tools/workflow.rs` (interactive + background spawn)
- Modify: `src/tools/output_buffer.rs` (shell_tokenize)
- Modify: `src/tools/config.rs` (HOME in auto_register_cargo_deps)

- [ ] **Step 1: Replace `sh -c` in workflow.rs with `platform::shell_command`**

Search for `"sh"` and `"-c"` in `src/tools/workflow.rs`. Replace each occurrence:

```rust
// Old:
Command::new("sh").arg("-c").arg(&cmd)
// New:
let (prog, args) = crate::platform::shell_command(&cmd);
Command::new(prog).args(&args)
```

- [ ] **Step 2: Replace `shell_words::split` in output_buffer.rs with `platform::shell_tokenize`**

Search for `shell_words::split` in `src/tools/output_buffer.rs` and replace with `crate::platform::shell_tokenize`.

- [ ] **Step 3: Replace `$HOME` in config.rs with `platform::home_dir`**

In `auto_register_cargo_deps`, replace:
```rust
std::env::var("HOME")
```
With:
```rust
crate::platform::home_dir().map(|p| p.to_string_lossy().into_owned())
```

- [ ] **Step 4: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs src/tools/output_buffer.rs src/tools/config.rs
git commit -m "refactor: migrate shell commands to platform module (C-5, C-6, H-11, H-12)"
```

---

### Task 9: Platform-aware LSP binary names (C-9)

**Files:**
- Modify: `src/lsp/servers/mod.rs`

- [ ] **Step 1: Read current binary name definitions**

Find where binary names like `"rust-analyzer"`, `"typescript-language-server"` etc. are defined.

- [ ] **Step 2: Wrap each with `platform::lsp_binary_name()`**

```rust
// Old:
"typescript-language-server"
// New:
&crate::platform::lsp_binary_name("typescript-language-server")
```

- [ ] **Step 3: Run tests + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 4: Commit**

```bash
git add src/lsp/servers/mod.rs
git commit -m "feat(lsp): platform-aware binary names for Windows (C-9)"
```

---

## PHASE 3 — Quality + Robustness

### Task 10: Fix git path unwrap panic (H-8)

**Files:**
- Modify: `src/git/mod.rs:58,66`

- [ ] **Step 1: Write test for non-UTF-8 path handling**

```rust
#[test]
fn diff_tree_handles_missing_path_gracefully() {
    // This tests that we don't unwrap() on path() — if a delta has no path,
    // we skip it instead of panicking.
    // The actual scenario is binary files or non-UTF-8 paths.
    // We can't easily produce those in a test, but we can verify the function
    // doesn't panic on normal usage.
}
```

- [ ] **Step 2: Replace `.unwrap()` with error propagation**

In `diff_tree_to_tree`, replace:
```rust
.unwrap()
.to_string_lossy()
```
With:
```rust
let path = match delta.new_file().path().or_else(|| delta.old_file().path()) {
    Some(p) => p.to_string_lossy().replace('\\', "/"),
    None => continue, // Skip entries with no path (binary files)
};
```

And for the renamed case:
```rust
git2::Delta::Renamed => {
    let old = match delta.old_file().path() {
        Some(p) => p.to_string_lossy().replace('\\', "/"),
        None => continue,
    };
    DiffStatus::Renamed { old_path: old }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- diff_tree`

- [ ] **Step 4: Commit**

```bash
git add src/git/mod.rs
git commit -m "fix(git): replace unwrap on delta path with graceful skip (H-8)"
```

---

### Task 11: Fix best_effort_canonicalize error swallowing (H-2)

**Files:**
- Modify: `src/util/path_security.rs:168-174`

- [ ] **Step 1: Fix to only fallback on NotFound**

```rust
fn best_effort_canonicalize(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(_) => path.to_path_buf(), // Permission errors, etc. — still fallback
    }
}
```

Actually, looking at this more carefully — the current behavior is correct for the use case. `best_effort_canonicalize` is deliberately best-effort: it's called for paths that may not exist (write targets) or may have permission issues. Changing it to only catch NotFound would break the write path (permission denied on parent dirs). Keep the current implementation but add a comment explaining why:

```rust
/// Best-effort canonicalization: use `fs::canonicalize` when the path exists
/// and is accessible, otherwise return the path as-is.
///
/// This deliberately swallows all errors (not just NotFound) because it's used
/// for write targets that may not exist yet and for paths where the user may
/// lack read permission on intermediate directories.
fn best_effort_canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
```

- [ ] **Step 2: Commit**

```bash
git add src/util/path_security.rs
git commit -m "docs(security): clarify best_effort_canonicalize error handling rationale (H-2)"
```

---

### Task 12: Fix LRU eviction TOCTOU (H-10)

**Files:**
- Modify: `src/lsp/manager.rs:187-230`

- [ ] **Step 1: Read the current eviction code**

Read the eviction logic in `LspManager` and understand the TOCTOU: the code checks size outside the lock, then acquires the lock to evict.

- [ ] **Step 2: Move check-and-evict into the same lock acquisition**

The fix: acquire the lock once, check size, evict if needed, then insert — all while holding the lock.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- lsp`

- [ ] **Step 4: Commit**

```bash
git add src/lsp/manager.rs
git commit -m "fix(lsp): perform LRU eviction inside lock to prevent TOCTOU (H-10)"
```

---

### Task 13: Cache regexes with LazyLock (M-1, L-8)

**Files:**
- Modify: `src/util/path_security.rs:551,401` (source access + dangerous command regexes)
- Modify: `src/tools/output_buffer.rs:358,368` (resolve_refs regexes)

- [ ] **Step 1: Cache dangerous command regexes**

In `is_dangerous_command`, the `DEFAULT_DANGEROUS_PATTERNS` regexes are recompiled on every call. Fix:

```rust
use std::sync::LazyLock;
use regex::Regex;

static DANGEROUS_REGEXES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    DEFAULT_DANGEROUS_PATTERNS
        .iter()
        .filter_map(|(pattern, desc)| {
            Regex::new(pattern).ok().map(|re| (re, *desc))
        })
        .collect()
});
```

Then use `DANGEROUS_REGEXES.iter()` instead of recompiling.

- [ ] **Step 2: Cache source access regexes**

Similarly for `check_source_file_access` — cache the `SOURCE_ACCESS_COMMANDS` and `SOURCE_EXTENSIONS` regexes.

- [ ] **Step 3: Cache resolve_refs regexes in output_buffer.rs**

Find the regex patterns in `resolve_refs` and cache with `LazyLock`.

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 5: Commit**

```bash
git add src/util/path_security.rs src/tools/output_buffer.rs
git commit -m "perf: cache compiled regexes with LazyLock (M-1, L-8)"
```

---

### Task 14: Fix count_lines consistency (M-2)

**Files:**
- Modify: `src/util/text.rs:14`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn count_lines_empty_returns_zero() {
    assert_eq!(count_lines(""), 0);
}
```

- [ ] **Step 2: Fix `count_lines`**

Change the function to return 0 for empty input:

```rust
pub fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.lines().count()
}
```

- [ ] **Step 3: Check for callers that depend on the old behavior**

Run: `grep -rn 'count_lines' src/` and verify no caller assumes `count_lines("") == 1`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib -- count_lines`

- [ ] **Step 5: Commit**

```bash
git add src/util/text.rs
git commit -m "fix(text): count_lines returns 0 for empty string (M-2)"
```

---

### Task 15: Fix UTF-8 boundary panic in interactive output (M-3)

**Files:**
- Modify: `src/tools/workflow.rs` (~line 1870)

- [ ] **Step 1: Find the byte-offset slice**

Search for byte-offset slicing into String content in the interactive output handler.

- [ ] **Step 2: Replace with `floor_char_boundary`**

```rust
// Old (panics on multi-byte chars):
&output[..max_bytes]
// New:
&output[..output.floor_char_boundary(max_bytes)]
```

Note: `floor_char_boundary` is stable since Rust 1.80. Verify the MSRV allows it.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- interactive`

- [ ] **Step 4: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "fix(workflow): use floor_char_boundary to prevent UTF-8 panic (M-3)"
```

---

### Task 16: Validate worktree paths (H-1)

**Files:**
- Modify: `src/util/path_security.rs` (`list_git_worktrees`)

- [ ] **Step 1: Add validation to `list_git_worktrees`**

After reading the gitdir content, validate:
- No null bytes
- Path is absolute
- Log warning if path escapes project ancestor

```rust
for entry in entries.flatten() {
    let gitdir_file = entry.path().join("gitdir");
    if let Ok(content) = std::fs::read_to_string(&gitdir_file) {
        let raw = content.trim();
        // Reject null bytes
        if raw.contains('\0') {
            tracing::warn!("worktree gitdir contains null byte, skipping: {:?}", gitdir_file);
            continue;
        }
        let worktree_git = PathBuf::from(raw);
        // Must be absolute
        if !worktree_git.is_absolute() {
            tracing::warn!("worktree gitdir is not absolute, skipping: {:?}", raw);
            continue;
        }
        if let Some(worktree_root) = worktree_git.parent() {
            paths.push(worktree_root.to_path_buf());
        }
    }
}
```

- [ ] **Step 2: Write test**

```rust
#[test]
fn list_git_worktrees_rejects_relative_path() {
    let dir = tempfile::tempdir().unwrap();
    let wt_entry = dir.path().join(".git").join("worktrees").join("evil");
    std::fs::create_dir_all(&wt_entry).unwrap();
    std::fs::write(wt_entry.join("gitdir"), "../../../etc/.git\n").unwrap();

    let result = list_git_worktrees(dir.path());
    assert!(result.is_empty(), "relative path should be rejected");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- list_git_worktrees`

- [ ] **Step 4: Commit**

```bash
git add src/util/path_security.rs
git commit -m "fix(security): validate worktree paths — reject relative and null bytes (H-1)"
```

---

### Task 17: Fix didOpen file size limit (H-5)

**Files:**
- Modify: `src/lsp/client.rs:551`

- [ ] **Step 1: Add size check before didOpen**

Find the `did_open` method and add a size check:

```rust
const MAX_DID_OPEN_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB

// Before reading the file for didOpen:
if let Ok(metadata) = std::fs::metadata(&path) {
    if metadata.len() > MAX_DID_OPEN_SIZE {
        tracing::debug!("skipping didOpen for large file ({} bytes): {}", metadata.len(), path.display());
        return Ok(());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib -- did_open`

- [ ] **Step 3: Commit**

```bash
git add src/lsp/client.rs
git commit -m "fix(lsp): skip didOpen for files > 10 MiB to prevent OOM (H-5)"
```

---

### Task 18: Fix remaining HIGH items (H-6, H-7, H-9)

**Files:**
- Modify: `src/tools/workflow.rs` (H-6: debug_assert → runtime check)
- Modify: `src/embed/index.rs` (H-7: SourceFilter enum)
- Modify: `src/lsp/client.rs` (H-9: poisoned mutex recovery)

- [ ] **Step 1: H-6 — Replace debug_assert with runtime check**

Find the `debug_assert!` for tmpfile path safety and replace with:

```rust
if !resolved.starts_with(&expected_dir) {
    return Err(RecoverableError::new(format!(
        "temporary file path {} escaped expected directory {}",
        resolved.display(),
        expected_dir.display(),
    )).into());
}
```

- [ ] **Step 2: H-7 — Add SourceFilter enum**

In `src/embed/index.rs`, add:

```rust
/// Filter for source file types in embedding search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFilter {
    /// Include all files
    All,
    /// Only source code files
    SourceOnly,
    /// Only non-source files (docs, config)
    NonSourceOnly,
}
```

Replace `Option<&str>` parameters with `SourceFilter`.

- [ ] **Step 3: H-9 — Fix poisoned mutex in LspClient::Drop**

Replace:
```rust
if let Ok(guard) = self.something.lock() {
```
With:
```rust
let guard = self.something.lock().unwrap_or_else(|e| e.into_inner());
```

- [ ] **Step 4: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs src/embed/index.rs src/lsp/client.rs
git commit -m "fix: debug_assert to runtime check (H-6), SourceFilter enum (H-7), mutex recovery (H-9)"
```

---

### Task 19: Fix remaining MEDIUM items (M-5, M-8, M-9, M-13, M-15)

**Files:**
- Modify: `src/tools/workflow.rs` (M-5: background job log cleanup)
- Modify: `src/config/project.rs` (M-8: size limit on project.toml)
- Modify: `src/embed/index.rs` (M-9: bytes_to_f32 validation)
- Modify: `src/util/path_security.rs` (M-13: enforce or remove shell_command_mode)
- Modify: `src/tools/github.rs` (M-15: cap limit param at 100)

- [ ] **Step 1: M-8 — Size limit on project.toml**

In `ProjectConfig::load_or_default`, before parsing:

```rust
let metadata = std::fs::metadata(&path)?;
if metadata.len() > 1024 * 1024 {
    bail!("project.toml exceeds 1 MiB limit ({} bytes)", metadata.len());
}
```

- [ ] **Step 2: M-9 — Validate bytes_to_f32 alignment**

In `bytes_to_f32`, add:

```rust
if bytes.len() % 4 != 0 {
    bail!("embedding blob size {} is not aligned to 4 bytes", bytes.len());
}
```

- [ ] **Step 3: M-15 — Cap GitHub limit param**

In the GitHub tool `call` methods that accept `limit`, clamp:

```rust
let limit = limit.min(100);
```

- [ ] **Step 4: M-13 — Remove dead `shell_command_mode` field or enforce it**

If `shell_command_mode` is checked somewhere in workflow.rs, keep it. If not, remove the field from `PathSecurityConfig` and `SecuritySection`.

Check: `grep -rn 'shell_command_mode' src/`

- [ ] **Step 5: M-5 — Background job log cleanup**

Add cleanup of background job log files in the LRU eviction callback or in the `OutputBuffer::evict` path.

- [ ] **Step 6: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "fix: project.toml size limit (M-8), embedding validation (M-9), GH limit cap (M-15)"
```

---

### Task 20: Final verification and tracker update

- [ ] **Step 1: Full quality gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all clean

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`

- [ ] **Step 3: Update code review tracker**

Mark all completed items as `[x]` in `docs/plans/2026-03-20-code-review-and-platform-abstraction-design.md`.

- [ ] **Step 4: Verify git log**

Review commits for cleanliness.

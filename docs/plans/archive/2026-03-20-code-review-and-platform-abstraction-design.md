# Code Review Findings + Security Profiles + Platform Abstraction

**Date:** 2026-03-20
**Status:** Approved design, pending implementation
**Branch:** experiments

---

## 1. Design: Security Profiles (`root` mode)

### Problem

The current security model assumes all projects are untrusted code sandboxes. This
breaks legitimate use cases:

- System administration projects (e.g. `~/agents/system/`) that need access to `/etc`,
  `~/.ssh`, and arbitrary system paths
- Projects that legitimately need `rm -rf target/`, `rm -rf .codescout/` without a
  two-round-trip speed bump
- Writing to paths outside the project root (e.g. deploying config files)

### Design

One new field in `.codescout/project.toml`:

```toml
[security]
profile = "default"  # or "root"
```

#### What `profile = "root"` disables

| Gate | Default mode | Root mode |
|------|-------------|-----------|
| Read deny-list (`~/.ssh`, `~/.aws`, etc.) | Hardcoded, active | Disabled |
| Write boundary (project root + temp + worktrees) | Active | Disabled |
| Dangerous command check (two-round-trip speed bump) | Active | Disabled |

#### What `profile = "root"` does NOT change

- Source-file shell access blocking ("use `read_file`/`find_symbol` instead" nudge) —
  this is tool-quality guidance, not a security gate
- Null-byte rejection in paths — always a bug, never intentional
- `check_tool_access` for disabled tool categories (`shell_enabled`,
  `file_write_enabled`, etc.) — orthogonal concerns
- Server instructions mention root mode so the LLM knows the project is unrestricted

#### Config options removed

- `shell_allow_always` — replaced by two-round-trip flow in default mode, skipped in
  root mode. The substring-match design was fundamentally broken (see finding C-1).
- `denied_read_patterns` — hardcoded deny-list is sufficient for default mode, gone in
  root mode. Never used in practice.

#### Implementation

- Add `SecurityProfile` enum (`Default`, `Root`) to `PathSecurityConfig`
- `validate_read_path` — early return `Ok` when `Root`
- `validate_write_path` — early return `Ok` when `Root`
- `is_dangerous_command` — early return `None` when `Root`
- Remove `shell_allow_always` and `denied_read_patterns` from config struct + TOML
- Update server instructions to mention root mode

---

## 2. Design: Platform Abstraction Layer

### Problem

Platform-specific code (`#[cfg(unix)]`, `#[cfg(windows)]`, `libc::kill`, `sh -c`,
`HOME` env var, `/tmp` hardcoding, POSIX shell tokenization) is scattered across
`workflow.rs`, `server.rs`, `lsp/client.rs`, `path_security.rs`, `output_buffer.rs`,
`config.rs`, and `logging.rs`. This makes Windows support fragile and untestable.

### Design

New module `src/platform/mod.rs` with OS-specific implementations:

```
src/platform/
  mod.rs       — public API (trait or free functions), #[cfg] dispatch
  unix.rs      — Unix/macOS implementations
  windows.rs   — Windows implementations
```

#### Public API surface

```rust
// Shell execution
pub fn shell_command(cmd: &str) -> tokio::process::Command
    // Unix: sh -c <cmd>
    // Windows: cmd /C <cmd>

pub fn shell_tokenize(input: &str) -> Vec<String>
    // Unix: POSIX quoting (single-quote, double-quote, backslash escape)
    // Windows: whitespace splitting (backslash is path separator, not escape)

// Home & paths
pub fn home_dir() -> Option<PathBuf>
    // Unix: $HOME
    // Windows: %USERPROFILE% (fallback %HOME%)

pub fn temp_dir() -> PathBuf
    // Canonicalized std::env::temp_dir()

pub fn denied_read_prefixes() -> Vec<PathBuf>
    // Unix: ~/.ssh, ~/.aws, ~/.gnupg, /etc/shadow, /etc/passwd
    // Windows: ~\.ssh, ~\.aws, ~\.gnupg, %SYSTEMROOT%\System32\config

pub fn path_display(path: &Path) -> String
    // Normalize separators for user-facing output

// Process management
pub fn terminate_process(pid: u32)
    // Unix: libc::kill(pid, SIGTERM)
    // Windows: TerminateProcess or kill_on_drop sufficiency

pub async fn shutdown_signal()
    // Unix: SIGTERM + SIGHUP + SIGINT
    // Windows: Ctrl-C only

// Filesystem
pub fn rename_overwrite(from: &Path, to: &Path) -> io::Result<()>
    // Unix: std::fs::rename (atomic, overwrites)
    // Windows: remove_file(to) then rename(from, to)
```

#### Migration

Every `#[cfg(unix)]` / `#[cfg(windows)]` block in the codebase is replaced by a call
to `platform::*`. The rest of the codebase never imports `libc`, references `SIGTERM`,
hardcodes `sh -c`, checks `HOME`, or writes `/tmp`. LSP server binary names
(`rust-analyzer` vs `rust-analyzer.exe`) also move to platform-aware config.

---

## 3. Code Review Findings Tracker

All findings from the 2026-03-20 full code review, organized by priority with
implementation status tracking.

### Legend

- Status: `[ ]` open, `[~]` in progress, `[x]` fixed, `[-]` removed/won't fix
- Severity after reclassification accounting for root mode design

---

### CRITICAL — Must fix

- [x] **C-2: `anchor_path_for_topic` path traversal** — `memory/anchors.rs:246`.
  Topic name passed unsanitized to `memories_dir.join()`. Fix: route through same
  `Path::components()` filtering as `topic_path`.

- [ ] **C-3: Uncapped `Content-Length` in LSP transport** — `lsp/transport.rs:35`.
  `vec![0u8; length]` with no upper bound. Fix: cap at 100 MiB.

- [ ] **C-5/C-6: Interactive + background spawn use `sh` unconditionally** —
  `tools/workflow.rs:1758,2035`. No `#[cfg(windows)]` path. Fix: use
  `platform::shell_command()`.

- [ ] **C-7: Hardcoded `/tmp`** — `path_security.rs:267`, `tools/workflow.rs:2003`.
  Fix: use `platform::temp_dir()`.

- [ ] **C-8: `libc::kill` in `LspClient::Drop`** — `lsp/client.rs:872-893`. Doesn't
  compile on Windows. Fix: use `platform::terminate_process()`.

- [ ] **C-9: LSP server binary names are Unix-only** — `lsp/servers/mod.rs:10-70`.
  Fix: platform-aware binary resolution.

- [x] **C-10: Dashboard memory topic path traversal** — `dashboard/api/memories.rs`.
  Same root cause as C-2. Fix: shared `sanitize_topic()` + post-join containment
  assertion.

### Reclassified/Removed from Critical

- [x] **C-1: `shell_allow_always` substring bypass** — REMOVED. `shell_allow_always`
  dropped entirely in the root mode design.

- [x] **C-4: Library path exemption doc mismatch** — downgraded to MEDIUM. Doc/test
  claim an exemption that doesn't exist in code. Fix: correct the docstring and test
  comment (no code change needed — deny-list should apply to library paths too).

---

### HIGH — Fix before Windows release

- [ ] **H-1: `list_git_worktrees` trusts unvalidated file content as write roots** —
  `path_security.rs:308`. Fix: validate absolute, no null bytes, log if escapes
  project ancestor.

- [ ] **H-2: `best_effort_canonicalize` swallows non-NotFound errors** —
  `path_security.rs:168`. Fix: only fall back for `ErrorKind::NotFound`.

- [ ] **H-3: `gh` API params not validated for special chars** — `tools/github.rs:114`.
  Fix: validate `owner`/`repo` against `[A-Za-z0-9._-]`.

- [ ] **H-4: No null-byte rejection on `gh` CLI args** — `tools/github.rs` (multiple).
  Fix: add `reject_null_bytes()` guard.

- [ ] **H-5: `did_open` reads entire file with no size limit** — `lsp/client.rs:551`.
  Fix: skip `didOpen` for files > 10 MiB.

- [ ] **H-6: `debug_assert!` for tmpfile path safety is no-op in release** —
  `tools/workflow.rs:~2190`. Fix: runtime check returning `RecoverableError`.

- [ ] **H-7: Stringly-typed `source_filter` in embedding search** —
  `embed/index.rs:993`. Fix: replace `Option<&str>` with `SourceFilter` enum.

- [ ] **H-8: `diff_tree_to_tree` unwrap on git path** — `git/mod.rs:58,66`. Panics on
  binary files or non-UTF-8 paths. Fix: propagate error with `?`.

- [ ] **H-9: `LspClient::Drop` swallows poisoned mutex** — `lsp/client.rs:872`. Fix:
  recover via `into_inner()` like rest of codebase.

- [ ] **H-10: LRU eviction TOCTOU in `LspManager`** — `lsp/manager.rs:187-230`. Fix:
  perform removal inside same lock acquisition.

- [ ] **H-11: `shell_words` treats `\` as escape — corrupts Windows paths** —
  `output_buffer.rs:554`. Fix: use `platform::shell_tokenize()`.

- [ ] **H-12: `$HOME` in `auto_register_cargo_deps`** — `tools/config.rs:401`. Fix:
  use `platform::home_dir()`.

- [ ] **H-13: `topic_path` doesn't handle Windows reserved names** —
  `memory/mod.rs:120`. Fix: use `Path::components()` filtering + reserved name check.

- [ ] **H-14: Logging uses `current_dir()` before CLI parsing** — `logging.rs:88`.
  Fix: defer or pass project root.

- [ ] **H-15: Deny-list `~/` prefix broken on Windows** — `path_security.rs:40-50`.
  Fix: use `platform::denied_read_prefixes()`.

- [ ] **H-16: HTTP embedding endpoint allows plaintext with API key** —
  `embed/remote.rs:68`. Fix: reject `http://` when API key is set.

---

### MEDIUM — Fix for robustness

- [ ] **M-1: Regex recompiled on every call** — `path_security.rs:551,401`. Fix: use
  `std::sync::LazyLock`.

- [ ] **M-2: `count_lines("")` returns 1, `extract_lines("",1,1)` returns `""`** —
  `util/text.rs:14`. Fix: make consistent (return 0 for empty).

- [ ] **M-3: UTF-8 boundary panic in interactive output** — `tools/workflow.rs:~1870`.
  Byte-offset slice into String. Fix: use `floor_char_boundary`.

- [ ] **M-4: `is_buffer_only` vs `resolve_refs` use different tokenizers** —
  `output_buffer.rs:531`. Fix: use `platform::shell_tokenize()` consistently.

- [ ] **M-5: Background job log files never cleaned up** — `tools/workflow.rs:~2040`.
  Fix: cleanup on LRU eviction.

- [ ] **M-6: Agent code duplication `new` vs `activate`** — `agent.rs:116,197`. Fix:
  extract `build_workspace`.

- [ ] **M-7: `normalize_path` doesn't validate containment** — `workspace.rs:404`.
  Fix: assert `result.starts_with(base)`.

- [ ] **M-8: No size limit on `project.toml` read** — `config/project.rs:292`. Fix:
  reject > 1 MiB.

- [ ] **M-9: `bytes_to_f32` silent truncation on corrupt blobs** —
  `embed/index.rs:1210`. Fix: validate `len() % 4 == 0`.

- [ ] **M-10: Nested `format!` SQL in `search_memories`** — `embed/index.rs:660`. Fix:
  single CTE statement.

- [ ] **M-11: LSP error -32800 detection via string matching** — `lsp/client.rs:294`.
  Fix: typed `LspError` with code field.

- [ ] **M-12: No CORS on dashboard write endpoints** — `dashboard/routes.rs:18`. Fix:
  add `CorsLayer` scoped to localhost.

- [ ] **M-13: `shell_command_mode` in config never enforced in `path_security.rs`** —
  `path_security.rs:73`. Fix: remove field or enforce.

- [ ] **M-14: Poisoned mutex recovery should log** — `agent.rs:344+`. Fix: add
  `tracing::error!` in `unwrap_or_else` closures.

- [ ] **M-15: `limit` param on GitHub API not capped at 100** —
  `tools/github.rs:262`. Fix: clamp and document.

- [ ] **M-16: `upsert_memory_by_title` docstring says bucket updated but isn't** —
  `embed/index.rs:724`. Fix: correct docstring or update bucket.

- [ ] **M-17 (was C-4): Library path exemption doc mismatch** —
  `path_security.rs:188`. Fix: correct docstring and test.

---

### LOW — Nice to fix

- [ ] **L-1: Line-join `\n` budget off-by-1 for `\r\n` files** — `util/text.rs:32`.
- [ ] **L-2: `ensure_gitignored` writes Unix line endings** — `memory/mod.rs:49`.
- [ ] **L-3: `std::fs::rename` doesn't overwrite on Windows** — `logging.rs:17`. Fix:
  use `platform::rename_overwrite()`.
- [ ] **L-4: `common_path_prefix` splits on `"/"`** — `tools/file.rs:1318`.
- [ ] **L-5: `format_list_dir_tree_body` hardcodes `/`** — `tools/file.rs:1304`.
- [ ] **L-6: `find_ancestor_with` uses `.exists()` TOCTOU** — `util/fs.rs:11`.
- [ ] **L-7: Absolute paths with `..` bypass deny-list on canonicalize fallback** —
  `path_security.rs:205`.
- [ ] **L-8: Regex recompiled in `resolve_refs`** — `output_buffer.rs:358,368`.
- [ ] **L-9: Auth token — confirm OsRng vs ThreadRng** — `server.rs:320`.
- [ ] **L-10: `LspKey` doesn't canonicalize path** — `lsp/manager.rs:38`.
- [ ] **L-11: `generate_instance_id` ~16 bits entropy** — `logging.rs:26`.
- [ ] **L-12: `purge_missing_files` N individual `exists()` checks** —
  `embed/index.rs:824`.
- [ ] **L-13: `auto_register_cargo_deps` lexicographic version sort** —
  `tools/config.rs:547`.
- [ ] **L-14: `main.rs` early-peek arg scan false positives** — `main.rs:83`.
- [ ] **L-15: `ActiveProject` Clone shares `Arc<Mutex>` undocumented** —
  `agent.rs:89`.
- [ ] **L-16: Idle eviction loop interval 7.5min** — `lsp/manager.rs:629`.
- [ ] **L-17: `serve_index` debug path hardcoded relative** — `dashboard/routes.rs:64`.
- [ ] **L-18: Missing `.bat`/`.ps1` in `detect_language`** — `tools/file.rs:1485`.

---

## 4. Implementation Order

### Phase 1 — Security + Root Mode
1. Add `SecurityProfile` enum and `profile` config field
2. Wire into `validate_read_path`, `validate_write_path`, `is_dangerous_command`
3. Remove `shell_allow_always` and `denied_read_patterns`
4. Fix anchor path traversal (C-2, C-10)
5. Cap LSP `Content-Length` (C-3)
6. Fix `gh` parameter validation (H-3, H-4)
7. Add CORS to dashboard (M-12)
8. Enforce HTTPS with API key (H-16)

### Phase 2 — Platform Abstraction Layer
1. Create `src/platform/{mod,unix,windows}.rs`
2. Implement: `shell_command`, `shell_tokenize`, `home_dir`, `temp_dir`,
   `denied_read_prefixes`, `terminate_process`, `shutdown_signal`,
   `rename_overwrite`, `path_display`
3. Migrate all `#[cfg]` blocks to use `platform::*`
4. Add Windows LSP binary names
5. Fix all hardcoded `/tmp`, `HOME`, `libc::kill`, `sh -c`

### Phase 3 — Quality + Robustness
1. Fix panics: git path unwrap (H-8), UTF-8 boundary (M-3)
2. Cache regexes with `LazyLock` (M-1, L-8)
3. Fix LRU eviction TOCTOU (H-10)
4. Typed errors: `LspError` (M-11), `SourceFilter` enum (H-7)
5. Remaining medium + low fixes

# LSP Mux Infrastructure + Rust Rollout — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the cross-cutting mux infrastructure (per-language idle timeout, env forwarding, project-config opt-out, shared test harness) AND flip `rust` to `mux: true` with a multi-`Agent` coherence test.

**Architecture:** Four small refactors to `LspServerConfig` / `LspManager` / the `Mux` CLI / `ProjectConfig`. Then flip one flag in `src/lsp/servers/mod.rs`. All tests live as unit tests in `src/lsp/mux/` so they can use a shared `#[cfg(test)]` test-support module — this is a minor deviation from the spec (which placed them under `tests/`) because `tests/` integration tests can't reach crate-private `#[cfg(test)]` code.

**Tech Stack:** Rust, tokio, clap, serde (TOML), existing `src/lsp/mux/` multiplexer.

**Reference spec:** `docs/superpowers/specs/2026-04-18-generalized-lsp-mux-design.md`.

---

## File Structure

**Modified:**
- `src/lsp/client.rs` — `LspServerConfig` gets `idle_timeout_secs: Option<u64>` field; every literal in the file updated.
- `src/lsp/manager.rs` — `get_or_start_via_mux` uses `config.idle_timeout_secs`, forwards `config.env` as repeated `--env KEY=VAL` args. `get_or_start` applies project-config opt-out after `default_config` returns.
- `src/lsp/servers/mod.rs` — every `LspServerConfig` literal gets `idle_timeout_secs: None` added (or `Some(180)` for rust). Rust flipped to `mux: true`.
- `src/main.rs` — `Mux` CLI subcommand gets a repeating `--env KEY=VAL` argument; passes parsed tuples into `mux::process::run`.
- `src/config/project.rs` — new `LspSection` field with a `HashMap<String, LspLangOverride>`.
- `src/lsp/mux/mod.rs` — declares `#[cfg(test)] pub(crate) mod test_support;` and `#[cfg(test)] mod coherence_rust;`.
- `docs/manual/src/experimental/index.md` — add link to the new experimental page.

**Created:**
- `src/lsp/mux/test_support.rs` — shared two-`Agent` coherence harness.
- `src/lsp/mux/coherence_rust.rs` — Rust-specific coherence test that exercises the harness.
- `tests/fixtures/lsp-mux/rust/Cargo.toml` — minimal Rust project fixture.
- `tests/fixtures/lsp-mux/rust/src/lib.rs` — one function for `find_symbol` to target.
- `docs/manual/src/experimental/mux-rust.md` — experimental-feature page for rust mux.

Each file above has a single responsibility: `test_support.rs` is the shared harness, `coherence_rust.rs` is the Rust test, etc. Further languages (Java, Python, TS/JS, Go) add sibling `coherence_<lang>.rs` files and `tests/fixtures/lsp-mux/<lang>/` fixtures in follow-up plans.

---

## Task 1: Add `idle_timeout_secs` field to `LspServerConfig`

**Files:**
- Modify: `src/lsp/client.rs` — `LspServerConfig` struct near line 100. Every `LspServerConfig { ... }` literal in the file gets the new field.

Search all literal constructions of `LspServerConfig` in the whole tree (not just `client.rs`) and add the field to every one.

- [ ] **Step 1: Write the failing test.** Add this test to the existing `#[cfg(test)] mod tests` inside `src/lsp/client.rs`:

```rust
#[test]
fn lsp_server_config_has_idle_timeout_field() {
    let cfg = LspServerConfig {
        command: "dummy".to_string(),
        args: vec![],
        workspace_root: std::path::PathBuf::from("/tmp"),
        init_timeout: None,
        mux: false,
        env: vec![],
        idle_timeout_secs: Some(42),
    };
    assert_eq!(cfg.idle_timeout_secs, Some(42));
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test --lib lsp::client::tests::lsp_server_config_has_idle_timeout_field`
Expected: **compile error** — `idle_timeout_secs` is not a field of `LspServerConfig`.

- [ ] **Step 3: Add the field to `LspServerConfig`.** In `src/lsp/client.rs`, locate the struct and add the field after `env`:

```rust
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub command: String,
    #[allow(dead_code)]
    pub args: Vec<String>,
    pub workspace_root: std::path::PathBuf,
    pub init_timeout: Option<std::time::Duration>,
    pub mux: bool,
    pub env: Vec<(String, String)>,
    /// Seconds the mux process waits with no connected clients before
    /// exiting. Only used when `mux == true`. `None` falls back to the
    /// mux default of 300s. Ignored on the direct-process path.
    pub idle_timeout_secs: Option<u64>,
}
```

- [ ] **Step 4: Add `idle_timeout_secs: None` to every existing `LspServerConfig` literal.**

Grep and fix every construction site. Expected sites (grep pattern: `LspServerConfig {`):

- `src/lsp/client.rs` — any inside the file besides the struct def (e.g. test helpers at ~1286, 1312, 1335, 1386, 1664, 1710, 1773, 1834, 1887).
- `src/lsp/manager.rs` — ~920, ~992.
- `src/lsp/servers/mod.rs` — all 13 per-language entries.

For every `mux: false,` literal add one more line `idle_timeout_secs: None,`.
For the `kotlin` entry at `src/lsp/servers/mod.rs` (the only `mux: true`), add `idle_timeout_secs: Some(300),` to preserve today's 300s hardcoded behaviour.

- [ ] **Step 5: Run the test.**

Run: `cargo test --lib lsp::client::tests::lsp_server_config_has_idle_timeout_field`
Expected: PASS.

- [ ] **Step 6: Confirm nothing else broke.**

Run: `cargo build --lib 2>&1 | tail -20`
Expected: no errors. Any missed `LspServerConfig { ... }` literal is a compile failure — go fix it.

- [ ] **Step 7: Commit.**

```bash
git add src/lsp/client.rs src/lsp/manager.rs src/lsp/servers/mod.rs
git commit -m "feat(lsp): add idle_timeout_secs field to LspServerConfig"
```

---

## Task 2: Plumb `config.env` through the `Mux` CLI

Today `config.env` is silently lost on the mux path. Kotlin's `GRADLE_USER_HOME` is effectively dead code, and Java will need this to work. Fix it now as part of infra.

**Files:**
- Modify: `src/main.rs` — add a repeating `--env KEY=VAL` arg to the `Mux` subcommand; parse into `Vec<(String, String)>`; pass into `mux::process::run`.
- Modify: `src/lsp/manager.rs` — extend `mux_args` with one `--env KEY=VAL` per entry of `config.env` before the `--` separator.

- [ ] **Step 1: Write the failing test.** Add to `src/lsp/manager.rs`'s existing `#[cfg(test)] mod tests` (grep for `mod tests` in that file and extend):

```rust
#[test]
fn build_mux_args_includes_env_forwarding() {
    use std::path::PathBuf;
    let cfg = crate::lsp::client::LspServerConfig {
        command: "fakelsp".into(),
        args: vec!["--stdio".into()],
        workspace_root: PathBuf::from("/tmp/ws"),
        init_timeout: None,
        mux: true,
        env: vec![
            ("GRADLE_USER_HOME".into(), "/tmp/g".into()),
            ("FOO".into(), "bar".into()),
        ],
        idle_timeout_secs: Some(123),
    };
    let args = crate::lsp::manager::build_mux_args(
        &PathBuf::from("/tmp/ws"),
        &PathBuf::from("/tmp/sock"),
        &PathBuf::from("/tmp/lock"),
        &cfg,
    );
    // idle timeout honoured
    let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
    assert_eq!(args[idle_idx + 1], "123");
    // env flags appear before `--`
    let dash_idx = args.iter().position(|a| a == "--").unwrap();
    let env_args: Vec<_> = args[..dash_idx]
        .iter()
        .zip(args[1..dash_idx].iter())
        .filter(|(a, _)| *a == "--env")
        .map(|(_, b)| b.clone())
        .collect();
    assert!(env_args.contains(&"GRADLE_USER_HOME=/tmp/g".to_string()));
    assert!(env_args.contains(&"FOO=bar".to_string()));
    // server command is last
    assert_eq!(args[dash_idx + 1], "fakelsp");
    assert_eq!(args[dash_idx + 2], "--stdio");
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test --lib lsp::manager::tests::build_mux_args_includes_env_forwarding`
Expected: compile error — `build_mux_args` does not exist.

- [ ] **Step 3: Extract the arg construction into a testable free function.** In `src/lsp/manager.rs`, above `impl LspManager`, add:

```rust
/// Build the CLI argv passed to a spawned `codescout mux` child. Factored out
/// for unit-testability; `get_or_start_via_mux` is the only caller.
#[cfg(unix)]
pub(super) fn build_mux_args(
    workspace_root: &std::path::Path,
    socket_path: &std::path::Path,
    lock_path: &std::path::Path,
    config: &crate::lsp::client::LspServerConfig,
) -> Vec<String> {
    let idle = config.idle_timeout_secs.unwrap_or(300);
    let mut args = vec![
        "mux".to_string(),
        "--socket".to_string(),
        socket_path.to_string_lossy().to_string(),
        "--lock".to_string(),
        lock_path.to_string_lossy().to_string(),
        "--cwd".to_string(),
        workspace_root.to_string_lossy().to_string(),
        "--idle-timeout".to_string(),
        idle.to_string(),
    ];
    for (k, v) in &config.env {
        args.push("--env".to_string());
        args.push(format!("{k}={v}"));
    }
    args.push("--".to_string());
    args.push(config.command.clone());
    args.extend(config.args.iter().cloned());
    args
}
```

Then replace the inline `mux_args` construction in `get_or_start_via_mux` with:

```rust
let mux_args = build_mux_args(workspace_root, &socket_path, &lock_path, &config);
```

(This replaces the `let mut mux_args = vec![...]; mux_args.extend(...);` block near the `--idle-timeout` / `"300"` hardcoded pair at ~line 392–405.)

- [ ] **Step 4: Run unit test.**

Run: `cargo test --lib lsp::manager::tests::build_mux_args_includes_env_forwarding`
Expected: PASS.

- [ ] **Step 5: Extend the `Mux` CLI subcommand to accept `--env`.** In `src/main.rs`, locate the `Mux { ... }` enum variant (near line 79) and add a new field before `server_cmd`:

```rust
        /// Environment variables to set on the LSP server process. Repeat
        /// flag per variable. Format: `KEY=VAL`.
        #[arg(long = "env", value_parser = parse_env_kv)]
        server_env: Vec<(String, String)>,
```

And add the parser helper at the top of `src/main.rs` (below any existing `use` block):

```rust
fn parse_env_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("--env expects KEY=VAL, got {s:?}"))?;
    Ok((k.to_string(), v.to_string()))
}
```

- [ ] **Step 6: Wire `server_env` into `mux::process::run`.** In the `Commands::Mux { ... }` match arm inside `main` (near line 158), add `server_env,` to the destructuring pattern and replace the `&[]` final argument with `&server_env`:

```rust
        Commands::Mux {
            socket,
            lock,
            cwd,
            idle_timeout,
            server_cmd,
            server_env,
        } => {
            codescout::lsp::mux::process::run(
                &socket,
                &lock,
                &cwd,
                idle_timeout,
                &server_cmd[0],
                &server_cmd[1..],
                &server_env,
            )
            .await?;
        }
```

- [ ] **Step 7: Build to verify.**

Run: `cargo build 2>&1 | tail -10`
Expected: builds cleanly.

- [ ] **Step 8: Commit.**

```bash
git add src/lsp/manager.rs src/main.rs
git commit -m "feat(lsp): forward LspServerConfig.env through the mux CLI"
```

---

## Task 3: Use `config.idle_timeout_secs` at spawn (and document Kotlin unchanged)

This is mostly covered by Task 2 (the new `build_mux_args` already honours `config.idle_timeout_secs`). This task adds one regression-guard test specifically for the 300s fallback path.

**Files:**
- Modify: `src/lsp/manager.rs` — one more test.

- [ ] **Step 1: Write the failing test.** Add to `#[cfg(test)] mod tests` in `src/lsp/manager.rs`:

```rust
#[test]
fn build_mux_args_defaults_idle_timeout_to_300_when_none() {
    use std::path::PathBuf;
    let cfg = crate::lsp::client::LspServerConfig {
        command: "x".into(),
        args: vec![],
        workspace_root: PathBuf::from("/tmp/ws"),
        init_timeout: None,
        mux: true,
        env: vec![],
        idle_timeout_secs: None,
    };
    let args = crate::lsp::manager::build_mux_args(
        &PathBuf::from("/tmp/ws"),
        &PathBuf::from("/tmp/sock"),
        &PathBuf::from("/tmp/lock"),
        &cfg,
    );
    let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
    assert_eq!(args[idle_idx + 1], "300");
}
```

- [ ] **Step 2: Run test.**

Run: `cargo test --lib lsp::manager::tests::build_mux_args_defaults_idle_timeout_to_300_when_none`
Expected: PASS immediately — `build_mux_args` already implements this. This is a regression guard.

- [ ] **Step 3: Commit.**

```bash
git add src/lsp/manager.rs
git commit -m "test(lsp): lock mux idle_timeout_secs=None → 300s fallback"
```

---

## Task 4: Add `[lsp.<lang>]` opt-out to `ProjectConfig`

**Files:**
- Modify: `src/config/project.rs` — new `LspSection` struct + field on `ProjectConfig`; `HashMap`-backed so we don't enumerate languages.

- [ ] **Step 1: Write the failing test.** Add to the `#[cfg(test)] mod tests` at the bottom of `src/config/project.rs`:

```rust
#[test]
fn lsp_section_parses_per_language_opt_out() {
    let toml = r#"
[project]
name = "demo"

[lsp.rust]
mux = false

[lsp.python]
mux = true
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.lsp.langs.get("rust").and_then(|o| o.mux), Some(false));
    assert_eq!(cfg.lsp.langs.get("python").and_then(|o| o.mux), Some(true));
    assert!(cfg.lsp.langs.get("go").is_none());
}

#[test]
fn lsp_section_absent_parses_to_empty_map() {
    let toml = r#"
[project]
name = "demo"
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert!(cfg.lsp.langs.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test --lib config::project::tests::lsp_section_parses_per_language_opt_out`
Expected: compile error — no `lsp` field on `ProjectConfig`.

- [ ] **Step 3: Add `LspSection` and the field.** Add near the other section structs in `src/config/project.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspSection {
    /// Per-language overrides, keyed by language name ("rust", "java", ...).
    #[serde(flatten)]
    pub langs: std::collections::HashMap<String, LspLangOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspLangOverride {
    /// Force `mux: false` (direct-process) or `mux: true` (multiplexer).
    /// `None` means "use the built-in default from servers::default_config".
    #[serde(default)]
    pub mux: Option<bool>,
}
```

And add the field on `ProjectConfig`:

```rust
pub struct ProjectConfig {
    pub project: ProjectSection,
    #[serde(default)]
    pub embeddings: EmbeddingsSection,
    #[serde(default)]
    pub ignored_paths: IgnoredPathsSection,
    #[serde(default)]
    pub security: SecuritySection,
    #[serde(default)]
    pub memory: MemorySection,
    #[serde(default)]
    pub libraries: LibrariesSection,
    #[serde(default)]
    pub lsp: LspSection,
}
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test --lib config::project::tests::lsp_section`
Expected: both new tests PASS.

- [ ] **Step 5: Commit.**

```bash
git add src/config/project.rs
git commit -m "feat(config): add [lsp.<lang>] per-language mux override"
```

---

## Task 5: Apply project-config opt-out in `LspManager::get_or_start`

**Approach:** Thread the resolved `Option<bool>` override through the `LspProvider::get_or_start` trait as a new parameter. Call sites resolve the override from `ctx.agent` before calling. `LspManager` uses it to override `default_config(lang).mux`.

**Files touched:**
- Modify: `src/lsp/ops.rs` — trait `LspProvider::get_or_start` gets a new `mux_override: Option<bool>` parameter.
- Modify: `src/lsp/manager.rs` — impl honours the override; add `resolve_mux_flag` helper + tests.
- Modify: `src/lsp/mock.rs` — `MockLspProvider::get_or_start` gets the new parameter.
- Modify: `src/mcp_resources/project_summary.rs` — test stubs `ReadyLspProvider` / `NotReadyLspProvider` get the new parameter.
- Modify: every tool call site. Grep: `.get_or_start(` inside `src/tools/`. Expected ~11 sites in `src/tools/symbol.rs`. Each passes the resolved override (typically `None` unless the tool has a reason to force a specific value).

- [ ] **Step 1: Write the failing unit test for the helper.** Append to the `#[cfg(test)] mod tests` in `src/lsp/manager.rs`:

```rust
#[test]
fn resolve_mux_flag_override_wins() {
    assert_eq!(crate::lsp::manager::resolve_mux_flag(true, Some(false)), false);
    assert_eq!(crate::lsp::manager::resolve_mux_flag(false, Some(true)), true);
}

#[test]
fn resolve_mux_flag_none_uses_default() {
    assert_eq!(crate::lsp::manager::resolve_mux_flag(true, None), true);
    assert_eq!(crate::lsp::manager::resolve_mux_flag(false, None), false);
}
```

- [ ] **Step 2: Run to verify it fails.**

Run: `cargo test --lib lsp::manager::tests::resolve_mux_flag`
Expected: compile error — `resolve_mux_flag` not found.

- [ ] **Step 3: Implement the helper.** In `src/lsp/manager.rs`, near `build_mux_args`:

```rust
/// Resolve the effective `mux` flag. `override_` (from project config) wins; else fall back to `default`.
pub(super) fn resolve_mux_flag(default: bool, override_: Option<bool>) -> bool {
    override_.unwrap_or(default)
}
```

- [ ] **Step 4: Run unit tests.**

Run: `cargo test --lib lsp::manager::tests::resolve_mux_flag`
Expected: both PASS.

- [ ] **Step 5: Add the parameter to the trait.** In `src/lsp/ops.rs`, change the method signature:

```rust
async fn get_or_start(
    &self,
    language: &str,
    workspace_root: &Path,
    mux_override: Option<bool>,
) -> anyhow::Result<Arc<dyn LspClientOps>>;
```

- [ ] **Step 6: Update `LspManager::get_or_start`.** Change the impl signature to match, and near the `default_config` call (around line 226):

```rust
let mut config = servers::default_config(language, workspace_root).ok_or_else(|| {
    anyhow::anyhow!("No LSP server configured for language: {}", language)
})?;
config.mux = resolve_mux_flag(config.mux, mux_override);
```

- [ ] **Step 7: Update all other impls.**

In `src/lsp/mock.rs`:

```rust
async fn get_or_start(
    &self,
    language: &str,
    workspace_root: &Path,
    _mux_override: Option<bool>,
) -> anyhow::Result<Arc<dyn LspClientOps>> {
    // existing body unchanged
}
```

In `src/mcp_resources/project_summary.rs` test stubs (both `ReadyLspProvider` and `NotReadyLspProvider`), update their `get_or_start` signatures to accept `_mux_override: Option<bool>`.

- [ ] **Step 8: Update every tool call site.**

Find them all:

```bash
rg -n 'get_or_start\(' src/ | rg -v 'fn get_or_start'
```

Expected call sites are in `src/tools/symbol.rs` (~11). At each site, resolve the override from the active project config and pass it:

```rust
// Before:
let client = ctx.lsp.get_or_start(lang, &root).await?;

// After:
let mux_override = ctx
    .agent
    .with_project(|p| Ok(p.config.lsp.langs.get(lang).and_then(|o| o.mux)))
    .await
    .unwrap_or(None);
let client = ctx.lsp.get_or_start(lang, &root, mux_override).await?;
```

Because this repeats, add a helper on `Agent` to keep call sites one-liner-able. Add to `impl Agent` in `src/agent/mod.rs` (near `index_status_label`):

```rust
/// Resolve the per-language `mux` override from the active project's config.
/// Returns `None` when no project is active or no override is set.
pub async fn lsp_mux_override(&self, language: &str) -> Option<bool> {
    self.with_project(|p| {
        Ok(p.config.lsp.langs.get(language).and_then(|o| o.mux))
    })
    .await
    .unwrap_or(None)
}
```

Then each call site becomes:

```rust
let mux_override = ctx.agent.lsp_mux_override(lang).await;
let client = ctx.lsp.get_or_start(lang, &root, mux_override).await?;
```

- [ ] **Step 9: Update test call sites in `src/lsp/manager.rs`.** Existing tests (grep in `src/lsp/manager.rs` for `.get_or_start(`) like at lines 936, 960, 1037, 1244: append `, None` to each call.

- [ ] **Step 10: Run the full suite.**

Run: `cargo test --lib`
Expected: clean pass.

- [ ] **Step 11: Write a focused integration test for the opt-out path.** Append to `#[cfg(test)] mod tests` in `src/lsp/manager.rs`:

```rust
#[tokio::test]
async fn project_override_forces_direct_path_for_rust() {
    // Before Task 7 flips rust to mux: true, default is false — override is a no-op.
    // After Task 7, default is true and this test verifies opt-out.
    // We test the resolve helper directly because end-to-end mux dispatch
    // requires rust-analyzer on PATH.
    let mgr = LspManager::new();
    let dir = tempfile::tempdir().unwrap();

    // With override=Some(false) and default=true (post-Task-7), mux must be bypassed.
    let default_mux = servers::default_config("rust", dir.path())
        .map(|c| c.mux)
        .unwrap_or(false);
    let effective = resolve_mux_flag(default_mux, Some(false));
    assert!(!effective, "project opt-out must force direct-process path");

    // Sanity: without override, whatever the default is wins.
    let effective_default = resolve_mux_flag(default_mux, None);
    assert_eq!(effective_default, default_mux);

    // Unused binding silences clippy if mgr isn't otherwise touched.
    drop(mgr);
}
```

- [ ] **Step 12: Run it.**

Run: `cargo test --lib lsp::manager::tests::project_override_forces_direct_path_for_rust`
Expected: PASS.

- [ ] **Step 13: Commit.**

```bash
git add src/lsp/ops.rs src/lsp/manager.rs src/lsp/mock.rs src/mcp_resources/project_summary.rs src/agent/mod.rs src/tools/symbol.rs
git commit -m "feat(lsp): apply per-language mux opt-out from project config"
```

(If any other tool file needed changes, add it to the `git add` above.)
## Task 6: Create the two-`Agent` coherence test harness

**Files:**
- Create: `src/lsp/mux/test_support.rs` — shared harness.
- Modify: `src/lsp/mux/mod.rs` — add `#[cfg(test)] pub(crate) mod test_support;`.

- [ ] **Step 1: Declare the new module in `mod.rs`.** In `src/lsp/mux/mod.rs` below the existing `pub mod process;` / `pub mod protocol;` lines, add:

```rust
#[cfg(test)]
pub(crate) mod test_support;
```

- [ ] **Step 2: Write the harness module.** Create `src/lsp/mux/test_support.rs`:

```rust
//! Shared coherence test harness: spawn two `Agent` instances sharing one
//! mux, let one write, verify the other observes the fresh state.
//!
//! Per-language tests live in sibling `coherence_<lang>.rs` modules and
//! supply:
//!   • the path to a fixture project (see `tests/fixtures/lsp-mux/<lang>/`),
//!   • the language name passed to `get_or_start`,
//!   • a pre-write + post-write symbol name pair to assert on.

use std::path::{Path, PathBuf};

/// Copy a fixture directory into a fresh tempdir.
/// Returns the tempdir (keep it alive) and the root path.
pub(crate) fn stage_fixture(fixture: &Path) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().to_path_buf();
    copy_dir_all(fixture, &dest).expect("copy fixture");
    (dir, dest)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_all(&src_child, &dst_child)?;
        } else {
            std::fs::copy(&src_child, &dst_child)?;
        }
    }
    Ok(())
}

/// Spawn two agents on the same workspace, both pointed at the same mux.
/// Returns `(agent_a, agent_b, workspace_root, _tempdir)`. Drop the tempdir
/// to remove the workspace.
pub(crate) async fn two_agents_on_fixture(
    fixture_rel: &str,
) -> (
    crate::agent::Agent,
    crate::agent::Agent,
    PathBuf,
    tempfile::TempDir,
) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lsp-mux")
        .join(fixture_rel);
    assert!(
        fixture.exists(),
        "fixture missing: {}",
        fixture.display()
    );
    let (tempdir, root) = stage_fixture(&fixture);
    let a = crate::agent::Agent::new(Some(root.clone()))
        .await
        .expect("Agent A");
    let b = crate::agent::Agent::new(Some(root.clone()))
        .await
        .expect("Agent B");
    (a, b, root, tempdir)
}
```

- [ ] **Step 3: Verify the module compiles.**

Run: `cargo test --lib lsp::mux:: 2>&1 | tail -20`
Expected: compiles; existing mux tests still pass.

- [ ] **Step 4: Commit.**

```bash
git add src/lsp/mux/mod.rs src/lsp/mux/test_support.rs
git commit -m "test(lsp): add two-Agent coherence test harness"
```

---

## Task 7: Flip rust to `mux: true` and add the Rust coherence test

**Files:**
- Modify: `src/lsp/servers/mod.rs` — `rust` entry: `mux: true`, `idle_timeout_secs: Some(180)`.
- Modify: `src/lsp/mux/mod.rs` — declare `#[cfg(test)] mod coherence_rust;`.
- Create: `src/lsp/mux/coherence_rust.rs` — the test.
- Create: `tests/fixtures/lsp-mux/rust/Cargo.toml`.
- Create: `tests/fixtures/lsp-mux/rust/src/lib.rs`.

- [ ] **Step 1: Create the Rust fixture.**

`tests/fixtures/lsp-mux/rust/Cargo.toml`:

```toml
[package]
name = "lsp-mux-fixture"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
```

`tests/fixtures/lsp-mux/rust/src/lib.rs`:

```rust
pub fn original_symbol() -> &'static str {
    "original"
}
```

- [ ] **Step 2: Write the failing coherence test.** Create `src/lsp/mux/coherence_rust.rs`:

```rust
//! Rust-specific mux coherence test.
//!
//! Two Agents share one mux. A writes a new function; B must see it.
//! The bug being regression-tested: before mux, B's direct rust-analyzer
//! still saw the pre-write file because A's didChange only went to A's LSP.

use super::test_support::two_agents_on_fixture;

#[tokio::test]
#[ignore = "requires rust-analyzer on PATH; gated by CI job"]
async fn two_agents_coherent_after_edit() {
    let (a, b, root, _td) = two_agents_on_fixture("rust").await;
    let lsp_a = std::sync::Arc::new(crate::lsp::manager::LspManager::new());
    let lsp_b = std::sync::Arc::new(crate::lsp::manager::LspManager::new());

    // 1. Both warm up a client; first wins the file-lock and spawns mux;
    //    second connects. Returning the LspClient lets us open docs on it.
    let ca = lsp_a
        .get_or_start("rust", &root, None)
        .await
        .expect("A start");
    let cb = lsp_b
        .get_or_start("rust", &root, None)
        .await
        .expect("B start");

    // 2. Open the target file in both clients so both maintain document state.
    let target = root.join("src/lib.rs");
    ca.did_open(&target).await.expect("A didOpen");
    cb.did_open(&target).await.expect("B didOpen");

    // 3. A writes a new symbol and notifies the LSP.
    let updated = r#"
pub fn original_symbol() -> &'static str { "original" }
pub fn fresh_symbol() -> &'static str { "fresh" }
"#;
    std::fs::write(&target, updated).unwrap();
    lsp_a.notify_file_changed(&target).await;

    // 4. B queries document symbols — must see `fresh_symbol`.
    //    `document_symbols` is the LSP `textDocument/documentSymbol` request.
    let syms = cb
        .document_symbols(&target)
        .await
        .expect("B document_symbols");
    let names: Vec<_> = syms.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.iter().any(|n| n == "fresh_symbol"),
        "Agent B's view stale: {:?}",
        names
    );

    lsp_a.shutdown_all().await;
    lsp_b.shutdown_all().await;
    drop(a);
    drop(b);
}
```

*Note on `#[ignore]`:* rust-analyzer is not guaranteed on CI boxes. Keep the test ignore-by-default; CI adds a dedicated job that runs `cargo test -- --ignored lsp::mux::coherence_rust`. If `list_document_symbols` is not the correct method name in this codebase, substitute the equivalent API by grepping `src/lsp/client.rs` for `fn.*document_symbol`.

- [ ] **Step 3: Declare the test module.** In `src/lsp/mux/mod.rs` below the `test_support` declaration:

```rust
#[cfg(test)]
mod coherence_rust;
```

- [ ] **Step 4: Run to verify it fails with the current `mux: false`.** First commit the test-only changes:

```bash
git add src/lsp/mux/coherence_rust.rs src/lsp/mux/mod.rs tests/fixtures/lsp-mux/rust
git commit -m "test(lsp): rust coherence test + fixture (ignored by default)"
```

Run: `cargo test --lib -- --ignored lsp::mux::coherence_rust`
Expected: **test fails or hangs** (confirming the stale-doc bug) — no mux in play, B's LSP has no way to see A's `didChange`. If rust-analyzer is missing, you'll see a spawn error instead — that's also an acceptable "confirms not-yet-fixed" signal.

- [ ] **Step 5: Flip the flag.** In `src/lsp/servers/mod.rs`, change the `"rust" =>` arm:

```rust
        "rust" => Some(LspServerConfig {
            command: crate::platform::lsp_binary_name("rust-analyzer"),
            args: vec![],
            workspace_root: root,
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: Some(180),
        }),
```

- [ ] **Step 6: Run the test again.**

Run: `cargo test --lib -- --ignored lsp::mux::coherence_rust`
Expected: PASS.

- [ ] **Step 7: Run the full suite + clippy.**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean. (The coherence test stays `--ignored`; everything else must be green.)

- [ ] **Step 8: Commit.**

```bash
git add src/lsp/servers/mod.rs
git commit -m "feat(lsp): enable mux for rust (idle 180s)"
```

---

## Task 8: Experimental documentation

**Files:**
- Create: `docs/manual/src/experimental/mux-rust.md`.
- Modify: `docs/manual/src/experimental/index.md` — add a link to the new page.

- [ ] **Step 1: Create the experimental page.**

`docs/manual/src/experimental/mux-rust.md`:

```markdown
# Rust LSP multiplexer

> ⚠ Experimental — may change without notice.

## What it does

When two `codescout` instances open the same Rust project, they now share a
single `rust-analyzer` process via the existing LSP multiplexer (first used
for `kotlin-lsp`). This eliminates the stale-hover / stale-goto bug that
appeared after a write in instance A was not reflected in instance B.

## Footprint

- One `rust-analyzer` per `(project-root)` across all `codescout` instances
  on the machine.
- Idle-shutdown after 180 seconds with no connected clients.
- Memory saved: one full `rust-analyzer` (2–4 GB on a medium Cargo
  workspace) per extra `codescout` instance.

## Opt out

Add to `.codescout/project.toml`:

```toml
[lsp.rust]
mux = false
```

Then `/mcp` restart. Codescout will fall back to spawning a dedicated
`rust-analyzer` per instance, as before.

## Known limits

- Unix only (the mux is `#[cfg(unix)]`).
- `rust-analyzer` must be on `PATH`.
- If two clients connect before `rust-analyzer` completes initialization,
  the second client waits on a 5-retry / 1-second backoff. No-op
  behaviourally; you may see a brief startup delay under heavy
  concurrency.
```

- [ ] **Step 2: Link from the experimental index.** Open `docs/manual/src/experimental/index.md` and add this entry under the existing list (alphabetically or at the end — follow existing convention):

```markdown
- [Rust LSP multiplexer](./mux-rust.md)
```

- [ ] **Step 3: Commit.**

```bash
git add docs/manual/src/experimental/mux-rust.md docs/manual/src/experimental/index.md
git commit -m "docs(experimental): rust LSP multiplexer"
```

---

## Task 9: Final verification

- [ ] **Step 1: Run fmt + clippy + all tests.**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Expected: clean exit; no warnings promoted to errors.

- [ ] **Step 2: Build release and smoke-test via live MCP.**

```bash
cargo build --release
```

Then, in the Claude Code session, run `/mcp` to restart. Exercise at least:

1. `activate_project` on a real Rust project.
2. `find_symbol("some_fn", include_body=true)` — verify result.
3. Open a second codescout instance on the *same* project path (new terminal).
4. In instance A, `edit_file` a function body; verify in instance B that
   `find_symbol` on that function returns the updated body.

Expected: step 4 returns the post-edit content. If it returns pre-edit,
investigate mux socket path / file-lock behaviour with `tail
target/codescout.log`.

- [ ] **Step 3: Push the branch.**

```bash
git push
```

The `experiments` branch now has a working Rust mux. Follow-up plans cover
Java (with jdtls workspace-lock investigation), then Python / TypeScript /
Go.

---

## Spec coverage (self-review)

| Spec section | Task(s) | Notes |
|---|---|---|
| §5.1 `idle_timeout_secs` on `LspServerConfig` | Task 1, Task 3 | Field added + spawn wiring + 300s fallback test |
| §5.2 `[lsp.<lang>]` opt-out | Task 4, Task 5 | Parse + resolve + call-site integration |
| §5.3 `env` overrides (flagged for use) | Task 2 | Plumbed through `Mux` CLI; Kotlin's `GRADLE_USER_HOME` now actually reaches the server |
| §6.1 Rust | Task 7 | `mux: true`, `idle_timeout_secs: Some(180)`, `env: []` |
| §7.1 two-`Agent` coherence harness | Task 6 | `src/lsp/mux/test_support.rs` (lives in `src/` not `tests/` — see plan header) |
| §7.2 shared harness module | Task 6 | `#[cfg(test)] pub(crate) mod test_support` |
| §8.1 per-language PR shape | Tasks 7, 8, 9 | Flip → test → doc → smoke |
| §9 per-language acceptance | Task 9 | `cargo test lsp::`, coherence test, release smoke, docs, clippy clean |

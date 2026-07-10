# codescout Startup Dotenv Self-Load Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** codescout loads a startup dotenv file (`$CODESCOUT_ENV_FILE` or `~/.config/codescout/.env`) into its environment before config resolution, so the MCP launcher needs no env injection and the `~/.local/bin/codescout-mcp` wrapper can be retired.

**Architecture:** All embedding/retrieval config is read via `std::env::var()` at use time. A `load_startup_env()` called early in `main()` populates the process env from a dotenv file using `dotenvy::from_path` (non-overriding), so every existing consumer picks it up with zero plumbing. The `.env.<profile>` files gain the librarian embedder vars so one loaded file configures both stacks.

**Tech Stack:** Rust, `dotenvy`, `tracing`, `tempfile` + `serial_test` (dev).

## Global Constraints

- `dotenvy::from_path` **must not override** already-set env vars — precedence is explicit env > dotenv file > built-in defaults. Pinned by a test.
- Loader is **opt-in**: missing default path → silent no-op; missing *explicit* `$CODESCOUT_ENV_FILE` → `tracing::warn!`. Never reads cwd (security: no arbitrary repo `.env`).
- Loader runs in `main()` **after logging init + crypto install, before `Cli::parse()`**, and applies to all subcommands.
- Env-mutating tests use `#[serial]` + `lock_env_for_tests()` + save/restore (existing pattern in `src/config/global.rs`).
- Pre-commit gate: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
- Branch: `experiments` (master is protected).
- Live-MCP build is `cargo rb`; reconnect with `/mcp`.

---

## File Structure

- **Modify:** `Cargo.toml` — add `dotenvy` to `[dependencies]`.
- **Modify:** `src/config/global.rs` — extract `global_config_dir()`, add `global_env_path()` + `load_startup_env()` + 4 tests.
- **Modify:** `src/config/mod.rs` — `pub use global::load_startup_env;`.
- **Modify:** `src/main.rs` — call `codescout::config::load_startup_env()` before `Cli::parse()`.
- **Modify:** `.env.amd` / `.env.cpu` / `.env.gpu` / `.env.lite` / `.env.example` — add `LIBRARIAN_EMBED_*`.
- **Modify:** `docs/manual/src/concepts/librarian-embedded.md` — self-load setup replaces the `env`-block example.

---

## Task 1: Startup dotenv loader

**Files:**
- Modify: `Cargo.toml` (`[dependencies]`, after `fs4 = "0.12"` at line 45)
- Modify: `src/config/global.rs` (`global_config_path` at 43-48; tests module at 112)
- Modify: `src/config/mod.rs` (add re-export after line 6)
- Modify: `src/main.rs` (in `main()`, after `codescout::install_default_crypto_provider();`)
- Test: `src/config/global.rs` tests module

**Interfaces:**
- Produces: `codescout::config::global::global_env_path() -> Option<PathBuf>` and `codescout::config::load_startup_env()` (re-exported).

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]` (add after the `fs4 = "0.12"` line):

```toml
# Startup dotenv self-load (non-overriding)
dotenvy = "0.15"
```

- [ ] **Step 2: Write the failing tests**

Add to the `tests` module in `src/config/global.rs` (module already imports `super::*`, `serial_test::serial`, and defines `lock_env_for_tests()`):

```rust
    #[test]
    #[serial]
    fn global_env_path_derives_from_config_dir() {
        let _guard = lock_env_for_tests();
        let saved = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test-codescout");
        let path = global_env_path().unwrap();
        match saved {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/xdg-test-codescout/codescout/.env")
        );
    }

    #[test]
    #[serial]
    fn load_startup_env_reads_explicit_file() {
        let _guard = lock_env_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("custom.env");
        std::fs::write(&file, "CODESCOUT_TEST_LOADER_VAR=from_file\n").unwrap();

        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::remove_var("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_ENV_FILE", &file);

        load_startup_env();
        let got = std::env::var("CODESCOUT_TEST_LOADER_VAR").ok();

        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert_eq!(got.as_deref(), Some("from_file"));
    }

    #[test]
    #[serial]
    fn load_startup_env_does_not_override_existing() {
        let _guard = lock_env_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("custom.env");
        std::fs::write(&file, "CODESCOUT_TEST_LOADER_VAR=from_file\n").unwrap();

        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_TEST_LOADER_VAR", "from_env");
        std::env::set_var("CODESCOUT_ENV_FILE", &file);

        load_startup_env();
        let got = std::env::var("CODESCOUT_TEST_LOADER_VAR").ok();

        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert_eq!(got.as_deref(), Some("from_env"), "real env must win over the file");
    }

    #[test]
    #[serial]
    fn load_startup_env_noop_when_explicit_file_missing() {
        let _guard = lock_env_for_tests();
        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::remove_var("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_ENV_FILE", "/nonexistent/codescout-test-xyz/.env");

        load_startup_env(); // must not panic; must not set anything

        let got = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert!(got.is_none());
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p codescout --lib config::global::tests::load_startup_env`
Expected: FAIL to compile — `global_env_path` / `load_startup_env` not defined.

- [ ] **Step 4: Extract `global_config_dir()` and add the two functions**

In `src/config/global.rs`, replace the existing `global_config_path`:

```rust
pub fn global_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("codescout").join("config.toml"))
}
```

with:

```rust
/// `$XDG_CONFIG_HOME/codescout` or `$HOME/.config/codescout`. `None` when neither
/// `XDG_CONFIG_HOME` nor `HOME` is set (e.g. some Windows/CI environments).
fn global_config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("codescout"))
}

pub fn global_config_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join("config.toml"))
}

/// Default startup-dotenv path: `<global_config_dir>/.env`.
pub fn global_env_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join(".env"))
}

/// Load a startup dotenv into the process environment before config resolution.
///
/// Path: `$CODESCOUT_ENV_FILE` if set, else [`global_env_path`]. Uses
/// `dotenvy::from_path`, which does NOT override already-set vars — so an
/// explicit process env var always wins over the file. A missing default path is
/// a silent no-op (opt-in); a missing *explicit* `$CODESCOUT_ENV_FILE` is a
/// surfaced warning. Never reads the current working directory — a user-scoped
/// server must not absorb an arbitrary repo's `.env`.
pub fn load_startup_env() {
    let explicit = std::env::var_os("CODESCOUT_ENV_FILE").map(PathBuf::from);
    let path = match explicit.clone().or_else(global_env_path) {
        Some(p) => p,
        None => return,
    };
    if !path.exists() {
        if explicit.is_some() {
            tracing::warn!(
                "CODESCOUT_ENV_FILE set to {} but the file was not found",
                path.display()
            );
        }
        return;
    }
    match dotenvy::from_path(&path) {
        Ok(()) => tracing::debug!("loaded startup env from {}", path.display()),
        Err(e) => tracing::warn!("failed to load startup env from {}: {e}", path.display()),
    }
}
```

- [ ] **Step 5: Re-export from the config module**

In `src/config/mod.rs`, after `pub use project::ProjectConfig;` add:

```rust
pub use global::load_startup_env;
```

- [ ] **Step 6: Wire the loader into `main()`**

In `src/main.rs`, insert the call between the crypto install and `Cli::parse()`:

```rust
    codescout::install_default_crypto_provider();

    // Load a startup dotenv (opt-in) so the MCP launcher needs no env injection.
    codescout::config::load_startup_env();

    let cli = Cli::parse();
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p codescout --lib config::global::tests::load_startup_env config::global::tests::global_env_path`
Expected: PASS (4 tests).

- [ ] **Step 8: Full gate + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add Cargo.toml Cargo.lock src/config/global.rs src/config/mod.rs src/main.rs
git commit -m "feat(config): self-load a startup dotenv so the MCP launcher needs no env injection

load_startup_env() loads \$CODESCOUT_ENV_FILE or ~/.config/codescout/.env via
dotenvy::from_path (non-overriding) early in main(). Opt-in: missing default is a
no-op, missing explicit file warns, cwd is never read.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Config-file unification + docs

**Files:**
- Modify: `.env.amd`, `.env.cpu`, `.env.gpu` (identical block), `.env.lite`, `.env.example`
- Modify: `docs/manual/src/concepts/librarian-embedded.md`

**Interfaces:** none (data + docs).

- [ ] **Step 1: Add librarian embedder vars to the local-stack profiles**

Append to **`.env.amd`, `.env.cpu`, and `.env.gpu`** (all three point their embedder at `:48081`):

```bash
# Librarian artifact embedder — OpenAI-compatible /v1 endpoint on the same dense
# model as the retrieval stack. Read separately from CODESCOUT_EMBEDDER_* by the
# librarian, so it must be set here too (that split caused the 2026-07-10 outage).
LIBRARIAN_EMBED_MODEL=CodeRankEmbed
LIBRARIAN_EMBED_URL=http://127.0.0.1:48081/v1
```

- [ ] **Step 2: Add librarian embedder vars to the lite (remote) profile**

Append to **`.env.lite`** (its embedder URL is already a remote `/v1` placeholder):

```bash
# Librarian artifact embedder — mirror CODESCOUT_EMBEDDER_URL (already /v1 here).
LIBRARIAN_EMBED_MODEL=your-model-name
LIBRARIAN_EMBED_URL=https://embed.corp.example/v1
```

- [ ] **Step 3: Document the vars in `.env.example`**

Append to **`.env.example`**:

```bash
# --- Librarian artifact semantic search (embedded librarian) ---
# The librarian embeds tracker/spec/bug markdown for artifact(find, semantic=…)
# and librarian(context). It reads its OWN env surface (not CODESCOUT_EMBEDDER_*),
# so set it here. Point at any OpenAI-compatible /v1 embeddings endpoint.
# LIBRARIAN_EMBED_MODEL=CodeRankEmbed
# LIBRARIAN_EMBED_URL=http://127.0.0.1:48081/v1
# LIBRARIAN_EMBED_API_KEY=            # only if the endpoint requires it
```

- [ ] **Step 4: Update the migration doc**

In `docs/manual/src/concepts/librarian-embedded.md` § *Migration from standalone librarian-mcp*, replace the JSON `env`-block example with the self-load setup:

```markdown
codescout self-loads a startup dotenv, so the registration needs no `env` block.
Point it at a retrieval-profile file once:

    ln -s /abs/path/to/codescout/.env.amd ~/.config/codescout/.env
    # or export CODESCOUT_ENV_FILE=/abs/path/to/.env.amd

Then register the bare binary:

    claude mcp add codescout -s user -- /abs/path/to/codescout start --debug

The `.env.<profile>` files include both `CODESCOUT_EMBEDDER_*` (retrieval stack)
and `LIBRARIAN_EMBED_*` (artifact embedder), so one file configures both. Real
environment variables still override the file.
```

- [ ] **Step 5: Verify + commit**

Run: `git diff --stat` and confirm only the six files above changed. (No build needed — data/docs.)

```bash
git add .env.amd .env.cpu .env.gpu .env.lite .env.example docs/manual/src/concepts/librarian-embedded.md
git commit -m "feat(env): add LIBRARIAN_EMBED_* to retrieval-profile files; document self-load setup

One loaded .env.<profile> now configures both the retrieval stack and the
librarian embedder, closing the two-env-surface gap.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Cutover + live verification

**Files:** none (config + verification). Requires a `/mcp` reconnect (user action).

**Interfaces:** consumes the shipped loader + config files.

- [ ] **Step 1: Build the live binary**

Run: `cargo rb`
Expected: release build succeeds.

- [ ] **Step 2: Point the global dotenv at the AMD profile**

Run: `ln -sf /home/marius/work/claude/codescout/.env.amd ~/.config/codescout/.env`
Expected: symlink created (`ls -l ~/.config/codescout/.env`).

- [ ] **Step 3: Re-register codescout as the bare binary (this profile)**

Run:
```bash
claude mcp remove codescout -s user
claude mcp add codescout -s user -- /home/marius/.cargo/bin/codescout start --debug
```
Expected: `claude mcp get codescout` shows Command `/home/marius/.cargo/bin/codescout`, Args `start --debug`.

- [ ] **Step 4: Reconnect and verify (user runs `/mcp`)**

After `/mcp` reconnect, verify in-process env + semantic:
- `run_command("printenv LIBRARIAN_EMBED_MODEL CODESCOUT_EMBEDDER_URL")` → both set.
- `artifact(find, semantic="LSP shutdown race")` → returns the target bug (not the "requires an embedding service" error).

- [ ] **Step 5: Remove the wrapper**

Run: `rm /home/marius/.local/bin/codescout-mcp`
Expected: gone; server still works (it's the bare binary now).

- [ ] **Step 6: Mirror to the other profiles**

For each of `~/.claude-sdd` and `~/.claude-kat` (only if they register codescout):
```bash
CLAUDE_CONFIG_DIR=~/.claude-sdd claude mcp remove codescout -s user
CLAUDE_CONFIG_DIR=~/.claude-sdd claude mcp add codescout -s user -- /home/marius/.cargo/bin/codescout start --debug
```

- [ ] **Step 7: No commit** — verification/config only. If Step 4 fails, return to the owning task.

---

## Self-Review

**Spec coverage:**
- Loader (path resolution, non-override, missing-file behavior, all subcommands, no-cwd, tracing) → Task 1 Steps 4/6. ✓
- `dotenvy` dependency → Task 1 Step 1. ✓
- Config-file unification (`.env.<profile>` + `.env.example`) → Task 2 Steps 1-3. ✓
- Docs (`librarian-embedded.md`) → Task 2 Step 4. ✓
- Tests (explicit-file load, non-override, missing-explicit no-op, path derivation) → Task 1 Step 2. ✓
- Cutover (symlink, bare re-register, wrapper rm, reconnect verify, 3 profiles) → Task 3. ✓
- `development-commands` memory note (spec § 4) → NOT a plan task; it's a codescout `memory()` write, done post-merge outside the code diff. Noted here so it isn't dropped.

**Placeholder scan:** no TBD/TODO; all code and `.env` blocks are literal. Per-profile URLs are exact (amd/cpu/gpu → `:48081/v1`; lite → the remote placeholder mirroring its own file).

**Type consistency:** `load_startup_env` / `global_env_path` / `global_config_dir` names identical across global.rs, the re-export, main.rs, and the tests. `dotenvy::from_path` return `Result<(), _>` matched in the `match`.

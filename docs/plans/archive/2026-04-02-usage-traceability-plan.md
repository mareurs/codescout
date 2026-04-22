# Usage Traceability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reproducible post-mortem debugging to `usage.db` — store codescout SHA, project SHA, session ID on every call; store full input/output JSON in debug mode.

**Architecture:** New nullable columns on the existing `tool_calls` table. A `build.rs` bakes codescout's git SHA at compile time. Project SHA is cached on `ActiveProject` at activation. A `--debug` flag (consolidating the existing `--diagnostic`) gates verbose recording. `UsageRecorder` gains access to debug mode, session ID, and project SHA via the `Agent`.

**Tech Stack:** Rust, SQLite (rusqlite), clap (CLI flags), `build.rs` for compile-time SHA.

**Spec:** `docs/plans/2026-04-02-usage-traceability-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `build.rs` | Create | Compile-time `CODESCOUT_GIT_SHA` env var |
| `src/main.rs` | Modify (L11-42, L101-135) | Consolidate `--diagnostic` into `--debug`, update `run()` call |
| `src/logging.rs` | Modify (L74-81) | Accept single `debug` bool instead of two separate flags |
| `src/server.rs` | Modify (L39-52, L401-444) | Add `session_id` to server, consolidate diagnostic flag, thread debug to recorder |
| `src/agent.rs` | Modify (L88-101, L116-198, L201-276) | Add `head_sha` to `ActiveProject`, populate on `new()` and `activate()` |
| `src/usage/db.rs` | Modify (L5-55) | Schema migration, expanded `write_record` |
| `src/usage/mod.rs` | Modify (L9-49) | `UsageRecorder` gains debug/session/sha fields, `record_content` passes input/output |

---

### Task 1: `build.rs` — Compile-Time SHA

**Files:**
- Create: `build.rs`

- [ ] **Step 1: Write the failing test**

There's no test to write for `build.rs` itself — the validation comes in Task 6 when we assert the env var is available. Instead, verify that no `build.rs` exists yet.

Run: `ls build.rs`
Expected: "No such file or directory"

- [ ] **Step 2: Create `build.rs`**

```rust
fn main() {
    // Bake the codescout git SHA into the binary at compile time.
    // Falls back to "unknown" for non-git builds (e.g. crates.io install).
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha}");

    // Only re-run when HEAD changes (not on every source edit).
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -5`
Expected: Compiles without errors. The `CODESCOUT_GIT_SHA` env var is now available via `env!()`.

- [ ] **Step 4: Commit**

```bash
git add build.rs
git commit -m "build: add build.rs to bake git SHA at compile time"
```

---

### Task 2: Schema Migration in `usage/db.rs`

**Files:**
- Modify: `src/usage/db.rs:5-55` (`open_db` and `write_record`)
- Test: `src/usage/db.rs` (tests module, L385+)

- [ ] **Step 1: Write the failing test for migration**

Add to the `tests` module in `src/usage/db.rs`:

```rust
#[test]
fn open_db_migrates_traceability_columns() {
    let dir = tempdir().unwrap();
    // First open creates the base schema (no new columns).
    let conn = open_db(dir.path()).unwrap();
    // Verify columns exist by inserting a row that uses them.
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, codescout_sha, project_sha, session_id, input_json, output_json)
         VALUES ('test', datetime('now'), 10, 'success', 'abc1234', 'def5678', 'sess-1', '{\"q\":\"x\"}', NULL)",
        [],
    )
    .unwrap();
    let (cs, ps, sid, inp, out): (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(cs.as_deref(), Some("abc1234"));
    assert_eq!(ps.as_deref(), Some("def5678"));
    assert_eq!(sid.as_deref(), Some("sess-1"));
    assert_eq!(inp.as_deref(), Some("{\"q\":\"x\"}"));
    assert!(out.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test open_db_migrates_traceability_columns -- --nocapture`
Expected: FAIL — columns don't exist yet.

- [ ] **Step 3: Add migration to `open_db`**

In `src/usage/db.rs`, in the `open_db` function, after the existing `CREATE TABLE IF NOT EXISTS` statements (before the final `Ok(conn)`), add:

```rust
    // Migration: add traceability columns (idempotent — IF NOT EXISTS not needed,
    // ALTER TABLE ADD COLUMN is a no-op if the column already exists in SQLite... 
    // but SQLite actually errors on duplicate columns, so we check first).
    let has_session_id: bool = conn
        .prepare("SELECT session_id FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_session_id {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN codescout_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN project_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN session_id TEXT;
             ALTER TABLE tool_calls ADD COLUMN input_json TEXT;
             ALTER TABLE tool_calls ADD COLUMN output_json TEXT;",
        )?;
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test open_db_migrates_traceability_columns -- --nocapture`
Expected: PASS

- [ ] **Step 5: Write the failing test for expanded `write_record`**

```rust
#[test]
fn write_record_stores_traceability_fields() {
    let (_dir, conn) = tmp();
    write_record(
        &conn,
        "find_symbol",
        42,
        "error",
        false,
        Some("not found"),
        "abc1234",
        Some("def5678"),
        "sess-1",
        Some("{\"query\":\"foo\"}"),
        Some("{\"error\":\"not found\"}"),
    )
    .unwrap();
    let (cs, ps, sid, inp, out): (String, String, String, String, String) = conn
        .query_row(
            "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(cs, "abc1234");
    assert_eq!(ps, "def5678");
    assert_eq!(sid, "sess-1");
    assert_eq!(inp, "{\"query\":\"foo\"}");
    assert_eq!(out, "{\"error\":\"not found\"}");
}

#[test]
fn write_record_traceability_fields_nullable() {
    let (_dir, conn) = tmp();
    write_record(
        &conn,
        "find_symbol",
        42,
        "success",
        false,
        None,
        "abc1234",
        None,
        "sess-1",
        None,
        None,
    )
    .unwrap();
    let (ps, inp, out): (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT project_sha, input_json, output_json FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(ps.is_none());
    assert!(inp.is_none());
    assert!(out.is_none());
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test write_record_stores_traceability -- --nocapture`
Expected: FAIL — `write_record` doesn't accept the new params yet.

- [ ] **Step 7: Update `write_record` signature and implementation**

Replace the `write_record` function in `src/usage/db.rs`:

```rust
pub fn write_record(
    conn: &Connection,
    tool_name: &str,
    latency_ms: i64,
    outcome: &str,
    overflowed: bool,
    error_msg: Option<&str>,
    codescout_sha: &str,
    project_sha: Option<&str>,
    session_id: &str,
    input_json: Option<&str>,
    output_json: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, error_msg, codescout_sha, project_sha, session_id, input_json, output_json)
         VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            tool_name,
            latency_ms,
            outcome,
            overflowed as i64,
            error_msg,
            codescout_sha,
            project_sha,
            session_id,
            input_json,
            output_json,
        ],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')",
        [],
    )?;
    Ok(())
}
```

- [ ] **Step 8: Fix all existing `write_record` call sites**

The existing callers in tests pass fewer arguments. Update all existing test calls to use the new signature by adding the trailing params. Search for all calls:

Run: `cargo test 2>&1 | grep "error\[E" | head -20`

Fix each call site by appending: `"unknown", None, "test-session", None, None` — the existing tests don't care about traceability fields.

- [ ] **Step 9: Run all usage tests**

Run: `cargo test --lib usage -- --nocapture`
Expected: All PASS

- [ ] **Step 10: Commit**

```bash
git add src/usage/db.rs
git commit -m "feat(usage): add traceability columns to tool_calls schema"
```

---

### Task 3: `ActiveProject` Head SHA

**Files:**
- Modify: `src/agent.rs:88-101` (`ActiveProject` struct)
- Modify: `src/agent.rs:116-198` (`Agent::new`)
- Modify: `src/agent.rs:201-276` (`Agent::activate`)
- Test: `src/agent.rs` (tests module)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/agent.rs`:

```rust
#[tokio::test]
async fn activate_populates_head_sha() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Init a git repo so there's a HEAD to read.
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let sha = agent
        .with_project(|p| Ok(p.head_sha.clone()))
        .await
        .unwrap();
    assert!(sha.is_some(), "head_sha should be set for a git project");
    assert!(
        sha.as_ref().unwrap().len() >= 7,
        "SHA should be at least 7 chars"
    );
}

#[tokio::test]
async fn head_sha_none_for_non_git_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // No git init — not a git repo.
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let sha = agent
        .with_project(|p| Ok(p.head_sha.clone()))
        .await
        .unwrap();
    assert!(sha.is_none(), "head_sha should be None for non-git project");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test activate_populates_head_sha -- --nocapture`
Expected: FAIL — `head_sha` field doesn't exist on `ActiveProject`.

- [ ] **Step 3: Add `head_sha` field to `ActiveProject`**

In `src/agent.rs`, add to the `ActiveProject` struct:

```rust
    /// Git HEAD SHA of the project at activation time. None for non-git projects.
    pub head_sha: Option<String>,
```

- [ ] **Step 4: Add helper function to resolve HEAD SHA**

Add a free function in `src/agent.rs` (near the other helpers like `load_discover_settings`):

```rust
/// Resolve the short git HEAD SHA for a directory. Returns None if not a git repo.
fn resolve_head_sha(root: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
```

- [ ] **Step 5: Populate `head_sha` in `Agent::new()` and `Agent::activate()`**

In `Agent::new()` (around L135), where `ActiveProject` is constructed, add:

```rust
    head_sha: resolve_head_sha(&root),
```

In `Agent::activate()` (around L219), where `ActiveProject` is constructed, add the same:

```rust
    head_sha: resolve_head_sha(&root),
```

- [ ] **Step 6: Run tests**

Run: `cargo test activate_populates_head_sha head_sha_none_for_non_git -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run full agent tests to check nothing broke**

Run: `cargo test --lib agent -- --nocapture`
Expected: All PASS

- [ ] **Step 8: Commit**

```bash
git add src/agent.rs
git commit -m "feat(agent): cache git HEAD SHA on ActiveProject at activation"
```

---

### Task 4: Consolidate `--diagnostic` into `--debug`

**Files:**
- Modify: `src/main.rs:11-42` (CLI flags), `src/main.rs:101-135` (main match arm)
- Modify: `src/logging.rs:74-81` (`init` signature)
- Modify: `src/server.rs:401-444` (`run` signature)

- [ ] **Step 1: Update CLI flags in `src/main.rs`**

In the `Commands::Start` enum variant, replace the two separate flags:

```rust
        /// Enable debug mode: verbose logging + detailed usage recording.
        /// Subsumes the former --diagnostic flag.
        #[arg(long)]
        debug: bool,

        /// Deprecated alias for --debug.
        #[arg(long, hide = true)]
        diagnostic: bool,
```

- [ ] **Step 2: Merge flags in `main()`**

In the `main()` function, update the raw arg peek (around L108-109):

```rust
    let debug_mode = std::env::args().any(|a| a == "--debug" || a == "--diagnostic");
    let log_state = codescout::logging::init(debug_mode);
```

And in the match arm for `Commands::Start` (around L126-135), merge the two bools:

```rust
        Commands::Start {
            project,
            transport,
            host,
            port,
            auth_token,
            debug,
            diagnostic,
        } => {
            let debug = debug || diagnostic;
            tracing::info!("Starting codescout MCP server (transport={})", transport);
            codescout::server::run(
                project,
                &transport,
                &host,
                port,
                auth_token,
                debug,
                log_state.instance_id,
            )
            .await?;
        }
```

- [ ] **Step 3: Update `logging::init` to accept single bool**

In `src/logging.rs`, change the signature of `init`:

```rust
pub fn init(debug: bool) -> LoggingGuards {
```

Inside the function, replace all references to `diagnostic` with `debug`. Both the debug file layer AND the diagnostic file layer are now gated on the single `debug` bool. The diagnostic log file is still written (with its instance ID and rotation) — it just shares the same flag.

- [ ] **Step 4: Update `server::run` to accept single `debug` bool**

In `src/server.rs`, change the `run` function signature — remove the `diagnostic` param:

```rust
pub async fn run(
    project: Option<PathBuf>,
    transport: &str,
    host: &str,
    port: u16,
    auth_token: Option<String>,
    debug: bool,
    instance_id: Option<String>,
) -> Result<()> {
```

Replace `if diagnostic {` with `if debug {` and `if debug || diagnostic {` with `if debug {`.

- [ ] **Step 5: Verify it compiles and tests pass**

Run: `cargo build && cargo test --lib -- --nocapture 2>&1 | tail -5`
Expected: Compiles, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/logging.rs src/server.rs
git commit -m "refactor: consolidate --diagnostic into --debug flag"
```

---

### Task 5: Thread Debug Context Through `UsageRecorder`

**Files:**
- Modify: `src/server.rs:39-52` (add `session_id` field)
- Modify: `src/server.rs:63-115` (`from_parts` — generate session ID)
- Modify: `src/server.rs:165-195` (call site — pass debug context to recorder)
- Modify: `src/usage/mod.rs:9-74` (`UsageRecorder` struct and methods)

- [ ] **Step 1: Add `session_id` and `debug` to `CodeScoutServer`**

In `src/server.rs`, add fields to the `CodeScoutServer` struct:

```rust
    session_id: String,
    debug: bool,
```

- [ ] **Step 2: Generate session ID in `from_parts` and store debug flag**

The `from_parts` method currently doesn't receive a `debug` flag. Update its signature:

```rust
pub async fn from_parts(agent: Agent, lsp: Arc<dyn LspProvider>, debug: bool) -> Self {
```

In the `Self { ... }` block at the end, add:

```rust
    session_id: uuid::Uuid::new_v4().to_string(),
    debug,
```

Add `uuid` dependency if not present — check `Cargo.toml` first.

- [ ] **Step 3: Update all `from_parts` call sites**

In `src/server.rs`, the `run()` function calls `from_parts`. Pass the `debug` flag:

```rust
    let server = CodeScoutServer::from_parts(agent, lsp.clone(), debug).await;
```

Search for other `from_parts` calls (HTTP multi-session path) and update those too.

- [ ] **Step 4: Update `UsageRecorder` to carry debug context**

In `src/usage/mod.rs`, expand the struct:

```rust
pub struct UsageRecorder {
    agent: Agent,
    debug: bool,
    session_id: String,
}
```

Update `UsageRecorder::new`:

```rust
pub fn new(agent: Agent, debug: bool, session_id: String) -> Self {
    Self { agent, debug, session_id }
}
```

- [ ] **Step 5: Update `record_content` to accept and pass input**

Change `record_content` to accept the tool input:

```rust
pub async fn record_content<F, Fut>(
    &self,
    tool_name: &str,
    input: &Value,
    f: F,
) -> Result<Vec<Content>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Content>>>,
{
    let start = Instant::now();
    let result = f().await;
    let latency_ms = start.elapsed().as_millis() as i64;
    let _ = self.write_content(tool_name, latency_ms, input, &result).await;
    result
}
```

- [ ] **Step 6: Update `write_content` to populate all fields**

```rust
async fn write_content(
    &self,
    tool_name: &str,
    latency_ms: i64,
    input: &Value,
    result: &Result<Vec<Content>>,
) -> Result<()> {
    let project_root = self.agent.with_project(|p| Ok(p.root.clone())).await?;
    let head_sha = self
        .agent
        .with_project(|p| Ok(p.head_sha.clone()))
        .await
        .unwrap_or(None);
    let conn = db::open_db(&project_root)?;
    let (outcome, overflowed, error_msg) = classify_content_result(result);

    let input_json = if self.debug {
        serde_json::to_string(input).ok()
    } else {
        None
    };

    let output_json = if self.debug && outcome != "success" {
        // Serialize the Content blocks for error responses.
        result
            .as_ref()
            .ok()
            .and_then(|blocks| serde_json::to_string(blocks).ok())
            .or_else(|| {
                result
                    .as_ref()
                    .err()
                    .map(|e| format!("{{\"error\":\"{}\"}}", e))
            })
    } else {
        None
    };

    db::write_record(
        &conn,
        tool_name,
        latency_ms,
        outcome,
        overflowed,
        error_msg.as_deref(),
        env!("CODESCOUT_GIT_SHA"),
        head_sha.as_deref(),
        &self.session_id,
        input_json.as_deref(),
        output_json.as_deref(),
    )?;
    Ok(())
}
```

- [ ] **Step 7: Update the call site in `server.rs`**

In `src/server.rs`, where `UsageRecorder` is created and `record_content` is called (around L173-178):

```rust
let recorder = UsageRecorder::new(
    self.agent.clone(),
    self.debug,
    self.session_id.clone(),
);

// ... in the record_content call, pass input:
recorder.record_content(&req.name, &input, || tool.call_content(input.clone(), &ctx))
```

Note: `input` is consumed by `call_content`, so we need to pass a reference first, then clone for the call. Check the existing code — `input` is a `Value` which is `Clone`.

- [ ] **Step 8: Verify it compiles**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 9: Commit**

```bash
git add src/server.rs src/usage/mod.rs
git commit -m "feat(usage): thread debug context through UsageRecorder"
```

---

### Task 6: Integration Test

**Files:**
- Modify: `src/usage/mod.rs` (tests module, L77+)

- [ ] **Step 1: Write test for debug mode recording**

Add to the `content_tests` module in `src/usage/mod.rs`:

```rust
#[tokio::test]
async fn record_content_stores_input_in_debug_mode() {
    use serde_json::json;

    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let recorder = UsageRecorder::new(agent.clone(), true, "test-session".to_string());
    let input = json!({"query": "test_symbol", "path": "src/lib.rs"});

    let _ = recorder
        .record_content("find_symbol", &input, || async {
            Ok(vec![Content::text("found it")])
        })
        .await;

    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let (inp, out, sid, cs): (Option<String>, Option<String>, String, String) = conn
        .query_row(
            "SELECT input_json, output_json, session_id, codescout_sha FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();

    assert!(inp.is_some(), "input_json should be populated in debug mode");
    assert!(inp.unwrap().contains("test_symbol"));
    assert!(out.is_none(), "output_json should be None for success");
    assert_eq!(sid, "test-session");
    assert!(!cs.is_empty(), "codescout_sha should be set");
}

#[tokio::test]
async fn record_content_stores_output_for_errors_in_debug_mode() {
    use serde_json::json;

    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let recorder = UsageRecorder::new(agent.clone(), true, "test-session".to_string());
    let input = json!({"path": "/bad/path"});

    let _ = recorder
        .record_content("read_file", &input, || async {
            Err(anyhow::anyhow!("file not found"))
        })
        .await;

    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let (inp, out): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT input_json, output_json FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert!(inp.is_some(), "input_json should be populated");
    assert!(out.is_some(), "output_json should be populated for errors");
    assert!(out.unwrap().contains("file not found"));
}

#[tokio::test]
async fn record_content_no_input_in_normal_mode() {
    use serde_json::json;

    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let recorder = UsageRecorder::new(agent.clone(), false, "test-session".to_string());
    let input = json!({"query": "test_symbol"});

    let _ = recorder
        .record_content("find_symbol", &input, || async {
            Ok(vec![Content::text("found it")])
        })
        .await;

    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let (inp, sid, cs): (Option<String>, String, String) = conn
        .query_row(
            "SELECT input_json, session_id, codescout_sha FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert!(inp.is_none(), "input_json should be None in normal mode");
    assert_eq!(sid, "test-session", "session_id should always be set");
    assert!(!cs.is_empty(), "codescout_sha should always be set");
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --lib usage -- --nocapture`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add src/usage/mod.rs
git commit -m "test(usage): integration tests for debug mode traceability"
```

---

### Task 7: Full Validation

**Files:** None — verification only.

- [ ] **Step 1: Run cargo fmt**

Run: `cargo fmt`

- [ ] **Step 2: Run cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Check uuid dependency**

If Task 5 needed `uuid`, verify it's in `Cargo.toml`. If already present as a transitive dependency, add it as a direct dependency with the `v4` feature:

```bash
cargo add uuid --features v4
```

- [ ] **Step 5: Build release binary**

Run: `cargo build --release`
Expected: Compiles. The binary now has `CODESCOUT_GIT_SHA` baked in.

- [ ] **Step 6: Verify SHA is baked in**

Run: `strings target/release/codescout | grep -E '^[0-9a-f]{7,}$' | head -3`

This is a rough check — the definitive validation is the integration tests from Task 6 which assert `codescout_sha` is non-empty.

- [ ] **Step 7: Commit any fixups**

If fmt/clippy required changes:

```bash
git add -A
git commit -m "chore: fmt + clippy fixes for usage traceability"
```

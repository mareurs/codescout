# Honest usage.db Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `usage.db` tell the truth about friction — fix the dead `overflowed` column and add four structured fields (`friction_target`, `overflow_tokens`, `err_family`, `project_root`) so the upcoming `legibility_scan` probe queries columns instead of regex-matching `output_json`.

**Architecture:** Additive, idempotent SQLite migration on `tool_calls`; a one-field addition to the overflow envelope so the token count is recoverable at record time; three small pure helper functions for extraction/normalization; the recorder (`write_content`) wires them through an extended `write_record`. No retention change, no change to the debug-gating of full payloads.

**Tech Stack:** Rust, `rusqlite`, `serde_json`, `anyhow`. This is Phase 1 of the design spec `docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md`; Phase 2 (the `legibility_scan` librarian action) is planned separately against the columns this plan ships.

---

## File structure

| File | Responsibility | Change |
|---|---|---|
| `src/usage/db.rs` | schema + `write_record` INSERT | migration (4 cols) + 4 new params |
| `src/tools/core/types.rs` | overflow envelope construction | add `buffered_bytes` field (~line 586) |
| `src/usage/mod.rs` | `classify_content_result`, helpers, `write_content` wiring | fix overflow key, add 3 helpers, populate fields |

All three already exist; no new files. Helpers live beside `classify_content_result` in `src/usage/mod.rs` (same private module, same test module).

---

### Task 1: Schema migration — add the four friction columns

**Files:**
- Modify: `src/usage/db.rs` (inside `open_db`, after the v0.10 `cc_session_id` migration block, ~line 70)
- Test: `src/usage/db.rs` (tests module)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/usage/db.rs`:

```rust
#[test]
fn open_db_migrates_friction_columns() {
    let dir = TempDir::new().unwrap();
    let conn = open_db(dir.path()).unwrap();
    conn.execute(
        "INSERT INTO tool_calls (tool_name, latency_ms, outcome, friction_target, overflow_tokens, err_family, project_root)
         VALUES ('symbols', 10, 'success', 'LspManager/get_or_start', 1045, NULL, '/repo')",
        [],
    )
    .unwrap();
    let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
    assert_eq!(tok, Some(1045));
    assert_eq!(ef, None);
    assert_eq!(pr.as_deref(), Some("/repo"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::db::tests::open_db_migrates_friction_columns`
Expected: FAIL — `no such column: friction_target`.

- [ ] **Step 3: Add the migration block**

In `open_db`, immediately after the `has_lsp_outcome` migration block and before `Ok(conn)`, insert:

```rust
    // Migration: legibility friction fields (v0.11). Additive + nullable so every
    // pre-existing row and the unchanged INSERTs stay correct.
    let has_friction_target: bool = conn
        .prepare("SELECT friction_target FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_friction_target {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN friction_target TEXT;
             ALTER TABLE tool_calls ADD COLUMN overflow_tokens INTEGER;
             ALTER TABLE tool_calls ADD COLUMN err_family TEXT;
             ALTER TABLE tool_calls ADD COLUMN project_root TEXT;",
        )?;
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::db::tests::open_db_migrates_friction_columns`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/usage/db.rs
git commit -m "feat(usage): migrate tool_calls with friction columns (friction_target, overflow_tokens, err_family, project_root)"
```

---

### Task 2: Expose `buffered_bytes` on the overflow envelope

**Files:**
- Modify: `src/tools/core/types.rs:~586` (the `buffered` json object)
- Test: `src/tools/core/types.rs` (tests module) — or the nearest existing overflow test module

- [ ] **Step 1: Write the failing test**

Add to the tests module in `src/tools/core/types.rs` (a test that drives a tool whose output overflows; mirror an existing overflow test in that module for the harness shape). If the module has an existing overflow fixture helper, reuse it; otherwise this asserts on the envelope JSON directly:

```rust
#[test]
fn overflow_envelope_carries_buffered_bytes() {
    // GIVEN a buffered json string of known length
    let json = "x".repeat(20_000); // > MAX_INLINE_TOKENS*4 (~10KB)
    let parsed: serde_json::Value = build_overflow_envelope_for_test(&json);
    let bytes = parsed.get("buffered_bytes").and_then(|v| v.as_u64());
    assert_eq!(bytes, Some(20_000));
}
```

> If `types.rs` has no test seam that returns the raw envelope, instead assert through the public `call_content` path on a tool whose result exceeds the budget, parsing the first content block and checking `buffered_bytes` is present and equals the buffered length. Prefer the existing overflow test's harness in this file.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib tools::core::types::tests::overflow_envelope_carries_buffered_bytes`
Expected: FAIL — `buffered_bytes` absent (`None`).

- [ ] **Step 3: Add the field**

In `types.rs`, the `buffered` object is built as:

```rust
            let mut buffered = serde_json::json!({
                "output_id": ref_id,
                "summary": summary,
                "hint": hint,
            });
```

Add `buffered_bytes` (the `json_len` already bound a few lines above):

```rust
            let mut buffered = serde_json::json!({
                "output_id": ref_id,
                "summary": summary,
                "hint": hint,
                "buffered_bytes": json_len,
            });
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib tools::core::types::tests::overflow_envelope_carries_buffered_bytes`
Expected: PASS.

- [ ] **Step 5: Verify no snapshot/contract test broke**

Run: `cargo test --lib tools::core::types`
Expected: PASS (the new key is additive; if a serialization snapshot asserts exact envelope keys, update it to include `buffered_bytes`).

- [ ] **Step 6: Commit**

```bash
git add src/tools/core/types.rs
git commit -m "feat(output): expose buffered_bytes on the overflow envelope"
```

---

### Task 3: Fix the dead `overflowed` detection

**Files:**
- Modify: `src/usage/mod.rs:97` (`classify_content_result`)
- Test: `src/usage/mod.rs` (tests module — update the existing overflow-detection test fixture)

- [ ] **Step 1: Write/repair the failing test**

In `src/usage/mod.rs` tests, replace the overflow-detection test so it uses the REAL envelope marker (`output_id`) and also pins that the OLD key (`overflow`) no longer triggers:

```rust
#[test]
fn classify_detects_overflow_by_output_id_not_legacy_key() {
    // real overflow envelope marker
    let real = Ok(vec![Content::text(
        r#"{"output_id":"@tool_abc","summary":"...","buffered_bytes":12000}"#.to_string(),
    )]);
    let (_outcome, overflowed, _) = classify_content_result(&real);
    assert!(overflowed, "output_id envelope must set overflowed=true");

    // legacy key must NOT trigger (guards the exact wrong-key regression)
    let legacy = Ok(vec![Content::text(r#"{"overflow":true}"#.to_string())]);
    let (_o2, overflowed_legacy, _) = classify_content_result(&legacy);
    assert!(!overflowed_legacy, "legacy 'overflow' key must not be treated as overflow");

    // normal result
    let normal = Ok(vec![Content::text(r#"{"result":"ok"}"#.to_string())]);
    let (_o3, overflowed_normal, _) = classify_content_result(&normal);
    assert!(!overflowed_normal);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::classify_detects_overflow_by_output_id_not_legacy_key`
Expected: FAIL — `output_id` envelope currently yields `overflowed=false`.

- [ ] **Step 3: Fix the key**

In `classify_content_result`, change the overflow check:

```rust
                    // before:
                    // if v.get("overflow").is_some() {
                    // after — the real buffer-handle marker:
                    if v.get("output_id").is_some() {
                        return ("success", true, None);
                    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::classify_detects_overflow_by_output_id_not_legacy_key`
Expected: PASS.

- [ ] **Step 5: Run the module to catch any old fixture still using the legacy key**

Run: `cargo test --lib usage::`
Expected: PASS. If a pre-existing classify test used `{"overflow":...}` to assert true, update it to `{"output_id":...}`.

- [ ] **Step 6: Commit**

```bash
git add src/usage/mod.rs
git commit -m "fix(usage): detect overflow by output_id, not the never-emitted 'overflow' key"
```

---

### Task 4: `extract_overflow_tokens` helper

**Files:**
- Modify: `src/usage/mod.rs` (add free fn beside `classify_content_result`)
- Test: `src/usage/mod.rs` (tests module)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn extract_overflow_tokens_reads_buffered_bytes_over_four() {
    let env = Ok(vec![Content::text(
        r#"{"output_id":"@tool_x","buffered_bytes":10000}"#.to_string(),
    )]);
    assert_eq!(extract_overflow_tokens(&env), Some(2500));

    let no_bytes = Ok(vec![Content::text(r#"{"output_id":"@tool_x"}"#.to_string())]);
    assert_eq!(extract_overflow_tokens(&no_bytes), None);

    let err: Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
    assert_eq!(extract_overflow_tokens(&err), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::extract_overflow_tokens_reads_buffered_bytes_over_four`
Expected: FAIL — `extract_overflow_tokens` not defined.

- [ ] **Step 3: Implement**

Add beside `classify_content_result`:

```rust
/// Token estimate of a buffered (overflowed) result: `buffered_bytes / 4`.
/// `None` when the result is not an overflow envelope or carries no size.
fn extract_overflow_tokens(result: &Result<Vec<Content>>) -> Option<i64> {
    let blocks = result.as_ref().ok()?;
    let text = blocks.first().and_then(|c| c.as_text()).map(|t| t.text.as_str())?;
    let v: Value = serde_json::from_str(text).ok()?;
    let bytes = v.get("buffered_bytes").and_then(Value::as_i64)?;
    Some(bytes / 4)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::extract_overflow_tokens_reads_buffered_bytes_over_four`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/usage/mod.rs
git commit -m "feat(usage): extract_overflow_tokens from buffered_bytes"
```

---

### Task 5: `normalize_err_family` helper

**Files:**
- Modify: `src/usage/mod.rs`
- Test: `src/usage/mod.rs` (tests module)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn normalize_err_family_maps_known_messages() {
    let cases = [
        ("cannot determine end of 'Inner' for insert-after — AST parse failed", Some("ast_extent_fail")),
        ("ambiguous name_path \"LspManager/get_or_start\" matches 2 symbols", Some("ambiguous_name_path")),
        ("edit_code replace('X') would have dropped sibling", Some("replace_dropped_sibling")),
        ("LSP server disconnected", Some("lsp_disconnect")),
        ("kotlin LSP index is locked by another process", Some("lsp_index_locked")),
        ("mux startup failed for kotlin: Failed to spawn mux process", Some("mux_startup_fail")),
        ("LSP server is not running", Some("lsp_not_running")),
        ("symbol not found: ActionContribution/toDTO", Some("symbol_not_found")),
        ("some unrecognized failure", None),
    ];
    for (msg, want) in cases {
        assert_eq!(normalize_err_family(msg), want, "msg: {msg}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::normalize_err_family_maps_known_messages`
Expected: FAIL — `normalize_err_family` not defined.

- [ ] **Step 3: Implement**

```rust
/// Map an error message to a stable, low-cardinality family tag for the probe.
/// Order matters: more specific patterns first. `None` for unrecognized messages.
fn normalize_err_family(msg: &str) -> Option<&'static str> {
    // infra / tool-class (excluded from the probe's code-class score)
    if msg.contains("index is locked") {
        return Some("lsp_index_locked");
    }
    if msg.contains("Failed to spawn mux") || msg.contains("mux startup failed") {
        return Some("mux_startup_fail");
    }
    if msg.contains("LSP server is not running") {
        return Some("lsp_not_running");
    }
    if msg.contains("LSP server disconnected") {
        return Some("lsp_disconnect");
    }
    // code / extractor-shape class
    if msg.contains("AST parse failed") || msg.contains("cannot determine end of") {
        return Some("ast_extent_fail");
    }
    if msg.contains("ambiguous name_path") {
        return Some("ambiguous_name_path");
    }
    if msg.contains("dropped sibling") || msg.contains("dropped the symbol") {
        return Some("replace_dropped_sibling");
    }
    if msg.contains("symbol not found") {
        return Some("symbol_not_found");
    }
    None
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::normalize_err_family_maps_known_messages`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/usage/mod.rs
git commit -m "feat(usage): normalize_err_family — stable error-family tags"
```

---

### Task 6: `extract_friction_target` helper

**Files:**
- Modify: `src/usage/mod.rs`
- Test: `src/usage/mod.rs` (tests module)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn extract_friction_target_coalesces_input_keys() {
    use serde_json::json;
    assert_eq!(
        extract_friction_target(&json!({"name_path": "A/b", "path": "src/x.rs"})),
        Some("A/b".to_string()),
        "name_path wins over path"
    );
    assert_eq!(
        extract_friction_target(&json!({"symbol": "Foo/bar"})),
        Some("Foo/bar".to_string())
    );
    assert_eq!(
        extract_friction_target(&json!({"path": "src/lib.rs"})),
        Some("src/lib.rs".to_string())
    );
    assert_eq!(extract_friction_target(&json!({"unrelated": 1})), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::extract_friction_target_coalesces_input_keys`
Expected: FAIL — `extract_friction_target` not defined.

- [ ] **Step 3: Implement**

```rust
/// The symbol/path a call addressed, for friction attribution. Priority order:
/// the most specific address first (name_path/symbol), then name, then path/query/pattern.
fn extract_friction_target(input: &Value) -> Option<String> {
    const KEYS: [&str; 6] = ["name_path", "symbol", "name", "query", "path", "pattern"];
    for k in KEYS {
        if let Some(s) = input.get(k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::extract_friction_target_coalesces_input_keys`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/usage/mod.rs
git commit -m "feat(usage): extract_friction_target — coalesce input address keys"
```

---

### Task 7: Extend `write_record` with the four columns

**Files:**
- Modify: `src/usage/db.rs:91` (`write_record` signature + INSERT)
- Modify: `src/usage/db.rs` tests — every existing `write_record(...)` call gains four trailing args
- Test: `src/usage/db.rs` (tests module) — new field-roundtrip test

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn write_record_stores_friction_fields() {
    let (_dir, conn) = tmp();
    write_record(
        &conn, "symbols", 42, "success", true, None,
        "cs-sha", Some("proj-sha"), "sess-1", None, None, None,
        Some("LspManager/get_or_start"), Some(1045), None, Some("/repo"),
    )
    .unwrap();
    let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
    assert_eq!(tok, Some(1045));
    assert_eq!(ef, None);
    assert_eq!(pr.as_deref(), Some("/repo"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::db::tests::write_record_stores_friction_fields`
Expected: FAIL — `write_record` takes 12 args, got 16 (compile error).

- [ ] **Step 3: Extend the signature and INSERT**

Replace `write_record`'s signature tail and INSERT:

```rust
#[allow(clippy::too_many_arguments)]
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
    cc_session_id: Option<&str>,
    friction_target: Option<&str>,
    overflow_tokens: Option<i64>,
    err_family: Option<&str>,
    project_root: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, error_msg, codescout_sha, project_sha, session_id, input_json, output_json, cc_session_id, friction_target, overflow_tokens, err_family, project_root)
         VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            cc_session_id,
            friction_target,
            overflow_tokens,
            err_family,
            project_root,
        ],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')",
        [],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Update every existing `write_record` call in this file's tests**

The existing test calls (`write_record_roundtrip`, `write_record_stores_all_fields`, `write_record_overflow_flag`, `write_record_stores_traceability_fields`, `write_record_traceability_fields_nullable`, and the `retention_prunes_old_rows` write) end with `... None, None, None,` (the last three being `input_json, output_json, cc_session_id`). Append four `None`s to each so they read `... None, None, None, None, None, None, None,` (12→16 args). Example for `write_record_roundtrip`:

```rust
    write_record(
        &conn, "symbols", 42, "success", false, None,
        "unknown", None, "test-session", None, None, None,
        None, None, None, None,
    )
    .unwrap();
```

- [ ] **Step 5: Run the db tests to verify all pass**

Run: `cargo test --lib usage::db::tests`
Expected: PASS (new test green; all migrated call sites compile and pass).

- [ ] **Step 6: Commit**

```bash
git add src/usage/db.rs
git commit -m "feat(usage): write_record persists friction_target/overflow_tokens/err_family/project_root"
```

---

### Task 8: Wire the helpers into `write_content` (end-to-end)

**Files:**
- Modify: `src/usage/mod.rs:44` (`write_content`)
- Test: `src/usage/mod.rs` (tests module) — end-to-end via `record_content`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
#[serial_test::serial]
async fn record_content_populates_friction_fields_on_overflow() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let recorder = UsageRecorder::new(agent.clone(), false, "test-session".to_string());
    let input = json!({"name_path": "LspManager/get_or_start", "path": "src/lsp/manager.rs"});

    let _ = recorder
        .record_content("symbols", &input, || async {
            Ok(vec![Content::text(
                r#"{"output_id":"@tool_x","summary":"...","buffered_bytes":10000}"#.to_string(),
            )])
        })
        .await;

    let conn = crate::usage::db::open_db(dir.path()).unwrap();
    let (overflowed, ft, tok, pr): (i64, Option<String>, Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT overflowed, friction_target, overflow_tokens, project_root FROM tool_calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(overflowed, 1, "output_id envelope → overflowed");
    assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
    assert_eq!(tok, Some(2500), "10000 bytes / 4");
    assert!(pr.is_some(), "project_root always set");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage::record_content_populates_friction_fields_on_overflow`
Expected: FAIL — `friction_target`/`overflow_tokens` are `None` (not yet wired).

- [ ] **Step 3: Wire the helpers in `write_content`**

In `write_content`, after the `let (outcome, overflowed, error_msg) = classify_content_result(result);` line, add the field derivations, and pass them to `write_record`:

```rust
        let (outcome, overflowed, error_msg) = classify_content_result(result);

        // Friction fields (Phase 1 of the legibility probe).
        let is_friction = overflowed || outcome != "success";
        let friction_target = if is_friction {
            extract_friction_target(input)
        } else {
            None
        };
        let overflow_tokens = if overflowed {
            extract_overflow_tokens(result)
        } else {
            None
        };
        let err_family = error_msg.as_deref().and_then(normalize_err_family);
        let project_root_str = project_root.to_string_lossy().to_string();
```

Then extend the `write_record(...)` call with the four new trailing args:

```rust
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
            cc_session_id.as_deref(),
            friction_target.as_deref(),
            overflow_tokens,
            err_family,
            Some(project_root_str.as_str()),
        )?;
```

> Note: `friction_target` extraction uses `input` directly (the `&Value` param), so it works in normal mode even though `input_json` is debug-gated — required by the spec.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage::record_content_populates_friction_fields_on_overflow`
Expected: PASS.

- [ ] **Step 5: Run the full usage module + clippy**

Run: `cargo test --lib usage:: && cargo clippy -- -D warnings`
Expected: PASS, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add src/usage/mod.rs
git commit -m "feat(usage): populate friction fields in write_content (end-to-end)"
```

---

### Task 9: Full-suite gate + manual MCP verification

**Files:** none (verification only)

- [ ] **Step 1: Full test + fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all green, clippy clean.

- [ ] **Step 2: Build release + restart MCP, confirm live capture**

```bash
cargo build --release
# restart the server with /mcp, then make any overflowing call (e.g. symbols on a big file)
```

Then query the live DB:

```bash
sqlite3 -readonly .codescout/usage.db \
  "SELECT tool_name, overflowed, friction_target, overflow_tokens, project_root FROM tool_calls WHERE overflowed=1 ORDER BY id DESC LIMIT 3;"
```

Expected: rows with `overflowed=1`, a non-null `friction_target`, a plausible `overflow_tokens`, and `project_root` = this repo. (Before this plan, `overflowed` was always 0.)

- [ ] **Step 3: Commit nothing / done**

This task is a gate; if anything fails, fix in the relevant task above before proceeding to Phase 2.

---

## Self-Review

**Spec coverage** (against `2026-06-13-dzo-friction-probes-design.md` § "Deliverable 1 — Honest logging"):
- (a) Fix dead `overflowed` → Task 3. ✓
- (b) Four columns `friction_target` / `overflow_tokens` / `err_family` / `project_root` → Tasks 1 (schema), 4/5/6 (extraction), 7 (persist), 8 (wire). ✓
- `buffered_bytes` on envelope → Task 2. ✓
- (c) Additive nullable migration, no backfill, retention unchanged → Task 1 (no `DELETE`/retention edit). ✓
- `friction_target` not debug-gated, from `input` directly → Task 8 Step 3 note + test. ✓
- Tests called out in the spec's Testing section: `overflowed` regression (Task 3), `err_family` normalization (Task 5). The F-1 `project_root`-exclusion test and the reconcile/no-line-count tests belong to **Phase 2** (the probe) and are out of scope here. ✓

**Placeholder scan:** Task 2's test has a conditional ("if `types.rs` has no test seam…") — this is a genuine harness-shape unknown in that file, with a concrete fallback (assert through `call_content`); the implementer picks the seam that exists. Not a content placeholder. All code blocks are complete.

**Type consistency:** helper names are stable across tasks — `extract_overflow_tokens` (T4/T8), `normalize_err_family` (T5/T8), `extract_friction_target` (T6/T8); `write_record`'s 16-arg order is identical in T7 (def) and T8 (call). Column names `friction_target`/`overflow_tokens`/`err_family`/`project_root` identical across T1/T7/T8.

## Out of scope (this plan)

- The `legibility_scan` librarian action, the scorer, the tracker reconcile — **Phase 2**, planned separately against the columns shipped here.
- The 16-arg `write_record` is a legibility smell in its own right (a `ToolCallRecord` struct would be cleaner). Deliberately deferred — the probe may flag it later, which is the system eating its own dogfood. Not bundled to keep this plan minimal.

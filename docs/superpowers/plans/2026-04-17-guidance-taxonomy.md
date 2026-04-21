# Three-Level Guidance Taxonomy + `read_markdown` Overflow — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce three severity-tagged fields (`hint`, `warning`, `must_follow`) in tool responses, upgrade `read_markdown` oversized-heading behavior to a `must_follow`-carrying recoverable error with a nested section map and concrete next actions, add IRON LAW #6 (reuse `@file_*` buffer refs) to the server instructions, and prime the generated system prompt to teach the rule before the first call.

**Architecture:** Extend `RecoverableError` with a `Guidance` enum variant and an `extra` JSON bag; update `route_tool_error` to splice both into the response body. Modify `read_markdown` so large-tier responses (bare calls and oversized heading matches) emit `must_follow` citing IRON LAW #6, and extend the `@file_*` buffer-ref branch to accept heading navigation so the suggested `next_actions` actually work. Bump `ONBOARDING_VERSION` so existing projects regenerate stale system prompts.

**Tech Stack:** Rust, Serde JSON, MCP, tree-sitter (via existing markdown heading extraction)

**Spec:** `docs/superpowers/specs/2026-04-17-guidance-taxonomy-design.md`

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/tools/mod.rs` | Modify | Add `Guidance` enum; extend `RecoverableError` with `guidance` and `extra` fields; add `::with_warning` and `::with_must_follow` builders |
| `src/server.rs` | Modify | Update `route_tool_error` to serialize `Guidance` under its variant-named key and splice `extra` fields into the body |
| `src/tools/markdown.rs` | Modify | Allow `heading=` / `headings=` on `@file_*` buffer refs; upgrade Tier-3 bare-call response to carry `must_follow`; convert oversized-heading branch to `ok: false` + `must_follow` + `section_map` + `next_actions` |
| `src/prompts/server_instructions.md` | Modify | Add IRON LAW #6; add `read_markdown` row to the Output Buffers table; reclassify two existing hints to `must_follow` |
| `src/tools/workflow.rs` | Modify | Extend `read_markdown` guidance in `build_system_prompt_draft` for both single- and multi-project branches; bump `ONBOARDING_VERSION` from 6 to 7 |
| `docs/PROGRESSIVE_DISCOVERABILITY.md` | Modify | Add a taxonomy section explaining when to use `hint` / `warning` / `must_follow` |

---

### Task 1: Add `Guidance` enum and extend `RecoverableError`

**Files:**
- Modify: `src/tools/mod.rs` (struct def around line 145; tests around line 723)

- [ ] **Step 1: Write failing tests for the new API**

Add these tests at the bottom of the existing `#[cfg(test)] mod tests` block in `src/tools/mod.rs` (near `recoverable_error_stores_message`, line ~723):

```rust
#[test]
fn recoverable_error_with_warning_stores_warning_variant() {
    let e = RecoverableError::with_warning("too many results", "narrow with path=");
    assert_eq!(e.message, "too many results");
    assert!(matches!(e.guidance, Some(Guidance::Warning(ref s)) if s == "narrow with path="));
}

#[test]
fn recoverable_error_with_must_follow_stores_must_follow_variant() {
    let e = RecoverableError::with_must_follow(
        "heading too large",
        "IRON LAW #6: use @file_xxx",
    );
    assert_eq!(e.message, "heading too large");
    assert!(
        matches!(e.guidance, Some(Guidance::MustFollow(ref s)) if s == "IRON LAW #6: use @file_xxx")
    );
}

#[test]
fn recoverable_error_with_hint_still_produces_hint_variant() {
    let e = RecoverableError::with_hint("not found", "check path");
    assert!(matches!(e.guidance, Some(Guidance::Hint(ref s)) if s == "check path"));
    // Back-compat: legacy `hint` accessor still works.
    assert_eq!(e.hint(), Some("check path"));
}

#[test]
fn recoverable_error_extra_fields_roundtrip() {
    let mut e = RecoverableError::new("heading too large");
    e.extra.insert("file_id".into(), serde_json::json!("@file_abc"));
    e.extra.insert(
        "section_map".into(),
        serde_json::json!([{"level": 2, "text": "## X", "line": 10}]),
    );
    assert_eq!(e.extra["file_id"], "@file_abc");
    assert_eq!(e.extra["section_map"][0]["line"], 10);
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cargo test -p codescout recoverable_error --lib 2>&1 | head -40`

Expected: compile errors mentioning `Guidance`, `with_warning`, `with_must_follow`, and `extra` not found.

- [ ] **Step 3: Replace the `RecoverableError` struct and impl block**

In `src/tools/mod.rs`, replace the existing `RecoverableError` struct (currently lines ~144-167) and its `impl` block with:

```rust
/// Severity-tagged guidance attached to a [`RecoverableError`].
///
/// Serialized into the response body under the variant-named key
/// (`hint` / `warning` / `must_follow`). The field name itself carries the
/// register — agents scan JSON responses and react to the key, not the prose.
#[derive(Debug, Clone)]
pub enum Guidance {
    /// Optional narrowing — "you could try X".
    Hint(String),
    /// Off-golden-path — "reconsider before proceeding".
    Warning(String),
    /// Binding, iron-law-grade rule — violating produces wrong results or
    /// wastes significant context. Cite the specific rule where applicable
    /// (e.g. "IRON LAW #6: ...").
    MustFollow(String),
}

impl Guidance {
    /// JSON field name the variant serializes under.
    pub fn field_name(&self) -> &'static str {
        match self {
            Self::Hint(_) => "hint",
            Self::Warning(_) => "warning",
            Self::MustFollow(_) => "must_follow",
        }
    }

    /// The guidance text.
    pub fn text(&self) -> &str {
        match self {
            Self::Hint(s) | Self::Warning(s) | Self::MustFollow(s) => s.as_str(),
        }
    }
}

/// A recoverable tool error: the LLM gave bad input and can self-correct.
///
/// When a tool returns this error type, the MCP server serialises it as
/// `isError: false` with a JSON body containing `"error"`, optional
/// `guidance` (under one of `hint` / `warning` / `must_follow`), and any
/// structured `extra` fields spliced in at the top level.  This prevents
/// Claude Code from aborting sibling parallel tool calls (which it does
/// when it sees `isError: true`).
///
/// Use this for **expected, input-driven failures**: path not found,
/// unsupported file type, empty glob match, no index built yet, etc.
///
/// Keep returning plain `anyhow` errors (→ `isError: true`) for genuine
/// failures: panics, security violations, LSP crashes.
#[derive(Debug)]
pub struct RecoverableError {
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Optional severity-tagged guidance for how to correct the call.
    pub guidance: Option<Guidance>,
    /// Structured payload spliced into the response body at the top level
    /// (e.g. `file_id`, `section_map`, `next_actions`).
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl RecoverableError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: None,
            extra: serde_json::Map::new(),
        }
    }

    pub fn with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::Hint(hint.into())),
            extra: serde_json::Map::new(),
        }
    }

    pub fn with_warning(message: impl Into<String>, warning: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::Warning(warning.into())),
            extra: serde_json::Map::new(),
        }
    }

    pub fn with_must_follow(message: impl Into<String>, must_follow: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::MustFollow(must_follow.into())),
            extra: serde_json::Map::new(),
        }
    }

    /// Attach a structured field to the response body. Chainable.
    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }

    /// Back-compat accessor: returns the text of the attached `Hint` variant,
    /// or `None` for other variants or no guidance. Used by tests and the
    /// small number of call sites that inspect the hint directly.
    pub fn hint(&self) -> Option<&str> {
        match &self.guidance {
            Some(Guidance::Hint(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Fix the existing `recoverable_error_stores_hint` test**

The existing test at `src/tools/mod.rs` around line 733 reads:

```rust
#[test]
fn recoverable_error_stores_hint() {
    let e = RecoverableError::with_hint("path not found", "use list_dir to explore");
    assert_eq!(e.message, "path not found");
    assert_eq!(e.hint.as_deref(), Some("use list_dir to explore"));
}
```

Replace the `e.hint.as_deref()` assertion with the new accessor:

```rust
#[test]
fn recoverable_error_stores_hint() {
    let e = RecoverableError::with_hint("path not found", "use list_dir to explore");
    assert_eq!(e.message, "path not found");
    assert_eq!(e.hint(), Some("use list_dir to explore"));
}
```

Also update `recoverable_error_stores_message` (line ~725) to use the accessor:

```rust
#[test]
fn recoverable_error_stores_message() {
    let e = RecoverableError::new("path not found");
    assert_eq!(e.message, "path not found");
    assert!(e.hint().is_none());
}
```

- [ ] **Step 5: Run all `recoverable_error_*` tests to verify they pass**

Run: `cargo test -p codescout recoverable_error --lib`

Expected: all tests pass, including the new four from Step 1 and the two updated tests from Step 4.

- [ ] **Step 6: Run full test suite to check nothing else broke**

Run: `cargo test -p codescout --lib 2>&1 | tail -20`

Expected: all tests pass. If any external call site referenced the old public `.hint` field directly (not via `hint()`), fix by switching to `.hint()`. Based on the grep audit, the only such reference is in the existing updated test — external call sites all use `with_hint(...)`.

- [ ] **Step 7: Commit**

```bash
git add src/tools/mod.rs
git commit -m "$(cat <<'EOF'
feat(errors): three-level Guidance taxonomy (hint/warning/must_follow)

Extends RecoverableError with a Guidance enum and structured extra
fields. Adds with_warning, with_must_follow, and with_extra builders.
with_hint continues to work; hint() accessor replaces the public field
for inspection.
EOF
)"
```

---

### Task 2: Serialize `Guidance` and `extra` in `route_tool_error`

**Files:**
- Modify: `src/server.rs` (`route_tool_error` around line 602; tests around line 1263)

- [ ] **Step 1: Write failing tests for the new serialization**

Add these tests to the existing `route_tool_error` test block in `src/server.rs` (after `recoverable_error_body_includes_hint_when_present`, around line 1273):

```rust
#[test]
fn recoverable_error_body_serializes_warning_under_warning_key() {
    let err = anyhow::Error::new(crate::tools::RecoverableError::with_warning(
        "too many results",
        "narrow with path=",
    ));
    let result = route_tool_error(err);
    let text = &result.content[0].as_text().unwrap().text;
    let body: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(body["warning"], "narrow with path=");
    assert!(body.get("hint").is_none());
    assert!(body.get("must_follow").is_none());
}

#[test]
fn recoverable_error_body_serializes_must_follow_under_must_follow_key() {
    let err = anyhow::Error::new(crate::tools::RecoverableError::with_must_follow(
        "heading too large",
        "IRON LAW #6: use @file_xxx",
    ));
    let result = route_tool_error(err);
    let text = &result.content[0].as_text().unwrap().text;
    let body: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(body["must_follow"], "IRON LAW #6: use @file_xxx");
    assert!(body.get("hint").is_none());
    assert!(body.get("warning").is_none());
}

#[test]
fn recoverable_error_body_splices_extra_fields_at_top_level() {
    let err_struct = crate::tools::RecoverableError::with_must_follow(
        "heading too large",
        "IRON LAW #6",
    )
    .with_extra("file_id", serde_json::json!("@file_abc"))
    .with_extra(
        "section_map",
        serde_json::json!([{"level": 2, "text": "## X", "line": 10}]),
    );
    let err: anyhow::Error = err_struct.into();
    let result = route_tool_error(err);
    let text = &result.content[0].as_text().unwrap().text;
    let body: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(body["file_id"], "@file_abc");
    assert_eq!(body["section_map"][0]["line"], 10);
    assert_eq!(body["ok"], serde_json::Value::Bool(false));
    assert_eq!(body["error"], "heading too large");
    assert_eq!(body["must_follow"], "IRON LAW #6");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout route_tool_error --lib`

Expected: the three new tests fail. Existing tests (`recoverable_error_body_includes_hint_when_present`, etc.) still pass because we haven't changed the happy path yet.

- [ ] **Step 3: Update `route_tool_error` in `src/server.rs`**

Find `route_tool_error` (around line 602). Replace the `RecoverableError` branch body:

```rust
fn route_tool_error(e: anyhow::Error) -> CallToolResult {
    if let Some(rec) = e.downcast_ref::<crate::tools::RecoverableError>() {
        let mut body = serde_json::json!({ "ok": false, "error": rec.message });
        if let Some(g) = &rec.guidance {
            body[g.field_name()] = serde_json::json!(g.text());
        }
        if let Some(obj) = body.as_object_mut() {
            for (k, v) in &rec.extra {
                obj.insert(k.clone(), v.clone());
            }
        }
        let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
```

Keep the rest of the function (the text wrapping / `CallToolResult::structured_error` call) unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout route_tool_error --lib`

Expected: all tests pass — the three new ones from Step 1, plus all existing body-shape tests.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs
git commit -m "$(cat <<'EOF'
feat(server): serialize Guidance and extra fields in route_tool_error

RecoverableError with warning/must_follow guidance now emits the
appropriate top-level field (warning or must_follow) instead of
everything being flattened to "hint". Structured extra fields
(file_id, section_map, next_actions, etc.) spliced into the body.
EOF
)"
```

---

### Task 3: Accept `heading=` / `headings=` on `@file_*` buffer refs in `read_markdown`

**Files:**
- Modify: `src/tools/markdown.rs` (buffer-ref branch lines ~55-94; tests at the bottom)

- [ ] **Step 1: Write failing tests for heading nav on buffer refs**

Add these tests inside the existing `mod tests` block in `src/tools/markdown.rs` (near `read_markdown_large_returns_summary_no_content`):

```rust
#[tokio::test]
async fn buffer_ref_accepts_single_heading_nav() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    // 200 sections, each ~20 lines — large-tier, so we get a file_id back.
    std::fs::write(&file, synth_md(200, 20)).unwrap();

    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let fid = first["file_id"].as_str().unwrap().to_string();

    let second = super::ReadMarkdown
        .call(
            json!({ "path": fid, "heading": "## Section 5" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        second.get("content").is_some() || second.get("file_id").is_some(),
        "heading nav on @file_* must return content or a nested buffer, got: {second}"
    );
}

#[tokio::test]
async fn buffer_ref_accepts_multi_heading_nav() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    std::fs::write(&file, synth_md(200, 20)).unwrap();

    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let fid = first["file_id"].as_str().unwrap().to_string();

    let second = super::ReadMarkdown
        .call(
            json!({
                "path": fid,
                "headings": ["## Section 3", "## Section 5"],
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(second["sections_returned"], 2);
}
```

Also update the existing rejection test (if one exists — search for the assertion that heading nav on a buffer ref returns an error). If it exists, flip it into an acceptance test; if not, the new tests above are sufficient.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout buffer_ref --lib`

Expected: both tests fail with an error containing `"heading navigation is not supported on @file_* buffer refs"` — matching the current behavior at `src/tools/markdown.rs:66-71`.

- [ ] **Step 3: Update the buffer-ref branch to delegate heading nav to the shared code path**

In `src/tools/markdown.rs`, replace the `@file_` branch (currently starts at line ~52, ends around line 94) so it loads the buffered text, then falls through to the shared heading-nav / line-range logic. The simplest structural change is to resolve the buffered path/text into local variables and skip the disk-read step:

```rust
// Buffer-ref branch — resolve the buffered text, then fall through to the
// shared heading / line-range logic below. `resolved` becomes a synthetic
// PathBuf used only for logging + coverage keys; it is never touched on disk.
let (resolved, text) = if path.starts_with("@file_") {
    let buf = ctx.output_buffer.get(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("buffer reference not found: '{}'", path),
            "Buffer refs expire when the session resets. Re-run read_markdown on the file to get a fresh ref.",
        )
    })?;
    // Best-effort: recover the original path if the buffer stores it; otherwise
    // synthesize one from the ref. Only used for coverage dedup keys.
    let resolved = buf
        .source_path
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(path));
    (resolved, buf.stdout.clone())
} else {
    // Gate: .md files only
    if !path.ends_with(".md") && !path.ends_with(".markdown") {
        return Err(RecoverableError::with_hint(
            "read_markdown only supports .md files",
            "Use read_file for non-markdown files.",
        )
        .into());
    }

    let project_root = ctx.agent.project_root().await;
    let security = ctx.agent.security_config().await;
    let resolved = crate::util::path_security::validate_read_path(
        path,
        project_root.as_deref(),
        &security,
    )?;

    if resolved.is_dir() {
        return Err(RecoverableError::with_hint(
            format!("'{}' is a directory, not a file", path),
            "Use list_dir to browse directory contents, or provide a specific file path",
        )
        .into());
    }

    let text = std::fs::read_to_string(&resolved).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RecoverableError::with_hint(
            format!("file not found: '{}'", path),
            "Check the path with list_dir, or use find_file to locate the file",
        )
        .into(),
        _ => anyhow::anyhow!("failed to read {}: {}", resolved.display(), e),
    })?;
    (resolved, text)
};
```

Delete the previous standalone `if path.starts_with("@file_") { ... return ... }` block (lines ~53-94 of the original) and the standalone gate/validate/read block that follows — they are now consolidated above.

(`BufferEntry` already carries `source_path: Option<PathBuf>` at `src/tools/output_buffer.rs:26`, populated by `store_file`. No output-buffer changes required.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout buffer_ref --lib`

Expected: both new tests pass. Also run existing markdown tests to verify no regression:

`cargo test -p codescout --lib markdown`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "$(cat <<'EOF'
feat(read_markdown): accept heading= and headings= on @file_* buffer refs

Extends the buffer-ref branch to load the buffered text and fall through
to the shared heading-nav / line-range path, so agents can navigate
cached large-tier buffers without re-reading the original file.
EOF
)"
```

---

### Task 4: Upgrade bare `read_markdown(path)` Tier-3 response to carry `must_follow`

**Files:**
- Modify: `src/tools/markdown.rs` (Tier-3 branch around lines ~325-370; test `read_markdown_large_returns_summary_no_content` around line 962)

- [ ] **Step 1: Write failing test for `must_follow` on Tier-3 bare call**

Add this test in `src/tools/markdown.rs` tests block:

```rust
#[tokio::test]
async fn read_markdown_large_includes_must_follow_citing_iron_law_6() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    std::fs::write(&file, synth_md(200, 20)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let mf = out["must_follow"]
        .as_str()
        .expect("large-tier response must include must_follow");
    assert!(
        mf.contains("IRON LAW #6"),
        "must_follow must cite IRON LAW #6, got: {mf}"
    );
    assert!(
        mf.contains("@file_"),
        "must_follow must reference the file_id to steer reuse, got: {mf}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout read_markdown_large_includes_must_follow --lib`

Expected: fails because current Tier-3 response has `recipe` but no `must_follow` field.

- [ ] **Step 3: Replace the Tier-3 `recipe` string with a `must_follow`**

In `src/tools/markdown.rs`, inside the `if oversized { ... }` block (lines ~325-370), replace the `recipe` construction and its insertion into `result` with a `must_follow` string. The new block:

```rust
// ── Tier 3: large — heading map + must_follow, no body ─────────────────
if oversized {
    let all_headings = crate::tools::file_summary::parse_all_headings(&text);
    let heading_count = all_headings.len();
    let heading_map: Vec<Value> = all_headings
        .iter()
        .map(|h| {
            json!({
                "level": h.level,
                "text": h.text,
                "line": h.line,
            })
        })
        .collect();

    let file_id = ctx
        .output_buffer
        .store_file(resolved.to_string_lossy().to_string(), text.clone());

    let must_follow = if heading_count == 0 {
        format!(
            "IRON LAW #6: For subsequent reads, use {:?} (NOT the original path). \
             Slice with read_markdown({:?}, start_line=N, end_line=M).",
            file_id, file_id
        )
    } else {
        format!(
            "IRON LAW #6: For subsequent reads, use {:?} (NOT the original path). \
             Pick a heading: read_markdown({:?}, heading=\"## Section\"). \
             Or slice: read_markdown({:?}, start_line=N, end_line=M).",
            file_id, file_id, file_id
        )
    };

    let mut result = json!({
        "format": "markdown",
        "total_lines": total_lines,
        "total_bytes": total_bytes,
        "heading_count": heading_count,
        "heading_map": heading_map,
        "file_id": file_id,
        "must_follow": must_follow,
    });
    if let Some(c) = md_cov {
        result["coverage"] = c;
    }
    return Ok(result);
}
```

- [ ] **Step 4: Update the existing `read_markdown_large_returns_summary_no_content` test**

Find the test around line 962. The assertion `assert!(out.get("recipe").is_some(), "large tier includes recipe string");` must change to:

```rust
assert!(
    out.get("must_follow").is_some(),
    "large tier includes must_follow citing IRON LAW #6"
);
assert!(
    out.get("recipe").is_none(),
    "large tier no longer uses the recipe field — must_follow supersedes it"
);
```

Also find the sibling test `read_markdown_large_no_headings_recipe_pivots_to_line_ranges` (line ~839): update it to assert on `must_follow` instead of `recipe`. The test body should check the `must_follow` contains `start_line` and not mention `heading` when the file has no headings:

```rust
#[tokio::test]
async fn read_markdown_large_no_headings_must_follow_pivots_to_line_ranges() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("noheadings.md");
    // Generate a large file with no headings.
    let body = "word ".repeat(100_000);
    std::fs::write(&file, &body).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let mf = out["must_follow"].as_str().unwrap();
    assert!(mf.contains("start_line"), "must_follow must mention start_line; got: {mf}");
    assert!(
        !mf.contains("heading=\""),
        "must_follow must not suggest heading nav when there are no headings; got: {mf}"
    );
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p codescout read_markdown_large --lib`

Expected: all three tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "$(cat <<'EOF'
feat(read_markdown): upgrade Tier-3 recipe to must_follow citing IRON LAW #6

Large-tier responses now emit must_follow instead of recipe, steering
the agent toward reusing the returned @file_* ref for subsequent reads
rather than re-reading the original path.
EOF
)"
```

---

### Task 5: Upgrade oversized-heading response to `ok: false` with `must_follow`, `section_map`, and `next_actions`

**Files:**
- Modify: `src/tools/markdown.rs` (single-heading oversized branch around lines ~220-235)

- [ ] **Step 1: Write failing test**

Add to the `mod tests` block in `src/tools/markdown.rs`:

```rust
#[tokio::test]
async fn heading_on_large_section_returns_ok_false_with_must_follow_and_section_map() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    // synth_md generates "# Section N\n...\n" for each section. To get an
    // oversized `#` (root) section, build a file with one H1 containing many
    // H2 subsections.
    let mut body = String::from("# Root\n\n");
    for i in 0..200 {
        body.push_str(&format!("## Sub {i}\n\n"));
        body.push_str(&"word ".repeat(500));
        body.push_str("\n\n");
    }
    std::fs::write(&file, &body).unwrap();

    let err = super::ReadMarkdown
        .call(
            json!({ "path": file.to_str().unwrap(), "heading": "# Root" }),
            &ctx,
        )
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("oversized heading must be RecoverableError (isError:false)");
    assert!(
        rec.message.contains("too large") || rec.message.contains("exceeds"),
        "error message should explain oversize; got: {}",
        rec.message
    );
    match &rec.guidance {
        Some(crate::tools::Guidance::MustFollow(s)) => {
            assert!(
                s.contains("IRON LAW #6"),
                "must_follow must cite IRON LAW #6; got: {s}"
            );
        }
        other => panic!("expected MustFollow guidance, got {:?}", other),
    }
    assert!(
        rec.extra.get("file_id").is_some(),
        "extra must include file_id for subsequent buffer-ref reads"
    );
    let sm = rec
        .extra
        .get("section_map")
        .expect("extra must include nested section_map");
    let arr = sm.as_array().expect("section_map is an array");
    assert!(
        !arr.is_empty(),
        "section_map must list nested sub-headings (H2s under H1)"
    );
    assert!(
        rec.extra.get("next_actions").is_some(),
        "extra must include concrete next_actions"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout heading_on_large_section --lib`

Expected: fails because the current oversized-heading branch returns `Ok(...)` with `file_id`, not `Err(RecoverableError::with_must_follow(...))`.

- [ ] **Step 3: Replace the oversized-heading branch**

In `src/tools/markdown.rs`, locate the single-heading branch (current code around lines ~215-235):

```rust
// Buffer large sections
if crate::tools::exceeds_inline_limit(&section_result.content) {
    let file_id = ctx.output_buffer.store_file(
        resolved.to_string_lossy().to_string(),
        section_result.content.clone(),
    );
    let mut val = json!({ ... });
    if let Some(c) = cov { val["coverage"] = c; }
    return Ok(val);
}
```

Replace with:

```rust
// Oversized match — return ok:false with must_follow + nested section_map
// + next_actions. The agent must pick a sub-heading or a line range, not
// retry against the original path.
if crate::tools::exceeds_inline_limit(&section_result.content) {
    let file_id = ctx.output_buffer.store_file(
        resolved.to_string_lossy().to_string(),
        section_result.content.clone(),
    );
    let section_lines = section_result
        .content
        .lines()
        .count();

    // Build nested section map: only sub-headings whose line falls within
    // the matched section's line_range, excluding the matched heading itself.
    let (start_ln, end_ln) = section_result.line_range;
    let all_headings = crate::tools::file_summary::parse_all_headings(&text);
    let nested: Vec<serde_json::Value> = all_headings
        .iter()
        .filter(|h| h.line > start_ln && h.line <= end_ln)
        .map(|h| json!({"level": h.level, "text": h.text, "line": h.line}))
        .collect();

    let heading_label = section_result
        .breadcrumb
        .last()
        .cloned()
        .unwrap_or_else(|| heading_query.to_string());

    let must_follow = format!(
        "IRON LAW #6: Use {:?} for subsequent reads — NOT the original path. \
         Pick a sub-heading from section_map OR use read_markdown({:?}, start_line=N, end_line=M).",
        file_id, file_id
    );

    let next_actions: Vec<String> = {
        let mut actions = Vec::new();
        if let Some(first) = nested.first() {
            if let Some(h) = first.get("text").and_then(|v| v.as_str()) {
                actions.push(format!(
                    "read_markdown({:?}, heading={:?})",
                    file_id, h
                ));
            }
        }
        actions.push(format!(
            "read_markdown({:?}, start_line={}, end_line={})",
            file_id,
            start_ln,
            start_ln + 100.min(section_lines)
        ));
        actions
    };

    let err = crate::tools::RecoverableError::with_must_follow(
        format!(
            "heading {:?} spans {} lines — exceeds inline threshold",
            heading_label, section_lines
        ),
        must_follow,
    )
    .with_extra("file_id", serde_json::json!(file_id))
    .with_extra("section_map", serde_json::json!(nested))
    .with_extra("next_actions", serde_json::json!(next_actions))
    .with_extra(
        "breadcrumb",
        serde_json::json!(section_result.breadcrumb),
    )
    .with_extra(
        "line_range",
        serde_json::json!([start_ln, end_ln]),
    );
    return Err(err.into());
}
```

- [ ] **Step 4: Apply the same pattern to the multi-heading branch**

The `headings=[...]` branch joins multiple sections with `"\n\n"`. If the joined content exceeds the inline limit, it currently returns `Ok(...)` with the full content (there is no buffer fallback in that branch today, per `src/tools/markdown.rs` lines ~163-194). Add an overflow guard *before* constructing the `content` join:

```rust
let content = sections.join("\n\n");

// Oversized multi-heading join — fall back to must_follow, do not return inline.
if crate::tools::exceeds_inline_limit(&content) {
    let file_id = ctx.output_buffer.store_file(
        resolved.to_string_lossy().to_string(),
        content.clone(),
    );
    let lines = content.lines().count();
    let must_follow = format!(
        "IRON LAW #6: The combined section content is too large to return inline. \
         Use {:?} for subsequent reads — NOT the original path. \
         Request one heading at a time, or slice with start_line/end_line.",
        file_id
    );
    let next_actions: Vec<String> = seen_headings
        .iter()
        .take(3)
        .map(|h| format!("read_markdown({:?}, heading={:?})", file_id, h))
        .collect();
    let err = crate::tools::RecoverableError::with_must_follow(
        format!("combined headings span {} lines — exceeds inline threshold", lines),
        must_follow,
    )
    .with_extra("file_id", serde_json::json!(file_id))
    .with_extra("requested_headings", serde_json::json!(seen_headings))
    .with_extra("next_actions", serde_json::json!(next_actions));
    return Err(err.into());
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p codescout heading_on_large --lib`

Expected: the new test passes. Run the full markdown suite:

`cargo test -p codescout --lib markdown`

Expected: all tests pass. If the existing multi-heading test uses small sections, it is unaffected (the new overflow guard only fires on oversized joins).

- [ ] **Step 6: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "$(cat <<'EOF'
feat(read_markdown): oversized heading returns ok:false with must_follow

Single- and multi-heading oversized matches now route through
RecoverableError with must_follow citing IRON LAW #6, a nested
section_map (children of the matched heading), and concrete
next_actions using the emitted @file_* buffer ref.
EOF
)"
```

---

### Task 6: Add IRON LAW #6 and update Output Buffers table in `server_instructions.md`

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Append IRON LAW #6**

Locate the `## Iron Laws` section (starts at line 11, ends with rule 5 around line 36). Append after rule 5:

```markdown

6. **REUSE `@file_*` BUFFER REFS.** After a tool emits `@file_*`, subsequent
   reads of that content MUST use the buffer ref, not the original path.
   Re-reading the original path duplicates disk work and destroys the
   progressive-disclosure contract. Applies to `read_file`, `read_markdown`,
   and any tool that consumes `@file_*`.
```

- [ ] **Step 2: Update the Output Buffers table**

Locate the `#### Buffer ref types and access` table (around line 118). The current `@file_*` row mentions only `read_file` as producer. Replace that row with:

```markdown
| `"file_id": "@file_abc"` from `read_file` or `read_markdown` | `@file_*` | plain text | For code/text: `grep pattern @file_abc` or `read_file("@file_abc", start_line=N)`. For markdown: `read_markdown("@file_abc", heading="## Section")` or `start_line`/`end_line`. |
```

- [ ] **Step 3: Grep to confirm both changes landed**

Run: `grep -n "IRON LAW #6\|REUSE.*BUFFER REFS\|read_markdown.*@file_abc" src/prompts/server_instructions.md`

Expected: at least three matches (the rule heading, the rule body, the updated table row).

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "$(cat <<'EOF'
docs(prompts): add IRON LAW #6 and list read_markdown as @file_* producer

Iron Law #6 codifies the reuse rule for @file_* buffer refs. Output
Buffers table now lists read_markdown as a producer and shows the
markdown-specific access path.
EOF
)"
```

---

### Task 7: Reclassify two existing hints to `must_follow` in `server_instructions.md`

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Audit candidate hints**

Run: `grep -n "must\|Do NOT\|don't\|never\|Never" src/prompts/server_instructions.md | head -30`

Confirm the two candidates below still exist (or pick replacements of equivalent severity):

1. The "Don't grep `@tool_*` for code" note in the Output Buffers section (currently prose under the table — "Don't grep `@tool_*` for code — bodies are JSON strings, not raw text.").
2. The `rename_symbol` gotcha at the top of `### Gotchas` ("`rename_symbol` may corrupt string literals containing the old name — verify compilation after use.").

- [ ] **Step 2: Reclassify the `@tool_*` note**

Find the line in the Output Buffers section that says `Don't grep @tool_* for code — bodies are JSON strings, not raw text.`. Replace with:

```markdown
**MUST FOLLOW:** Do not grep `@tool_*` for code. Bodies are JSON-escaped
strings, so grep returns escaped matches, not raw text. Use
`read_file("@tool_id", json_path="$.symbols[0].body")` to extract a specific
field first.
```

- [ ] **Step 3: Reclassify the `rename_symbol` gotcha**

Find the line `- \`rename_symbol\` may corrupt string literals containing the old name — verify compilation after use.` in the `### Gotchas` subsection. Replace with:

```markdown
- **MUST FOLLOW:** `rename_symbol` may corrupt string literals containing the
  old name. Always verify compilation (`cargo check` / `tsc --noEmit` / etc.)
  after use, especially if the symbol name is a common word.
```

- [ ] **Step 4: Grep to confirm reclassifications**

Run: `grep -c "MUST FOLLOW:" src/prompts/server_instructions.md`

Expected: at least 2.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "$(cat <<'EOF'
docs(prompts): reclassify two high-severity hints to MUST FOLLOW

@tool_* grep prohibition and rename_symbol-string-literal gotcha are
correctness hazards, not optional suggestions. Elevating their prose
register from soft guidance to MUST FOLLOW so the agent treats them
as binding at scan time.
EOF
)"
```

---

### Task 8: Update `build_system_prompt_draft` and bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/tools/workflow.rs` (two branches inside `build_system_prompt_draft`; version constant at line 15; version-assert test at line 5743)

- [ ] **Step 1: Write a failing test for the new guidance line**

Add to the tests module in `src/tools/workflow.rs` (near existing `build_system_prompt_draft` tests — grep for `build_system_prompt_draft(` in the test block):

```rust
#[test]
fn system_prompt_draft_read_markdown_hint_mentions_file_ref_reuse() {
    let draft = build_system_prompt_draft(
        &["rust".to_string()],
        &["src/main.rs".to_string()],
        None,
        None,
        &[],
    );
    assert!(
        draft.contains("@file_ref") || draft.contains("@file_"),
        "draft must teach @file_* reuse for read_markdown; got:\n{draft}"
    );
    assert!(
        draft.contains("IRON LAW #6"),
        "draft must cite IRON LAW #6 in the read_markdown guidance; got:\n{draft}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout system_prompt_draft_read_markdown_hint --lib`

Expected: fails; current draft lines do not mention `@file_ref` or `IRON LAW #6`.

- [ ] **Step 3: Update both `read_markdown` guidance lines**

In `src/tools/workflow.rs`, locate the two branches of `build_system_prompt_draft` that currently emit a `read_markdown` line (multi-project branch ~line 130 and single-project branch ~line 150 — confirm with grep for `read_markdown` inside the file).

**Multi-project branch** — replace:

```rust
draft.push_str(
    "**Markdown files** (memories, plans, docs): \
     `read_markdown(\"path\")` — returns heading map; add `heading=\"## Section\"` to read one section.\n\n",
);
```

with:

```rust
draft.push_str(
    "**Markdown files** (memories, plans, docs): \
     `read_markdown(\"path\")` — returns heading map + `@file_ref` for large files. \
     **IRON LAW #6:** subsequent reads MUST use `@file_ref` (not the original path): \
     `read_markdown(\"@file_ref\", heading=\"## Section\")` or `start_line=/end_line=`.\n\n",
);
```

**Single-project branch** — replace:

```rust
draft.push_str(
    "6. `read_markdown(\"path/to/file.md\")` — memories, plans, docs; \
     add `heading=\"## Section\"` to target a specific section\n\n",
);
```

with:

```rust
draft.push_str(
    "6. `read_markdown(\"path/to/file.md\")` — returns heading map + `@file_ref` for large files. \
     **IRON LAW #6:** subsequent reads MUST use `@file_ref` (not the original path): \
     `read_markdown(\"@file_ref\", heading=\"## Section\")` or `start_line=/end_line=`.\n\n",
);
```

- [ ] **Step 4: Bump `ONBOARDING_VERSION`**

At `src/tools/workflow.rs:15`, change:

```rust
const ONBOARDING_VERSION: u32 = 6;
```

to:

```rust
const ONBOARDING_VERSION: u32 = 7;
```

- [ ] **Step 5: Update the version-assert test**

At `src/tools/workflow.rs:5743`, change:

```rust
assert_eq!(ONBOARDING_VERSION, 6);
```

to:

```rust
assert_eq!(ONBOARDING_VERSION, 7);
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p codescout --lib workflow`

Expected: all tests pass, including the new `system_prompt_draft_read_markdown_hint_mentions_file_ref_reuse`.

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "$(cat <<'EOF'
feat(prompts): prime @file_* reuse for read_markdown; bump ONBOARDING_VERSION to 7

Generated per-project system prompt now teaches IRON LAW #6 before the
first read_markdown call. ONBOARDING_VERSION bump forces regeneration
for existing onboarded projects on next onboarding pass.
EOF
)"
```

---

### Task 9: Document the taxonomy in `PROGRESSIVE_DISCOVERABILITY.md`

**Files:**
- Modify: `docs/PROGRESSIVE_DISCOVERABILITY.md`

- [ ] **Step 1: Add a new section after `### Pattern 5: Errors Are Hints Too`**

Locate `### Pattern 5: Errors Are Hints Too` in `docs/PROGRESSIVE_DISCOVERABILITY.md` (currently line 153). After that pattern's body, insert:

```markdown
### Pattern 5a: Severity-Tagged Guidance — `hint` / `warning` / `must_follow`

Tool responses carry guidance under one of three field names. The field name
itself is the prompt: agents scanning JSON react to the key, not to prose
severity markers buried inside a generic `hint` string.

| Field | Severity | When to use |
|---|---|---|
| `hint` | take-it-or-leave-it | Optional narrowing ("you could use `json_path` to extract one field"). Agent can ignore without consequence. |
| `warning` | off-golden-path | Result is suboptimal but valid. Reconsider before proceeding ("returned 50 of 401 — narrow before paginating"). |
| `must_follow` | binding, iron-law-grade | Violating produces wrong results or wastes significant context. Cite the specific rule ("IRON LAW #6: use `@file_abc` for subsequent reads — NOT the original path"). |

The three fields are mutually exclusive — at most one appears on any response.

**When to pick `must_follow`:**

- Violating the guidance produces **wrong results** (not just suboptimal).
- The rule is already in the Iron Laws — cite it by number.
- The agent has been observed to silently drift past the `hint` register.

`must_follow` is rare by construction. If every tool response carries one, the
register loses its weight. Aim for <10% of recoverable-error responses.

**Rust API:** `RecoverableError::with_must_follow(message, text)`. Attach
structured context with `.with_extra("file_id", json!(...))`; extras are
spliced into the response body at the top level.
```

- [ ] **Step 2: Commit**

```bash
git add docs/PROGRESSIVE_DISCOVERABILITY.md
git commit -m "$(cat <<'EOF'
docs(progressive-discoverability): document hint/warning/must_follow taxonomy

New Pattern 5a codifies when tool authors should pick each severity level
and how to attach them via RecoverableError.
EOF
)"
```

---

### Task 10: Final verification — `cargo fmt`, `cargo clippy`, `cargo test`, manual MCP smoke test

**Files:** none — verification only.

- [ ] **Step 1: Format and lint**

Run:

```bash
cargo fmt
cargo clippy -- -D warnings
```

Expected: no diff from `fmt`; no warnings from `clippy`.

- [ ] **Step 2: Full test suite**

Run: `cargo test`

Expected: all tests pass.

- [ ] **Step 3: Release build for MCP smoke test**

Run: `cargo build --release`

Expected: build succeeds.

- [ ] **Step 4: Restart the MCP server**

In the Claude Code session, run `/mcp` and select the restart/reload option for codescout. (This is required because the MCP server runs the release binary — per `CLAUDE.md § Development Commands`.)

- [ ] **Step 5: Smoke-test the large-tier bare call**

Call from Claude Code:

```
read_markdown("docs/superpowers/plans/2026-04-17-guidance-taxonomy.md")
```

Expected response shape:

```json
{
  "format": "markdown",
  "total_lines": ...,
  "heading_count": ...,
  "heading_map": [...],
  "file_id": "@file_...",
  "must_follow": "IRON LAW #6: For subsequent reads, use \"@file_...\" (NOT the original path). ..."
}
```

Key checks: `must_follow` present, `recipe` absent, `file_id` present.

- [ ] **Step 6: Smoke-test heading nav on `@file_*`**

Using the `@file_...` from step 5:

```
read_markdown("@file_xxx", heading="## File Map")
```

Expected: content returned (not an error). Heading nav on buffer refs works.

- [ ] **Step 7: Smoke-test oversized-heading overflow**

Find or create a markdown file where a single heading encloses >~1000 lines. Call:

```
read_markdown("path.md", heading="# Title")
```

Expected response:

```json
{
  "ok": false,
  "error": "heading ... spans ... lines — exceeds inline threshold",
  "must_follow": "IRON LAW #6: Use \"@file_...\" for subsequent reads — NOT the original path. ...",
  "file_id": "@file_...",
  "section_map": [...],
  "next_actions": [...]
}
```

Key checks: `ok: false`, `must_follow` cites IRON LAW #6, `section_map` contains nested sub-headings (not the whole file), `next_actions` references the `@file_*` ref.

- [ ] **Step 8: Smoke-test onboarding regeneration**

On a test project that has already been onboarded at `ONBOARDING_VERSION` 6 (find one in `~/.codescout/projects/` or equivalent), call `onboarding()` and verify the new generated system prompt contains the `IRON LAW #6` citation in the `read_markdown` guidance line.

- [ ] **Step 9: No commit needed**

Verification-only — no file changes.

---

## Self-Review Checklist (pre-handoff)

- [x] **Spec coverage — D1 (three fields):** Task 1 adds enum + builders; Task 2 wires serialization.
- [x] **Spec coverage — D2 (IRON LAW #6):** Task 6 adds the rule.
- [x] **Spec coverage — D3 (overflow response):** Task 5 converts single- and multi-heading oversized branches; Task 4 upgrades the bare-call Tier-3 response.
- [x] **Spec coverage — D4 (Rust types):** Task 1 defines `Guidance` enum + `with_extra`.
- [x] **Spec coverage — D5 (priming):** Task 8 updates both draft branches + version bump.
- [x] **Spec coverage — D6 (2-3 reclassifications):** Task 7 picks two.
- [x] **Spec coverage — D7 (`hint` stays for soft guidance):** Task 9 documents when each severity applies.
- [x] **Spec coverage — buffer-ref heading nav extension:** Task 3.
- [x] **Type consistency:** `Guidance::MustFollow`, `with_must_follow`, `must_follow` field name — consistent across all tasks.
- [x] **Commit discipline:** one logical change per commit; nine commits across nine implementation tasks; Task 10 is verification-only.

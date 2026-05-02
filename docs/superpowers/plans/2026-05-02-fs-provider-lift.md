# Phase 6.2 — `src/fs/` Provider Lift Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move all path-resolution and LSP-acquisition helpers from `src/tools/symbol/path_helpers.rs` into a new crate-level `src/fs/` module, decoupling them from `ToolContext`.

**Architecture:** Six context-coupled functions get their `ctx: &ToolContext` parameter replaced with `agent: &Agent` (four functions) or `(agent: &Agent, lsp: &dyn LspProvider)` (two functions + LspTimer::record which uses only `lsp`). All other helpers move unchanged. Consumer files update their imports and call sites. The old file is deleted.

**Tech Stack:** Rust, tokio async, existing `Agent`, `LspProvider`, `LspClientOps` types.

---

> **Spec correction:** The design doc states `LspTimer::record` takes `agent: &Agent`, but its body calls `ctx.lsp.record_first_response(...)`. The correct new signature is `record(self, lsp: &dyn LspProvider, lang: &str, root: &Path)`.

---

## File Map

| Action | File |
|--------|------|
| Create | `src/fs/mod.rs` |
| Modify | `src/lib.rs` — add `pub mod fs;` |
| Modify | `src/tools/symbol/mod.rs` — remove `mod path_helpers;` |
| Delete | `src/tools/symbol/path_helpers.rs` |
| Modify | `src/tools/symbol/edit_code.rs` — update imports + 8 call sites |
| Modify | `src/tools/symbol/list_overview.rs` — update imports + 6 call sites |
| Modify | `src/tools/symbol/references.rs` — update imports + 3 call sites |
| Modify | `src/tools/symbol/symbol_at.rs` — update imports + 8 call sites |
| Modify | `src/tools/symbol/symbols.rs` — update imports + 2 call sites |
| Modify | `src/tools/symbol/call_graph/mod.rs` — update imports + 2 call sites |
| Modify | `src/tools/symbol/tests.rs` — update imports (no call site changes) |

---

## Task 1: Create `src/fs/mod.rs`

**Files:**
- Create: `src/fs/mod.rs`

- [ ] **Step 1: Copy path_helpers.rs as the starting point**

```bash
cp src/tools/symbol/path_helpers.rs src/fs/mod.rs
```

- [ ] **Step 2: Replace the module header and imports**

In `src/fs/mod.rs`, replace the top of the file (lines 1–12) with:

```rust
//! Path resolution, glob expansion, and LSP-client acquisition.
//!
//! Moved from `src/tools/symbol/path_helpers.rs` (Phase 6.2). All helpers
//! take `&Agent` or `(&Agent, &dyn LspProvider)` instead of `&ToolContext`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent::Agent;
use crate::ast;
use crate::lsp::LspProvider;
use crate::tools::RecoverableError;
```

- [ ] **Step 3: Change all `pub(super)` to `pub(crate)`**

Use a global find-and-replace across `src/fs/mod.rs`:
- Find: `pub(super)`
- Replace: `pub(crate)`

There are ~10 occurrences (one per exported item).

- [ ] **Step 4: Update `LspTimer::record` signature and body**

Old:
```rust
    pub(super) async fn record(self, ctx: &ToolContext, lang: &str, root: &Path) {
        ctx.lsp
            .record_first_response(lang, root, self.start.elapsed().as_millis() as i64)
            .await;
    }
```

New:
```rust
    pub(crate) async fn record(self, lsp: &dyn LspProvider, lang: &str, root: &Path) {
        lsp.record_first_response(lang, root, self.start.elapsed().as_millis() as i64)
            .await;
    }
```

- [ ] **Step 5: Update `resolve_read_path` signature and body**

Old:
```rust
pub(super) async fn resolve_read_path(
    ctx: &ToolContext,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    if relative_path == "." || relative_path.is_empty() {
        return ctx.agent.require_project_root().await;
    }
    let project_root = ctx.agent.project_root().await;
    let security = ctx.agent.security_config().await;
```

New:
```rust
pub(crate) async fn resolve_read_path(
    agent: &Agent,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    if relative_path == "." || relative_path.is_empty() {
        return agent.require_project_root().await;
    }
    let project_root = agent.project_root().await;
    let security = agent.security_config().await;
```

- [ ] **Step 6: Update `resolve_write_path` signature and body**

Old:
```rust
pub(super) async fn resolve_write_path(
    ctx: &ToolContext,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
```

New:
```rust
pub(crate) async fn resolve_write_path(
    agent: &Agent,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = agent.require_project_root().await?;
    let security = agent.security_config().await;
```

- [ ] **Step 7: Update `resolve_glob` signature and body**

Old:
```rust
pub(super) async fn resolve_glob(
    ctx: &ToolContext,
    path_or_glob: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    let root = ctx.agent.require_project_root().await?;

    if !is_glob(path_or_glob) {
        let full = resolve_read_path(ctx, path_or_glob).await?;
```

New:
```rust
pub(crate) async fn resolve_glob(
    agent: &Agent,
    path_or_glob: &str,
) -> anyhow::Result<Vec<PathBuf>> {
    let root = agent.require_project_root().await?;

    if !is_glob(path_or_glob) {
        let full = resolve_read_path(agent, path_or_glob).await?;
```

- [ ] **Step 8: Update `get_lsp_client` — replace the full function**

Old:
```rust
pub(super) async fn get_lsp_client(
    ctx: &ToolContext,
    path: &Path,
) -> anyhow::Result<(std::sync::Arc<dyn crate::lsp::LspClientOps>, String)> {
    let lang = ast::detect_language(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("unsupported file type: {:?}", path),
            "LSP symbol analysis supports: rust, python, typescript, tsx, \
             javascript, jsx, go, java, kotlin, c, cpp, csharp, ruby. \
             Use list_functions for a tree-sitter fallback on other file types.",
        )
    })?;
    let root = ctx.agent.require_project_root().await?;
    let mux_override = ctx.agent.lsp_mux_override(lang).await;
    let client = ctx.lsp.get_or_start(lang, &root, mux_override).await?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    Ok((client, language_id.to_string()))
}
```

New:
```rust
pub(crate) async fn get_lsp_client(
    agent: &Agent,
    lsp: &dyn LspProvider,
    path: &Path,
) -> anyhow::Result<(std::sync::Arc<dyn crate::lsp::LspClientOps>, String)> {
    let lang = ast::detect_language(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("unsupported file type: {:?}", path),
            "LSP symbol analysis supports: rust, python, typescript, tsx, \
             javascript, jsx, go, java, kotlin, c, cpp, csharp, ruby. \
             Use list_functions for a tree-sitter fallback on other file types.",
        )
    })?;
    let root = agent.require_project_root().await?;
    let mux_override = agent.lsp_mux_override(lang).await;
    let client = lsp.get_or_start(lang, &root, mux_override).await?;
    let language_id = crate::lsp::servers::lsp_language_id(lang);
    Ok((client, language_id.to_string()))
}
```

- [ ] **Step 9: Update `retry_on_mux_disconnect` signature and internal call**

Old:
```rust
pub(super) async fn retry_on_mux_disconnect<F, Fut, T>(
    ctx: &ToolContext,
    path: &Path,
    initial_client: std::sync::Arc<dyn crate::lsp::LspClientOps>,
    initial_lang: String,
    op: F,
) -> anyhow::Result<T>
where
    F: Fn(std::sync::Arc<dyn crate::lsp::LspClientOps>, String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    match op(initial_client, initial_lang).await {
        Err(e) if is_mux_disconnect(&e) => {
            tracing::warn!("LSP mux disconnect, retrying once: {}", e);
            let (client, lang) = get_lsp_client(ctx, path).await?;
            op(client, lang).await
        }
        other => other,
    }
}
```

New:
```rust
pub(crate) async fn retry_on_mux_disconnect<F, Fut, T>(
    agent: &Agent,
    lsp: &dyn LspProvider,
    path: &Path,
    initial_client: std::sync::Arc<dyn crate::lsp::LspClientOps>,
    initial_lang: String,
    op: F,
) -> anyhow::Result<T>
where
    F: Fn(std::sync::Arc<dyn crate::lsp::LspClientOps>, String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    match op(initial_client, initial_lang).await {
        Err(e) if is_mux_disconnect(&e) => {
            tracing::warn!("LSP mux disconnect, retrying once: {}", e);
            let (client, lang) = get_lsp_client(agent, lsp, path).await?;
            op(client, lang).await
        }
        other => other,
    }
}
```

- [ ] **Step 10: Verify no remaining `ctx` or `ToolContext` references**

```bash
grep -n "ToolContext\|ctx\." src/fs/mod.rs
```

Expected: no output. If any appear, fix them before continuing.

---

## Task 2: Wire the new module

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/tools/symbol/mod.rs`

- [ ] **Step 1: Register `src/fs` in `src/lib.rs`**

In `src/lib.rs`, add after `pub mod embed;` (line 10):

```rust
pub mod fs;
```

- [ ] **Step 2: Remove `path_helpers` from `src/tools/symbol/mod.rs`**

Find and remove this line:

```rust
mod path_helpers;
```

- [ ] **Step 3: Check it compiles (errors expected in consumer files)**

```bash
cargo check 2>&1 | head -40
```

Expected: `src/fs/mod.rs` compiles cleanly. Errors appear only in consumer files that still reference `super::path_helpers`. That's fine — Task 3 onwards fixes them.

---

## Task 3: Update `src/tools/symbol/edit_code.rs`

**Files:**
- Modify: `src/tools/symbol/edit_code.rs`

- [ ] **Step 1: Replace the import block**

Old (lines 12–14):
```rust
use super::path_helpers::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path, uri_to_path,
};
```

New:
```rust
use crate::fs::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path, uri_to_path,
};
```

- [ ] **Step 2: Update the 4 `resolve_write_path` call sites**

Each occurrence of:
```rust
let full_path = resolve_write_path(ctx, rel_path).await?;
```
becomes:
```rust
let full_path = resolve_write_path(&ctx.agent, rel_path).await?;
```

There are 4 occurrences (lines ~116, ~374, ~430, ~543). Apply to all four.

- [ ] **Step 3: Update the 4 `get_lsp_client` call sites**

Each occurrence of:
```rust
let (client, lang) = get_lsp_client(ctx, &full_path).await?;
```
becomes:
```rust
let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;
```

There are 4 occurrences (lines ~118, ~376, ~432, ~545). Apply to all four.

- [ ] **Step 4: Verify**

```bash
cargo check --message-format short 2>&1 | grep "edit_code"
```

Expected: no errors from `edit_code.rs`.

---

## Task 4: Update `src/tools/symbol/list_overview.rs`

**Files:**
- Modify: `src/tools/symbol/list_overview.rs`

- [ ] **Step 1: Replace the import block**

Old (lines 14–16):
```rust
use super::path_helpers::{
    format_library_path, get_lsp_client, get_path_param, is_glob, resolve_glob,
    resolve_library_roots, resolve_read_path, LspTimer,
};
```

New:
```rust
use crate::fs::{
    format_library_path, get_lsp_client, get_path_param, is_glob, resolve_glob,
    resolve_library_roots, resolve_read_path, LspTimer,
};
```

- [ ] **Step 2: Update `resolve_glob` call site (line ~222)**

Old:
```rust
let files = resolve_glob(ctx, rel_path).await?;
```
New:
```rust
let files = resolve_glob(&ctx.agent, rel_path).await?;
```

- [ ] **Step 3: Update `resolve_read_path` call site (line ~273)**

Old:
```rust
let full_path = resolve_read_path(ctx, rel_path).await?;
```
New:
```rust
let full_path = resolve_read_path(&ctx.agent, rel_path).await?;
```

- [ ] **Step 4: Update `get_lsp_client` call site (line ~279)**

Old:
```rust
let (client, lang) = get_lsp_client(ctx, &full_path).await?;
```
New:
```rust
let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;
```

- [ ] **Step 5: Update the 3 `timer.record` call sites**

Each occurrence of:
```rust
timer.record(ctx, lang, &root).await;
```
becomes:
```rust
timer.record(&*ctx.lsp, lang, &root).await;
```

There are 3 occurrences (lines ~239, ~282, ~430). One uses `raw_lang` instead of `lang` — keep that variable name as-is, only the first argument changes.

- [ ] **Step 6: Verify**

```bash
cargo check --message-format short 2>&1 | grep "list_overview"
```

Expected: no errors.

---

## Task 5: Update `src/tools/symbol/references.rs`

**Files:**
- Modify: `src/tools/symbol/references.rs`

- [ ] **Step 1: Replace the import block**

Old (lines 10–12):
```rust
use super::path_helpers::{
    classify_reference_path, get_lsp_client, path_in_excluded_dir, require_path_param,
    resolve_library_roots, resolve_read_path, uri_to_path, LspTimer,
};
```

New:
```rust
use crate::fs::{
    classify_reference_path, get_lsp_client, path_in_excluded_dir, require_path_param,
    resolve_library_roots, resolve_read_path, uri_to_path, LspTimer,
};
```

- [ ] **Step 2: Update `resolve_read_path` call site (line ~45)**

Old:
```rust
let full_path = resolve_read_path(ctx, rel_path).await?;
```
New:
```rust
let full_path = resolve_read_path(&ctx.agent, rel_path).await?;
```

- [ ] **Step 3: Update `get_lsp_client` call site (line ~49)**

Old:
```rust
let (client, lang) = get_lsp_client(ctx, &full_path).await?;
```
New:
```rust
let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;
```

- [ ] **Step 4: Update `timer.record` call site (line ~54)**

Old:
```rust
timer.record(ctx, raw_lang, &root).await;
```
New:
```rust
timer.record(&*ctx.lsp, raw_lang, &root).await;
```

- [ ] **Step 5: Verify**

```bash
cargo check --message-format short 2>&1 | grep "references"
```

Expected: no errors.

---

## Task 6: Update `src/tools/symbol/symbol_at.rs`

**Files:**
- Modify: `src/tools/symbol/symbol_at.rs`

- [ ] **Step 1: Replace the import block**

Old (lines 11–13):
```rust
use super::path_helpers::{
    get_lsp_client, require_path_param, resolve_read_path, retry_on_mux_disconnect,
    tag_external_path, uri_to_path, LspTimer,
};
```

New:
```rust
use crate::fs::{
    get_lsp_client, require_path_param, resolve_read_path, retry_on_mux_disconnect,
    tag_external_path, uri_to_path, LspTimer,
};
```

- [ ] **Step 2: Update both `resolve_read_path` call sites (lines ~74, ~198)**

Each occurrence of:
```rust
let full_path = resolve_read_path(ctx, &rel_path).await?;
```
becomes:
```rust
let full_path = resolve_read_path(&ctx.agent, &rel_path).await?;
```

- [ ] **Step 3: Update both `get_lsp_client` call sites (lines ~78, ~201)**

Each occurrence of:
```rust
let (client, lang) = get_lsp_client(ctx, &full_path).await?;
```
becomes:
```rust
let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;
```

- [ ] **Step 4: Update both `retry_on_mux_disconnect` call sites (lines ~118, ~240)**

Old:
```rust
let definitions = retry_on_mux_disconnect(ctx, &full_path, client, lang, |c, l| {
```
New:
```rust
let definitions = retry_on_mux_disconnect(&ctx.agent, &*ctx.lsp, &full_path, client, lang, |c, l| {
```

And:
```rust
let hover_text = retry_on_mux_disconnect(ctx, &full_path, client, lang, |c, l| {
```
becomes:
```rust
let hover_text = retry_on_mux_disconnect(&ctx.agent, &*ctx.lsp, &full_path, client, lang, |c, l| {
```

- [ ] **Step 5: Update both `timer.record` call sites (lines ~123, ~246)**

Each occurrence of:
```rust
timer.record(ctx, raw_lang, &root).await;
```
becomes:
```rust
timer.record(&*ctx.lsp, raw_lang, &root).await;
```

- [ ] **Step 6: Verify**

```bash
cargo check --message-format short 2>&1 | grep "symbol_at"
```

Expected: no errors.

---

## Task 7: Update `src/tools/symbol/symbols.rs`

**Files:**
- Modify: `src/tools/symbol/symbols.rs`

- [ ] **Step 1: Replace the import block**

Old (line 20–21):
```rust
use super::path_helpers::{
    format_library_path, get_path_param, is_glob, resolve_glob, resolve_library_roots, LspTimer,
};
```

New:
```rust
use crate::fs::{
    format_library_path, get_path_param, is_glob, resolve_glob, resolve_library_roots, LspTimer,
};
```

- [ ] **Step 2: Update `resolve_glob` call site (line ~231)**

Old:
```rust
resolve_glob(ctx, rel).await?
```
New:
```rust
resolve_glob(&ctx.agent, rel).await?
```

- [ ] **Step 3: Update `timer.record` call site (line ~263)**

Old:
```rust
timer.record(ctx, lang, &root).await;
```
New:
```rust
timer.record(&*ctx.lsp, lang, &root).await;
```

- [ ] **Step 4: Verify**

```bash
cargo check --message-format short 2>&1 | grep "symbols.rs"
```

Expected: no errors.

---

## Task 8: Update `src/tools/symbol/call_graph/mod.rs`

**Files:**
- Modify: `src/tools/symbol/call_graph/mod.rs`

- [ ] **Step 1: Replace the import block**

Old (lines 199–201):
```rust
use crate::tools::symbol::path_helpers::{
    get_lsp_client, require_path_param, resolve_read_path,
};
```

New:
```rust
use crate::fs::{
    get_lsp_client, require_path_param, resolve_read_path,
};
```

- [ ] **Step 2: Update `resolve_read_path` call site (line ~227)**

Old:
```rust
let seed_path = resolve_read_path(ctx, rel_path).await?;
```
New:
```rust
let seed_path = resolve_read_path(&ctx.agent, rel_path).await?;
```

- [ ] **Step 3: Update `get_lsp_client` call site (line ~228)**

Old:
```rust
let (client, lang) = get_lsp_client(ctx, &seed_path).await?;
```
New:
```rust
let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &seed_path).await?;
```

- [ ] **Step 4: Verify**

```bash
cargo check --message-format short 2>&1 | grep "call_graph"
```

Expected: no errors.

---

## Task 9: Update `src/tools/symbol/tests.rs`

**Files:**
- Modify: `src/tools/symbol/tests.rs`

- [ ] **Step 1: Replace the import**

Old (lines 13–15):
```rust
use super::path_helpers::{
    classify_reference_path, format_library_path, resolve_library_roots, tag_external_path,
    uri_to_path,
};
```

New:
```rust
use crate::fs::{
    classify_reference_path, format_library_path, resolve_library_roots, tag_external_path,
    uri_to_path,
};
```

No call site changes needed — these functions already took `&Agent`, not `&ToolContext`.

- [ ] **Step 2: Verify**

```bash
cargo check --message-format short 2>&1 | grep "tests.rs"
```

Expected: no errors.

---

## Task 10: Full validation, delete old file, commit

**Files:**
- Delete: `src/tools/symbol/path_helpers.rs`

- [ ] **Step 1: Confirm zero remaining references to `path_helpers`**

```bash
grep -rn "path_helpers" src/ --include="*.rs"
```

Expected: no output. If anything appears, fix it before continuing.

- [ ] **Step 2: Delete the old file**

```bash
rm src/tools/symbol/path_helpers.rs
```

- [ ] **Step 3: Run the full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass, zero failures. If any fail, do not proceed — diagnose and fix first.

- [ ] **Step 4: Run fmt and clippy**

```bash
cargo fmt && cargo clippy -- -D warnings
```

Expected: zero warnings, zero errors.

- [ ] **Step 5: Update `docs/TODO-phase6-provider-lifts.md`**

In the `### 6.2` section, change the status line to:

```
**Status:** ✅ Complete — shipped 2026-05-02. `src/fs/` created; `path_helpers.rs` deleted.
```

- [ ] **Step 6: Commit**

```bash
git add src/fs/mod.rs src/lib.rs \
  src/tools/symbol/mod.rs \
  src/tools/symbol/edit_code.rs \
  src/tools/symbol/list_overview.rs \
  src/tools/symbol/references.rs \
  src/tools/symbol/symbol_at.rs \
  src/tools/symbol/symbols.rs \
  src/tools/symbol/call_graph/mod.rs \
  src/tools/symbol/tests.rs \
  docs/TODO-phase6-provider-lifts.md
git rm src/tools/symbol/path_helpers.rs
git commit -m "refactor(fs): lift path/LSP helpers to src/fs/ (Phase 6.2)

Replace ToolContext coupling with &Agent / &dyn LspProvider parameters.
Delete src/tools/symbol/path_helpers.rs; all callers updated."
```

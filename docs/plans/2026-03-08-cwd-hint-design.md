# CWD Hint in activate_project

**Date:** 2026-03-08
**Status:** Approved

## Problem

The LLM consuming codescout tools has no persistent awareness of its current working
directory. The MCP system prompt (via `get_info`) includes the project path at startup,
but after `activate_project` switches to a library or worktree, the LLM loses track of
where it is. When switching back, there's no signal that it has "returned home."

## Design

### New state: `home_root`

Add `home_root: Option<PathBuf>` to `AgentInner`. Set it once on the first `activate()`
call. Never mutate it after that. This represents the project the server started with.

Add two accessors on `Agent`:
- `home_root() -> Option<PathBuf>` — returns the home project path
- `is_home() -> bool` — true when the active project root equals `home_root`

### Modified `activate_project` response

Add a `hint` string field to the JSON response. The hint varies by scenario:

| Scenario | `hint` value |
|----------|-------------|
| First activation (no home yet) | `"CWD: /path/to/project"` |
| Switching to a different project | `"Switched project. CWD: /path/to/lib (home: /path/to/original)"` |
| Returning to home project | `"Returned to original project. CWD: /path/to/project"` |

The hint is also included in `format_compact` output so it appears in the tool's
compact representation.

### What changes

| File | Change |
|------|--------|
| `src/agent.rs` | Add `home_root` field, set on first `activate()`. Add `is_home()`, `home_root()`. |
| `src/tools/config.rs` | Build hint in `ActivateProject::call`, include in response. Update `format_activate_project`. |
| `src/agent.rs` | Tests: home_root set once, is_home true/false. |
| `src/tools/config.rs` | Tests: hint text for first/switch/return scenarios. |

### What doesn't change

- `onboarding` — always local, CWD is implicit. No remote onboarding allowed.
- `server_instructions.md` — already has `at {path}` in Project Status section.
- `build_server_instructions` — unchanged.
- No new tools, no new config fields.

### Edge cases

- **Server starts without a project** (no `--project` flag): `home_root` stays `None`
  until the first `activate_project` call, which sets it.
- **`activate_project` called with the same path twice**: second call is a no-op
  from the home tracking perspective. Hint says "CWD: ..." (home case).
- **Worktree activation**: worktree path differs from home, so hint correctly shows
  "Switched project" with the home reminder.

---

## Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a CWD hint to `activate_project` responses so the LLM always knows its working directory and whether it's at the home project.

**Architecture:** Add `home_root: Option<PathBuf>` to `AgentInner`, set once on first activation (either at startup or via `activate_project`). The `ActivateProject` tool reads home state from the agent to build a context-aware hint string in its response.

**Tech Stack:** Rust, no new dependencies.

---

### Task 1: Add `home_root` field to `AgentInner` + accessors

**Files:**
- Modify: `src/agent.rs:43-46` (AgentInner struct)
- Modify: `src/agent.rs:56-88` (Agent::new — set home_root from initial project)
- Modify: `src/agent.rs:90-108` (Agent::activate — set home_root on first call)
- Add methods to `impl Agent` block

**Step 1: Write the failing tests**

Add to `src/agent.rs` test module:

```rust
#[tokio::test]
async fn home_root_set_from_initial_project() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let home = agent.home_root().await;
    assert_eq!(home, Some(dir.path().to_path_buf()));
}

#[tokio::test]
async fn home_root_none_without_project() {
    let agent = Agent::new(None).await.unwrap();
    assert_eq!(agent.home_root().await, None);
}

#[tokio::test]
async fn home_root_set_on_first_activate() {
    let agent = Agent::new(None).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    agent.activate(dir.path().to_path_buf()).await.unwrap();
    assert_eq!(agent.home_root().await, Some(dir.path().to_path_buf()));
}

#[tokio::test]
async fn home_root_not_changed_by_second_activate() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    agent.activate(dir2.path().to_path_buf()).await.unwrap();
    // home_root stays as dir1
    assert_eq!(agent.home_root().await, Some(dir1.path().to_path_buf()));
}

#[tokio::test]
async fn is_home_true_when_at_home() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    assert!(agent.is_home().await);
}

#[tokio::test]
async fn is_home_false_after_switching() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    agent.activate(dir2.path().to_path_buf()).await.unwrap();
    assert!(!agent.is_home().await);
}

#[tokio::test]
async fn is_home_true_after_returning() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    agent.activate(dir2.path().to_path_buf()).await.unwrap();
    agent.activate(dir1.path().to_path_buf()).await.unwrap();
    assert!(agent.is_home().await);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p code-explorer home_root is_home -- --nocapture 2>&1 | head -40`
Expected: Compilation errors — `home_root()` and `is_home()` don't exist yet.

**Step 3: Add `home_root` field and accessors**

In `AgentInner` (L43-46), add the field:
```rust
pub struct AgentInner {
    pub active_project: Option<ActiveProject>,
    pub project_explicitly_activated: bool,
    pub home_root: Option<PathBuf>,
}
```

In `Agent::new` (L80-86), initialize from the startup project:
```rust
let home_root = active_project.as_ref().map(|p| p.root.clone());
// ...
Ok(Self {
    inner: Arc::new(RwLock::new(AgentInner {
        active_project,
        project_explicitly_activated,
        home_root,
    })),
    // ...
})
```

In `Agent::activate` (L90-108), set on first call only:
```rust
// After setting inner.active_project:
if inner.home_root.is_none() {
    inner.home_root = Some(root.clone());
}
```
Note: `root` has been moved into `ActiveProject` by then — capture it before the move:
```rust
let root_clone = root.clone();
// ... existing code that moves root into ActiveProject ...
if inner.home_root.is_none() {
    inner.home_root = Some(root_clone);
}
```

Add accessors to `impl Agent`:
```rust
pub async fn home_root(&self) -> Option<PathBuf> {
    self.inner.read().await.home_root.clone()
}

pub async fn is_home(&self) -> bool {
    let inner = self.inner.read().await;
    match (&inner.active_project, &inner.home_root) {
        (Some(project), Some(home)) => project.root == *home,
        (None, None) => true,
        _ => false,
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p code-explorer home_root is_home -- --nocapture`
Expected: All 7 new tests pass.

**Step 5: Commit**

```bash
git add src/agent.rs
git commit -m "feat: add home_root tracking to Agent for CWD awareness"
```

---

### Task 2: Add CWD hint to `activate_project` response

**Files:**
- Modify: `src/tools/config.rs:28-49` (ActivateProject::call)
- Modify: `src/tools/config.rs:241-250` (format_activate_project)

**Step 1: Write the failing tests**

Add to `src/tools/config.rs` test module:

```rust
#[tokio::test]
async fn activate_includes_cwd_hint() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::new(None).await.unwrap();
    let lsp = lsp();
    let ctx = crate::tools::test_helpers::make_ctx_with(&agent, &lsp);
    let input = json!({ "path": dir.path().to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.starts_with("CWD: "), "hint should start with CWD: but was: {hint}");
    assert!(hint.contains(dir.path().to_str().unwrap()));
}

#[tokio::test]
async fn activate_hint_shows_switched_when_away_from_home() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    let lsp = lsp();
    let ctx = crate::tools::test_helpers::make_ctx_with(&agent, &lsp);
    let input = json!({ "path": dir2.path().to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.starts_with("Switched project."), "hint: {hint}");
    assert!(hint.contains(dir2.path().to_str().unwrap()), "hint should contain new path");
    assert!(hint.contains(dir1.path().to_str().unwrap()), "hint should contain home path");
}

#[tokio::test]
async fn activate_hint_shows_returned_when_back_home() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    let lsp = lsp();
    let ctx = crate::tools::test_helpers::make_ctx_with(&agent, &lsp);
    // Switch away
    let input = json!({ "path": dir2.path().to_str().unwrap() });
    ActivateProject.call(input, &ctx).await.unwrap();
    // Return home
    let input = json!({ "path": dir1.path().to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.starts_with("Returned to original project."), "hint: {hint}");
    assert!(hint.contains(dir1.path().to_str().unwrap()));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p code-explorer activate_includes_cwd activate_hint -- --nocapture`
Expected: FAIL — no `hint` field in response.

**Step 3: Build the hint in `ActivateProject::call`**

After the existing `ctx.agent.activate(root).await?;` and `config` retrieval, add:

```rust
// Build CWD hint
let project_root_str = ctx
    .agent
    .with_project(|p| Ok(p.root.display().to_string()))
    .await?;
let home = ctx.agent.home_root().await;
let is_home = ctx.agent.is_home().await;

let hint = match (is_home, home) {
    (true, _) | (_, None) => {
        format!("CWD: {}", project_root_str)
    }
    (false, Some(home_path)) => {
        format!(
            "Switched project. CWD: {} (home: {})",
            project_root_str,
            home_path.display()
        )
    }
};

// Special case: detect "returned home" — is_home is true but we just activated,
// and home was already set (meaning this isn't the first activation).
// Actually is_home covers this: if we just activated home, is_home is true.
// We need to distinguish "first activation" from "returned home".
// If home was already set before this activate call, and is_home is now true,
// then we returned. But we can't know "was home set before" easily.
```

Wait — there's a subtlety. We need to know whether this is the **first** activation or a **return**. The cleanest approach: check if `home_root` was already set **before** `activate()` was called. Since `activate()` sets it, we need to capture it before.

Revised approach in `ActivateProject::call`:

```rust
let had_home = ctx.agent.home_root().await.is_some();

ctx.agent.activate(root).await?;

let project_root_str = ctx
    .agent
    .with_project(|p| Ok(p.root.display().to_string()))
    .await?;
let is_home = ctx.agent.is_home().await;
let home = ctx.agent.home_root().await;

let hint = if !had_home {
    // First activation ever — this becomes home
    format!("CWD: {}", project_root_str)
} else if is_home {
    // Returned to home project
    format!("Returned to original project. CWD: {}", project_root_str)
} else {
    // Switched to a different project
    format!(
        "Switched project. CWD: {} (home: {})",
        project_root_str,
        home.as_ref().map(|p| p.display().to_string()).unwrap_or_default()
    )
};
```

Update the return value:
```rust
Ok(json!({ "status": "ok", "activated": config, "hint": hint }))
```

**Step 4: Update `format_activate_project` to include hint**

```rust
fn format_activate_project(result: &Value) -> String {
    let root = result["activated"]["project_root"]
        .as_str()
        .or_else(|| result["path"].as_str())
        .unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    if let Some(hint) = result["hint"].as_str() {
        format!("activated · {name} · {hint}")
    } else {
        format!("activated · {name}")
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p code-explorer activate_includes_cwd activate_hint -- --nocapture`
Expected: All 3 new tests pass.

**Step 6: Run full test suite + lint**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: Clean.

**Step 7: Commit**

```bash
git add src/tools/config.rs
git commit -m "feat: add CWD hint to activate_project response"
```

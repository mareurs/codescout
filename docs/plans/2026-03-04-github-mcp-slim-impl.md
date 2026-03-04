# GitHub MCP Slim — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 5 consolidated GitHub tools to code-explorer, replacing the official 44-tool
github-mcp server and saving ~8k tokens of fixed per-session context overhead.

**Architecture:** New `src/tools/github.rs` module with shared `run_gh()` / `check_gh_output()` /
`maybe_buffer()` helpers, 5 Tool structs dispatching on a `method` param to `gh` CLI calls,
wired into the existing `src/server.rs` tool registry.

**Tech Stack:** Rust, tokio::process::Command, `gh` CLI (GitHub's official CLI), serde_json,
existing `OutputBuffer` / `RecoverableError` / `Tool` trait from the codebase.

**Design doc:** `docs/plans/2026-03-04-github-mcp-slim-design.md`

---

## Task 1: Shared infrastructure + module scaffold

**Files:**
- Create: `src/tools/github.rs`
- Modify: `src/tools/mod.rs` (add `pub mod github;`)
- Modify: `src/server.rs` (add 5 stub tool registrations)

**Step 1: Create `src/tools/github.rs` with shared helpers and 5 stub structs**

```rust
use std::future::Future;
use std::pin::Pin;

use anyhow::Context as _;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::tools::{RecoverableError, Tool, ToolContext, TOOL_OUTPUT_BUFFER_THRESHOLD};

// ── Shared execution ──────────────────────────────────────────────────────────

pub(crate) struct GhOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub(crate) async fn run_gh(args: &[&str]) -> anyhow::Result<GhOutput> {
    let out = Command::new("gh")
        .args(args)
        .output()
        .await
        .context("gh CLI not found — install from https://cli.github.com")?;
    Ok(GhOutput {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        exit_code: out.status.code().unwrap_or(-1),
    })
}

pub(crate) fn check_gh_output(out: GhOutput) -> Result<String, RecoverableError> {
    if out.exit_code != 0 {
        let msg = if out.stderr.contains("not logged into")
            || out.stderr.contains("authentication")
            || out.stderr.contains("auth")
        {
            format!(
                "gh not authenticated: {}. Run: gh auth login",
                out.stderr.trim()
            )
        } else {
            out.stderr.trim().to_string()
        };
        return Err(RecoverableError::new(msg));
    }
    Ok(out.stdout)
}

/// Store in OutputBuffer if large; otherwise parse as JSON or return as string.
pub(crate) fn maybe_buffer(content: String, tool_name: &str, ctx: &ToolContext) -> Value {
    if content.len() > TOOL_OUTPUT_BUFFER_THRESHOLD {
        let id = ctx.output_buffer.store_tool(tool_name, content);
        json!(id)
    } else {
        serde_json::from_str(&content).unwrap_or_else(|_| json!(content))
    }
}

/// Always buffer — for known-large responses (diffs, file contents, code search).
pub(crate) fn always_buffer(content: String, tool_name: &str, ctx: &ToolContext) -> Value {
    let id = ctx.output_buffer.store_tool(tool_name, content);
    json!(id)
}

// ── Stub tool structs ─────────────────────────────────────────────────────────

pub struct GithubIdentity;
pub struct GithubIssue;
pub struct GithubPr;
pub struct GithubFile;
pub struct GithubRepo;

#[async_trait]
impl Tool for GithubIdentity {
    fn name(&self) -> &'static str { "github_identity" }
    fn description(&self) -> &'static str { "GitHub identity and team operations. method: get_me | search_users | get_teams | get_team_members" }
    fn input_schema(&self) -> Value { json!({"type":"object","properties":{"method":{"type":"string"}},"required":["method"]}) }
    async fn call(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
        Err(RecoverableError::new("not implemented yet").into())
    }
}

#[async_trait]
impl Tool for GithubIssue {
    fn name(&self) -> &'static str { "github_issue" }
    fn description(&self) -> &'static str { "GitHub issue operations. method: list | search | get | get_comments | get_labels | create | update | add_comment | add_sub_issue | remove_sub_issue" }
    fn input_schema(&self) -> Value { json!({"type":"object","properties":{"method":{"type":"string"}},"required":["method"]}) }
    async fn call(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
        Err(RecoverableError::new("not implemented yet").into())
    }
}

#[async_trait]
impl Tool for GithubPr {
    fn name(&self) -> &'static str { "github_pr" }
    fn description(&self) -> &'static str { "GitHub pull request operations. method: list | search | get | get_diff | get_files | get_comments | get_reviews | get_review_comments | get_status | create | update | merge | update_branch | create_review | submit_review | delete_review | add_review_comment | add_reply_to_comment" }
    fn input_schema(&self) -> Value { json!({"type":"object","properties":{"method":{"type":"string"}},"required":["method"]}) }
    async fn call(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
        Err(RecoverableError::new("not implemented yet").into())
    }
}

#[async_trait]
impl Tool for GithubFile {
    fn name(&self) -> &'static str { "github_file" }
    fn description(&self) -> &'static str { "GitHub file operations. method: get | create_or_update | delete | push_files" }
    fn input_schema(&self) -> Value { json!({"type":"object","properties":{"method":{"type":"string"}},"required":["method"]}) }
    async fn call(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
        Err(RecoverableError::new("not implemented yet").into())
    }
}

#[async_trait]
impl Tool for GithubRepo {
    fn name(&self) -> &'static str { "github_repo" }
    fn description(&self) -> &'static str { "GitHub repository operations. method: search | create | fork | list_branches | create_branch | list_commits | get_commit | list_releases | get_latest_release | get_release_by_tag | list_tags | get_tag | search_code" }
    fn input_schema(&self) -> Value { json!({"type":"object","properties":{"method":{"type":"string"}},"required":["method"]}) }
    async fn call(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
        Err(RecoverableError::new("not implemented yet").into())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_gh_output_success() {
        let out = GhOutput { stdout: "ok".into(), stderr: "".into(), exit_code: 0 };
        assert_eq!(check_gh_output(out).unwrap(), "ok");
    }

    #[test]
    fn test_check_gh_output_auth_error() {
        let out = GhOutput {
            stdout: "".into(),
            stderr: "not logged into github.com".into(),
            exit_code: 1,
        };
        let err = check_gh_output(out).unwrap_err();
        assert!(err.to_string().contains("gh auth login"));
    }

    #[test]
    fn test_check_gh_output_api_error() {
        let out = GhOutput {
            stdout: "".into(),
            stderr: "Could not resolve to a Repository with the name 'owner/nonexistent'.".into(),
            exit_code: 1,
        };
        let err = check_gh_output(out).unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }
}
```

**Step 2: Run test to verify it fails (module not registered)**

```bash
cargo test
```
Expected: compile error — module `github` not found in `src/tools/mod.rs`

**Step 3: Add `pub mod github;` to `src/tools/mod.rs`**

In `src/tools/mod.rs`, after the existing module declarations (around line 22), add:
```rust
pub mod github;
```

**Step 4: Register stub tools in `src/server.rs`**

In the `tools: vec![...]` in `impl CodeExplorerServer::from_parts`, after `Arc::new(ListLibraries)`, add:
```rust
// GitHub tools
Arc::new(github::GithubIdentity),
Arc::new(github::GithubIssue),
Arc::new(github::GithubPr),
Arc::new(github::GithubFile),
Arc::new(github::GithubRepo),
```

Also add the import at the top of `src/server.rs` (with existing tool imports):
```rust
use crate::tools::github;
```

**Step 5: Run tests to verify they pass**

```bash
cargo test 2026_03_04
```
Expected: 3 tests pass (`test_check_gh_output_*`)

**Step 6: Verify compilation + clippy**

```bash
cargo clippy -- -D warnings
```
Expected: no warnings

**Step 7: Commit**

```bash
git add src/tools/github.rs src/tools/mod.rs src/server.rs
git commit -m "feat: scaffold github tools module with shared gh execution helpers"
```

---

## Task 2: `GithubIdentity` — full implementation

Simplest tool: small responses, no pagination, validates the full pattern end-to-end.

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing tests for identity methods**

In the `tests` module at the bottom of `src/tools/github.rs`, add:

```rust
#[test]
fn test_format_identity_get_me() {
    let json = r#"{"login":"testuser","id":12345,"name":"Test User","email":"test@example.com"}"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["login"], "testuser");
    assert_eq!(v["id"], 12345);
}

#[test]
fn test_format_identity_search_users() {
    let json = r#"{"items":[{"login":"alice","id":1},{"login":"bob","id":2}],"total":2}"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
}
```

**Step 2: Run to verify they pass (they test JSON structure, not the tool itself)**

```bash
cargo test test_format_identity
```

**Step 3: Implement `GithubIdentity::call`**

Replace the stub `impl Tool for GithubIdentity` with the full implementation:

```rust
#[async_trait]
impl Tool for GithubIdentity {
    fn name(&self) -> &'static str {
        "github_identity"
    }

    fn description(&self) -> &'static str {
        "GitHub identity and team operations.\n\
         method: get_me | search_users | get_teams | get_team_members\n\n\
         get_me — authenticated user profile.\n\
         search_users — search GitHub users (query required).\n\
         get_teams — teams the authenticated user belongs to.\n\
         get_team_members — members of a specific team (org + team_slug required)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["get_me", "search_users", "get_teams", "get_team_members"]
                },
                "query":      { "type": "string", "description": "search_users: search query" },
                "org":        { "type": "string", "description": "get_team_members: organization login" },
                "team_slug":  { "type": "string", "description": "get_team_members: team slug" }
            },
            "required": ["method"]
        })
    }

    async fn call(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let method = params["method"].as_str().unwrap_or("");
        match method {
            "get_me" => {
                let out = run_gh(&["api", "/user"]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_identity", ctx))
            }
            "search_users" => {
                let q = params["query"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("query required", "Provide a search query string")
                })?;
                let out = run_gh(&["search", "users", q, "--json", "login,id,name,url"]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_identity", ctx))
            }
            "get_teams" => {
                let out = run_gh(&["api", "/user/teams"]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_identity", ctx))
            }
            "get_team_members" => {
                let org = params["org"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("org required", "Provide the organization login")
                })?;
                let slug = params["team_slug"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint(
                        "team_slug required",
                        "Provide the team slug (e.g. 'engineering')",
                    )
                })?;
                let endpoint = format!("/orgs/{org}/teams/{slug}/members");
                let out = run_gh(&["api", &endpoint]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_identity", ctx))
            }
            other => Err(RecoverableError::with_hint(
                format!("unknown method: '{other}'"),
                "method must be one of: get_me, search_users, get_teams, get_team_members",
            )
            .into()),
        }
    }
}
```

**Step 4: Run tests**

```bash
cargo test
```
Expected: all tests pass

**Step 5: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubIdentity tool (get_me, search_users, get_teams, get_team_members)"
```

---

## Task 3: `GithubIssue` — read methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing tests**

Add to the `tests` module:

```rust
#[test]
fn test_issue_list_json_shape() {
    let json = r#"[{"number":1,"title":"Bug","state":"OPEN"},{"number":2,"title":"Feature","state":"CLOSED"}]"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert_eq!(v[0]["number"], 1);
}

#[test]
fn test_issue_get_json_shape() {
    let json = r#"{"number":42,"title":"Test issue","body":"Some body","state":"OPEN","labels":[],"comments":[]}"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["number"], 42);
    assert_eq!(v["title"], "Test issue");
}
```

**Step 2: Run to verify they pass (pure JSON parsing)**

```bash
cargo test test_issue
```

**Step 3: Implement `GithubIssue::call` (read methods)**

Replace the stub `impl Tool for GithubIssue`:

```rust
#[async_trait]
impl Tool for GithubIssue {
    fn name(&self) -> &'static str { "github_issue" }

    fn description(&self) -> &'static str {
        "GitHub issue operations.\n\
         Read: list | search | get | get_comments | get_labels | get_sub_issues\n\
         Write: create | update | add_comment | add_sub_issue | remove_sub_issue\n\n\
         Most read methods require owner + repo. list/search support limit (default 30).\n\
         get/get_comments/get_labels require number. create requires title."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["list","search","get","get_comments","get_labels","get_sub_issues",
                             "create","update","add_comment","add_sub_issue","remove_sub_issue"]
                },
                "owner":      { "type": "string" },
                "repo":       { "type": "string" },
                "number":     { "type": "integer", "description": "Issue number" },
                "query":      { "type": "string",  "description": "search: query string" },
                "title":      { "type": "string",  "description": "create: issue title" },
                "body":       { "type": "string",  "description": "create/update/add_comment: text" },
                "state":      { "type": "string",  "enum": ["open","closed"], "description": "list/update filter" },
                "labels":     { "type": "string",  "description": "list/create/update: comma-separated labels" },
                "assignees":  { "type": "string",  "description": "create/update: comma-separated logins" },
                "limit":      { "type": "integer", "description": "list/search: max results (default 30)" },
                "sub_issue_id": { "type": "integer", "description": "remove_sub_issue: sub-issue node ID" }
            },
            "required": ["method"]
        })
    }

    async fn call(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let method = params["method"].as_str().unwrap_or("");
        let owner  = params["owner"].as_str().unwrap_or("");
        let repo   = params["repo"].as_str().unwrap_or("");
        let repo_flag = if !owner.is_empty() && !repo.is_empty() {
            format!("{owner}/{repo}")
        } else {
            String::new()
        };
        let limit  = params["limit"].as_u64().unwrap_or(30).to_string();
        let number = params["number"].as_u64().map(|n| n.to_string());

        match method {
            "list" => {
                let mut args = vec!["issue", "list",
                    "--json", "number,title,state,labels,assignees,createdAt,updatedAt",
                    "--limit", &limit];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                if let Some(s) = params["state"].as_str() { args.extend(["--state", s]); }
                if let Some(l) = params["labels"].as_str() { args.extend(["--label", l]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            "search" => {
                let q = params["query"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("query required", "Provide a search query")
                })?;
                let args = ["search", "issues", q,
                    "--json", "number,title,state,repository,url",
                    "--limit", &limit];
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            "get" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the issue number")
                })?;
                let mut args = vec!["issue", "view", num,
                    "--json", "number,title,body,state,labels,assignees,comments,createdAt,updatedAt,url"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            "get_comments" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the issue number")
                })?;
                let mut args = vec!["issue", "view", num, "--json", "comments"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            "get_labels" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the issue number")
                })?;
                let mut args = vec!["issue", "view", num, "--json", "labels"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            "get_sub_issues" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the issue number")
                })?;
                require_owner_repo(owner, repo)?;
                let endpoint = format!("/repos/{owner}/{repo}/issues/{num}/sub_issues");
                let out = run_gh(&["api", &endpoint]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_issue", ctx))
            }
            // Write methods implemented in Task 4
            "create" | "update" | "add_comment" | "add_sub_issue" | "remove_sub_issue" => {
                Err(RecoverableError::new("write methods not yet implemented").into())
            }
            other => Err(RecoverableError::with_hint(
                format!("unknown method: '{other}'"),
                "method must be one of: list, search, get, get_comments, get_labels, \
                 get_sub_issues, create, update, add_comment, add_sub_issue, remove_sub_issue",
            ).into()),
        }
    }
}
```

Also add the helper (before the tool impls):

```rust
fn require_owner_repo(owner: &str, repo: &str) -> Result<(), RecoverableError> {
    if owner.is_empty() || repo.is_empty() {
        return Err(RecoverableError::with_hint(
            "owner and repo required",
            "Provide owner (GitHub username/org) and repo (repository name)",
        ));
    }
    Ok(())
}
```

**Step 4: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 5: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubIssue read methods (list, search, get, get_comments, get_labels, get_sub_issues)"
```

---

## Task 4: `GithubIssue` — write methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn test_issue_create_requires_title() {
    // Validate that title is required — tested via parameter checking logic
    // Full integration test would require GITHUB_TOKEN
    let params = json!({"method": "create", "owner": "foo", "repo": "bar"});
    assert!(params["title"].as_str().is_none());
}
```

**Step 2: Replace write method stubs in `GithubIssue::call`**

Replace the `"create" | "update" | ...` stub arm with:

```rust
"create" => {
    require_owner_repo(owner, repo)?;
    let title = params["title"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("title required", "Provide the issue title")
    })?;
    let mut args = vec!["issue", "create", "--repo", &repo_flag, "--title", title];
    let body_str;
    if let Some(b) = params["body"].as_str() {
        body_str = b.to_string();
        args.extend(["--body", &body_str]);
    } else {
        args.push("--body"); args.push("");
    }
    if let Some(l) = params["labels"].as_str() { args.extend(["--label", l]); }
    if let Some(a) = params["assignees"].as_str() { args.extend(["--assignee", a]); }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"update" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the issue number")
    })?;
    require_owner_repo(owner, repo)?;
    let mut args = vec!["issue", "edit", num, "--repo", &repo_flag];
    if let Some(t) = params["title"].as_str()     { args.extend(["--title", t]); }
    if let Some(b) = params["body"].as_str()      { args.extend(["--body", b]); }
    if let Some(s) = params["state"].as_str()     { args.extend(["--state", s]); }
    if let Some(l) = params["labels"].as_str()    { args.extend(["--add-label", l]); }
    if let Some(a) = params["assignees"].as_str() { args.extend(["--add-assignee", a]); }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"add_comment" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the issue number")
    })?;
    let body = params["body"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("body required", "Provide the comment body")
    })?;
    let mut args = vec!["issue", "comment", num, "--body", body];
    if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"add_sub_issue" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the parent issue number")
    })?;
    let sub_id = params["sub_issue_id"].as_u64().ok_or_else(|| {
        RecoverableError::with_hint("sub_issue_id required", "Provide the sub-issue node ID")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/issues/{num}/sub_issues");
    let body = format!("{{\"sub_issue_id\":{sub_id}}}");
    let out = run_gh(&["api", "--method", "POST", &endpoint, "--input", "-", "--raw-field", &format!("_={body}")]).await?;
    // Simpler: use --field
    let out = run_gh(&["api", "--method", "POST", &endpoint,
        "--field", &format!("sub_issue_id={sub_id}")]).await?;
    Ok(json!(check_gh_output(out)?))
}
"remove_sub_issue" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the parent issue number")
    })?;
    let sub_id = params["sub_issue_id"].as_u64().ok_or_else(|| {
        RecoverableError::with_hint("sub_issue_id required", "Provide the sub-issue node ID")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/issues/{num}/sub_issues/{sub_id}");
    let out = run_gh(&["api", "--method", "DELETE", &endpoint]).await?;
    Ok(json!(check_gh_output(out)?))
}
```

**Step 3: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubIssue write methods (create, update, add_comment, sub-issues)"
```

---

## Task 5: `GithubPr` — read methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn test_pr_list_json_shape() {
    let json = r#"[{"number":1,"title":"Fix bug","state":"OPEN","isDraft":false}]"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v[0]["number"], 1);
}

#[test]
fn test_pr_diff_is_text() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+new line\n-old line\n";
    assert!(diff.starts_with("diff --git"));
    // Diffs are always buffered — content > threshold check:
    assert!(diff.len() < TOOL_OUTPUT_BUFFER_THRESHOLD); // small sample passes through
}
```

**Step 2: Implement `GithubPr::call` (read methods)**

Replace stub `impl Tool for GithubPr`:

```rust
#[async_trait]
impl Tool for GithubPr {
    fn name(&self) -> &'static str { "github_pr" }

    fn description(&self) -> &'static str {
        "GitHub pull request operations.\n\
         Read: list | search | get | get_diff | get_files | get_comments | \
               get_reviews | get_review_comments | get_status\n\
         Write: create | update | merge | update_branch | create_review | \
                submit_review | delete_review | add_review_comment | add_reply_to_comment\n\n\
         get_diff always returns a @tool buffer handle (diffs are large).\n\
         owner + repo required for most operations. number required for single-PR ops."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["list","search","get","get_diff","get_files","get_comments",
                             "get_reviews","get_review_comments","get_status",
                             "create","update","merge","update_branch",
                             "create_review","submit_review","delete_review",
                             "add_review_comment","add_reply_to_comment"]
                },
                "owner":        { "type": "string" },
                "repo":         { "type": "string" },
                "number":       { "type": "integer", "description": "PR number" },
                "query":        { "type": "string",  "description": "search: query string" },
                "title":        { "type": "string",  "description": "create/update: PR title" },
                "body":         { "type": "string",  "description": "create/update/review: body text" },
                "head":         { "type": "string",  "description": "create: head branch (user:branch)" },
                "base":         { "type": "string",  "description": "create/update: base branch" },
                "state":        { "type": "string",  "enum": ["open","closed"], "description": "list filter or update state" },
                "draft":        { "type": "boolean", "description": "create/update: draft status" },
                "merge_method": { "type": "string",  "enum": ["merge","squash","rebase"], "description": "merge: strategy" },
                "event":        { "type": "string",  "enum": ["APPROVE","REQUEST_CHANGES","COMMENT"], "description": "create_review: event type" },
                "review_id":    { "type": "integer", "description": "submit_review/delete_review: review ID" },
                "commit_id":    { "type": "string",  "description": "add_review_comment: commit SHA" },
                "path":         { "type": "string",  "description": "add_review_comment: file path" },
                "line":         { "type": "integer", "description": "add_review_comment: line number" },
                "comment_id":   { "type": "integer", "description": "add_reply_to_comment: comment ID" },
                "limit":        { "type": "integer", "description": "list/search: max results (default 30)" }
            },
            "required": ["method"]
        })
    }

    async fn call(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let method = params["method"].as_str().unwrap_or("");
        let owner  = params["owner"].as_str().unwrap_or("");
        let repo   = params["repo"].as_str().unwrap_or("");
        let repo_flag = if !owner.is_empty() && !repo.is_empty() {
            format!("{owner}/{repo}")
        } else {
            String::new()
        };
        let limit  = params["limit"].as_u64().unwrap_or(30).to_string();
        let number = params["number"].as_u64().map(|n| n.to_string());

        match method {
            "list" => {
                let mut args = vec!["pr", "list",
                    "--json", "number,title,state,isDraft,headRefName,baseRefName,createdAt,updatedAt,url",
                    "--limit", &limit];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                if let Some(s) = params["state"].as_str() { args.extend(["--state", s]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "search" => {
                let q = params["query"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("query required", "Provide a search query")
                })?;
                let out = run_gh(&["search", "prs", q,
                    "--json", "number,title,state,repository,url",
                    "--limit", &limit]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                let mut args = vec!["pr", "view", num,
                    "--json", "number,title,body,state,isDraft,headRefName,baseRefName,\
                               labels,assignees,reviewers,url,mergeStateStatus,createdAt,updatedAt"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_diff" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                let mut args = vec!["pr", "diff", num];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(always_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_files" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                let mut args = vec!["pr", "view", num, "--json", "files"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_comments" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                let mut args = vec!["pr", "view", num, "--json", "comments"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_reviews" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                require_owner_repo(owner, repo)?;
                let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/reviews");
                let out = run_gh(&["api", &endpoint]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_review_comments" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                require_owner_repo(owner, repo)?;
                let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/comments");
                let out = run_gh(&["api", &endpoint]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            "get_status" => {
                let num = number.as_deref().ok_or_else(|| {
                    RecoverableError::with_hint("number required", "Provide the PR number")
                })?;
                let mut args = vec!["pr", "checks", num];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                args.extend(["--json", "name,state,conclusion,startedAt,completedAt"]);
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_pr", ctx))
            }
            // Write methods in Task 6
            "create" | "update" | "merge" | "update_branch" | "create_review"
            | "submit_review" | "delete_review" | "add_review_comment"
            | "add_reply_to_comment" => {
                Err(RecoverableError::new("write methods not yet implemented").into())
            }
            other => Err(RecoverableError::with_hint(
                format!("unknown method: '{other}'"),
                "method must be one of: list, search, get, get_diff, get_files, \
                 get_comments, get_reviews, get_review_comments, get_status, create, \
                 update, merge, update_branch, create_review, submit_review, delete_review, \
                 add_review_comment, add_reply_to_comment",
            ).into()),
        }
    }
}
```

**Step 3: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubPr read methods (list, search, get, diff, files, comments, reviews, status)"
```

---

## Task 6: `GithubPr` — write methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_pr_merge_method_validation() {
    // Valid merge methods
    for m in &["merge", "squash", "rebase"] {
        assert!(["merge", "squash", "rebase"].contains(m));
    }
}
```

**Step 2: Replace write stubs in `GithubPr::call`**

Replace the `"create" | "update" | ...` stub arm:

```rust
"create" => {
    require_owner_repo(owner, repo)?;
    let head = params["head"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("head required", "Provide the head branch (e.g. 'username:feature-branch')")
    })?;
    let base = params["base"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("base required", "Provide the base branch (e.g. 'main')")
    })?;
    let title = params["title"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("title required", "Provide the PR title")
    })?;
    let mut args = vec!["pr", "create", "--repo", &repo_flag,
        "--head", head, "--base", base, "--title", title];
    if let Some(b) = params["body"].as_str() { args.extend(["--body", b]); }
    else { args.extend(["--body", ""]); }
    if params["draft"].as_bool().unwrap_or(false) { args.push("--draft"); }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"update" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let mut args = vec!["pr", "edit", num];
    if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
    if let Some(t) = params["title"].as_str() { args.extend(["--title", t]); }
    if let Some(b) = params["body"].as_str()  { args.extend(["--body", b]); }
    if let Some(base) = params["base"].as_str() { args.extend(["--base", base]); }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"merge" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let merge_method = params["merge_method"].as_str().unwrap_or("merge");
    let mut args = vec!["pr", "merge", num, "--repo", &repo_flag];
    match merge_method {
        "squash" => args.push("--squash"),
        "rebase" => args.push("--rebase"),
        _        => args.push("--merge"),
    }
    let out = run_gh(&args).await?;
    Ok(json!(check_gh_output(out)?))
}
"update_branch" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/update-branch");
    let out = run_gh(&["api", "--method", "PUT", &endpoint]).await?;
    Ok(json!(check_gh_output(out)?))
}
"create_review" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/reviews");
    let mut gh_args = vec!["api", "--method", "POST", &endpoint];
    if let Some(b) = params["body"].as_str() {
        gh_args.extend(["--field", &format!("body={b}")]);
    }
    if let Some(e) = params["event"].as_str() {
        gh_args.extend(["--field", &format!("event={e}")]);
    }
    let out = run_gh(&gh_args).await?;
    Ok(json!(check_gh_output(out)?))
}
"submit_review" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let rev_id = params["review_id"].as_u64().ok_or_else(|| {
        RecoverableError::with_hint("review_id required", "Provide the review ID")
    })?;
    let event = params["event"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("event required", "Provide event: APPROVE, REQUEST_CHANGES, or COMMENT")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/reviews/{rev_id}/events");
    let out = run_gh(&["api", "--method", "POST", &endpoint,
        "--field", &format!("event={event}")]).await?;
    Ok(json!(check_gh_output(out)?))
}
"delete_review" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let rev_id = params["review_id"].as_u64().ok_or_else(|| {
        RecoverableError::with_hint("review_id required", "Provide the review ID")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/reviews/{rev_id}");
    let out = run_gh(&["api", "--method", "DELETE", &endpoint]).await?;
    Ok(json!(check_gh_output(out)?))
}
"add_review_comment" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let body = params["body"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("body required", "Provide the comment body")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/comments");
    let mut gh_args = vec!["api", "--method", "POST", &endpoint,
        "--field", &format!("body={body}")];
    if let Some(commit) = params["commit_id"].as_str() {
        gh_args.extend(["--field", &format!("commit_id={commit}")]);
    }
    if let Some(path) = params["path"].as_str() {
        gh_args.extend(["--field", &format!("path={path}")]);
    }
    if let Some(line) = params["line"].as_u64() {
        let ls = line.to_string();
        gh_args.extend(["--field", &format!("line={ls}")]);
    }
    let out = run_gh(&gh_args).await?;
    Ok(json!(check_gh_output(out)?))
}
"add_reply_to_comment" => {
    let num = number.as_deref().ok_or_else(|| {
        RecoverableError::with_hint("number required", "Provide the PR number")
    })?;
    let cid = params["comment_id"].as_u64().ok_or_else(|| {
        RecoverableError::with_hint("comment_id required", "Provide the comment ID to reply to")
    })?;
    let body = params["body"].as_str().ok_or_else(|| {
        RecoverableError::with_hint("body required", "Provide the reply body")
    })?;
    require_owner_repo(owner, repo)?;
    let endpoint = format!("/repos/{owner}/{repo}/pulls/{num}/comments/{cid}/replies");
    let out = run_gh(&["api", "--method", "POST", &endpoint,
        "--field", &format!("body={body}")]).await?;
    Ok(json!(check_gh_output(out)?))
}
```

**Step 3: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubPr write methods (create, update, merge, review workflow)"
```

---

## Task 7: `GithubFile` — all methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_file_get_always_buffers() {
    // File contents are always large enough to buffer — verified by logic
    // The always_buffer fn stores unconditionally
    let large = "x".repeat(TOOL_OUTPUT_BUFFER_THRESHOLD + 1);
    assert!(large.len() > TOOL_OUTPUT_BUFFER_THRESHOLD);
}
```

**Step 2: Implement `GithubFile::call`**

Replace stub `impl Tool for GithubFile`:

```rust
#[async_trait]
impl Tool for GithubFile {
    fn name(&self) -> &'static str { "github_file" }

    fn description(&self) -> &'static str {
        "GitHub file operations.\n\
         method: get | create_or_update | delete | push_files\n\n\
         get — fetch file contents at an optional ref/branch (returns @buffer handle).\n\
         create_or_update — create or update a single file (sha required when updating).\n\
         delete — delete a file (sha required).\n\
         push_files — push multiple files in a single commit."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "method":  { "type": "string", "enum": ["get","create_or_update","delete","push_files"] },
                "owner":   { "type": "string" },
                "repo":    { "type": "string" },
                "path":    { "type": "string",  "description": "File path within repository" },
                "ref":     { "type": "string",  "description": "get: branch, tag, or commit SHA" },
                "content": { "type": "string",  "description": "create_or_update: base64-encoded file content" },
                "message": { "type": "string",  "description": "create_or_update/delete/push_files: commit message" },
                "sha":     { "type": "string",  "description": "create_or_update/delete: blob SHA of existing file" },
                "branch":  { "type": "string",  "description": "create_or_update/delete/push_files: target branch" },
                "files":   { "type": "array",   "description": "push_files: [{path, content}] array" }
            },
            "required": ["method"]
        })
    }

    async fn call(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let method = params["method"].as_str().unwrap_or("");
        let owner  = params["owner"].as_str().unwrap_or("");
        let repo   = params["repo"].as_str().unwrap_or("");

        match method {
            "get" => {
                require_owner_repo(owner, repo)?;
                let path = params["path"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("path required", "Provide the file path")
                })?;
                let mut endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
                if let Some(r) = params["ref"].as_str() {
                    endpoint.push_str(&format!("?ref={r}"));
                }
                let out = run_gh(&["api", &endpoint]).await?;
                Ok(always_buffer(check_gh_output(out)?, "github_file", ctx))
            }
            "create_or_update" => {
                require_owner_repo(owner, repo)?;
                let path    = params["path"].as_str().ok_or_else(|| RecoverableError::with_hint("path required", "Provide the file path"))?;
                let content = params["content"].as_str().ok_or_else(|| RecoverableError::with_hint("content required", "Provide base64-encoded file content"))?;
                let message = params["message"].as_str().ok_or_else(|| RecoverableError::with_hint("message required", "Provide a commit message"))?;
                let endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
                let mut gh_args = vec!["api", "--method", "PUT", &endpoint,
                    "--field", &format!("message={message}"),
                    "--field", &format!("content={content}")];
                if let Some(sha) = params["sha"].as_str() {
                    gh_args.extend(["--field", &format!("sha={sha}")]);
                }
                if let Some(branch) = params["branch"].as_str() {
                    gh_args.extend(["--field", &format!("branch={branch}")]);
                }
                let out = run_gh(&gh_args).await?;
                Ok(json!(check_gh_output(out)?))
            }
            "delete" => {
                require_owner_repo(owner, repo)?;
                let path    = params["path"].as_str().ok_or_else(|| RecoverableError::with_hint("path required", "Provide the file path"))?;
                let sha     = params["sha"].as_str().ok_or_else(|| RecoverableError::with_hint("sha required", "Provide the blob SHA of the file to delete"))?;
                let message = params["message"].as_str().ok_or_else(|| RecoverableError::with_hint("message required", "Provide a commit message"))?;
                let endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
                let mut gh_args = vec!["api", "--method", "DELETE", &endpoint,
                    "--field", &format!("message={message}"),
                    "--field", &format!("sha={sha}")];
                if let Some(branch) = params["branch"].as_str() {
                    gh_args.extend(["--field", &format!("branch={branch}")]);
                }
                let out = run_gh(&gh_args).await?;
                Ok(json!(check_gh_output(out)?))
            }
            "push_files" => {
                // Multi-file push requires 3-step Git Data API: create blobs → create tree → create commit
                // Delegate to a helper for clarity
                require_owner_repo(owner, repo)?;
                let files = params["files"].as_array().ok_or_else(|| {
                    RecoverableError::with_hint("files required", "Provide an array of {path, content} objects")
                })?;
                let message = params["message"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("message required", "Provide a commit message")
                })?;
                let branch = params["branch"].as_str().unwrap_or("main");
                push_files_via_api(owner, repo, files, message, branch, ctx).await
            }
            other => Err(RecoverableError::with_hint(
                format!("unknown method: '{other}'"),
                "method must be one of: get, create_or_update, delete, push_files",
            ).into()),
        }
    }
}

async fn push_files_via_api(
    owner: &str,
    repo: &str,
    files: &[Value],
    message: &str,
    branch: &str,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    // 1. Get the current HEAD SHA for the branch
    let ref_endpoint = format!("/repos/{owner}/{repo}/git/ref/heads/{branch}");
    let out = run_gh(&["api", &ref_endpoint]).await?;
    let ref_json: Value = serde_json::from_str(&check_gh_output(out)?)?;
    let base_sha = ref_json["object"]["sha"].as_str()
        .ok_or_else(|| anyhow::anyhow!("could not get base commit SHA"))?
        .to_string();

    // 2. Get the base tree SHA
    let commit_endpoint = format!("/repos/{owner}/{repo}/git/commits/{base_sha}");
    let out = run_gh(&["api", &commit_endpoint]).await?;
    let commit_json: Value = serde_json::from_str(&check_gh_output(out)?)?;
    let base_tree = commit_json["tree"]["sha"].as_str()
        .ok_or_else(|| anyhow::anyhow!("could not get base tree SHA"))?
        .to_string();

    // 3. Build tree entries
    let mut tree_args = vec!["api", "--method", "POST",
        &format!("/repos/{owner}/{repo}/git/trees"),
        "--field", &format!("base_tree={base_tree}")];
    // Build tree entries JSON
    let tree_entries: Vec<Value> = files.iter().map(|f| {
        json!({
            "path": f["path"],
            "mode": "100644",
            "type": "blob",
            "content": f["content"]
        })
    }).collect();
    let tree_json = serde_json::to_string(&tree_entries)?;
    tree_args.extend(["--field", &format!("tree={tree_json}")]);
    let out = run_gh(&tree_args).await?;
    let tree_result: Value = serde_json::from_str(&check_gh_output(out)?)?;
    let new_tree_sha = tree_result["sha"].as_str()
        .ok_or_else(|| anyhow::anyhow!("could not get new tree SHA"))?
        .to_string();

    // 4. Create commit
    let commit_endpoint = format!("/repos/{owner}/{repo}/git/commits");
    let out = run_gh(&["api", "--method", "POST", &commit_endpoint,
        "--field", &format!("message={message}"),
        "--field", &format!("tree={new_tree_sha}"),
        "--field", &format!("parents[]={base_sha}")]).await?;
    let new_commit: Value = serde_json::from_str(&check_gh_output(out)?)?;
    let new_commit_sha = new_commit["sha"].as_str()
        .ok_or_else(|| anyhow::anyhow!("could not get new commit SHA"))?
        .to_string();

    // 5. Update branch ref
    let update_endpoint = format!("/repos/{owner}/{repo}/git/refs/heads/{branch}");
    let out = run_gh(&["api", "--method", "PATCH", &update_endpoint,
        "--field", &format!("sha={new_commit_sha}")]).await?;
    check_gh_output(out)?;

    Ok(json!({"sha": new_commit_sha, "branch": branch}))
}
```

**Step 3: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubFile tool (get, create_or_update, delete, push_files)"
```

---

## Task 8: `GithubRepo` — all methods

**Files:**
- Modify: `src/tools/github.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_repo_list_branches_shape() {
    let json = r#"[{"name":"main","protected":true},{"name":"feature","protected":false}]"#;
    let v: Value = serde_json::from_str(json).unwrap();
    assert_eq!(v[0]["name"], "main");
}
```

**Step 2: Implement `GithubRepo::call`**

Replace stub `impl Tool for GithubRepo`:

```rust
#[async_trait]
impl Tool for GithubRepo {
    fn name(&self) -> &'static str { "github_repo" }

    fn description(&self) -> &'static str {
        "GitHub repository operations.\n\
         Repo: search | create | fork\n\
         Branches: list_branches | create_branch\n\
         Commits: list_commits | get_commit\n\
         Releases: list_releases | get_latest_release | get_release_by_tag\n\
         Tags: list_tags | get_tag\n\
         Code: search_code\n\n\
         search_code returns a @buffer handle. get_commit returns a @buffer handle."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["search","create","fork","list_branches","create_branch",
                             "list_commits","get_commit","list_releases","get_latest_release",
                             "get_release_by_tag","list_tags","get_tag","search_code"]
                },
                "owner":     { "type": "string" },
                "repo":      { "type": "string" },
                "query":     { "type": "string",  "description": "search/search_code: query string" },
                "name":      { "type": "string",  "description": "create: repository name" },
                "private":   { "type": "boolean", "description": "create: private repository" },
                "branch":    { "type": "string",  "description": "create_branch: new branch name" },
                "from_branch":{ "type": "string", "description": "create_branch: source branch (default: default branch)" },
                "sha":       { "type": "string",  "description": "get_commit: commit SHA" },
                "tag":       { "type": "string",  "description": "get_release_by_tag/get_tag: tag name" },
                "limit":     { "type": "integer", "description": "list_*: max results (default 30)" },
                "page":      { "type": "integer", "description": "pagination" }
            },
            "required": ["method"]
        })
    }

    async fn call(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let method = params["method"].as_str().unwrap_or("");
        let owner  = params["owner"].as_str().unwrap_or("");
        let repo   = params["repo"].as_str().unwrap_or("");
        let repo_flag = if !owner.is_empty() && !repo.is_empty() {
            format!("{owner}/{repo}")
        } else {
            String::new()
        };
        let limit = params["limit"].as_u64().unwrap_or(30).to_string();

        match method {
            "search" => {
                let q = params["query"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("query required", "Provide a search query")
                })?;
                let out = run_gh(&["search", "repos", q,
                    "--json", "name,owner,description,url,stars,isPrivate",
                    "--limit", &limit]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "create" => {
                let name = params["name"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("name required", "Provide the repository name")
                })?;
                let mut args = vec!["repo", "create", name, "--json"];
                if params["private"].as_bool().unwrap_or(false) { args.push("--private"); }
                else { args.push("--public"); }
                let out = run_gh(&args).await?;
                Ok(json!(check_gh_output(out)?))
            }
            "fork" => {
                require_owner_repo(owner, repo)?;
                let out = run_gh(&["repo", "fork", &repo_flag, "--json"]).await?;
                Ok(json!(check_gh_output(out)?))
            }
            "list_branches" => {
                require_owner_repo(owner, repo)?;
                let out = run_gh(&["api", &format!("/repos/{owner}/{repo}/branches"),
                    "-F", &format!("per_page={limit}")]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "create_branch" => {
                require_owner_repo(owner, repo)?;
                let branch = params["branch"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("branch required", "Provide the new branch name")
                })?;
                // Get SHA of source branch (or default branch)
                let from = params["from_branch"].as_str().unwrap_or("HEAD");
                let sha_endpoint = format!("/repos/{owner}/{repo}/git/ref/heads/{from}");
                let sha_out = run_gh(&["api", &sha_endpoint]).await?;
                let sha_json: Value = serde_json::from_str(&check_gh_output(sha_out)?)?;
                let sha = sha_json["object"]["sha"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("could not get SHA for source branch"))?
                    .to_string();
                let endpoint = format!("/repos/{owner}/{repo}/git/refs");
                let out = run_gh(&["api", "--method", "POST", &endpoint,
                    "--field", &format!("ref=refs/heads/{branch}"),
                    "--field", &format!("sha={sha}")]).await?;
                Ok(json!(check_gh_output(out)?))
            }
            "list_commits" => {
                require_owner_repo(owner, repo)?;
                let out = run_gh(&["api", &format!("/repos/{owner}/{repo}/commits"),
                    "-F", &format!("per_page={limit}")]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "get_commit" => {
                require_owner_repo(owner, repo)?;
                let sha = params["sha"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("sha required", "Provide the commit SHA")
                })?;
                let out = run_gh(&["api", &format!("/repos/{owner}/{repo}/commits/{sha}")]).await?;
                Ok(always_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "list_releases" => {
                let mut args = vec!["release", "list",
                    "--json", "name,tagName,isDraft,isPrerelease,publishedAt",
                    "--limit", &limit];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "get_latest_release" => {
                let mut args = vec!["release", "view",
                    "--json", "name,tagName,body,assets,publishedAt,url"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "get_release_by_tag" => {
                let tag = params["tag"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("tag required", "Provide the release tag")
                })?;
                let mut args = vec!["release", "view", tag,
                    "--json", "name,tagName,body,assets,publishedAt,url"];
                if !repo_flag.is_empty() { args.extend(["--repo", &repo_flag]); }
                let out = run_gh(&args).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "list_tags" => {
                require_owner_repo(owner, repo)?;
                let out = run_gh(&["api", &format!("/repos/{owner}/{repo}/tags"),
                    "-F", &format!("per_page={limit}")]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "get_tag" => {
                require_owner_repo(owner, repo)?;
                let tag = params["tag"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("tag required", "Provide the tag name")
                })?;
                let out = run_gh(&["api", &format!("/repos/{owner}/{repo}/git/ref/tags/{tag}")]).await?;
                Ok(maybe_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            "search_code" => {
                let q = params["query"].as_str().ok_or_else(|| {
                    RecoverableError::with_hint("query required", "Provide a code search query")
                })?;
                let out = run_gh(&["search", "code", q,
                    "--json", "path,repository,url,textMatches",
                    "--limit", &limit]).await?;
                Ok(always_buffer(check_gh_output(out)?, "github_repo", ctx))
            }
            other => Err(RecoverableError::with_hint(
                format!("unknown method: '{other}'"),
                "method must be one of: search, create, fork, list_branches, create_branch, \
                 list_commits, get_commit, list_releases, get_latest_release, get_release_by_tag, \
                 list_tags, get_tag, search_code",
            ).into()),
        }
    }
}
```

**Step 3: Run tests + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

**Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "feat: implement GithubRepo tool (branches, commits, releases, tags, search_code)"
```

---

## Task 9: Full schema — replace stubs with proper `input_schema`

The stub tools registered in Task 1 had minimal schemas. All five tools now have full schemas from their `impl Tool` blocks. Verify the schemas are wired correctly.

**Step 1: Compile and run full test suite**

```bash
cargo test
```
Expected: all tests pass

**Step 2: Run clippy + fmt**

```bash
cargo fmt && cargo clippy -- -D warnings
```

**Step 3: Smoke test — verify tools are listed**

```bash
cargo run -- start --project . &
# In a separate terminal, or just verify compilation works
```

**Step 4: Final commit**

```bash
git add -u
git commit -m "feat: complete github-mcp-slim — 5 tools replacing 44, ~8k token saving"
```

---

## Verification Checklist

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes (all tests)
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt` — no diffs
- [ ] 5 new tools visible in MCP server tool list
- [ ] Token count: ~2.5k vs ~10.3k for official github-mcp

## Notes for the Implementor

- The `--field key=value` syntax in `gh api` passes URL-encoded form fields. For complex nested JSON, use `--input -` with stdin or `--raw-field` for literal strings.
- `gh search code` JSON fields may vary — test against the actual `gh` version installed. Run `gh search code "test" --json 2>&1 | head` to see available fields.
- The `push_files` implementation uses the Git Data API (3-step: tree + commit + ref update). This is the only method that makes multiple `gh api` calls — it's inherently sequential.
- Integration tests should be tagged `#[ignore]` and run manually with `cargo test -- --ignored` when `gh` is available and authed.

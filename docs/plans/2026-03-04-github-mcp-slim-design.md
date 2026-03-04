# GitHub MCP Slim — Design

**Date:** 2026-03-04  
**Status:** Approved  
**Goal:** Replace the official `github-mcp` server (44 tools, ~10.3k tokens) with a lean
alternative built directly into code-explorer (5 tools, ~2.5k tokens) — saving ~8k tokens
of fixed per-session overhead.

---

## Problem

The official `github-mcp` server registers 44 individual tools, each costing ~233 tokens
of schema overhead = ~10.3k tokens burned every session before any conversation starts.
Many of those tools are thin wrappers around a single API endpoint. The fragmentation is
unnecessary — it's a "one tool per endpoint" design rather than a "one tool per domain"
design.

---

## Solution

5 domain-grouped tools with a `method` enum parameter each, implemented in
`src/tools/github.rs` and registered alongside the existing 25 code-explorer tools.
Same binary, no new config, no new auth — inherits `gh` CLI auth automatically.

---

## Tools

### `github_issue`
Covers: `list_issues`, `search_issues`, `issue_read` (get/get_comments/get_labels/get_sub_issues),
`issue_write` (create/update), `add_issue_comment`, `sub_issue_write`, `get_label`

| Method | `gh` command |
|---|---|
| `list` | `gh issue list --repo owner/repo --json ...` |
| `search` | `gh search issues ...` |
| `get` | `gh issue view <number> --repo owner/repo --json ...` |
| `get_comments` | `gh issue view <number> --repo owner/repo --json comments` |
| `get_labels` | `gh issue view <number> --repo owner/repo --json labels` |
| `get_sub_issues` | `gh api /repos/{owner}/{repo}/issues/{number}/sub_issues` |
| `create` | `gh issue create --repo owner/repo ...` |
| `update` | `gh issue edit <number> --repo owner/repo ...` |
| `add_comment` | `gh issue comment <number> --repo owner/repo --body ...` |
| `add_sub_issue` | `gh api POST /repos/{owner}/{repo}/issues/{number}/sub_issues` |
| `remove_sub_issue` | `gh api DELETE /repos/{owner}/{repo}/issues/{number}/sub_issues/{sub_id}` |

---

### `github_pr`
Covers: `list_pull_requests`, `search_pull_requests`, `pull_request_read` (all methods),
`create_pull_request`, `update_pull_request`, `merge_pull_request`,
`update_pull_request_branch`, `pull_request_review_write`, `add_comment_to_pending_review`,
`add_reply_to_pull_request_comment`, `request_copilot_review`

| Method | `gh` command |
|---|---|
| `list` | `gh pr list --repo owner/repo --json ...` |
| `search` | `gh search prs ...` |
| `get` | `gh pr view <number> --repo owner/repo --json ...` |
| `get_diff` | `gh pr diff <number> --repo owner/repo` (always buffered) |
| `get_files` | `gh pr view <number> --repo owner/repo --json files` |
| `get_comments` | `gh pr view <number> --repo owner/repo --json comments` |
| `get_reviews` | `gh api /repos/{owner}/{repo}/pulls/{number}/reviews` |
| `get_review_comments` | `gh api /repos/{owner}/{repo}/pulls/{number}/comments` |
| `get_status` | `gh api /repos/{owner}/{repo}/commits/{sha}/status` |
| `create` | `gh pr create --repo owner/repo ...` |
| `update` | `gh pr edit <number> --repo owner/repo ...` |
| `merge` | `gh pr merge <number> --repo owner/repo ...` |
| `update_branch` | `gh api PUT /repos/{owner}/{repo}/pulls/{number}/update-branch` |
| `create_review` | `gh api POST /repos/{owner}/{repo}/pulls/{number}/reviews` |
| `submit_review` | `gh api POST /repos/{owner}/{repo}/pulls/{number}/reviews/{id}/events` |
| `delete_review` | `gh api DELETE /repos/{owner}/{repo}/pulls/{number}/reviews/{id}` |
| `add_review_comment` | `gh api POST /repos/{owner}/{repo}/pulls/{number}/comments` |
| `add_reply_to_comment` | `gh api POST /repos/{owner}/{repo}/pulls/{number}/comments/{id}/replies` |

---

### `github_file`
Covers: `get_file_contents`, `create_or_update_file`, `delete_file`, `push_files`

| Method | `gh` command |
|---|---|
| `get` | `gh api /repos/{owner}/{repo}/contents/{path}?ref={ref}` (always buffered) |
| `create_or_update` | `gh api PUT /repos/{owner}/{repo}/contents/{path}` |
| `delete` | `gh api DELETE /repos/{owner}/{repo}/contents/{path}` |
| `push_files` | `gh api POST /repos/{owner}/{repo}/git/trees` + commit chain |

---

### `github_repo`
Covers: `search_repositories`, `create_repository`, `fork_repository`,
`list_branches`, `create_branch`, `list_commits`, `get_commit`,
`list_releases`, `get_latest_release`, `get_release_by_tag`,
`list_tags`, `get_tag`, `search_code`

| Method | `gh` command |
|---|---|
| `search` | `gh search repos ...` |
| `create` | `gh repo create ...` |
| `fork` | `gh repo fork owner/repo` |
| `list_branches` | `gh api /repos/{owner}/{repo}/branches` |
| `create_branch` | `gh api POST /repos/{owner}/{repo}/git/refs` |
| `list_commits` | `gh api /repos/{owner}/{repo}/commits` |
| `get_commit` | `gh api /repos/{owner}/{repo}/commits/{sha}` (always buffered) |
| `list_releases` | `gh release list --repo owner/repo --json ...` |
| `get_latest_release` | `gh release view --repo owner/repo --json ...` |
| `get_release_by_tag` | `gh release view <tag> --repo owner/repo --json ...` |
| `list_tags` | `gh api /repos/{owner}/{repo}/tags` |
| `get_tag` | `gh api /repos/{owner}/{repo}/git/ref/tags/{tag}` |
| `search_code` | `gh search code ...` (always buffered) |

---

### `github_identity`
Covers: `get_me`, `search_users`, `get_teams`, `get_team_members`, `assign_copilot_to_issue`

| Method | `gh` command |
|---|---|
| `get_me` | `gh api /user` |
| `search_users` | `gh search users ...` |
| `get_teams` | `gh api /user/teams` |
| `get_team_members` | `gh api /orgs/{org}/teams/{team_slug}/members` |
| `assign_copilot` | `gh api POST /repos/{owner}/{repo}/issues/{number}/assignees` (Copilot user) |

---

## Execution Model

Each tool shells out to `gh` via `tokio::process::Command`. No reuse of the `run_command`
MCP tool — that's a user-facing tool, not an internal API. Auth is inherited from
`gh auth` with no additional config.

```
tool.call(params)
  → build gh argv
  → tokio::process::Command::new("gh").args(...).output().await
  → if exit != 0: RecoverableError(stderr)
  → if output.len() > TOOL_OUTPUT_BUFFER_THRESHOLD (10k bytes):
        ctx.output_buffer.store_tool("github_*", stdout)
        return compact summary + @tool_handle
  → else: return parsed JSON inline
```

Methods that **always buffer** (output is always large): `get_diff`, `get_commit`,
`get` (file contents), `search_code`.

---

## Error Handling

| Condition | Response |
|---|---|
| `gh` not in PATH | `RecoverableError` — "gh CLI not found. Install: https://cli.github.com" |
| `gh auth` not set up | `RecoverableError` — "Not authenticated. Run: gh auth login" |
| API 404 / 403 | `RecoverableError` — stderr as message |
| Non-zero exit (other) | `RecoverableError` — stderr as message |
| JSON parse failure | `anyhow::bail!` — programming error |

---

## File Structure

```
src/tools/github.rs    — 5 Tool structs + helper fns
src/server.rs          — +5 tool registrations
```

No new directories, no new config, no new dependencies (octocrab deferred unless a
specific method proves `gh` can't cover it cleanly).

---

## Testing

- **Unit tests** — canned `gh` JSON fixtures fed through formatters; no process spawning
- **Integration tests** — gated behind `GITHUB_TOKEN` env check; hit real `gh` CLI
- **Error path tests** — mock missing binary, auth error, 404 responses

---

## Token Budget

| Server | Tools | Est. tokens |
|---|---|---|
| Official `github-mcp` | 44 | ~10.3k |
| `github-mcp-slim` (this) | 5 | ~2.5k |
| **Saving** | | **~7.8k** |

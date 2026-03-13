## GitHub Tools

These tools are enabled via `security.github_enabled = true` in `.codescout/project.toml`.

### When to use

| Task | Tool | NOT this |
|---|---|---|
| GitHub identity / teams | `github_identity(method, ...)` | — |
| GitHub issues | `github_issue(method, owner, repo, ...)` | — |
| GitHub pull requests | `github_pr(method, owner, repo, ...)` | — |
| GitHub file contents / writes | `github_file(method, owner, repo, path, ...)` | — |

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("gh issue list")` or `run_command("gh pr ...")` | `github_issue(method, owner, repo, ...)` / `github_pr(...)` | Structured output, pagination, buffer handling built-in |

### Reference

- `github_identity(method)` — authenticated user profile, team membership, user search.
  - `method`: `get_me` | `search_users` (query required) | `get_teams` | `get_team_members` (org + team_slug required)
- `github_issue(method, owner, repo, ...)` — issue read/write operations.
  - Read: `list` | `search` | `get` | `get_comments` | `get_labels` | `get_sub_issues`
  - Write: `create` (title required) | `update` | `add_comment` | `add_sub_issue` | `remove_sub_issue`
  - `limit` defaults to 30 for list/search.
- `github_pr(method, owner, repo, ...)` — pull request read/write operations.
  - Read: `list` | `search` | `get` | `get_diff` | `get_files` | `get_comments` | `get_reviews` | `get_review_comments` | `get_status`
  - Write: `create` | `update` | `merge` | `update_branch` | `create_review` | `submit_review` | `delete_review` | `add_review_comment` | `add_reply_to_comment`
  - `get_diff` always returns a `@tool` buffer handle (diffs are large).
- `github_file(method, owner, repo, path, ...)` — file contents and writes via GitHub API.
  - `get` — fetch file at optional ref/branch (returns `@buffer` handle).
  - `create_or_update` — create or update a single file (`sha` required when updating).
  - `delete` — delete a file (`sha` required).
  - `push_files` — push multiple files in a single commit.

# Conditional GitHub Tool Registration

**Date:** 2026-03-13
**Branch:** experiments
**Status:** Design approved

## Problem

The 5 GitHub tools (`github_identity`, `github_issue`, `github_pr`, `github_file`,
`github_repo`) cost ~2,350 tokens per MCP round-trip (schemas + instructions). Most
codescout users don't need issue/PR/file tools — they use Claude Code's native GitHub
integration or the `gh` CLI directly. Only `github_repo` (code search, releases, branches)
provides unique value that isn't covered elsewhere.

## Decision

- **`github_repo`** — always registered, always documented in server instructions.
- **`github_identity`, `github_issue`, `github_pr`, `github_file`** — opt-in via
  `security.github_enabled = true` in `.codescout/project.toml`. Default: `false`.
- When enabled, a dedicated `github_instructions.md` is appended to the system prompt.
- When disabled, the 4 tools are not registered at all (not in `tools/list`).
- Restart required to toggle (`/mcp`).

## Token savings

| State | Schema tokens | Instruction tokens | Total |
|-------|---:|---:|---:|
| `github_enabled = false` (default) | ~313 | ~0 extras | ~313 |
| `github_enabled = true` | ~1,668 | ~685 | ~2,353 |
| **Savings when disabled** | | | **~2,040** |

## Changes

### 1. Config (`src/config/project.rs` + `src/util/path_security.rs`)

- `SecuritySection.github_enabled`: change default from `true` to `false`
- Change serde annotation from `#[serde(default = "default_true")]` to `#[serde(default)]`
- Update doc comment: "Enable additional GitHub tools: github_identity, github_issue,
  github_pr, github_file. github_repo is always available. (default: false)"
- **Also flip `PathSecurityConfig::default()`** (`src/util/path_security.rs`) to
  `github_enabled: false` — keeps the "no project active" path consistent with the
  TOML-driven path. Without this, no-project defaults to all GitHub tools allowed while
  project-without-config defaults to disabled.

### 2. Tool registration (`src/server.rs::from_parts`)

- `github::GithubRepo` stays in the base `vec![...]`
- The 4 optional tools are pushed conditionally:

```rust
// GitHub tools — github_repo always available
Arc::new(github::GithubRepo),
// ... end of vec!

// Optional GitHub tools (issue/PR/file/identity)
let github_enabled = agent.security_config().await.github_enabled;
if github_enabled {
    tools.push(Arc::new(github::GithubIdentity));
    tools.push(Arc::new(github::GithubIssue));
    tools.push(Arc::new(github::GithubPr));
    tools.push(Arc::new(github::GithubFile));
}
```

### 3. Server instructions split

**`src/prompts/server_instructions.md`** — retains only `github_repo` content:

- "By knowledge level" table row: update to reference only `github_repo`
- "By task" table: keep the `github_repo` row, remove the other 4 GitHub rows
- Anti-patterns table: keep `github_repo(list_commits)` row, remove `gh issue/pr` row
- `### GitHub` section: replace with `github_repo`-only documentation + brief usage guidance

Usage guidance to add (replaces the bare method listing):

> **`github_repo`** is for operations that require the GitHub API — code search across repos,
> listing releases/tags, creating branches remotely, forking. For local history (blame, log,
> diff), prefer `run_command("git ...")` — it's faster and has full history.

**New file: `src/prompts/github_instructions.md`** — appended to system prompt when
`github_enabled = true`. Contains:

- Tool reference docs for `github_identity`, `github_issue`, `github_pr`, `github_file`
- "By task" rows for the 4 tools
- Anti-pattern row (`gh issue list` → `github_issue`)
- Usage guidance: when to use which tool, owner/repo patterns

### 4. Prompt builder (`src/prompts/mod.rs`)

- New constant: `pub const GITHUB_INSTRUCTIONS: &str = include_str!("github_instructions.md");`
- Add `github_enabled: bool` field to `ProjectStatus` struct (default `false`)
- `build_server_instructions()` signature unchanged — it reads `status.github_enabled`
  from the existing `ProjectStatus` param. When `true`, appends `\n\n` + `GITHUB_INSTRUCTIONS`.
- The `None` path (no project) naturally means no GitHub instructions appended.
- Populate `github_enabled` in `Agent::project_status()` from the security config.

### 5. Security gating (`src/util/path_security.rs`)

- Remove `github_repo` from the `check_tool_access` match arm
- Keep the other 4 gated on `github_enabled` (belt-and-suspenders)

```rust
"github_identity" | "github_issue" | "github_pr" | "github_file" => {
    if !config.github_enabled {
        bail!("GitHub tools (identity/issue/pr/file) are disabled. \
               Set security.github_enabled = true in .codescout/project.toml to enable.");
    }
}
// github_repo: always allowed, no gate
```

### 6. Tests

| Test | Change |
|------|--------|
| `github_enabled_allows_github_tools` | Split: one for `github_repo` always allowed, one for 4 optional tools |
| `check_tool_access_error_message_includes_config_hint` | Assert `github_repo` NOT blocked when `github_enabled = false` |
| `server_registers_all_tools` | Update to expect 25 tools (default). Add `server_registers_github_tools_when_enabled` that writes `project.toml` with `github_enabled = true` to temp dir, expects 29 tools |
| Config default tests | Assert `github_enabled` defaults to `false` |
| `build_server_instructions` tests | Add test: instructions include `GITHUB_INSTRUCTIONS` content when `ProjectStatus.github_enabled = true`, exclude when `false` |
| `PathSecurityConfig::default()` | Assert `github_enabled` is `false` |

### 7. Prompt surface consistency

All 3 prompt surfaces updated:
- `src/prompts/server_instructions.md` — `github_repo` only
- `src/prompts/github_instructions.md` — the other 4 (conditional)
- `src/prompts/onboarding_prompt.md` — no changes needed (no GitHub tool refs)

## Out of scope

- Slimming down `github_repo` methods (code search, releases, tags overlap with local git)
- Dynamic tool list updates via `activate_project` (restart is fine)
- Per-tool granularity (e.g. enable `github_pr` but not `github_issue`)

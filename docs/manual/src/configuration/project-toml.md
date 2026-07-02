# Project Configuration

Every project managed by codescout has an optional configuration file at
`.codescout/project.toml`. The file uses [TOML](https://toml.io/) syntax.

## File Location and Auto-Creation

The file lives at:

```
<project-root>/.codescout/project.toml
```

If the file does not exist when a project is first activated, codescout creates it with
sensible defaults derived from the directory name. You can also create it manually before
activating a project.

The configuration is loaded by `ProjectConfig::load_or_default`: if the file is present it is
parsed, otherwise defaults are applied. All sections except `[project]` are fully optional —
omit any section and its defaults take effect silently.

---

## `[project]` — General Settings

```toml
[project]
name = "my-service"
languages = ["rust", "toml", "markdown"]
encoding = "utf-8"
tool_timeout_secs = 60
```

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | directory basename | Human-readable project name shown in tool output. |
| `languages` | array of strings | `[]` | Languages detected in the project. Populated automatically by `onboarding`. You can set this manually if auto-detection is wrong. |
| `encoding` | string | `"utf-8"` | Character encoding used when reading source files. |
| `tool_timeout_secs` | integer | `60` | Maximum seconds any single tool call may run before it is cancelled. Increase this for very large projects where LSP startup or indexing takes longer. |

**Note:** `languages` is populated by the `onboarding` tool and written back to the config file.
You rarely need to set it by hand.

---

## `[embeddings]` — Semantic Search Settings

Controls which embedding model is used and how source files are chunked before embedding.

```toml
[embeddings]
model = "ollama:mxbai-embed-large"
drift_detection_enabled = true
```

| Field | Type | Default | Description |
|---|---|---|---|
| `model` | string | `"ollama:mxbai-embed-large"` | Embedding model. The prefix selects the backend. See [Embedding Backends](embedding-backends.md) for the full list of supported prefixes and models. |
| `chunk_size` | integer | model-derived | Target characters per chunk. Unset → derived from the model's context window (capped at 4096). Set explicitly to opt into smaller/larger chunks (capped at the model's input limit). |
| `drift_detection_enabled` | bool | `true` | Enable semantic drift detection during index builds. `index(action: build)` compares old and new chunk embeddings to score how much each changed file's *meaning* shifted. Results queryable via `workspace(action: status, threshold=...)`. Set to `false` to opt out. Experimental — adds memory overhead proportional to changed-file count. |

> **Note — chunk size is automatic by default.** When `chunk_size` is unset,
> codescout derives the chunk budget from the model's published context window
> (a conservative `max_tokens × 3 chars/token` formula at 85 % utilisation, capped
> at 4096 chars). Set `chunk_size` explicitly to override — the value is honoured,
> capped at the model's input limit. `chunk_overlap` was removed; existing
> `project.toml` files containing that key are silently ignored.

**Changing the model after indexing:** If you change `model`, you must rebuild the index
(`index(action: build)` with `force: true`). codescout detects model mismatches and will warn
rather than return wrong results.

---

## `[ignored_paths]` — Indexing Exclusions

Glob patterns for directories and files that should be excluded from semantic search indexing,
`tree`, and file traversal.

```toml
[ignored_paths]
patterns = [
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".codescout",
]
```

| Field | Type | Default | Description |
|---|---|---|---|
| `patterns` | array of strings | (list above) | Path components or glob patterns to skip during traversal. Each pattern is matched against every path segment, not just the full path. |

The default list covers common build artifact and dependency directories. To add your own
exclusions, replace the entire `patterns` array — there is no "append" mode:

```toml
[ignored_paths]
patterns = [
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".codescout",
    "vendor",
    "generated",
    "*.pb.go",
]
```

---

## `[security]` — Access Controls

> See [Security & Permissions](../concepts/security.md) for the rationale behind these settings and how the permission model works end-to-end.

Controls what operations the AI agent is permitted to perform. These settings are
conservative by default: shell execution warns before running, file writes are on, git reads are on.

```toml
[security]
profile = "default"
extra_write_roots = []
shell_command_mode = "warn"
file_write_enabled = true
indexing_enabled = true
```

| Field | Type | Default | Description |
|---|---|---|---|
| `profile` | string | `"default"` | `"default"` enforces the built-in read deny-list; `"root"` skips it entirely for absolute-path reads (see [Security & Permissions](../concepts/security.md)). Not a stronger sandbox — a bypass. |
| `extra_write_roots` | array of strings | `[]` | Additional directories where file write tools are allowed. By default writes are restricted to the project root. |
| `shell_command_mode` | string | `"warn"` | Controls `run_command` behaviour. One of `"unrestricted"`, `"warn"`, or `"disabled"`. |
| `file_write_enabled` | bool | `true` | Enables file write tools: `create_file` and the symbol write tools. Set to `false` for a read-only session. |
| `indexing_enabled` | bool | `true` | Enables `index(action: build)` and `workspace(action: status)`. Set to `false` to prevent the agent from kicking off potentially long-running indexing. |

### Built-in Read Deny-List

codescout always blocks reads from a built-in set of credential and secret
locations — SSH/cloud/git/package-registry credentials, password managers,
shell history files, and OS-level secret stores. A representative sample:

```
~/.ssh
~/.aws
~/.gnupg
~/.config/gcloud
~/.docker/config.json
~/.netrc
~/.npmrc
```

On Linux, `/etc/shadow`, `/etc/sudoers`, and `/proc/self/environ` are also
blocked; on macOS, `/etc/master.passwd` and `~/Library/Keychains`. The list is
**not configurable** and grows over time — the authoritative, current list is
`denied_read_prefixes()` in `src/platform/unix.rs` (and the Windows equivalent).
The only way to bypass it is `profile = "root"`, which disables the check
entirely for absolute-path reads rather than extending or shrinking it.

### Shell Command Mode

The `shell_command_mode` field fine-tunes what happens when `run_command` is called:

| Value | Behaviour |
|---|---|
| `"warn"` | Commands execute normally. This is the default. |
| `"unrestricted"` | Commands execute normally. Currently identical to `"warn"`. |
| `"disabled"` | All shell calls return an error immediately. |

### Controlling Shell Execution

Shell execution is **on by default** (`shell_command_mode = "warn"`). To turn it off
entirely, set the mode to `"disabled"`:

```toml
[security]
shell_command_mode = "disabled"
```

With `"disabled"`, every `run_command` call returns an error immediately. `"warn"` (the
default) and `"unrestricted"` currently behave the same — both run the command.

---

## Complete Example

A complete `project.toml` for a Rust service that uses local CPU embeddings:

```toml
[project]
name = "payment-service"
languages = ["rust", "toml", "sql", "markdown"]
encoding = "utf-8"
tool_timeout_secs = 120

[embeddings]
model = "local:AllMiniLML6V2Q"
drift_detection_enabled = true    # set to false to opt out of semantic drift scoring

[ignored_paths]
patterns = [
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".codescout",
    "vendor",
    "migrations/archive",
]

[security]
denied_read_patterns = ["~/.config/stripe"]
shell_command_mode = "warn"
file_write_enabled = true
indexing_enabled = true
```

---

## How Configuration Is Loaded

At startup and whenever `workspace(action: activate)` is called, codescout:

1. Looks for `.codescout/project.toml` in the project root.
2. If found, parses it. Any section that is missing falls back to its defaults.
3. If not found, constructs a default config using the directory name as the project name.

The effective configuration is always visible via the `workspace(action: status)` tool:

```json
{ "name": "workspace(action: status)", "arguments": {} }
```

Changes to `project.toml` take effect the next time the project is activated — either by
restarting the MCP server or by calling `workspace(action: activate)` again with the same path.

---

## Workspace Configuration

For multi-project repos, create `.codescout/workspace.toml` alongside
`project.toml`:

```toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
depends_on = ["backend"]
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique project identifier, used in `project` parameter across tools |
| `root` | Yes | Path relative to workspace root |
| `languages` | No | Restrict LSP servers to listed languages |
| `depends_on` | No | Project IDs whose symbols are visible during cross-project navigation |

Each project gets its own LSP servers, memory store, and semantic index.
See [Multi-Project Workspaces](../concepts/multi-project-workspace.md) for
usage details.

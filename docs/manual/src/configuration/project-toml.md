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
```

| Field | Type | Default | Description |
|---|---|---|---|
| `model` | string | `"ollama:mxbai-embed-large"` | Embedding model. The prefix selects the backend. See [Embedding Backends](embedding-backends.md) for the full list of supported prefixes and models. |
| `chunk_size` | integer | model-derived | Target characters per chunk. Unset → derived from the model's context window (capped at 4096). Set explicitly to opt into smaller/larger chunks (capped at the model's input limit). |


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
| `extra_write_roots` | array of strings | `[]` | Additional directories where file write tools are allowed. By default writes are restricted to the project root. Each entry must be an absolute path with no `..` component; see below. |
| `shell_command_mode` | string | `"warn"` | Controls `run_command` behaviour. One of `"unrestricted"`, `"warn"`, or `"disabled"`. `"disabled"` also hides the tool from `list_tools`. An unrecognised value leaves the tool visible and fails the call with an error naming the bad value. |
| `file_write_enabled` | bool | `true` | Enables file write tools: `create_file` and the symbol write tools. Set to `false` for a read-only session. |
| `indexing_enabled` | bool | `true` | Enables `index(action: build)` and `workspace(action: status)`. Set to `false` to prevent the agent from kicking off potentially long-running indexing. |

### `[security]` is not editable by the write tools

`[security]` decides what the tools may do, and `.codescout/project.toml` sits under the always-writable
project root, so codescout's own write tools (`create_file`, `edit_file`, `doc(create)`, `doc(move)`)
refuse any write to that file, or to the global `config.toml`, that would change its `[security]`
table (or whose new content is not valid TOML). The rest of the file stays editable through them;
the onboarding embedding-model step edits `[embeddings]` that way. Change `[security]` in an editor;
the server re-reads the file on the next call. The file is recognised by identity as well as by name,
so a `.codescout` that is a link to a directory with another name, the target of a `project.toml`
link written under its own name (including a link whose target does not exist yet, singly or in a
chain), and a hard link to it are guarded too.

**This is not a boundary against a session that can run shell commands.** Under the default
`shell_command_mode = "warn"`, one `run_command` such as
`printf '[security]\nprofile = "root"\n' > .codescout/project.toml` rewrites the file and the next
call runs under `profile = "root"` (measured; `warn` does not flag it, and under `root` no command
is checked at all). The guard covers the file-writing tools (`create_file`, `edit_file`,
`doc(create)`, `doc(move)`) only. Treat a project whose session may run shell commands as
trusting that session with its own sandbox. Making a mid-session reload tighten-only would close
this, and is an open decision (see the issue
`2026-10-17-project-toml-under-the-writable-project-root-can-set-security-profile-root.md`).

`[embeddings].url` and `api_key` stay editable through `edit_file`: indexing sends code chunks to
that URL with that key as a bearer token (an `EMBED_API_KEY` from the environment is sent to a URL
the project file names, when no URL is set in the environment). Review that table in a project you
do not trust.

### `extra_write_roots` entries

Each entry must be an absolute path with no `..` component. An empty or relative entry, a `~`
path (a leading `~` is **not** expanded) and an absolute entry spelled with `..` are refused
when the project config loads, and the error names `.codescout/project.toml` and the entry.
At startup that stops the server for that project.

In a **running** session an edit that fails to load is **not applied, and it is reported**. The
last configuration that loaded stays in force, with two changes. First, every tool response, error
responses included (the `_workspace_notice` field, or a `⚠` line ahead of a plain-text error), and
every refused write carries the loader's error until a reload succeeds, so the failure cannot pass
unseen. That includes `onboarding` responses and the held first call of a destructive tool. The error
never quotes the file: `project.toml` can be a link to any readable file, so the notice gives the
parser's message, line and column and redacts quoted values. Second, from the file only a security *tightening* is
applied: `file_write_enabled = false`, `indexing_enabled = false`, a stricter `shell_command_mode`,
a `profile` other than `"root"`, and `extra_write_roots` entries removed. A key the edit deletes
reads as what the file would mean without it: the global config's value if it sets one, else the
built-in default (so deleting `shell_command_mode = "unrestricted"` next to an invalid entry gives
`warn`); a global config that cannot be read counts as the strictest value. A value that cannot be
read for one of those gates (for example `file_write_enabled = "no"`) counts as closed, and
nothing is ever loosened: an added root, `profile = "root"` or `file_write_enabled = true` saved
next to an invalid entry has no effect. If the file is not valid TOML, is unreadable or is over
1 MiB, nothing in it can be confirmed, so file writes are disabled and any `root` profile and
extra write roots are dropped until it loads again. So `file_write_enabled = false` saved together
with `extra_write_roots = ["~/scratch"]` takes effect, and the response says why the rest of the
edit did not.

Not covered by that rule: `shell_dangerous_patterns` an invalid edit adds and a lowered
`max_index_bytes`. For those, fix the entry the error names, or restart the server.

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
| `"disabled"` | `run_command` is hidden from the tool list; any call that still arrives returns an error immediately. |

### Controlling Shell Execution

Shell execution is **on by default** (`shell_command_mode = "warn"`). To turn it off
entirely, set the mode to `"disabled"`:

```toml
[security]
shell_command_mode = "disabled"
```

With `"disabled"`, `run_command` is filtered out of `list_tools` entirely — the agent never
sees its description or schema — and any call that arrives anyway returns an error. Both
halves matter: the tool list is computed for the *session-default* project, whereas the
refusal reads the `workspace`-pinned project, so a pinned call into a shell-disabled project
is caught only by the refusal.

Because `run_command` is also the only way to read an `@cmd_*` output buffer, hiding it
means buffer queries are unavailable too.

`"warn"` (the default) and `"unrestricted"` currently behave the same — both run the command.

---

## `[lsp.<language>]` — Language Server Settings

Per-language settings for the language server codescout starts. The table is named after the language (`java`, `kotlin`, `rust`, `python`, …). Both keys are optional.

| Key | Type | Effect |
|---|---|---|
| `mux` | bool | `true` shares one server across codescout instances. `false` starts a dedicated one. Omit it for the built-in default. |
| `env` | table of strings | Environment variables added to the server's environment. |

```toml
[lsp.java.env]
JAVA_HOME = "/usr/lib/jvm/java-21-openjdk"
```

Use `env` when the machine's default tool is one the server cannot run under. For example, `jdtls` fails at start-up under JDK 27 with `Unsupported class file major version 71`. Pointing `JAVA_HOME` at JDK 21 fixes that for this project only. Restart the server with `/mcp` after you edit the file.

A variable the server already sets is **not** replaced, and codescout logs a warning that names it. Kotlin's `JAVA_TOOL_OPTIONS` is one: it holds the per-workspace setting that keeps each index separate.

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

The effective configuration is always visible via the `workspace(action="status")` tool:

```json
{ "name": "workspace", "arguments": { "action": "status" } }
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

# Security & Permissions

codescout is designed to be safe to run autonomously: an agent can explore
any codebase it needs to understand, but it **cannot write outside its current
project without explicit opt-in**. This page explains the model, the defaults,
and the configuration knobs.

## The Core Model

The permission model is asymmetric by design:

| Operation | Default | Restriction |
|---|---|---|
| **Read** | Permissive — anywhere on disk | Deny-list of sensitive locations |
| **Write** | Restricted — project root only | Hard boundary; opt-in escapes via config |
| **Shell** | Enabled by default | Disable via `shell_command_mode = "disabled"`; cwd sandboxed to project root |
| **Git** | Enabled | Can disable per-project |

This asymmetry is intentional. An agent doing code intelligence work legitimately
needs to read widely — library source, system headers, adjacent repositories.
But writes touching unrelated projects or system files would be a serious
mistake. The boundary keeps agents capable and safe simultaneously.

## Why Write Restriction Matters for Agents

When an agent runs autonomously with multiple parallel tool calls in flight, a
write-boundary violation produces a `RecoverableError` — not a fatal crash. This
means:

- The agent receives a clear error message and a corrective hint
- **Sibling parallel tool calls are not aborted** — the rest of the work
  continues uninterrupted
- The user is never asked to intervene mid-task for a permissions issue

Writes outside the project root are blocked, not just warned about. This is
intentional: the boundary needs to be hard for the safety guarantee to hold.

## Read Policy

`read_file`, `grep`, `tree` (with glob), and all symbol tools can read from
any path on the filesystem, subject to one restriction: the **built-in deny
list**.

### Built-in Read Deny List

These locations are always blocked, regardless of configuration:

```
~/.ssh
~/.aws
~/.gnupg
~/.config/gcloud
~/.config/gh
~/.docker/config.json
~/.netrc
~/.npmrc
~/.kube/config
```

On Linux, `/etc/shadow` and `/etc/gshadow` are also blocked. On macOS,
`/etc/master.passwd` is blocked.

**This list cannot be overridden.** It exists to prevent an agent from
accidentally leaking credentials even if pointed at a project that tries to read
them.

### Extending the Deny List

To block additional paths specific to your environment, add them to
`project.toml`:

```toml
[security]
denied_read_patterns = [
    "~/.config/my-app/credentials",
    "/etc/internal",
]
```

Entries are prefix-matched, so `"~/.config/my-app"` blocks everything under
that directory.

## Write Policy

`create_file`, `edit_file`, and `edit_code` (all actions — `replace`,
`insert`, `remove`, `rename`) enforce a **project root
boundary**. The check happens before any I/O:

1. **Deny list first** — the target path is checked against the built-in deny
   list and `denied_read_patterns`. Even `extra_write_roots` cannot bypass this.
2. **Boundary check** — the canonicalized path must fall under the project root
   or an explicitly configured `extra_write_roots` entry.
3. **Symlink escape prevention** — the parent directory is canonicalized (not
   the target file, which may not exist yet), so symlinks pointing outside the
   root are caught.

### Allowing Writes Outside the Project Root

For multi-repo setups where the agent legitimately needs to write across
repositories, add the target directory to `extra_write_roots`:

```toml
[security]
extra_write_roots = [
    "/home/user/other-project",
]
```

The deny list still applies first. `extra_write_roots` only extends where writes
land — it cannot unlock credential paths.

### Disabling Writes Entirely

For read-only sessions:

```toml
[security]
file_write_enabled = false
```

## Shell Policy (`run_command`)

Shell execution is **on by default** (`shell_command_mode = "warn"`). To turn shell off,
set the mode to `"disabled"`:

```toml
[security]
shell_command_mode = "disabled"
```

`"warn"` is the default; `"unrestricted"` currently behaves identically — only `"disabled"`
changes `run_command` behaviour today.

| `shell_command_mode` | Behaviour |
|---|---|
| `"warn"` | Commands run normally. The default. |
| `"unrestricted"` | Commands run normally. Currently identical to `"warn"`. |
| `"disabled"` | All calls return an error. |

### Shell Sandbox

Even with shell enabled, the `cwd` parameter is restricted to subdirectories
within the project root — path traversal (`../`) is rejected. The shell command
itself is unrestricted (it can reference any absolute path), but the working
directory anchor is always the project.

Dangerous commands (`rm -rf`, `dd`, `mkfs`, etc.) require `acknowledge_risk:
true` to run. See [Workflow & Config](../tools/workflow-and-config.md) for the
full list.

## Per-Tool Switches

Individual feature categories can be toggled independently:

```toml
[security]
file_write_enabled = true    # create_file, edit_file, symbol writes
shell_command_mode = "warn"  # run_command: "warn" | "unrestricted" | "disabled"
indexing_enabled   = true    # index(action: build), workspace(action: status)
```

Disabling a category returns a `RecoverableError` with a hint explaining which
config field to set — the agent understands why it was blocked without user
intervention.

## Summary

- **Reads**: anywhere except the built-in credential deny list
- **Writes**: project root only, by default — hard boundary, not a warning
- **Shell**: on by default; disable via `shell_command_mode = "disabled"`; cwd sandboxed to project
- **Violations**: `RecoverableError` → agent gets a hint, no user interruption, no
  sibling call cancellation

→ [Configuration reference: `[security]`](../configuration/project-toml.md#security--access-controls)
→ [Troubleshooting access errors](../troubleshooting.md#file-operations)
